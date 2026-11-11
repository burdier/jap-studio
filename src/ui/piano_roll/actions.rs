//! Controlador compartido por menú contextual, toolbar y teclado del piano roll.
use super::{RollAction, RollTool, demo};
use crate::ui::DawApp;
use elevate::musical::time::{PPQ, Tick};
use gpui::{Context, Window};

impl DawApp {
    pub(super) fn open_roll_action_form(
        &mut self,
        command: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = elevate::catalog::entries().iter().find(|entry| {
            matches!(entry.action, elevate::catalog::CatalogAction::Invoke(value) if value == command)
        }) else { return; };
        let Some(spec) = entry.action.parameters() else {
            return;
        };
        self.open_action_form(entry, spec, window, cx);
    }

    pub(in crate::ui) fn insert_midi_clip_from_menu(&mut self) {
        let Some(target) = self.clip_menu.take() else {
            return;
        };
        if target.clip_kind.is_some() {
            return;
        }
        self.insert_midi_clip_at(target.track, target.beat);
    }

    pub(in crate::ui) fn insert_midi_clip_at(&mut self, track: usize, beat: f64) {
        if !self.track_accepts_clips(track) {
            self.status_message = "Selecciona una pista hija para insertar MIDI".into();
            return;
        }
        self.stop_audio();
        let clip = demo::reference_clip(
            self.active_instrument_name().to_string(),
            track,
            beat,
            self.tracks[track]
                .instrument
                .unwrap_or(self.instrument.id()),
        );
        self.timeline.insert_midi_clip(clip);
        self.open_piano_roll();
    }

    pub(in crate::ui) fn open_piano_roll(&mut self) {
        let Some(clip) = &self.timeline.midi_clip else {
            return;
        };
        self.cursor_row = clip.track;
        let cursor = Tick(
            (((self.timeline.edit_beat() - clip.start_beat + clip.trim_start) * PPQ as f64).round()
                as i64)
                .clamp(0, clip.pattern.length().0 - 1),
        );
        // Solo reiniciar estado ligado a la identidad del clip; conservar preferencias.
        self.piano_roll.opened_clip = Some(clip.clone());
        self.piano_roll.open = true;
        self.piano_roll.cursor = cursor;
        self.piano_roll.selected = None;
        self.piano_roll.selected_notes.clear();
        self.piano_roll.hovered_note = None;
        self.piano_roll.hovered_resize_edge = false;
        self.piano_roll.drag = None;
        self.piano_roll.motion.borrow_mut().clear();
        self.piano_roll.push = None;
        self.piano_roll.push_error = None;
        self.piano_roll.paint_stroke = None;
        self.piano_roll.control_drag = false;
        self.piano_roll.selected_control = None;
        self.piano_roll.lane_menu = None;
        self.piano_roll.choices = None;
        self.piano_roll.viewport = Default::default();
        self.timeline.end_clip_drag();
    }

    pub(in crate::ui) fn apply_roll_action(
        &mut self,
        action: RollAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            RollAction::ChooseLane => {
                self.piano_roll.choices = None;
                self.piano_roll.lane_menu = Some(super::RollLaneMenu {
                    position: gpui::point(gpui::px(10.0), gpui::px(320.0)),
                    selected: super::lane::LANE_MENU_ITEMS
                        .iter()
                        .position(|item| item.lane == self.piano_roll.active_lane)
                        .unwrap_or(0),
                });
            }
            RollAction::TrackSolo => {
                if let Some(clip) = &self.timeline.midi_clip {
                    self.toggle_track_solo(clip.track);
                }
            }
            RollAction::Form(command) => self.open_roll_action_form(command, window, cx),
            RollAction::ZoomWidth(factor) => {
                if let Some(clip) = &self.timeline.midi_clip {
                    let grid = self.piano_roll.grid.get();
                    self.piano_roll.viewport.zoom_at(
                        grid,
                        grid.notes.center(),
                        clip.pattern_beats(),
                        factor,
                    );
                }
            }
            RollAction::ZoomHeight(factor) => {
                let grid = self.piano_roll.grid.get();
                self.piano_roll
                    .viewport
                    .zoom_vertical_at(grid, grid.notes.center(), factor);
            }
            RollAction::ResetView => self.piano_roll.viewport = Default::default(),
            RollAction::Close if self.piano_roll_window.is_some() => {
                self.toggle_piano_roll_window(cx)
            }
            RollAction::Close => {
                self.piano_roll.open = false;
                window.focus(&self.focus_handle, cx);
            }
            RollAction::ToggleWindow => self.toggle_piano_roll_window(cx),
            RollAction::PreviousClip => self.step_roll_clip(-1),
            RollAction::NextClip => self.step_roll_clip(1),
            RollAction::Tool(tool) => {
                self.end_roll_drag();
                self.piano_roll.tool = tool;
                self.piano_roll.hint = match tool {
                    RollTool::Select => "Pointer: clic selecciona · arrastra para mover · doble clic crea / borra · Delete elimina selección",
                    RollTool::Box => "Arrastra una región para seleccionar · Delete: borrar · Esc: cancelar",
                    RollTool::EraseRegion => "Arrastra una región; al soltar se eliminan sus notas · Esc: cancelar",
                    _ => "Herramienta MIDI activa",
                }.into();
            }
            RollAction::Quantize => self.open_roll_action_form("midi.quantize", window, cx),
            RollAction::Humanize => {
                self.execute_script_command("midi.humanize", cx);
            }
            RollAction::Gate => self.open_roll_action_form("midi.gate", window, cx),
            RollAction::Swing => self.open_roll_action_form("midi.swing", window, cx),
            RollAction::ClampScale => {
                self.execute_script_command("midi.scale.clamp", cx);
            }
            RollAction::Velocity => self.open_roll_action_form("midi.velocity", window, cx),
            RollAction::Random => self.open_roll_action_form("midi.random", window, cx),
            RollAction::ChooseDrawLength => self.choose_roll_value("length"),
            RollAction::ChooseRoot => self.choose_roll_value("root"),
            RollAction::ChooseScale => self.choose_roll_value("scale"),
            RollAction::ToggleScaleKeyboard => {
                self.piano_roll.show_scale_keyboard = !self.piano_roll.show_scale_keyboard
            }
            RollAction::NoteNames => {
                self.piano_roll.show_note_names = !self.piano_roll.show_note_names
            }
            RollAction::ToggleClipLoop => {
                self.toggle_roll_clip_property(true);
            }
            RollAction::ToggleClipMute => {
                self.toggle_roll_clip_property(false);
            }
            RollAction::ChooseClipColor => self.choose_roll_value("color"),
            RollAction::ChooseClip => self.choose_roll_value("clip"),
            RollAction::OpenSettings => self.open_settings(),
            RollAction::InspectorTab(tab) => self.piano_roll.inspector_tab = tab.min(3),
            RollAction::Subdivide => {
                self.execute_script_command("midi.subdivide", cx);
            }
            RollAction::GridFiner => self.change_roll_grid(true),
            RollAction::GridCoarser => self.change_roll_grid(false),
            RollAction::HumanizeAmount(amount) => {
                self.piano_roll.humanize_amount = amount;
                self.piano_roll.hint = format!("Humanize: {amount}%");
            }
            RollAction::Copy => self.roll_copy(false),
            RollAction::Cut => self.roll_copy(true),
            RollAction::Paste => self.roll_paste(),
            RollAction::SelectAll => self.select_all_roll_notes(),
            RollAction::ClearSelection => self.clear_roll_selection(),
            RollAction::DeleteSelection => self.delete_roll_selection(),
            RollAction::GhostNotes => self.piano_roll.ghost_notes = !self.piano_roll.ghost_notes,
            RollAction::Follow => self.piano_roll.follow = !self.piano_roll.follow,
            RollAction::Velocities => self.piano_roll.velocities = !self.piano_roll.velocities,
            RollAction::SetLane(lane) => {
                self.piano_roll.active_lane = lane;
                self.piano_roll.selected_control = None;
                self.piano_roll.control_drag = false;
                self.piano_roll.lane_menu = None;
                self.piano_roll.hint =
                    format!("Carril activo: {} · {}", lane.label(), lane.description());
            }
            RollAction::HighlightScale => {
                self.piano_roll.highlight_scale = !self.piano_roll.highlight_scale
            }
        }
    }
}
