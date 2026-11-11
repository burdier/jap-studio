//! Ejecución VST3 independiente de GPUI, con el mismo player MIDI del piano/sampler.
use crate::audio::{
    device::AudioConfig,
    piano::{PianoPlayer, PreparedPiano},
};
use elevate::{
    devices::utility_slot::UtilitySlot,
    musical::{clip::MidiClip, model::InstrumentId},
};
use std::path::{Path, PathBuf};
pub(crate) const VST_ID: InstrumentId = InstrumentId(3);

#[derive(Default)]
pub(crate) struct VstInstrument {
    pub name: String,
    pub bundle: Option<PathBuf>,
    player: Option<PianoPlayer>,
    fx_rack: Option<std::sync::Arc<crate::audio::effects::Rack>>,
    editor: Option<crate::audio::vst3::EditorHandle>,
    pub held: std::collections::HashSet<u8>,
    pub mouse: Option<u8>,
}
impl VstInstrument {
    pub fn set_fx_rack(&mut self, rack: Option<std::sync::Arc<crate::audio::effects::Rack>>) { if let Some(player) = &self.player { player.set_fx_rack(rack.clone()); } self.fx_rack = rack; }

    pub fn prepare(path: &Path, config: &AudioConfig, class: Option<String>) -> Result<(PreparedPiano, String), String> {
        PreparedPiano::load_vst(path, config, class)
    }
    pub fn install(
        &mut self,
        prepared: PreparedPiano,
        name: String,
        bundle: PathBuf,
        gain: f32,
        pan: f32,
        fx: f32,
        utility: UtilitySlot,
    ) -> Result<(), String> {
        // Arranque transaccional: conservar la instancia anterior si abrir audio falla.
        let editor = prepared.vst_editor();
        let player = prepared.start(gain, utility, pan, fx)?;
        player.set_fx_rack(self.fx_rack.clone());
        self.player = Some(player);
        self.editor = editor;
        self.name = name;
        self.bundle = Some(bundle);
        Ok(())
    }
    pub fn ready(&self) -> bool {
        self.player.is_some()
    }
    pub fn sample_rate(&self) -> Option<u32> {
        self.player.as_ref().map(|player| player.sample_rate)
    }
    pub fn midi(&mut self, status: u8, a: u8, b: u8) {
        if let Some(p) = &self.player {
            p.midi(status, a, b);
            if status & 0xf0 == 0x90 && b > 0 {
                self.held.insert(a);
            } else if status & 0xf0 == 0x80 || status & 0xf0 == 0x90 {
                self.held.remove(&a);
            }
        }
    }
    pub fn all_off(&mut self) {
        self.held.clear();
        self.mouse = None;
        if let Some(p) = &self.player {
            p.all_notes_off();
        }
    }
    pub fn silence(&mut self) {
        self.stop_clip();
        self.all_off();
    }
    pub fn unload(&mut self) {
        self.silence();
        self.player = None;
        // El propietario del plugin termina y libera COM en su propio hilo.
        // Wine puede tardar en cerrar un diálogo: nunca esperarlo desde GPUI.
        self.editor = None;
    }
    pub fn open_editor(&self) {
        if let Some(editor) = &self.editor {
            editor.open();
        }
    }
    pub fn close_editor(&self) {
        if let Some(editor) = &self.editor {
            editor.close();
        }
    }
    pub fn editor_message(&self) -> Option<String> {
        self.editor.as_ref()?.take_message()
    }
    pub fn play_clip(&self, clip: &MidiClip, beat: f64, bpm: f64) -> Result<(), String> {
        let player = self
            .player
            .as_ref()
            .ok_or("Carga el instrumento VST3 desde [+]")?;
        player.set_channel(clip.channel.saturating_sub(1).min(15));
        player.play_pattern(
            &clip.playback_pattern(),
            (beat - clip.start_beat).max(0.),
            bpm,
            clip.props.looped,
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
    pub fn stop_clip(&self) {
        if let Some(p) = &self.player {
            p.stop_pattern();
        }
    }
    pub fn failed(&self) -> bool {
        self.player.as_ref().is_some_and(PianoPlayer::failed)
    }
    pub fn mix(&self, gain: f32, pan: f32, fx: f32) {
        if let Some(p) = &self.player {
            p.set_gain(gain);
            p.set_pan(pan);
            p.set_fx_gain(fx);
        }
    }
}
impl Drop for VstInstrument {
    fn drop(&mut self) {
        self.unload();
    }
}
