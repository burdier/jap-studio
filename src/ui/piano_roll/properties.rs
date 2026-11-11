//! Propiedades MIDI validadas antes de modificar el clip o sus preferencias.
use crate::ui::DawApp;
use elevate::musical::{grid::NoteGrid, time::Tick};

impl DawApp {
    pub(super) fn toggle_roll_clip_property(&mut self, loop_property: bool) {
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        if clip.props.locked {
            self.piano_roll.hint = "Clip bloqueado".into();
            return;
        }
        self.stop_audio();
        if let Some(clip) = self.timeline.midi_clip.as_mut() {
            if loop_property {
                clip.props.looped = !clip.props.looped;
            } else {
                clip.props.muted = !clip.props.muted;
            }
        }
        self.piano_roll.hint = "Propiedad aplicada; Play escucha el plan actualizado".into();
    }

    pub(super) fn apply_roll_parameter(
        &mut self,
        name: &str,
        args: &[String],
    ) -> Result<(), String> {
        let spec = elevate::actions::find(name).ok_or("Acción desconocida")?;
        if args.len() != 1 {
            return Err(format!("Uso: {name} <{}>", spec.parameters[0].label));
        }
        spec.parameters[0].parse(&args[0])?;
        let value = args[0].trim();
        match name {
            "midi.clip.channel" | "midi.clip.rename" => {
                let clip = self
                    .timeline
                    .midi_clip
                    .as_ref()
                    .ok_or("Selecciona un clip MIDI")?;
                if clip.props.locked {
                    return Err("Clip bloqueado".into());
                }
                if name == "midi.clip.channel" {
                    let channel = value.parse().map_err(|_| "Canal MIDI inválido")?;
                    self.stop_audio();
                    if let Some(clip) = self.timeline.midi_clip.as_mut() {
                        clip.channel = channel;
                    }
                } else if let Some(clip) = self.timeline.midi_clip.as_mut() {
                    clip.name = value.to_string();
                }
            }
            "midi.clip.length" => {
                let bars: f64 = value
                    .replace(',', ".")
                    .parse()
                    .map_err(|_| "Longitud inválida")?;
                self.resize_roll_pattern(Tick(
                    (bars * self.timeline.meter.ticks_per_bar() as f64).round() as i64,
                ))?;
            }
            "midi.draw.length" => {
                self.piano_roll.draw_length =
                    NoteGrid::new(value.parse().map_err(|_| "Figura inválida")?)?
            }
            "midi.humanize.amount" => {
                self.piano_roll.humanize_amount =
                    value.parse().map_err(|_| "Porcentaje inválido")?
            }
            "midi.scale.root" => {
                self.piano_roll.scale_root =
                    parse_root(value).ok_or("Tónica inválida; usa C, C#, Db… B")?
            }
            "midi.scale.mode" => {
                self.piano_roll.scale_minor = match value.to_lowercase().as_str() {
                    "minor" | "menor" => true,
                    "major" | "mayor" => false,
                    _ => return Err("Escala inválida; usa major o minor".into()),
                }
            }
            _ => return Err("Parámetro desconocido".into()),
        }
        Ok(())
    }

    fn resize_roll_pattern(&mut self, length: Tick) -> Result<(), String> {
        let clip = self
            .timeline
            .midi_clip
            .as_ref()
            .ok_or("Selecciona un clip MIDI")?;
        if clip.props.locked {
            return Err("Clip bloqueado".into());
        }
        if self.piano_roll.drag.is_some() {
            return Err("Termina el arrastre antes de cambiar la longitud".into());
        }
        let mut pattern = clip.pattern.clone();
        pattern.resize(length).map_err(|error| error.to_string())?;
        let beats = length.0 as f64 / elevate::musical::time::PPQ as f64;
        if clip.trim_start >= beats {
            return Err("La longitud debe superar el inicio del recorte".into());
        }
        self.stop_audio();
        let clip = self
            .timeline
            .midi_clip
            .as_mut()
            .ok_or("Selecciona un clip MIDI")?;
        clip.pattern = pattern;
        if let Some(trim) = &mut clip.trim_len {
            *trim = trim.min(beats - clip.trim_start);
        }
        self.piano_roll.cursor.0 = self.piano_roll.cursor.0.min(length.0 - 1);
        let viewport = &mut self.piano_roll.viewport;
        viewport.first_beat = viewport
            .first_beat
            .min((beats - beats / viewport.zoom).max(0.0));
        Ok(())
    }
}

pub(super) fn parse_root(value: &str) -> Option<u8> {
    match value
        .to_uppercase()
        .replace('♯', "#")
        .replace('♭', "B")
        .as_str()
    {
        "C" | "B#" => Some(0),
        "C#" | "DB" => Some(1),
        "D" => Some(2),
        "D#" | "EB" => Some(3),
        "E" | "FB" => Some(4),
        "F" | "E#" => Some(5),
        "F#" | "GB" => Some(6),
        "G" => Some(7),
        "G#" | "AB" => Some(8),
        "A" => Some(9),
        "A#" | "BB" => Some(10),
        "B" | "CB" => Some(11),
        _ => None,
    }
}
