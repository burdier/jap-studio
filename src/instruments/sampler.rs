//! Estado de ejecución del sampler; reutiliza el reproductor y scheduler de instrumentos.
pub(crate) use crate::audio::sampler::SampleAsset;
use crate::audio::{
    piano::PianoPlayer,
    sampler::{SamplerSnapshot, SfizzEngine},
};
use elevate::{
    devices::{sampler::SamplerParams, utility_slot::UtilitySlot},
    musical::{clip::MidiClip, model::InstrumentId},
};
use std::{collections::HashSet, sync::Arc};

pub(crate) const SAMPLER_ID: InstrumentId = InstrumentId(2);
pub(crate) struct Sampler {
    pub params: SamplerParams,
    pub asset: Option<Arc<SampleAsset>>,
    player: Option<PianoPlayer>,
    prepared: Option<SfizzEngine>,
    held: HashSet<u8>,
    pub mouse: Option<u8>,
}
impl Default for Sampler {
    fn default() -> Self {
        Self {
            params: SamplerParams::default(),
            asset: None,
            player: None,
            prepared: None,
            held: HashSet::new(),
            mouse: None,
        }
    }
}
impl Sampler {
    pub fn prepare(asset: Arc<SampleAsset>, params: SamplerParams) -> Result<SfizzEngine, String> {
        SfizzEngine::prepare(SamplerSnapshot { asset, params }, 48000)
    }
    pub fn update_live(&mut self) {
        if let Some(engine) = &mut self.prepared {
            engine.parameters(&self.params);
        }
        if let Some(player) = &self.player {
            for id in 0..elevate::devices::sampler::PARAMS.len() {
                if let Some((cc, v)) = elevate::devices::sampler::live_control(&self.params, id) {
                    player.sampler_control(cc, v);
                }
            }
        }
    }
    pub fn snapshot(&self) -> Result<SamplerSnapshot, String> {
        Ok(SamplerSnapshot {
            asset: self.asset.clone().ok_or("Carga un sample en Hay Sampler")?,
            params: self.params.clone(),
        })
    }
    pub fn install(&mut self, asset: Arc<SampleAsset>, engine: SfizzEngine) {
        self.stop();
        self.asset = Some(asset);
        self.prepared = Some(engine);
        self.update_live();
    }
    pub fn ready(&self) -> bool {
        self.prepared.is_some() || self.player.is_some()
    }
    pub fn name(&self) -> &str {
        self.asset
            .as_ref()
            .map_or("Hay Sampler", |a| a.file.name.as_str())
    }
    pub fn held(&self, note: u8) -> bool {
        self.held.contains(&note)
    }
    pub fn start(
        &mut self,
        gain: f32,
        utility: UtilitySlot,
        pan: f32,
        fx: f32,
    ) -> Result<(), String> {
        if self.player.is_none() {
            let engine = self
                .prepared
                .take()
                .ok_or("Sampler preparándose; espera a que termine la carga")?;
            self.player = Some(PianoPlayer::start_sampler(engine, gain, utility, pan, fx)?);
        }
        self.mix(gain, pan, fx);
        Ok(())
    }
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        if note > 127 {
            return;
        }
        if let Some(player) = &self.player {
            if self.held.insert(note) {
                player.note_on(note, velocity);
            }
        }
    }
    pub fn note_off(&mut self, note: u8) {
        self.held.remove(&note);
        if let Some(player) = &self.player {
            player.note_off(note);
        }
    }
    pub fn all_off(&mut self) {
        self.held.clear();
        self.mouse = None;
        if let Some(player) = &self.player {
            player.all_notes_off();
        }
    }
    pub fn midi(&self, command: u8, a: u8, b: u8) {
        if let Some(player) = &self.player {
            player.midi(command, a, b);
        }
    }
    pub fn play_clip(&mut self, clip: &MidiClip, beat: f64, bpm: f64) -> Result<(), String> {
        self.all_off();
        self.player
            .as_ref()
            .ok_or("Sampler no preparado")?
            .play_pattern(
                &clip.playback_pattern(),
                (beat - clip.start_beat).max(0.),
                bpm,
            )
    }
    pub fn clip_playing(&self) -> bool {
        self.player.as_ref().is_some_and(PianoPlayer::plan_playing)
    }
    pub fn clip_finished(&self) -> bool {
        self.player.as_ref().is_some_and(PianoPlayer::plan_finished)
    }
    pub fn beat(&self, start: f64, bpm: f64) -> Option<f64> {
        self.player
            .as_ref()
            .filter(|p| p.plan_playing())
            .map(|p| start + p.plan_frame() as f64 / p.sample_rate as f64 * bpm / 60.)
    }
    pub fn stop_clip(&mut self) {
        if let Some(player) = &self.player {
            player.stop_pattern();
        }
    }
    pub fn failed(&self) -> bool {
        self.player.as_ref().is_some_and(PianoPlayer::failed)
    }
    /// Stop musical conserva el motor preparado para el siguiente note-on.
    pub fn silence(&mut self) {
        self.stop_clip();
        self.all_off();
    }
    pub fn stop(&mut self) {
        self.silence();
        self.player = None;
        self.prepared = None;
    }
    pub fn mix(&self, gain: f32, pan: f32, fx: f32) {
        if let Some(p) = &self.player {
            p.set_gain(gain);
            p.set_pan(pan);
            p.set_fx_gain(fx);
        }
    }
}
