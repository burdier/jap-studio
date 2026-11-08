//! Estado de ejecución del piano/sintetizador SoundFont: bancos, presets, voces y ratón.
//! No importa GPUI. La vista consulta este objeto; el audio vive en `audio/piano`.
use super::piano_params::{self, Applies, Control, ParamId, PianoParams};
use crate::audio::piano::{
    PianoPlayer, PianoSettings, PresetEntry, SoundFontBank, discover_soundfonts,
    fluidsynth_available, load_soundfont, presets_of,
};
use elevate::{
    catalog::{self, CatalogEntry},
    musical::{clip::MidiClip, keyboard::KeyboardLayout, model::InstrumentId},
};
use std::collections::HashSet;

const ENTRY_ID: &str = "instrument.piano";
/// Rango visible: C3–B5. Las teclas del ordenador tocan desde C4 (o según octava Z/X).
const FIRST_NOTE: u8 = 48;
const LAST_NOTE: u8 = 83;

/// Sintetizador SoundFont del prototipo. Carga cualquier banco `.sf2` descubierto en
/// `assets/soundfonts/` y expone sus presets y créditos del autor.
pub(crate) struct Piano {
    player: Option<PianoPlayer>,
    held: HashSet<u8>,
    mouse: Option<u8>,
    layout: KeyboardLayout,
    params: PianoParams,
    banks: Vec<SoundFontBank>,
    bank_index: usize,
    presets: Vec<PresetEntry>,
    preset_index: usize,
}

impl Piano {
    pub fn new() -> Self {
        let banks = discover_soundfonts();
        let presets = banks
            .first()
            .and_then(|bank| load_soundfont(&bank.path).ok())
            .map(|sf| presets_of(&sf))
            .unwrap_or_default();
        Self {
            player: None,
            held: HashSet::new(),
            mouse: None,
            layout: KeyboardLayout::new(FIRST_NOTE, LAST_NOTE),
            params: PianoParams::new(),
            banks,
            bank_index: 0,
            presets,
            preset_index: 0,
        }
    }

    /// Identidad estable de la instancia del prototipo.
    pub fn id(&self) -> InstrumentId {
        InstrumentId(1)
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

    /// Re-escanea `assets/soundfonts` por si terminaron de descargarse bancos nuevos.
    pub fn refresh_banks(&mut self) {
        let current_path = self.banks.get(self.bank_index).map(|b| b.path.clone());
        self.banks = discover_soundfonts();
        if let Some(path) = current_path
            && let Some(pos) = self.banks.iter().position(|b| b.path == path)
        {
            self.bank_index = pos;
        } else {
            self.bank_index = 0;
        }
        if self.presets.is_empty()
            && let Some(bank) = self.banks.get(self.bank_index)
            && let Ok(sf) = load_soundfont(&bank.path)
        {
            self.presets = presets_of(&sf);
        }
    }

    pub fn select_bank(&mut self, index: usize) -> Result<(), String> {
        if index >= self.banks.len() {
            return Err("Banco fuera de rango".into());
        }
        let sf = load_soundfont(&self.banks[index].path)?;
        self.presets = presets_of(&sf);
        self.preset_index = 0;
        if self.bank_index != index {
            self.bank_index = index;
            self.stop();
        }
        Ok(())
    }

    pub fn step_bank(&mut self, delta: isize) -> Result<(), String> {
        self.refresh_banks();
        if self.banks.is_empty() {
            return Err("No hay bancos SoundFont disponibles".into());
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
        match piano_params::spec_of(id).applies {
            Applies::Live => {
                if let Some(player) = &self.player {
                    send(player, id, value);
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

    /// Abre el dispositivo en la primera nota. Devuelve si la orden se aceptó.
    fn ensure_started(&mut self, gain: f32, bypass: bool) -> Result<(), String> {
        if self.player.is_some() {
            return Ok(());
        }
        if self.banks.is_empty() {
            self.refresh_banks();
        }
        let bank = self
            .banks
            .get(self.bank_index)
            .ok_or_else(|| "No hay ningún SoundFont disponible".to_string())?;
        let settings = PianoSettings {
            maximum_polyphony: self.params.get(ParamId::Polyphony) as usize,
            reverb_and_chorus: self.params.get(ParamId::Effects) >= 0.5,
        };
        let player = PianoPlayer::start(&bank.path, gain, bypass, settings)?;
        if self.presets.is_empty()
            && let Ok(sf) = load_soundfont(&bank.path)
        {
            self.presets = presets_of(&sf);
        }
        if let Some(preset) = self.presets.get(self.preset_index) {
            send_preset(&player, preset);
        }
        for spec in piano_params::SPECS
            .iter()
            .filter(|s| s.applies == Applies::Live)
        {
            send(&player, spec.id, self.params.get(spec.id));
        }
        self.player = Some(player);
        Ok(())
    }

    pub fn note_on(
        &mut self,
        note: u8,
        velocity: u8,
        gain: f32,
        bypass: bool,
    ) -> Result<bool, String> {
        if note > 127 || self.held.contains(&note) {
            return Ok(false);
        }
        self.ensure_started(gain, bypass)?;
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
        bypass: bool,
    ) -> Result<(), String> {
        self.all_notes_off();
        self.ensure_started(gain, bypass)?;
        let offset_beats = (edit_beat - clip.start_beat).max(0.0);
        if let Some(player) = &self.player {
            player.play_pattern(&clip.pattern, offset_beats, bpm)?;
        }
        Ok(())
    }

    pub fn stop_clip(&mut self) {
        if let Some(player) = &self.player {
            player.stop_pattern();
        }
    }

    pub fn clip_playing(&self) -> bool {
        self.player.as_ref().is_some_and(PianoPlayer::plan_playing)
    }

    pub fn clip_finished(&self) -> bool {
        self.player.as_ref().is_some_and(PianoPlayer::plan_finished)
    }

    pub fn clip_beat(&self, clip_start_beat: f64, bpm: f64) -> Option<f64> {
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
        if let Some(player) = &self.player {
            player.note_off(note);
        }
        true
    }

    pub fn all_notes_off(&mut self) -> bool {
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
        self.all_notes_off();
        self.player = None;
        self.held.clear();
        self.mouse = None;
    }

    pub fn set_gain(&self, gain: f32) {
        if let Some(player) = &self.player {
            player.set_gain(gain);
        }
    }

    pub fn set_bypass(&self, bypass: bool) {
        if let Some(player) = &self.player {
            player.set_bypass(bypass);
        }
    }

    pub fn failed(&self) -> bool {
        self.player.as_ref().is_some_and(PianoPlayer::failed)
    }
}

/// Carga el SoundFont inicial fuera del callback antes del primer note-on.
pub(crate) fn preload() -> Result<(), String> {
    crate::audio::piano::preload()
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
