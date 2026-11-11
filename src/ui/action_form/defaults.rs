//! Valores iniciales de acciones nativas; no modifica el contrato del formulario.
use crate::ui::{DawApp, fx::FxTarget};
use std::collections::BTreeMap;

impl DawApp {
    pub(super) fn action_defaults(
        &self,
        command: &str,
        fx_target: Option<FxTarget>,
    ) -> BTreeMap<&'static str, String> {
        match command {
            "midi.cc" | "midi.pitch" | "midi.pressure" => {
                let mut values = BTreeMap::from([(
                    "beat",
                    (self.piano_roll.cursor.0 as f64 / elevate::musical::time::PPQ as f64)
                        .to_string(),
                )]);
                if command == "midi.cc" {
                    let cc = match self.piano_roll.active_lane.controller() {
                        Some(elevate::musical::automation::ControlLane::Cc(cc)) => cc,
                        _ => 1,
                    };
                    values.insert("controller", cc.to_string());
                }
                values
            }
            "midi.clip.channel" => BTreeMap::from([(
                "channel",
                self.timeline
                    .midi_clip
                    .as_ref()
                    .map_or(1, |clip| clip.channel)
                    .to_string(),
            )]),
            "midi.clip.rename" => BTreeMap::from([(
                "name",
                self.timeline
                    .midi_clip
                    .as_ref()
                    .map_or(String::new(), |clip| clip.name.clone()),
            )]),
            "midi.clip.length" => BTreeMap::from([(
                "bars",
                self.timeline
                    .midi_clip
                    .as_ref()
                    .map_or(4.0, |clip| {
                        clip.pattern_beats() / self.timeline.meter.beats_per_bar()
                    })
                    .to_string(),
            )]),
            "midi.draw.length" => BTreeMap::from([(
                "division",
                self.piano_roll.draw_length.denominator().to_string(),
            )]),
            "midi.grid" => BTreeMap::from([(
                "division",
                self.piano_roll.resolution.denominator().to_string(),
            )]),
            "midi.scale.root" => BTreeMap::from([(
                "root",
                [
                    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
                ][usize::from(self.piano_roll.scale_root)]
                .to_string(),
            )]),
            "midi.scale.mode" => BTreeMap::from([(
                "mode",
                if self.piano_roll.scale_minor {
                    "minor"
                } else {
                    "major"
                }
                .to_string(),
            )]),
            "midi.humanize.amount" => {
                BTreeMap::from([("amount", self.piano_roll.humanize_amount.to_string())])
            }
            "midi.quantize" => BTreeMap::from([
                ("grid", self.piano_roll.resolution.denominator().to_string()),
                ("strength", "100".into()),
            ]),
            "midi.subdivide" => {
                BTreeMap::from([("grid", self.piano_roll.resolution.denominator().to_string())])
            }
            "midi.humanize" => {
                BTreeMap::from([("amount", self.piano_roll.humanize_amount.to_string())])
            }
            "midi.gate" => BTreeMap::from([("amount", "100".into())]),
            "midi.swing" => BTreeMap::from([("amount", "0".into())]),
            "fx.rename" => BTreeMap::from([(
                "name",
                fx_target
                    .and_then(|target| self.fx_node(target))
                    .map_or(String::new(), |node| node.name.clone()),
            )]),
            "fx.parameters" => {
                let gain = fx_target
                    .and_then(|target| self.fx_node(target))
                    .and_then(|node| match &node.kind {
                        elevate::musical::fx::FxKind::Effect { parameters, .. } => {
                            parameters.get(&0).copied()
                        }
                        _ => None,
                    })
                    .unwrap_or(1.0);
                BTreeMap::from([("gain", gain.to_string())])
            }
            "export" => self.settings.export.saved.action_defaults(),
            "tempo.set" => BTreeMap::from([("bpm", self.timeline.tempo.bpm().to_string())]),
            "meter.set" => BTreeMap::from([
                ("numerator", self.timeline.meter.numerator().to_string()),
                ("denominator", self.timeline.meter.denominator().to_string()),
            ]),
            "track.height" => {
                BTreeMap::from([("height", self.track_height(self.cursor_row).to_string())])
            }
            "track.rename" => BTreeMap::from([(
                "name",
                self.tracks
                    .get(self.cursor_row)
                    .map_or(String::new(), |t| t.name.to_string()),
            )]),
            "utility.gain" => BTreeMap::from([("gain", self.utility.state().gain.to_string())]),
            _ => BTreeMap::new(),
        }
    }
}
