//! Arranque, carga del perfil y cambios de vista. El render vive en render.rs.
use super::{
    CenterView, DawApp, DesignSystem, VimMode,
    cursor_fx::CursorFx,
    icons::ToolbarBindings,
    palette::Palette,
    pattern_editor::PatternEditor,
    piano_roll::PianoRoll,
    settings::SettingsUi,
    timeline::Timeline,
    views::tracks::{DEMO_LEVELS, DEMO_PANS},
};
use crate::audio::MidiHardwareIn;
use crate::instruments::Piano;
use elevate::musical::{
    clip::MidiCapture,
    controller::ComputerKeyboard,
    midi::{MidiServer, MidiTarget},
    midi_map::MidiMapper,
    track::DEMO_TRACKS,
};
use gpui::application;
use gpui::{
    App, AppContext, Bounds, Context, ScrollHandle, TitlebarOptions, WindowBounds, WindowOptions,
    point, px, size,
};
use std::time::Instant;

pub(crate) fn run() {
    if let Err(error) = crate::audio::vst3::environment::configure() {
        eprintln!("Wine: {error}");
    }
    if crate::audio::vst3::scan::worker() {
        return;
    }
    application()
        .with_assets(gpui::assets::Assets)
        .run(|cx: &mut App| {
            cx.set_cursor_hide_mode(gpui::CursorHideMode::Never);
            super::kit::init(cx);
            let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
            if let Err(error) = gpui::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Hay DAW".into()),
                        ..Default::default()
                    }),
                    app_id: Some("hay-daw".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        let app = DawApp::new(cx);
                        window.focus(&app.focus_handle, cx);
                        let entity = cx.weak_entity();
                        window.on_window_should_close(cx, move |window, cx| {
                            entity
                                .update(cx, |this, cx| this.guard_project_close(window, cx))
                                .unwrap_or(true)
                        });
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
        let settings = SettingsUi::new();
        let mut instrument = Piano::new();
        instrument.configure_audio(settings.audio.saved.clone());
        let mut sampler = crate::instruments::sampler::Sampler::default();
        sampler.configure_audio(settings.audio.saved.clone());
        let initial_target = Some(MidiTarget::Instrument(instrument.id()));
        let explorer = super::explorer_host::ExplorerDock::new(&ds, cx);
        let fx = super::fx::FxDock::new((&ds).into(), cx);
        let lua_tools = super::lua_tools::LuaTools::new();
        let weave_rule = super::weave::load_from_env();
        let initial_status = match &weave_rule {
            Ok(Some(_)) => "Tejer: regla Lua preparada".into(),
            Ok(None) => "Ready".into(),
            Err(error) => error.clone(),
        };
        let mut palette = Palette::new();
        palette.replace_tools(elevate::catalog::tools::entries(&lua_tools.registry));
        Self {
            timeline: Timeline::new(ds.timeline_pixels_per_beat),
            pattern_editor: PatternEditor::new(),
            piano_roll: PianoRoll::default(),
            clip_menu: None,
            action_form: None,
            general_note: None,
            track_color_menu: None,
            piano_roll_focus: cx.focus_handle(),
            piano_roll_window: None,
            tracks: DEMO_TRACKS.to_vec(),
            track_revision: 0,
            selected_tracks: vec![],
            track_anchor: None,
            bar_menu: None,
            track_menu: None,
            track_presentation: Default::default(),
            track_delete_pending: false,
            track_levels: DEMO_LEVELS.to_vec(),
            track_pans: DEMO_PANS.to_vec(),
            track_control_drag: None,
            track_reorder_drag: None,
            player: None,
            file_playback: None,
            audio_loop: None,
            pattern_loop_beats: None,
            instrument,
            piano_instances: Default::default(),
            controller: ComputerKeyboard::default(),
            midi_server: MidiServer::new(initial_target),
            weave_rule,
            midi_armed_tracks: Default::default(),
            midi_mapper: MidiMapper::new(),
            midi_hw: MidiHardwareIn::connect_all(),
            midi_capture: MidiCapture::new(),
            session_epoch: Instant::now(),
            instrument_window: None,
            window_request: None,
            palette,
            settings,
            toolbar_icons: ToolbarBindings::new(),
            cursor_fx: CursorFx::new(ds.cursor_fx),
            split_drag: None,
            workspace_split_ratio: 0.38,
            explorer,
            fx,
            piano_ui: Default::default(),
            import_task: None,
            collect_task: None,
            project: Default::default(),
            script_task: None,
            lua_tools,
            lua_components: super::lua_components::LuaComponents::new(),
            sampler,
            active_sampler_id: crate::instruments::sampler::SAMPLER_ID,
            sampler_instances: Default::default(),
            sampler_ui: Default::default(),
            vst: Default::default(),
            vst_instances: Default::default(),
            vst_reloads: Default::default(),
            vst_task: None,
            vst_effects: Default::default(),
            export_job: None,
            utility: elevate::devices::utility_slot::UtilitySlot::new(),
            utility_open: false,
            ds,
            focus_handle: cx.focus_handle(),
            track_scroll: ScrollHandle::new(),
            mode: VimMode::Normal,
            vim_enabled: super::keyboard_preferences::load_vim(),
            workspace_focus: Default::default(),
            center_view: CenterView::Arrangement,
            cursor_row: 0,
            cursor_col: 0,
            command_buffer: String::new(),
            cmd_selected: None,
            last_key: String::new(),
            status_message: initial_status,
        }
    }
    pub(super) fn reload(&mut self) {
        self.vim_enabled = super::keyboard_preferences::load_vim();
        if !self.vim_enabled {
            self.set_mode(VimMode::Normal);
        }
        self.settings.export = super::export_preferences::ExportPreferences::load();
        let colors = std::mem::take(&mut self.ds.track_color_override);
        self.ds = DesignSystem::load();
        self.ds.track_color_override = colors;
        self.cursor_fx.set_mode(self.ds.cursor_fx);
        self.timeline.set_scale(self.ds.timeline_pixels_per_beat);
    }
    pub(super) fn set_mode(&mut self, mode: VimMode) {
        self.track_delete_pending = false;
        self.mode = if self.vim_enabled {
            mode
        } else {
            VimMode::Normal
        };
        if self.mode != VimMode::Command {
            self.command_buffer.clear();
            self.cmd_selected = None;
        } else {
            self.cmd_selected = None;
        }
    }
    pub(super) fn set_center_view(&mut self, view: CenterView) {
        if view != self.center_view {
            self.workspace_focus = Default::default();
        }
        self.track_delete_pending = false;
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
            self.cursor_row = self
                .cursor_row
                .min(self.pattern_editor.session.kit.pieces.len() - 1);
            self.cursor_col = self.cursor_col.min(self.pattern_editor.step_count() - 1);
        }
        if view != CenterView::Session {
            self.cursor_row = self.cursor_row.min(self.tracks.len().saturating_sub(1));
        }
        self.center_view = view;
        self.sync_midi_focus();
    }
    pub(super) fn reveal_track(&mut self) {
        self.sync_midi_focus();
        if self.center_view != CenterView::Arrangement {
            return;
        }
        let height = self.track_scroll.bounds().size.height;
        let Some(row) = self.track_layout().row(self.cursor_row) else {
            return;
        };
        let top = px(row.top);
        let bottom = top + px(row.height);
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
