//! Picking y apertura compartidos por clic derecho y botón de propiedades.
use super::{ClipAction, ClipMenu, MENU_HEIGHT, MENU_WIDTH, MenuPage};
use crate::ui::{
    DawApp,
    timeline::{ClipBarHit, DragKind, TimelineArea},
};
use gpui::{Pixels, Point, Size, point, px};

impl DawApp {
    pub(in crate::ui) fn open_clip_catalog_page(
        &mut self,
        command: &str,
        position: Point<Pixels>,
        viewport: Size<Pixels>,
    ) -> bool {
        let Some(kind) = self.clip_kind_at_cursor() else {
            self.status_message = "Coloca el cursor sobre un clip para editarlo".into();
            return false;
        };
        let track = self.cursor_row;
        self.show_clip_menu(Some(kind), track, position, viewport);
        match command {
            "clip.rename" => {
                let name = self.active_clip_name(kind).to_string();
                if let Some(menu) = &mut self.clip_menu {
                    menu.page = MenuPage::Rename;
                    menu.rename = name;
                    menu.select_name = true;
                }
            }
            "clip.gain" => {
                let reset_index = self
                    .clip_menu_items()
                    .iter()
                    .position(|item| matches!(item.action, ClipAction::Gain(None)))
                    .unwrap_or(0);
                if let Some(menu) = &mut self.clip_menu {
                    menu.page = MenuPage::Gain;
                    menu.selected = reset_index;
                }
            }
            "clip.color" => {
                let color_index = self
                    .clip_menu_items()
                    .iter()
                    .position(|item| matches!(item.action, ClipAction::Color(_)))
                    .unwrap_or(0);
                if let Some(menu) = &mut self.clip_menu {
                    menu.selected = color_index;
                }
            }
            _ => return false,
        }
        true
    }

    pub(in crate::ui) fn open_clip_menu(
        &mut self,
        position: Point<Pixels>,
        viewport: Size<Pixels>,
    ) {
        self.timeline.end_clip_drag();
        let cursor_track = self.timeline.place_cursor(
            position,
            TimelineArea::Lanes,
            self.ds.track_header_height,
            self.tracks.len(),
        );
        let kind = if self
            .timeline
            .activate_midi_clip_at(position, self.ds.track_header_height)
        {
            Some(DragKind::Midi)
        } else if self
            .timeline
            .activate_audio_clip_at(position, self.ds.track_header_height)
        {
            Some(DragKind::Audio)
        } else {
            None
        };
        let track = match kind {
            Some(DragKind::Midi) => self.timeline.midi_clip.as_ref().map(|c| c.track),
            Some(DragKind::Audio) => self.timeline.clip.as_ref().map(|c| c.track),
            None => cursor_track,
        };
        if let Some(track) = track {
            if !self.track_accepts_clips(track) {
                self.open_track_menu(track, position, viewport);
                return;
            }
            self.show_clip_menu(kind, track, position, viewport);
        }
    }

    fn show_clip_menu(
        &mut self,
        kind: Option<DragKind>,
        track: usize,
        position: Point<Pixels>,
        viewport: Size<Pixels>,
    ) {
        self.cursor_row = track;
        if kind == Some(DragKind::Midi) {
            if self
                .timeline
                .midi_clip
                .as_ref()
                .is_some_and(|clip| !self.timeline.selected_midi.contains(&clip.id))
            {
                self.timeline.select_active_midi(false);
            }
        } else {
            self.timeline.selected_midi.clear();
        }
        self.sync_midi_focus();
        let height = if kind.is_some() { MENU_HEIGHT } else { 140.0 };
        self.clip_menu = Some(ClipMenu {
            position: point(
                position.x.clamp(
                    px(0.0),
                    (viewport.width - px(MENU_WIDTH + 10.0)).max(px(0.0)),
                ),
                position
                    .y
                    .clamp(px(0.0), (viewport.height - px(height + 10.0)).max(px(0.0))),
            ),
            viewport,
            track,
            beat: self.timeline.edit_beat(),
            clip_kind: kind,
            page: MenuPage::Main,
            selected: 0,
            rename: String::new(),
            select_name: false,
        });
    }

    pub(in crate::ui) fn handle_clip_bar_hit(
        &mut self,
        hit: ClipBarHit,
        position: Point<Pixels>,
        viewport: Size<Pixels>,
    ) {
        match hit {
            ClipBarHit::Inspector(kind) => {
                let track = match kind {
                    DragKind::Midi => self.timeline.midi_clip.as_ref().map(|c| c.track),
                    DragKind::Audio => self.timeline.clip.as_ref().map(|c| c.track),
                };
                if let Some(track) = track {
                    self.show_clip_menu(Some(kind), track, position, viewport);
                }
            }
            ClipBarHit::Mute(kind) => self.toggle_clip_mute(kind),
            ClipBarHit::Lock(kind) => self.toggle_clip_lock(kind),
            ClipBarHit::Loop(_) => {
                self.status_message = "Repetición de clips pendiente de implementar".into();
            }
        }
    }
}
