//! Estado de ejecución de Hay Synth (sin GPUI). Reutiliza el reproductor común de instrumentos.
use crate::audio::piano::PianoPlayer;
use elevate::{
    devices::{synth::SynthParams, utility_slot::UtilitySlot},
    musical::model::InstrumentId,
};
use std::collections::HashSet;

pub(crate) const SYNTH_ID: InstrumentId = InstrumentId(4);

pub(crate) struct Synth {
    pub params: SynthParams,
    player: Option<PianoPlayer>,
    fx_rack: Option<std::sync::Arc<crate::audio::effects::Rack>>,
    output_config: crate::audio::device::AudioConfig,
    held: HashSet<u8>,
    pub mouse: Option<u8>,
    pub last_velocity: f32,
    pub last_note: u8,
}

impl Default for Synth {
    fn default() -> Self {
        Self {
            params: SynthParams::default(),
            player: None,
            fx_rack: None,
            output_config: Default::default(),
            held: HashSet::new(),
            mouse: None,
            last_velocity: 0.0,
            last_note: 60,
        }
    }
}

impl Synth {
    pub fn from_params(params: SynthParams) -> Self {
        Self {
            params,
            ..Self::default()
        }
    }

    pub fn set_fx_rack(&mut self, rack: Option<std::sync::Arc<crate::audio::effects::Rack>>) {
        if let Some(player) = &self.player {
            player.set_fx_rack(rack.clone());
        }
        self.fx_rack = rack;
    }

    pub fn configure_audio(&mut self, config: crate::audio::device::AudioConfig) {
        self.stop();
        self.output_config = config;
    }

    pub fn update_live(&mut self) {
        self.params.clamp_all();
        if let Some(player) = &self.player {
            player.set_synth_wavetable(0, self.params.custom_wt_a.clone());
            player.set_synth_wavetable(1, self.params.custom_wt_b.clone());
            for (id, v) in self.params.all_param_pairs() {
                player.synth_control(id, v);
            }
        }
    }

    pub fn update_live_wavetable(
        &mut self,
        slot: u8,
        table: Option<std::sync::Arc<elevate::devices::synth::CustomWavetable>>,
    ) {
        if let Some(player) = &self.player {
            player.set_synth_wavetable(slot, table);
        }
    }

    pub fn update_live_param(&mut self, cc: u8, value: f32) {
        if let Some(player) = &self.player {
            player.sampler_control(cc, value);
        }
    }

    pub fn update_live_voice_param(&mut self, id: u16, value: f32) {
        if let Some(player) = &self.player {
            player.synth_control(id, value);
        }
    }

    pub fn snapshot(&self) -> SynthParams {
        self.params.clone()
    }

    pub fn name(&self) -> &str {
        if self.params.preset_name.is_empty() {
            "Hay Synth"
        } else {
            &self.params.preset_name
        }
    }

    pub fn active_note_count(&self) -> usize {
        self.held.len()
    }

    pub fn held(&self, note: u8) -> bool {
        self.held.contains(&note)
    }

    pub fn failed(&self) -> bool {
        self.player.as_ref().is_some_and(|p| p.failed())
    }

    pub fn midi(&self, command: u8, data1: u8, data2: u8) {
        if let Some(player) = &self.player {
            player.midi(command, data1, data2);
        }
    }

    pub fn mix(&self, gain: f32, pan: f32, fx: f32) {
        self.set_gain(gain);
        self.set_pan(pan);
        self.set_fx_gain(fx);
    }

    pub fn silence(&mut self) {
        self.stop();
    }

    pub fn note_on_shared(&mut self, note: u8, velocity: u8) -> bool {
        if note <= 127 && self.held.insert(note) {
            self.last_note = note;
            self.last_velocity = (velocity as f32 / 127.0).clamp(0.05, 1.0);
            true
        } else {
            false
        }
    }

    pub fn note_off_shared(&mut self, note: u8) -> bool {
        self.held.remove(&note)
    }

    pub fn start(
        &mut self,
        gain: f32,
        utility: UtilitySlot,
        pan: f32,
        fx: f32,
    ) -> Result<(), String> {
        if self.player.is_none() {
            let player = PianoPlayer::start_synth(
                self.params.clone(),
                gain,
                utility,
                pan,
                fx,
                &self.output_config,
            )?;
            if let Some(rack) = &self.fx_rack {
                player.set_fx_rack(Some(rack.clone()));
            }
            self.player = Some(player);
        }
        Ok(())
    }

    pub fn note_on(
        &mut self,
        note: u8,
        velocity: u8,
        gain: f32,
        utility: UtilitySlot,
        pan: f32,
        fx: f32,
    ) -> Result<bool, String> {
        if note > 127 || !self.held.insert(note) {
            return Ok(false);
        }
        self.last_note = note;
        self.last_velocity = (velocity as f32 / 127.0).clamp(0.05, 1.0);
        if let Err(e) = self.start(gain, utility, pan, fx) {
            self.held.remove(&note);
            return Err(e);
        }
        if let Some(player) = &self.player {
            player.set_gain(gain);
            player.set_pan(pan);
            player.set_fx_gain(fx);
            player.note_on(note, velocity);
        }
        Ok(true)
    }

    pub fn note_off(&mut self, note: u8) -> bool {
        if !self.held.remove(&note) {
            return false;
        }
        if let Some(player) = &self.player {
            player.note_off(note);
        }
        true
    }

    pub fn set_gain(&self, gain: f32) {
        if let Some(player) = &self.player {
            player.set_gain(gain);
        }
    }

    pub fn set_pan(&self, pan: f32) {
        if let Some(player) = &self.player {
            player.set_pan(pan);
        }
    }

    pub fn set_fx_gain(&self, gain: f32) {
        if let Some(player) = &self.player {
            player.set_fx_gain(gain);
        }
    }

    pub fn all_notes_off(&mut self) {
        self.held.clear();
        if let Some(player) = &self.player {
            player.all_notes_off();
        }
    }

    pub fn stop(&mut self) {
        self.held.clear();
        self.player = None;
    }

    pub fn release_audio(&mut self) {
        self.stop();
    }
}
