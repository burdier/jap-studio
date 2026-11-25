//! Proyección de la topología validada al mecanismo atómico ya usado por los streams.
use crate::ui::DawApp;
use elevate::musical::{model::InstrumentId, track::Track};

// Durante un clip (incluida su carga pendiente), la fuente manda sobre la
// asignación del instrumento compartido. En vivo se usa la pista asignada.
fn piano_source_track(
    tracks: &[Track],
    instrument: InstrumentId,
    playing: bool,
    clip: Option<usize>,
) -> Option<usize> {
    let clip = clip.filter(|&index| index < tracks.len());
    if playing {
        clip
    } else {
        tracks
            .iter()
            .position(|track| track.instrument == Some(instrument))
            .or(clip)
    }
}
impl DawApp {
    pub(in crate::ui) fn piano_output_track(&self) -> Option<usize> {
        piano_source_track(
            &self.tracks,
            self.instrument.id(),
            self.instrument.clip_playing(),
            self.timeline.midi_clip.as_ref().map(|clip| clip.track),
        )
    }
    pub(in crate::ui) fn track_fx_gain(&self, index: usize) -> f32 {
        let parent = self
            .tracks
            .get(index)
            .and_then(|track| track.parent)
            .and_then(|id| self.tracks.iter().position(|track| track.id == id));
        self.own_track_fx_gain(index) * parent.map_or(1.0, |parent| self.own_track_fx_gain(parent))
    }

    pub(in crate::ui) fn own_track_fx_gain(&self, index: usize) -> f32 {
        self.tracks
            .get(index)
            .filter(|track| !crate::audio::effects::needs_rack(&track.chain))
            .map_or(1.0, |track| {
                elevate::devices::fx_gain::compile(&track.chain).unwrap_or(1.0)
            })
    }
    pub(in crate::ui) fn master_fx_gain(&self) -> f32 {
        if crate::audio::effects::needs_rack(&self.master_chain) {
            1.0
        } else {
            elevate::devices::fx_gain::compile(&self.master_chain).unwrap_or(1.0)
        }
    }
    pub(in crate::ui) fn publish_track_fx(&mut self) {
        self.mix_vst();
        if let Some(track) = self.sampler_track() {
            self.sampler.mix(
                self.sampler_gain(track),
                self.track_pan(track),
                self.track_fx_gain(track),
            );
        }
        if let Some(player) = &self.player {
            for (index, track) in self.tracks.iter().enumerate() {
                player.set_fx_gain(track.id, self.own_track_fx_gain(index));
            }
            player.set_master_fx_gain(self.master_fx_gain());
        }
        let track = self.piano_output_track();
        let gain = track.map_or(1.0, |index| self.track_fx_gain(index));
        self.instrument.set_fx_gain(gain);
        let piano_gains: Vec<_> = self
            .piano_instances
            .keys()
            .filter_map(|id| {
                self.tracks
                    .iter()
                    .position(|track| track.instrument == Some(*id))
                    .map(|index| (*id, self.track_fx_gain(index)))
            })
            .collect();
        for (id, gain) in piano_gains {
            if let Some(piano) = self.piano_instances.get_mut(&id) {
                piano.set_fx_gain(gain);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::effects::{self, ComponentEffect, Rack, Routing};
    use elevate::{
        musical::{
            fx::{FxEdit, FxKind, FxNode, FxPlacement},
            track::{TrackDevice, TrackKind},
        },
        scripting::audio_components::parse,
    };
    use std::{collections::BTreeMap, sync::Arc};

    #[test]
    fn midi_clip_uses_its_filter_and_live_notes_return_to_assigned_track() {
        let instrument = InstrumentId(42);
        let mut tracks = vec![
            Track::new("Piano en vivo", TrackKind::Synth),
            Track::new("Clip MIDI", TrackKind::Synth),
        ];
        tracks[0].instrument = Some(instrument);
        let descriptor = Arc::new(
            parse(include_str!(
                "../../../assets/scripts/components/audio/filter.lua"
            ))
            .unwrap(),
        );
        let effect = ComponentEffect::prepare(descriptor.clone(), 48000).unwrap();
        let node = FxNode::new(
            "Filtro del clip",
            FxKind::Effect {
                plugin_id: TrackDevice::Hay {
                    catalog_id: descriptor.id.clone(),
                },
                parameters: descriptor
                    .controls
                    .iter()
                    .map(|c| (c.id, c.default))
                    .collect(),
            },
        );
        let id = node.id;
        tracks[1]
            .chain
            .apply(FxEdit::Insert(node, FxPlacement::Inside(None)))
            .unwrap();
        let components = BTreeMap::from([(id, effect.clone())]);
        let source = piano_source_track(&tracks, instrument, true, Some(1)).unwrap();
        let plan = effects::compile(
            &tracks[source].chain,
            &BTreeMap::new(),
            &components,
            &BTreeMap::new(),
            0,
        )
        .unwrap();
        let routing = Routing::new();
        routing.set(Some(Rack::new(plan, 0)), 48000);
        let input: Vec<_> = (0..8192)
            .map(|i| {
                let x = (std::f32::consts::TAU * 4000. * i as f32 / 48000.).sin() * 0.1;
                [x, x]
            })
            .collect();
        let energy = |frames: &[[f32; 2]]| frames[4096..].iter().map(|f| f[0] * f[0]).sum::<f32>();
        let mut low = input.clone();
        effect.set(0, 200.).unwrap();
        routing.process(&mut low);
        let mut high = input.clone();
        effect.set(0, 18000.).unwrap();
        routing.process(&mut high);
        assert!(
            energy(&high) > energy(&low) * 100.,
            "El filtro del clip debe cambiar el audio del rack activo"
        );
        let live = piano_source_track(&tracks, instrument, false, Some(1)).unwrap();
        let plan = effects::compile(
            &tracks[live].chain,
            &BTreeMap::new(),
            &components,
            &BTreeMap::new(),
            0,
        )
        .unwrap();
        routing.set(Some(Rack::new(plan, 0)), 48000);
        let mut dry = input.clone();
        routing.process(&mut dry);
        assert_eq!(dry, input);
        assert_eq!(
            piano_source_track(&tracks, instrument, true, Some(99)),
            None
        );
    }
}
