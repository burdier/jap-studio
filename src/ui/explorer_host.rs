//! Puente pequeño entre el explorador autónomo y las acciones del Arrangement.
use super::{
    CenterView, DawApp, DesignSystem,
    components::tree::TreeStyle as ExplorerStyle,
    explorer::{Explorer, ExplorerEvent},
    workspace_focus::Panel,
};
use elevate::{explorer::EntryKind, musical::track::TrackId};
use gpui::{AppContext, Context, Entity, Subscription, Window};
use std::path::PathBuf;

pub(super) enum ImportSource {
    Path(PathBuf),
    ExplorerSelection,
}

struct ImportRequest {
    source: ImportSource,
    track: TrackId,
    beat: f64,
}

pub(super) struct ExplorerDock {
    pub view: Entity<Explorer>,
    pub width: f32,
    last_width: f32,
    focus_request: Option<bool>,
    pending_import: Option<ImportRequest>,
    _subscription: Subscription,
}

impl ExplorerDock {
    pub fn new(ds: &DesignSystem, cx: &mut Context<DawApp>) -> Self {
        let view = cx.new(|cx| Explorer::new(ExplorerStyle::from(ds), cx));
        let subscription = cx.subscribe(&view, |this, _, event, cx| {
            match event {
                ExplorerEvent::Insert(path) => {
                    this.request_file_import(ImportSource::Path(path.clone()));
                    this.flush_file_import(cx);
                }
                ExplorerEvent::Close => this.hide_explorer(),
                ExplorerEvent::Focused => {
                    this.workspace_focus.active = super::workspace_focus::Panel::Explorer;
                    this.track_delete_pending = false;
                    this.release_explorer_keyboard();
                }
            }
            cx.notify();
        });
        Self {
            view,
            width: 0.0,
            last_width: 270.0,
            focus_request: None,
            pending_import: None,
            _subscription: subscription,
        }
    }
}

impl DawApp {
    pub(super) fn explorer_visible(&self) -> bool {
        self.explorer.width > 0.0
            && self
                .workspace_focus
                .expanded
                .is_none_or(|panel| panel == Panel::Explorer)
    }

    pub(super) fn toggle_explorer(&mut self) {
        if self.explorer_visible() {
            self.hide_explorer();
            return;
        }
        self.workspace_focus.expanded = None;
        self.release_explorer_keyboard();
        self.explorer.width = self.explorer.last_width;
        self.explorer.focus_request = Some(true);
    }

    fn hide_explorer(&mut self) {
        if self.explorer.width >= 150.0 {
            self.explorer.last_width = self.explorer.width;
        }
        self.explorer.width = 0.0;
        self.explorer.focus_request = Some(false);
        if self.workspace_focus.expanded == Some(Panel::Explorer) {
            self.workspace_focus.expanded = None;
        }
        if self.workspace_focus.active == Panel::Explorer {
            self.workspace_focus.active = match self.center_view {
                CenterView::Arrangement => Panel::Arrangement,
                CenterView::Session | CenterView::Mixer => Panel::Center,
            };
            self.sync_midi_focus();
        }
    }

    fn release_explorer_keyboard(&mut self) {
        let released = self.controller.release_all();
        self.apply_midi(released);
    }

    pub(super) fn resize_explorer(&mut self, width: f32, viewport_width: f32) {
        let width = width.clamp(0.0, (viewport_width - 320.0).clamp(150.0, 600.0));
        self.explorer.width = if width < 40.0 { 0.0 } else { width };
        if width >= 150.0 {
            self.explorer.last_width = width;
        }
        if self.explorer.width == 0.0 {
            self.explorer.focus_request = Some(false);
        }
    }

    pub(super) fn sync_explorer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_file_import(cx);
        let style = ExplorerStyle::from(&self.ds);
        let audio = &self.settings.audio.saved;
        let visible = self.explorer_visible();
        let explorer = self.explorer.view.read(cx);
        if explorer.style != style
            || explorer.vim_enabled != self.vim_enabled
            || explorer.preview_needs_sync(audio, visible)
        {
            self.explorer.view.update(cx, |explorer, cx| {
                explorer.style = style;
                explorer.vim_enabled = self.vim_enabled;
                explorer.sync_preview(audio, visible);
                cx.notify();
            });
        }
        if let Some(explorer) = self.explorer.focus_request.take() {
            if explorer {
                let focus = self.explorer.view.read(cx).focus.clone();
                window.focus(&focus, cx);
            } else {
                window.focus(&self.focus_handle, cx);
            }
        }
    }

    /// Operación compartida: doble clic, :file.import y acciones asignables a iconos.
    pub(super) fn request_file_import(&mut self, source: ImportSource) {
        let Some(track) = self.tracks.get(self.cursor_row) else {
            self.status_message = "Añade y selecciona una pista para insertar el archivo".into();
            return;
        };
        if !self.track_accepts_clips(self.cursor_row) {
            self.status_message =
                "Selecciona una pista de audio/MIDI dentro del grupo para insertar".into();
            return;
        }
        self.explorer.pending_import = Some(ImportRequest {
            source,
            track: track.id,
            beat: self.timeline.edit_beat(),
        });
    }

    fn flush_file_import(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.explorer.pending_import.take() else {
            return;
        };
        let path = match request.source {
            ImportSource::Path(path) => Some(path),
            ImportSource::ExplorerSelection => self.explorer.view.read(cx).selected_file(),
        };
        let Some(path) = path else {
            self.status_message = "Selecciona un archivo MIDI o audio en el explorador".into();
            return;
        };
        if !EntryKind::for_file(&path).importable() {
            self.status_message =
                "Formato no compatible: usa MIDI, WAV, FLAC, MP3, Ogg, AAC o M4A".into();
            return;
        }
        if let Some(track) = self
            .tracks
            .iter()
            .position(|track| track.id == request.track)
        {
            self.import_audio(path, track, request.beat, cx);
        } else {
            self.status_message = "La pista de destino ya no existe".into();
        }
    }
}
