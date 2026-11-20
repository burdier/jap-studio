//! Edición de puntos MIDI compartida por canvas y comandos.
use super::RollTool;
use crate::ui::DawApp;
use elevate::musical::{
    automation::ControlLane,
    time::{PPQ, Tick},
};
use gpui::{Pixels, Point};

impl DawApp {
    pub(super) fn set_roll_control(
        &mut self,
        lane: ControlLane,
        at: Tick,
        value: i16,
    ) -> Result<(), String> {
        let clip = self
            .timeline
            .midi_clip
            .as_ref()
            .ok_or("Selecciona un clip MIDI")?;
        if clip.props.locked {
            return Err("Clip bloqueado".into());
        }
        if !lane.valid(value) || at.0 < 0 || at >= clip.pattern.length() {
            return Err("Valor o posición de control fuera de rango".into());
        }
        if let Some(point) = clip
            .pattern
            .controls()
            .points()
            .iter()
            .find(|p| p.lane == lane && p.at == at && p.value == value)
        {
            self.piano_roll.selected_control = Some(point.id);
            return Ok(());
        }
        self.stop_audio();
        let clip = self
            .timeline
            .midi_clip
            .as_mut()
            .ok_or("Selecciona un clip MIDI")?;
        self.piano_roll.selected_control =
            Some(clip.edit_pattern(|pattern| pattern.set_control(lane, at, value))?);
        self.piano_roll.hint = format!(
            "{}: {value} · {}",
            lane.key(),
            self.timeline.meter.position_label(at)
        );
        Ok(())
    }

    pub(super) fn edit_roll_control_at(&mut self, position: Point<Pixels>) -> bool {
        let Some(lane) = self.piano_roll.active_lane.controller() else {
            return false;
        };
        let Some(clip) = &self.timeline.midi_clip else {
            return true;
        };
        let grid = self.piano_roll.grid.get();
        if clip.props.locked {
            self.piano_roll.hint = "Clip bloqueado".into();
            return true;
        }
        if self.piano_roll.tool == RollTool::Erase {
            let hit = clip
                .pattern
                .controls()
                .points()
                .iter()
                .filter(|p| p.lane == lane)
                .filter(|p| f32::from(grid.x(p.at.0 as f64 / PPQ as f64) - position.x).abs() <= 7.0)
                .min_by(|a, b| {
                    (f32::from(grid.x(a.at.0 as f64 / PPQ as f64) - position.x))
                        .abs()
                        .total_cmp(
                            &f32::from(grid.x(b.at.0 as f64 / PPQ as f64) - position.x).abs(),
                        )
                })
                .map(|p| p.id);
            if let Some(id) = hit {
                self.remove_roll_control(id);
            }
            return true;
        }
        let step = self.piano_roll.resolution.ticks().0;
        let beat = grid.first_beat
            + f64::from(f32::from(position.x - grid.notes.left()) / grid.pixels_per_beat);
        let at = Tick(
            ((beat * PPQ as f64 / step as f64).round() as i64 * step)
                .clamp(0, clip.pattern.length().0 - 1),
        );
        let height = (f32::from(grid.velocity.size.height) - 16.0).max(1.0);
        let fraction = f32::from(grid.velocity.bottom() - position.y) / height;
        let value = self.piano_roll.active_lane.control_value(fraction);
        if let Err(error) = self.set_roll_control(lane, at, value) {
            self.piano_roll.hint = error;
        }
        true
    }

    fn remove_roll_control(&mut self, id: u64) {
        if self
            .timeline
            .midi_clip
            .as_ref()
            .is_some_and(|clip| !clip.props.locked)
        {
            self.stop_audio();
            if let Some(clip) = &mut self.timeline.midi_clip {
                clip.edit_pattern(|pattern| pattern.remove_control(id));
            }
            self.piano_roll.selected_control = None;
            self.piano_roll.hint = "Punto MIDI eliminado".into();
        }
    }

    pub(super) fn execute_roll_control_command(&mut self, command: &str) -> bool {
        let Ok(parts) = elevate::actions::tokenize(command) else {
            return false;
        };
        let Some(name) = parts.first().map(String::as_str) else {
            return false;
        };
        if name == "midi.control.remove" {
            if parts.len() == 1 {
                if let Some(id) = self.piano_roll.selected_control {
                    self.remove_roll_control(id);
                }
            } else {
                self.status_message = "midi.control.remove no acepta argumentos".into();
            }
            return true;
        }
        if !matches!(name, "midi.cc" | "midi.pitch" | "midi.pressure") {
            return false;
        }
        let result = (|| -> Result<(), String> {
            let spec = elevate::actions::find(name).ok_or("Acción no registrada")?;
            if parts.len() != spec.parameters.len() + 1 {
                return Err(format!(
                    "Uso: {name} {}",
                    spec.parameters
                        .iter()
                        .map(|p| p.key)
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            let values = spec
                .parameters
                .iter()
                .zip(&parts[1..])
                .map(|(p, value)| match p.parse(value)? {
                    elevate::actions::ParameterValue::Number(value) => Ok(value),
                    _ => Err("Se esperaba un número".into()),
                })
                .collect::<Result<Vec<_>, String>>()?;
            let (lane, value, beat) = match name {
                "midi.cc" => (
                    ControlLane::Cc(values[0] as u8),
                    values[1] as i16,
                    values[2],
                ),
                "midi.pitch" => (ControlLane::PitchBend, values[0] as i16, values[1]),
                _ => (ControlLane::Pressure, values[0] as i16, values[1]),
            };
            self.set_roll_control(lane, Tick((beat * PPQ as f64).round() as i64), value)
        })();
        if let Err(error) = result {
            self.status_message = error.clone();
            self.piano_roll.hint = error;
        }
        true
    }
}
