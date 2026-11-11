use crate::{instruments::sampler::SAMPLER_ID, ui::DawApp};
use elevate::musical::{midi::MidiMessage, model::InstrumentId};
impl DawApp {
    pub(in crate::ui) fn active_instrument_id(&self) -> InstrumentId {
        if self.vst_selected() { return crate::instruments::vst::VST_ID; }
        if self.sampler_selected() {
            SAMPLER_ID
        } else {
            self.instrument.id()
        }
    }
    pub(in crate::ui) fn active_instrument_name(&self) -> &str {
        if self.vst_selected() { return &self.vst.name; }
        if self.sampler_selected() {
            self.sampler.name()
        } else {
            self.instrument.active_name()
        }
    }
    pub(in crate::ui) fn sampler_track(&self) -> Option<usize> {
        self.tracks
            .iter()
            .position(|t| t.instrument == Some(SAMPLER_ID))
    }
    pub(in crate::ui) fn sampler_gain(&self, track: usize) -> f32 {
        let clip = if self.sampler.clip_playing() {
            self.timeline
                .midi_clip
                .as_ref()
                .filter(|c| c.track == track)
                .map_or(1., |c| c.props.gain_linear())
        } else {
            1.
        };
        self.track_gain(track) * clip
    }
    pub(in crate::ui) fn sample_note_on(&mut self, note: u8, velocity: u8) {
        let Some(track) = self.sampler_track() else {
            return;
        };
        self.stop_pattern();
        self.instrument.stop();
        self.stop_audio();
        if !self.sampler.ready() {
            self.status_message = "Carga un sample antes de tocar".into();
            return;
        }
        match self.sampler.start(
            self.track_gain(track),
            self.utility.clone(),
            self.track_pan(track),
            self.track_fx_gain(track),
        ) {
            Ok(()) => {
                self.sampler.note_on(note, velocity);
                self.midi_capture.record(
                    MidiMessage::NoteOn { note, velocity },
                    self.session_epoch.elapsed().as_nanos() as u64,
                    self.timeline.tempo.bpm(),
                );
            }
            Err(e) => self.status_message = e,
        }
    }
    pub(in crate::ui) fn sampler_midi(&mut self, message: MidiMessage) {
        match message {
            MidiMessage::NoteOn { note, velocity } => self.sample_note_on(note, velocity),
            MidiMessage::NoteOff { note } => {
                self.sampler.note_off(note);
                self.midi_capture.record(
                    MidiMessage::NoteOff { note },
                    self.session_epoch.elapsed().as_nanos() as u64,
                    self.timeline.tempo.bpm(),
                );
            }
            MidiMessage::AllNotesOff => self.sampler.all_off(),
            MidiMessage::ControlChange { cc, value } => self.sampler.midi(0xb0, cc, value),
            MidiMessage::PitchBend { bend } => {
                let value = (bend as i32 + 8192).clamp(0, 16383) as u16;
                self.sampler
                    .midi(0xe0, (value & 127) as u8, (value >> 7) as u8);
            }
            MidiMessage::ProgramChange { .. } => {}
        }
    }
    pub(in crate::ui) fn play_sampler_clip(
        &mut self,
        clip: &elevate::musical::clip::MidiClip,
        start: f64,
    ) {
        self.instrument.stop();
        let gain = self.track_gain(clip.track) * clip.props.gain_linear();
        let result = self
            .sampler
            .start(
                gain,
                self.utility.clone(),
                self.track_pan(clip.track),
                self.track_fx_gain(clip.track),
            )
            .and_then(|_| {
                self.sampler
                    .play_clip(clip, start, self.timeline.tempo.bpm())
            });
        match result {
            Ok(()) => {
                self.timeline.playhead = Some(start);
                self.status_message = format!("▶ {} → Hay Sampler", clip.name);
            }
            Err(e) => self.status_message = e,
        }
    }
}
