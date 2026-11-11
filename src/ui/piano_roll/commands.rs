//! Puente del catálogo al controlador existente: ninguna operación se duplica en la vista.
use super::{RollAction as A, RollTool as T};
use crate::ui::DawApp;
use gpui::{Context, Window};

impl DawApp {
    pub(in crate::ui) fn execute_roll_command(
        &mut self,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.execute_roll_control_command(command) {
            cx.notify();
            return true;
        }
        let Ok(parts) = elevate::actions::tokenize(command) else {
            return false;
        };
        let Some(name) = parts.first() else {
            return false;
        };
        if name == "midi.lane.select" {
            if let [_, key] = parts.as_slice() {
                if let Some(lane) = super::RollLane::from_key(key) {
                    self.apply_roll_action(A::SetLane(lane), window, cx);
                    cx.notify();
                    return true;
                }
            }
            self.status_message = "Carril inválido: velocity, release_velocity, pan, probability, pitch, pressure o un CC del selector".into();
            cx.notify();
            return true;
        }
        let action = match name.as_str() {
            "midi.tool.select" | "midi.tool.pointer" => A::Tool(T::Select),
            "midi.tool.draw" => A::Tool(T::Draw),
            "midi.tool.erase" => A::Tool(T::Erase),
            "midi.tool.paint" => A::Tool(T::Paint),
            "midi.tool.mute" => A::Tool(T::Mute),
            "midi.tool.box" | "midi.select.region" => A::Tool(T::Box),
            "midi.delete.region" => A::Tool(T::EraseRegion),
            "midi.select.all" => A::SelectAll,
            "midi.select.clear" => A::ClearSelection,
            "midi.delete" | "midi.delete.selection" => A::DeleteSelection,
            "midi.tool.length" => A::Tool(T::Length),
            "midi.lane" => A::ChooseLane,
            "midi.track.solo" => A::TrackSolo,
            "midi.copy" => A::Copy,
            "midi.cut" => A::Cut,
            "midi.paste" => A::Paste,
            "midi.ghost" => A::GhostNotes,
            "midi.follow" => A::Follow,
            "midi.velocities" => A::Velocities,
            "midi.scale.highlight" => A::HighlightScale,
            "midi.scale.keyboard" => A::ToggleScaleKeyboard,
            "midi.note.names" => A::NoteNames,
            "midi.clip.previous" => A::PreviousClip,
            "midi.clip.next" => A::NextClip,
            "midi.clip.select" if parts.len() == 2 => {
                let target = parts[1].parse::<u64>().ok().and_then(|id| {
                    self.timeline
                        .other_midi_clips
                        .iter()
                        .chain(self.timeline.midi_clip.iter())
                        .find(|clip| clip.id.value() == id)
                        .map(|clip| clip.id)
                });
                if let Some(id) = target {
                    self.select_roll_clip(id);
                } else {
                    self.status_message = "Clip MIDI no encontrado".into();
                }
                cx.notify();
                return true;
            }
            "midi.clip.select" => A::ChooseClip,
            "midi.clip.mute" => A::ToggleClipMute,
            "midi.clip.color" if parts.len() == 2 => {
                let color = if parts[1] == "auto" {
                    Some(None)
                } else {
                    parts[1]
                        .parse::<u8>()
                        .ok()
                        .filter(|value| *value < 7)
                        .map(Some)
                };
                match (color, self.timeline.midi_clip.as_mut()) {
                    (Some(value), Some(clip)) if !clip.props.locked => {
                        clip.props.color_index = value
                    }
                    (_, Some(clip)) if clip.props.locked => {
                        self.status_message = "Clip bloqueado".into()
                    }
                    _ => self.status_message = "Uso: midi.clip.color auto o 0–6".into(),
                }
                cx.notify();
                return true;
            }
            "midi.clip.loop" => A::ToggleClipLoop,
            "midi.clip.color" => A::ChooseClipColor,
            "midi.view.reset" => A::ResetView,
            "midi.view.zoom.in" => A::ZoomWidth(1.25),
            "midi.view.zoom.out" => A::ZoomWidth(0.8),
            "midi.view.taller" => A::ZoomHeight(1.25),
            "midi.view.shorter" => A::ZoomHeight(0.8),
            "midi.window" => A::ToggleWindow,
            "midi.close" => A::Close,
            "midi.settings" => A::OpenSettings,
            "midi.inspector.clip" => A::InspectorTab(0),
            "midi.inspector.notes" => A::InspectorTab(1),
            "midi.inspector.scale" => A::InspectorTab(2),
            "midi.inspector.view" => A::InspectorTab(3),
            "midi.clip.channel"
            | "midi.clip.rename"
            | "midi.clip.length"
            | "midi.draw.length"
            | "midi.scale.root"
            | "midi.scale.mode"
            | "midi.humanize.amount" => {
                let result = self.apply_roll_parameter(name, &parts[1..]);
                match result {
                    Ok(()) => self.status_message = "Parámetro MIDI actualizado".into(),
                    Err(error) => self.status_message = error,
                }
                self.piano_roll.hint = self.status_message.clone();
                cx.notify();
                return true;
            }
            _ => return false,
        };
        if parts.len() != 1 {
            self.status_message = format!("{name} no acepta argumentos");
        } else {
            self.apply_roll_action(action, window, cx);
        }
        cx.notify();
        true
    }
}
