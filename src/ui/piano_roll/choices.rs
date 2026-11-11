//! Opciones del editor; se presentan con el menú contextual compartido.
use crate::ui::DawApp;
use elevate::musical::clip::ClipId;

pub(in crate::ui) struct RollChoices {
    pub rows: Vec<(String, String, bool)>,
    pub selected: usize,
    pub colors: bool,
}

impl DawApp {
    pub(in crate::ui) fn choose_roll_value(&mut self, kind: &str) {
        let rows = match kind {
            "clip" => {
                let current = self.timeline.midi_clip.as_ref().map(|clip| clip.id);
                let mut clips: Vec<_> = self
                    .timeline
                    .other_midi_clips
                    .iter()
                    .chain(self.timeline.midi_clip.iter())
                    .collect();
                clips.sort_by(|a, b| {
                    a.track
                        .cmp(&b.track)
                        .then_with(|| a.start_beat.total_cmp(&b.start_beat))
                        .then_with(|| a.id.value().cmp(&b.id.value()))
                });
                clips
                    .into_iter()
                    .map(|clip| {
                        (
                            format!(
                                "{} · pista {} · compás {}",
                                clip.name,
                                clip.track + 1,
                                self.timeline.meter.bar_at(clip.start_beat)
                            ),
                            format!("midi.clip.select {}", clip.id.value()),
                            Some(clip.id) == current,
                        )
                    })
                    .collect()
            }
            "color" => (0..=7)
                .map(|index| {
                    let color = if index == 0 { None } else { Some(index - 1) };
                    (
                        if index == 0 {
                            "Automático".to_string()
                        } else {
                            format!("Color {index}")
                        },
                        format!(
                            "midi.clip.color {}",
                            color.map_or("auto".to_string(), |i| i.to_string())
                        ),
                        self.timeline
                            .midi_clip
                            .as_ref()
                            .is_some_and(|clip| clip.props.color_index == color),
                    )
                })
                .collect(),
            "root" => [
                "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
            ]
            .iter()
            .enumerate()
            .map(|(i, name)| {
                (
                    name.to_string(),
                    format!("midi.scale.root {name}"),
                    i == usize::from(self.piano_roll.scale_root),
                )
            })
            .collect(),
            "scale" => [("Mayor", "major", false), ("Menor natural", "minor", true)]
                .iter()
                .map(|(label, mode, minor)| {
                    (
                        label.to_string(),
                        format!("midi.scale.mode {mode}"),
                        *minor == self.piano_roll.scale_minor,
                    )
                })
                .collect(),
            "length" => [1, 2, 4, 8, 16, 32, 64, 128]
                .into_iter()
                .map(|division| {
                    (
                        format!("1/{division}"),
                        format!("midi.draw.length {division}"),
                        division == self.piano_roll.draw_length.denominator(),
                    )
                })
                .collect(),
            _ => return,
        };
        let mut choices = RollChoices {
            rows,
            selected: 0,
            colors: kind == "color",
        };
        choices.selected = choices.rows.iter().position(|row| row.2).unwrap_or(0);
        self.piano_roll.lane_menu = None;
        self.piano_roll.choices = Some(choices);
    }

    pub(in crate::ui) fn select_roll_clip(&mut self, id: ClipId) {
        if self
            .timeline
            .midi_clip
            .as_ref()
            .is_some_and(|clip| clip.id == id)
        {
            return;
        }
        let Some(index) = self
            .timeline
            .other_midi_clips
            .iter()
            .position(|clip| clip.id == id)
        else {
            self.status_message = "El clip ya no existe".into();
            return;
        };
        self.stop_audio();
        let clip = self.timeline.other_midi_clips.remove(index);
        self.timeline.set_edit_beat(clip.start_beat);
        if let Some(previous) = self.timeline.midi_clip.replace(clip) {
            self.timeline.other_midi_clips.push(previous);
        }
        self.open_piano_roll();
        self.piano_roll.hint = "Clip MIDI activo cambiado".into();
    }
}
