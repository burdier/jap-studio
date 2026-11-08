//! Arranque, carga del perfil y cambios de vista. El render vive en render.rs.
use super::{
    CenterView, DawApp, DesignSystem, VimMode, palette::Palette, pattern_editor::PatternEditor,
    timeline::Timeline,
};
use crate::instruments::{Piano, piano};
use elevate::musical::track::DEMO_TRACKS;
use gpui::{
    App, AppContext, Bounds, Context, ScrollHandle, WindowBounds, WindowOptions, point, px, size,
};
use gpui_platform::application;

pub(crate) fn run() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let app = DawApp::new(cx);
                    window.focus(&app.focus_handle, cx);
                    app
                })
            },
        ) {
            eprintln!("No se pudo abrir la ventana: {error}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}

impl DawApp {
    fn new(cx: &mut Context<Self>) -> Self {
        let ds = DesignSystem::load();
        let soundfont_task = cx.background_spawn(async { piano::preload() });
        Self {
            timeline: Timeline::new(ds.timeline_pixels_per_beat),
            pattern_editor: PatternEditor::new(),
            tracks: DEMO_TRACKS,
            player: None,
            pattern_player: None,
            instrument: Piano::new(),
            palette: Palette::new(),
            _soundfont_task: Some(soundfont_task),
            import_task: None,
            gain: 1.0,
            bypass: false,
            ds,
            focus_handle: cx.focus_handle(),
            track_scroll: ScrollHandle::new(),
            mode: VimMode::Normal,
            center_view: CenterView::Arrangement,
            cursor_row: 0,
            cursor_col: 0,
            command_buffer: String::new(),
            last_key: String::new(),
            status_message: "Ready".into(),
        }
    }
    pub(super) fn reload(&mut self) {
        self.ds = DesignSystem::load();
        self.timeline.set_scale(self.ds.timeline_pixels_per_beat);
    }
    pub(super) fn set_mode(&mut self, mode: VimMode) {
        self.mode = mode;
        if mode != VimMode::Command {
            self.command_buffer.clear();
        }
    }
    pub(super) fn set_center_view(&mut self, view: CenterView) {
        if self.center_view == CenterView::Instrument && view != CenterView::Instrument {
            self.all_notes_off();
        }
        if self.center_view == CenterView::Session && view != CenterView::Session {
            self.stop_pattern();
        }
        if view == CenterView::Session && self.center_view != CenterView::Session {
            self.stop_audio();
        }
        if view != CenterView::Arrangement {
            self.timeline.end_clip_drag();
        }
        if view == CenterView::Session {
            self.cursor_row = self.cursor_row.min(self.pattern_editor.lanes.len() - 1);
            self.cursor_col = self.cursor_col.min(self.pattern_editor.step_count() - 1);
        }
        self.center_view = view;
    }
    pub(super) fn reveal_track(&self) {
        if self.center_view != CenterView::Arrangement {
            return;
        }
        let height = self.track_scroll.bounds().size.height;
        let top = px(self.cursor_row as f32 * self.ds.track_header_height);
        let bottom = top + px(self.ds.track_header_height);
        let mut offset = self.track_scroll.offset();
        if top + offset.y < px(0.0) {
            offset.y = -top;
        } else if bottom + offset.y > height {
            offset.y = height - bottom;
        }
        self.track_scroll
            .set_offset(point(px(0.0), offset.y.min(px(0.0))));
    }
}
