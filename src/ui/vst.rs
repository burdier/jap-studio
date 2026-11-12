//! Inserción y control del instrumento externo; usa catálogo, pista y MIDI comunes.
mod commands;
pub(in crate::ui) mod effects;
mod view;
use super::DawApp;
use crate::instruments::vst::{VST_ID, VstInstrument};
use elevate::musical::{
    midi::MidiMessage,
    track::{TrackEdit, TrackId, assign_instrument},
};
use gpui::{AppContext, Context};
use std::path::PathBuf;

impl DawApp {
    pub(in crate::ui) fn vst_selected(&self) -> bool {
        self.tracks
            .get(self.cursor_row)
            .is_some_and(|track| track.instrument == Some(VST_ID))
    }
    pub(in crate::ui) fn vst_track(&self) -> Option<usize> {
        self.tracks
            .iter()
            .position(|track| track.instrument == Some(VST_ID))
    }
    pub(in crate::ui) fn load_vst(
        &mut self,
        bundle: PathBuf,
        target: Option<TrackId>,
        cx: &mut Context<Self>,
    ) {
        self.load_vst_class(bundle, target, None, cx);
    }
    pub(in crate::ui) fn load_vst_class(
        &mut self,
        bundle: PathBuf,
        target: Option<TrackId>,
        class: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.vst_task.is_some() {
            self.status_message = "Espera a que termine de cargar el VST3".into();
            return;
        }
        if let Some(track) = target.and_then(|id| self.tracks.iter().find(|track| track.id == id)) {
            if !track.accepts_clips() {
                self.status_message = "Selecciona una pista MIDI para el instrumento".into();
                return;
            }
        }
        let config = self.settings.audio.saved.clone();
        let previous_instrument = target
            .and_then(|id| self.tracks.iter().find(|track| track.id == id))
            .map(|track| track.instrument);
        let expected = config.clone();
        let path = bundle.clone();
        self.status_message = "Cargando instrumento VST3 mediante yabridge…".into();
        let work =
            cx.background_spawn(async move { VstInstrument::prepare(&path, &config, class) });
        self.vst_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.vst_task = None;
                if this.settings.audio.saved != expected {
                    this.status_message =
                        "La salida cambió durante la carga; vuelve a añadir el VST3".into();
                    cx.notify();
                    return;
                }
                let index =
                    target.and_then(|id| this.tracks.iter().position(|track| track.id == id));
                if target.is_some() && index.is_none() {
                    return;
                }
                if index.map(|i| this.tracks[i].instrument) != previous_instrument {
                    this.status_message =
                        "El instrumento de destino cambió; carga VST3 descartada".into();
                    cx.notify();
                    return;
                }
                match result {
                    Ok((prepared, name)) => {
                        let gain = index.map_or(1., |i| this.track_gain(i));
                        let pan = index.map_or(0., |i| this.track_pan(i));
                        let fx = index.map_or(1., |i| this.track_fx_gain(i));
                        let mut candidate = VstInstrument::default();
                        match candidate.install(
                            prepared,
                            name,
                            bundle,
                            gain,
                            pan,
                            fx,
                            this.utility.clone(),
                        ) {
                            Ok(()) => {
                                this.stop_all();
                                let index = index.unwrap_or_else(|| {
                                    this.edit_track_structure(TrackEdit::Insert);
                                    this.cursor_row
                                });
                                assign_instrument(&mut this.tracks, index, VST_ID);
                                this.track_revision = this.track_revision.wrapping_add(1);
                                this.vst = candidate;
                                this.cursor_row = index;
                                this.open_track_instrument(this.tracks[index].id);
                                this.sync_midi_focus();
                                this.status_message = format!(
                                    "{} cargado · M: teclado MIDI · Espacio: Play/Stop",
                                    this.vst.name
                                );
                            }
                            Err(error) => this.status_message = format!("Salida VST3: {error}"),
                        }
                    }
                    Err(error) => this.status_message = format!("VST3: {error}"),
                }
                cx.notify();
            });
        }));
    }
    pub(in crate::ui) fn vst_midi(&mut self, message: MidiMessage) {
        if !self.vst.ready() {
            self.status_message = "VST3 sin cargar; selecciónalo desde [+]".into();
            return;
        }
        self.mix_vst();
        match message {
            MidiMessage::NoteOn { note, velocity } => self.vst.midi(0x90, note, velocity),
            MidiMessage::NoteOff { note } => self.vst.midi(0x80, note, 64),
            MidiMessage::ControlChange { cc, value } => self.vst.midi(0xb0, cc, value),
            MidiMessage::PitchBend { bend } => {
                let value = (bend as i32 + 8192).clamp(0, 16383) as u16;
                self.vst.midi(0xe0, (value & 127) as u8, (value >> 7) as u8);
            }
            MidiMessage::AllNotesOff => self.vst.all_off(),
            MidiMessage::ProgramChange { .. } => {}
        }
        self.midi_capture.record(
            message,
            self.session_epoch.elapsed().as_nanos() as u64,
            self.timeline.tempo.bpm(),
        );
    }
    pub(in crate::ui) fn play_vst_clip(
        &mut self,
        clip: &elevate::musical::clip::MidiClip,
        start: f64,
    ) {
        self.instrument.stop();
        self.sampler.silence();
        self.vst.mix(
            self.track_gain(clip.track) * clip.props.gain_linear(),
            self.track_pan(clip.track),
            self.track_fx_gain(clip.track),
        );
        match self.vst.play_clip(clip, start, self.timeline.tempo.bpm()) {
            Ok(()) => {
                self.timeline.playhead = Some(start);
                self.status_message = format!("▶ {} → {}", clip.name, self.vst.name);
            }
            Err(error) => self.status_message = error,
        }
    }
    pub(in crate::ui) fn mix_vst(&mut self) {
        if let Some(track) = self.vst_track() {
            let clip_gain = if self.vst.clip_playing() {
                self.timeline
                    .midi_clip
                    .as_ref()
                    .filter(|c| c.track == track)
                    .map_or(1., |c| c.props.gain_linear())
            } else {
                1.
            };
            self.vst.mix(
                self.track_gain(track) * clip_gain,
                self.track_pan(track),
                self.track_fx_gain(track),
            );
        } else if self.vst.ready() {
            self.vst.unload();
        }
    }
    pub(in crate::ui) fn remove_vst(&mut self) {
        self.vst_task = None;
        self.vst.unload();
        if let Some(index) = self.vst_track() {
            self.tracks[index].instrument = None;
            self.track_revision = self.track_revision.wrapping_add(1);
        }
        self.fx.editor_track = None;
        self.sync_midi_focus();
        self.status_message = "Instrumento VST3 retirado".into();
    }
}
