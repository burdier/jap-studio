//! La barra llama a los mismos controladores que teclado, buscador y menús.
use super::ToolbarAction;
use crate::ui::{
    DawApp,
    piano_roll::{RollAction, RollTool},
};
use elevate::catalog;
use gpui::{Context, Window};

impl DawApp {
    pub(in crate::ui) fn execute_toolbar_action(
        &mut self,
        action: ToolbarAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            ToolbarAction::Catalog(id) | ToolbarAction::RollCatalog(id) => {
                if let Some(entry) = catalog::find(id) {
                    self.activate_catalog_entry(entry, window, cx);
                }
            }
            ToolbarAction::Record => self.toggle_midi_record(),
            ToolbarAction::GoToStart => {
                self.stop_all();
                self.timeline.go_to_start();
            }
            ToolbarAction::ToggleController => self.toggle_controller(),
            ToolbarAction::OpenPalette => self.open_palette(),
            ToolbarAction::OpenInstrument => self.execute_command("instrument"),
            ToolbarAction::OpenPianoRoll => {
                if self.clip_kind_at_cursor() == Some(crate::ui::timeline::DragKind::Midi) {
                    self.open_piano_roll();
                    self.focus_piano_roll(window, cx);
                } else {
                    self.status_message =
                        "Coloca el cursor sobre un clip MIDI para abrir el Piano Roll".into();
                }
            }
            ToolbarAction::DetachInstrument => self.execute_command("detach"),
            ToolbarAction::DockInstrument => self.execute_command("dock"),
            ToolbarAction::ShowShortcuts => self.show_shortcuts(),
            ToolbarAction::TrackMute => self.toggle_track_mute(self.cursor_row),
            ToolbarAction::TrackSolo => self.toggle_track_solo(self.cursor_row),
            ToolbarAction::TrackRecord => self.toggle_track_record(self.cursor_row),
            ToolbarAction::GainUp => self.change_gain(0.1),
            ToolbarAction::GainDown => self.change_gain(-0.1),
            ToolbarAction::TimelineZoomIn => self.timeline.zoom_at_cursor(2.0),
            ToolbarAction::TimelineZoomOut => self.timeline.zoom_at_cursor(0.5),
            ToolbarAction::PresetNext => self.step_soundfont_preset(1),
            ToolbarAction::PresetPrevious => self.step_soundfont_preset(-1),
            ToolbarAction::BankNext => self.step_soundfont_bank(1),
            ToolbarAction::BankPrevious => self.step_soundfont_bank(-1),
            ToolbarAction::RollResetView => {
                self.apply_roll_action(RollAction::ResetView, window, cx)
            }
            ToolbarAction::RollClose => self.apply_roll_action(RollAction::Close, window, cx),
            ToolbarAction::RollZoomIn => {
                self.apply_roll_action(RollAction::ZoomWidth(1.25), window, cx)
            }
            ToolbarAction::RollZoomOut => {
                self.apply_roll_action(RollAction::ZoomWidth(0.8), window, cx)
            }
            ToolbarAction::RollTaller => {
                self.apply_roll_action(RollAction::ZoomHeight(1.25), window, cx)
            }
            ToolbarAction::RollShorter => {
                self.apply_roll_action(RollAction::ZoomHeight(0.8), window, cx)
            }
            ToolbarAction::InsertMidiClip => {
                self.insert_midi_clip_at(self.cursor_row, self.timeline.edit_beat());
                self.focus_piano_roll(window, cx);
                self.status_message = "Clip MIDI insertado desde botón de icono".into();
            }
            ToolbarAction::CaptureMidi => self.capture_midi_to_track(),
            ToolbarAction::DuplicateClip => self.execute_clip_catalog_action("clip.duplicate"),
            ToolbarAction::ToggleClipMute => self.execute_clip_catalog_action("clip.mute"),
            ToolbarAction::MidiLearn => self.toggle_midi_learn(),
            ToolbarAction::MidiScan => {
                self.midi_hw.reconnect();
                let ports = self.midi_hw.port_names();
                self.status_message = if ports.is_empty() {
                    "Sin controladores MIDI USB detectados".into()
                } else {
                    format!("USB MIDI conectado: {}", ports.join(", "))
                };
            }
            ToolbarAction::ToggleBypass => self.toggle_bypass(),
            ToolbarAction::OpenSettings => self.open_settings(),
            ToolbarAction::RollSelect => {
                self.apply_roll_action(RollAction::Tool(RollTool::Select), window, cx)
            }
            ToolbarAction::RollDraw => {
                self.apply_roll_action(RollAction::Tool(RollTool::Draw), window, cx)
            }
            ToolbarAction::RollErase => {
                self.apply_roll_action(RollAction::Tool(RollTool::Erase), window, cx)
            }
            ToolbarAction::RollPaint => {
                self.apply_roll_action(RollAction::Tool(RollTool::Paint), window, cx)
            }
            ToolbarAction::RollMute => {
                self.apply_roll_action(RollAction::Tool(RollTool::Mute), window, cx)
            }
            ToolbarAction::RollQuantize => self.apply_roll_action(RollAction::Quantize, window, cx),
            ToolbarAction::RollHumanize => self.apply_roll_action(RollAction::Humanize, window, cx),
            ToolbarAction::RollSubdivide => {
                self.apply_roll_action(RollAction::Subdivide, window, cx)
            }
            ToolbarAction::RollGridFiner => {
                self.apply_roll_action(RollAction::GridFiner, window, cx)
            }
            ToolbarAction::RollGridCoarser => {
                self.apply_roll_action(RollAction::GridCoarser, window, cx)
            }
            ToolbarAction::RollCut => self.apply_roll_action(RollAction::Cut, window, cx),
            ToolbarAction::RollCopy => self.apply_roll_action(RollAction::Copy, window, cx),
            ToolbarAction::RollPaste => self.apply_roll_action(RollAction::Paste, window, cx),
            ToolbarAction::RollBox => {
                self.apply_roll_action(RollAction::Tool(RollTool::Box), window, cx)
            }
            ToolbarAction::RollLength => {
                self.apply_roll_action(RollAction::Tool(RollTool::Length), window, cx)
            }
            ToolbarAction::RollGhostNotes => {
                self.apply_roll_action(RollAction::GhostNotes, window, cx)
            }
            ToolbarAction::RollFollow => self.apply_roll_action(RollAction::Follow, window, cx),
            ToolbarAction::RollVelocities => {
                self.apply_roll_action(RollAction::Velocities, window, cx)
            }
            ToolbarAction::RollHighlightScale => {
                self.apply_roll_action(RollAction::HighlightScale, window, cx)
            }
            ToolbarAction::RollToggleWindow => {
                self.apply_roll_action(RollAction::ToggleWindow, window, cx)
            }
        }
        if self.clip_menu.is_some() || self.palette.is_open() || self.settings.is_visible() {
            window.focus(&self.focus_handle, cx);
        }
        self.sync_windows(cx);
    }
}
