//! Estado de ejecución del piano/sintetizador SoundFont: bancos, presets, voces y ratón.
//! No importa GPUI. La vista consulta este objeto; el audio vive en `audio/piano`.
mod loading;
use super::piano_params::{self, Applies, Control, ParamId, PianoParams};
use crate::audio::piano::{
    PianoPlayer, PianoSettings, PresetEntry, SoundFontBank, discover_soundfonts,
    fluidsynth_available,
};
use elevate::devices::utility_slot::UtilitySlot;
use elevate::{
    catalog::{self, CatalogEntry},
    musical::{clip::MidiClip, keyboard::KeyboardLayout, model::InstrumentId},
};
use loading::{Loading, PendingPlayback};
use std::collections::HashSet;

const ENTRY_ID: &str = "instrument.piano";
/// Rango visible: C3–B5. Las teclas del ordenador tocan desde C4 (o según octava Z/X).
const FIRST_NOTE: u8 = 48;
const LAST_NOTE: u8 = 83;

/// Sintetizador SoundFont del prototipo. Carga cualquier banco `.sf2` descubierto en
/// `assets/soundfonts/` y expone sus presets y créditos del autor.
pub(crate) struct Piano {
    id: InstrumentId,
    player: Option<PianoPlayer>,
    fx_rack: Option<std::sync::Arc<crate::audio::effects::Rack>>,
    held: HashSet<u8>,
    mouse: Option<u8>,
    layout: KeyboardLayout,
    params: PianoParams,
    banks: Vec<SoundFontBank>,
    bank_index: usize,
    presets: Vec<PresetEntry>,
    preset_index: usize,
    pan: f32,
    fx_gain: f32,
    loading: Loading,
    output_config: crate::audio::device::AudioConfig,
}

impl Piano {
    pub fn set_fx_rack(&mut self, rack: Option<std::sync::Arc<crate::audio::effects::Rack>>) {
        if let Some(player) = &self.player {
            player.set_fx_rack(rack.clone());
        }
        self.fx_rack = rack;
    }

    pub fn drain_spectrum(&self, analyzer: &mut crate::audio::analyzer::SpectrumAnalyzer) {
        if let Some(player) = &self.player {
            player.drain_spectrum(analyzer);
        }
    }

    pub fn set_spectrum_enabled(&self, enabled: bool) {
        if let Some(player) = &self.player {
            player.set_spectrum_enabled(enabled);
        }
    }

    pub fn new() -> Self {
        Self::new_with_id(InstrumentId(1))
    }

    pub fn new_with_id(id: InstrumentId) -> Self {
        Self {
            id,
            player: None,
            fx_rack: None,
            held: HashSet::new(),
            mouse: None,
            layout: KeyboardLayout::new(FIRST_NOTE, LAST_NOTE),
            params: PianoParams::new(),
            banks: Vec::new(),
            bank_index: 0,
            presets: Vec::new(),
            preset_index: 0,
            pan: 0.0,
            fx_gain: 1.0,
            loading: Loading::default(),
            output_config: Default::default(),
        }
    }

    /// Identidad estable de esta instancia.
    pub fn id(&self) -> InstrumentId {
        self.id
    }

    pub fn descriptor(&self) -> &'static CatalogEntry {
        catalog::find(ENTRY_ID).expect("el catálogo debe declarar instrument.piano")
    }

    pub fn active_name(&self) -> &str {
        self.banks
            .get(self.bank_index)
            .map(|bank| bank.name.as_str())
            .unwrap_or_else(|| self.descriptor().name)
    }

    pub fn active_credits(&self) -> &str {
        self.banks
            .get(self.bank_index)
            .map(|bank| bank.credits.as_str())
            .unwrap_or_else(|| self.descriptor().summary)
    }

    pub fn banks(&self) -> &[SoundFontBank] {
        &self.banks
    }

    pub fn bank_index(&self) -> usize {
        self.bank_index
    }

    pub fn presets(&self) -> &[PresetEntry] {
        &self.presets
    }

    pub fn preset_index(&self) -> usize {
        self.preset_index
    }

    pub fn select_bank(&mut self, index: usize) -> Result<(), String> {
        if index >= self.banks.len() {
            return Err("Banco fuera de rango".into());
        }
        if self.bank_index != index {
            self.stop();
            self.invalidate_resources();
            self.bank_index = index;
            self.presets.clear();
            self.preset_index = 0;
        }
        self.request_resources();
        Ok(())
    }

    pub fn step_bank(&mut self, delta: isize) -> Result<(), String> {
        if self.banks.is_empty() {
            self.request_resources();
            return Err("Preparando la lista de bancos SoundFont…".into());
        }
        let len = self.banks.len() as isize;
        let next = (self.bank_index as isize + delta).rem_euclid(len) as usize;
        self.select_bank(next)
    }

    pub fn select_preset(&mut self, index: usize) {
        if index >= self.presets.len() {
            return;
        }
        self.preset_index = index;
        self.all_notes_off();
        if let Some(player) = &self.player {
            send_preset(player, &self.presets[index]);
        }
    }

    pub fn step_preset(&mut self, delta: isize) {
        if self.presets.is_empty() {
            return;
        }
        let len = self.presets.len() as isize;
        let next = (self.preset_index as isize + delta).rem_euclid(len) as usize;
        self.select_preset(next);
    }

    pub fn layout(&self) -> &KeyboardLayout {
        &self.layout
    }

    pub fn held(&self, note: u8) -> bool {
        self.held.contains(&note)
    }

    /// Estado visual de una nota cuya voz pertenece al transporte compartido.
    pub fn note_on_shared(&mut self, note: u8) -> bool {
        note <= 127 && self.held.insert(note)
    }
    pub fn note_off_shared(&mut self, note: u8) -> bool {
        self.held.remove(&note)
    }

    pub fn set_mouse_note(&mut self, note: Option<u8>) {
        self.mouse = note;
    }

    pub fn take_mouse_note(&mut self) -> Option<u8> {
        self.mouse.take()
    }

    pub fn param(&self, id: ParamId) -> f32 {
        self.params.get(id)
    }

    /// Aplica un parámetro. `true` si exige reabrir el sintetizador (se hace en el
    /// siguiente note-on y corta las notas que suenen ahora).
    pub fn set_param(&mut self, id: ParamId, value: f32) -> bool {
        let changed = self.params.get(id) != value;
        let value = self.params.set(id, value);
        if changed && piano_params::spec_of(id).applies == Applies::Restart {
            self.invalidate_resources();
        }
        match piano_params::spec_of(id).applies {
            Applies::Live => {
                if let Some(player) = &self.player {
                    send(player, id, value);
                    if matches!(
                        id,
                        ParamId::FilterEnabled
                            | ParamId::FilterMode
                            | ParamId::Cutoff
                            | ParamId::Resonance
                    ) {
                        player.set_filter(self.params.filter());
                    }
                }
                false
            }
            Applies::Restart if fluidsynth_available() => {
                if let Some(player) = &self.player {
                    match id {
                        ParamId::Polyphony => player.set_polyphony(value as usize),
                        ParamId::Effects => player.set_effects(value >= 0.5),
                        _ => {}
                    }
                }
                false
            }
            Applies::Restart if changed && self.player.is_some() => {
                self.stop();
                true
            }
            Applies::Restart => false,
        }
    }

    pub fn is_running(&self) -> bool {
        self.player.is_some()
    }

    pub fn sample_rate(&self) -> Option<u32> {
        self.player.as_ref().map(|player| player.sample_rate)
    }

    /// Datos propios: el worker construye voces nuevas sin leer estado vivo ni abrir CPAL.
    pub fn export_snapshot(&self) -> Result<crate::audio::piano::PianoSnapshot, String> {
        let banks = if self.banks.is_empty() {
            discover_soundfonts()
        } else {
            self.banks.clone()
        };
        let bank = banks
            .get(self.bank_index)
            .ok_or("No hay SoundFont para exportar MIDI")?;
        let mut midi = Vec::new();
        if let Some(preset) = self.presets.get(self.preset_index) {
            midi.extend([(0xB0, 0x00, preset.bank), (0xC0, preset.patch, 0)]);
        }
        let mut master_volume = 0.7;
        for spec in piano_params::SPECS {
            for control in piano_params::controls(spec.id, self.params.get(spec.id)) {
                match control {
                    Control::Midi(command, a, b) => midi.push((command, a, b)),
                    Control::MasterVolume(value) => master_volume = value,
                }
            }
        }
        Ok(crate::audio::piano::PianoSnapshot {
            path: bank.path.clone(),
            midi,
            master_volume,
            filter: self.params.filter(),
            settings: PianoSettings {
                maximum_polyphony: self.params.get(ParamId::Polyphony) as usize,
                reverb_and_chorus: self.params.get(ParamId::Effects) >= 0.5,
            },
        })
    }

    /// Abre el dispositivo en la primera nota. Devuelve si la orden se aceptó.
    fn ensure_started(&mut self, gain: f32, utility: UtilitySlot) -> Result<bool, String> {
        if self.player.is_some() {
            return Ok(true);
        }
        let Some(prepared) = self.loading.prepared.take() else {
            if let Some(error) = &self.loading.error {
                return Err(error.clone());
            }
            self.request_resources();
            return Ok(false);
        };
        let player = prepared
            .start(gain, utility, self.pan, self.fx_gain)
            .map_err(|error| {
                self.loading.error = Some(error.clone());
                error
            })?;
        if let Some(preset) = self.presets.get(self.preset_index) {
            send_preset(&player, preset);
        }
        for spec in piano_params::SPECS
            .iter()
            .filter(|s| s.applies == Applies::Live)
        {
            send(&player, spec.id, self.params.get(spec.id));
        }
        player.set_fx_rack(self.fx_rack.clone());
        player.set_filter(self.params.filter());
        self.player = Some(player);
        Ok(true)
    }

    pub fn note_on(
        &mut self,
        note: u8,
        velocity: u8,
        gain: f32,
        utility: UtilitySlot,
    ) -> Result<bool, String> {
        if note > 127 || self.held.contains(&note) {
            return Ok(false);
        }
        if !self.ensure_started(gain, utility.clone())? {
            let pending = self
                .loading
                .pending
                .get_or_insert_with(|| PendingPlayback::Notes {
                    notes: Default::default(),
                    gain,
                    utility,
                });
            if let PendingPlayback::Notes { notes, .. } = pending {
                notes.insert(note, velocity);
            }
        }
        if let Some(player) = &self.player {
            player.note_on(note, velocity);
        }
        self.held.insert(note);
        Ok(true)
    }

    pub fn play_clip(
        &mut self,
        clip: &MidiClip,
        edit_beat: f64,
        bpm: f64,
        gain: f32,
        utility: UtilitySlot,
    ) -> Result<(), String> {
        self.all_notes_off();
        if !self.ensure_started(gain, utility.clone())? {
            self.loading.pending = Some(PendingPlayback::Clip {
                clip: clip.clone(),
                beat: edit_beat,
                bpm,
                gain,
                utility,
            });
            return Ok(());
        }
        let offset_beats = (edit_beat - clip.start_beat).max(0.0);
        if let Some(player) = &self.player {
            player.set_channel(clip.channel.saturating_sub(1).min(15));
            if let Some(preset) = self.presets.get(self.preset_index) {
                send_preset(player, preset);
            }
            for spec in piano_params::SPECS {
                send(player, spec.id, self.params.get(spec.id));
            }
            player.play_pattern(
                &clip.playback_pattern(),
                offset_beats,
                bpm,
                clip.props.looped,
            )?;
        }
        Ok(())
    }

    pub fn stop_clip(&mut self) {
        if matches!(self.loading.pending, Some(PendingPlayback::Clip { .. })) {
            self.loading.pending = None;
        }
        if let Some(player) = &self.player {
            player.stop_pattern();
        }
    }

    pub fn clip_playing(&self) -> bool {
        matches!(self.loading.pending, Some(PendingPlayback::Clip { .. }))
            || self.player.as_ref().is_some_and(PianoPlayer::plan_playing)
    }

    pub fn clip_finished(&self) -> bool {
        self.player.as_ref().is_some_and(PianoPlayer::plan_finished)
    }

    pub fn clip_beat(&self, clip_start_beat: f64, bpm: f64) -> Option<f64> {
        if let Some(PendingPlayback::Clip { beat, .. }) = self.loading.pending {
            return Some(beat);
        }
        let player = self.player.as_ref()?;
        if !player.plan_playing() {
            return None;
        }
        let seconds = player.plan_frame() as f64 / f64::from(player.sample_rate);
        Some(clip_start_beat + seconds * (bpm / 60.0))
    }

    pub fn note_off(&mut self, note: u8) -> bool {
        if !self.held.remove(&note) {
            return false;
        }
        if let Some(PendingPlayback::Notes { notes, .. }) = &mut self.loading.pending {
            notes.remove(&note);
            if notes.is_empty() {
                self.loading.pending = None;
            }
        }
        if let Some(player) = &self.player {
            player.note_off(note);
        }
        true
    }

    pub fn all_notes_off(&mut self) -> bool {
        if matches!(self.loading.pending, Some(PendingPlayback::Notes { .. })) {
            self.loading.pending = None;
        }
        self.mouse = None;
        if self.held.is_empty() {
            return false;
        }
        if let Some(player) = &self.player {
            player.all_notes_off();
        }
        self.held.clear();
        true
    }

    pub fn stop(&mut self) {
        self.loading.pending = None;
        self.all_notes_off();
        self.player = None;
        self.held.clear();
        self.mouse = None;
    }

    pub fn set_fx_gain(&mut self, gain: f32) {
        self.fx_gain = gain;
        if let Some(player) = &self.player {
            player.set_fx_gain(gain);
        }
    }
    pub fn set_gain(&mut self, gain: f32) {
        if let Some(
            PendingPlayback::Notes { gain: pending, .. }
            | PendingPlayback::Clip { gain: pending, .. },
        ) = &mut self.loading.pending
        {
            *pending = gain;
        }
        if let Some(player) = &self.player {
            player.set_gain(gain);
        }
    }

    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan.clamp(-1.0, 1.0);
        if let Some(player) = &self.player {
            player.set_pan(self.pan);
        }
    }

    pub fn failed(&self) -> bool {
        self.player.as_ref().is_some_and(PianoPlayer::failed)
    }
}

#[cfg(test)]
mod instance_tests {
    use super::*;

    #[test]
    fn two_pianos_keep_distinct_ids_and_parameter_state() {
        let mut first = Piano::new_with_id(InstrumentId(1));
        let second = Piano::new_with_id(InstrumentId(1000));
        first.set_param(ParamId::Cutoff, 800.0);
        assert_ne!(first.id(), second.id());
        assert_ne!(first.param(ParamId::Cutoff), second.param(ParamId::Cutoff));
    }
}

fn send_preset(player: &PianoPlayer, preset: &PresetEntry) {
    player.midi(0xB0, 0x00, preset.bank);
    player.midi(0xC0, preset.patch, 0);
}

fn send(player: &PianoPlayer, id: ParamId, value: f32) {
    for control in piano_params::controls(id, value) {
        match control {
            Control::Midi(command, data1, data2) => player.midi(command, data1, data2),
            Control::MasterVolume(volume) => player.set_master_volume(volume),
        }
    }
}
