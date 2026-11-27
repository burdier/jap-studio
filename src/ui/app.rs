//! Arranque, carga del perfil y cambios de vista. El render vive en render.rs.
use super::{
    CenterView, DawApp, DesignSystem, cursor_fx::CursorFx, icons::ToolbarBindings,
    palette::Palette, pattern_editor::PatternEditor, piano_roll::PianoRoll, settings::SettingsUi,
    timeline::Timeline, workspace_focus::Panel,
};
use crate::audio::MidiHardwareIn;
use crate::instruments::Piano;
use elevate::musical::{
    clip::MidiCapture,
    controller::ComputerKeyboard,
    midi::{MidiServer, MidiTarget},
    midi_map::MidiMapper,
};
use gpui::application;
use gpui::{
    App, AppContext, Bounds, Context, ScrollHandle, TitlebarOptions, WindowBounds,
    WindowDecorations, WindowOptions, point, px, size,
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
                        title: Some("Jap Studio".into()),
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    // Sin borde del compositor: la barra propia dibuja los botones.
                    window_decorations: Some(WindowDecorations::Client),
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
        let mut synth = crate::instruments::synth::Synth::default();
        synth.configure_audio(settings.audio.saved.clone());
        let initial_target = Some(MidiTarget::Instrument(instrument.id()));
        let explorer = super::explorer_host::ExplorerDock::new(&ds, cx);
        let fx = super::fx::FxDock::new((&ds).into(), cx);
        let lua_tools = super::lua_tools::LuaTools::new();
        let lua_components = super::lua_components::LuaComponents::new();
        let weave_rule = super::weave::load_from_env();
        let initial_status = match &weave_rule {
            Ok(Some(_)) => "Tejer: regla Lua preparada".into(),
            Ok(None) => "Ready".into(),
            Err(error) => error.clone(),
        };
        let mut palette = Palette::new();
        palette.replace_tools(elevate::catalog::tools::entries(&lua_tools.registry));
        palette.replace_components(
            lua_components
                .registry
                .values()
                .map(|c| (c.id.clone(), c.name.clone(), c.description.clone()))
                .collect(),
        );
        let (mcp_host, mcp_task) = Self::start_mcp_bridge(cx);
        let midi_hotplug_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1250))
                    .await;
                let keep_going = this.update(cx, |this, cx| {
                    if this.poll_midi_hotplug() {
                        cx.notify();
                    }
                });
                if keep_going.is_err() {
                    break;
                }
            }
        });
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
            tracks: Vec::new(),
            track_revision: 0,
            selected_tracks: vec![],
            track_anchor: None,
            bar_menu: None,
            track_menu: None,
            track_presentation: Default::default(),
            track_levels: Vec::new(),
            track_pans: Vec::new(),
            track_control_drag: None,
            mixer_drag: None,
            track_reorder_drag: None,
            player: None,
            tail_player: None,
            file_playback: None,
            drum_audition: None,
            drum_note_repeat: std::sync::Arc::new(crate::audio::drums::NoteRepeatControl::default()),
            audio_loop: None,
            pattern_loop_beats: None,
            metronome: std::sync::Arc::new(crate::audio::metronome::MetronomeControl::new()),
            recording_waiting_count_in: false,
            audio_recorder: None,
            audio_record_stereo: false,
            input_monitor: None,
            instrument,
            piano_instances: Default::default(),
            controller: ComputerKeyboard::default(),
            midi_server: MidiServer::new(initial_target),
            weave_rule,
            midi_armed_tracks: Default::default(),
            midi_mapper: MidiMapper::new(),
            midi_hw: MidiHardwareIn::connect_all(),
            midi_last_note: None,
            midi_last_vel: 0,
            midi_pitch_bend: 0,
            _midi_hotplug_task: midi_hotplug_task,
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
            audio_to_midi_task: None,
            history: Default::default(),
            project: super::project::ProjectUi::new(),
            script_task: None,
            lua_tools,
            lua_components,
            sampler,
            active_sampler_id: crate::instruments::sampler::SAMPLER_ID,
            sampler_instances: Default::default(),
            sampler_ui: Default::default(),
            synth,
            active_synth_id: crate::instruments::synth::SYNTH_ID,
            synth_instances: Default::default(),
            synth_ui: Default::default(),
            vst: Default::default(),
            vst_instances: Default::default(),
            vst_reloads: Default::default(),
            vst_task: None,
            vst_effects: Default::default(),
            _mcp_host: mcp_host,
            _mcp_task: mcp_task,
            export_job: None,
            bounce_pending: None,
            freeze_pending: None,
            frozen_tracks: Default::default(),
            utility: elevate::devices::utility_slot::UtilitySlot::new(),
            utility_open: false,
            master_chain: elevate::musical::fx::FxChain::new(),
            ds,
            focus_handle: cx.focus_handle(),
            track_scroll: ScrollHandle::new(),
            tap_tempo: Vec::with_capacity(8),
            command_open: false,
            workspace_focus: Default::default(),
            center_view: CenterView::Arrangement,
            cursor_row: 0,
            command_buffer: String::new(),
            cmd_selected: None,
            last_key: String::new(),
            status_message: initial_status,
        }
    }
    pub(super) fn reload(&mut self) {
        self.settings.export = super::export_preferences::ExportPreferences::load();
        let colors = std::mem::take(&mut self.ds.track_color_override);
        self.ds = DesignSystem::load();
        self.ds.track_color_override = colors;
        self.cursor_fx.set_mode(self.ds.cursor_fx);
        self.timeline.set_scale(self.ds.timeline_pixels_per_beat);
    }
    pub(in crate::ui) fn open_command_bar(&mut self) {
        self.command_open = true;
        self.cmd_selected = None;
    }
    pub(in crate::ui) fn close_command_bar(&mut self) {
        self.command_open = false;
        self.command_buffer.clear();
        self.cmd_selected = None;
    }
    pub(super) fn set_center_view(&mut self, view: CenterView) {
        if view != self.center_view {
            self.mixer_drag = None;
            self.workspace_focus = Default::default();
        }
        if view != CenterView::Arrangement {
            self.timeline.end_clip_drag();
        }
        if view == CenterView::Session && self.center_view != view {
            self.workspace_focus.active = Panel::Center;
        } else if view == CenterView::Mixer {
            self.workspace_focus.active = Panel::Center;
        }
        self.cursor_row = self.cursor_row.min(self.tracks.len().saturating_sub(1));
        self.center_view = view;
        self.sync_midi_focus();
    }
    pub(super) fn reveal_track(&mut self) {
        self.sync_midi_focus();
        if !self.arrangement_visible() {
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
