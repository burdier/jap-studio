use crate::{instruments::sampler::SAMPLER_ID, ui::DawApp};
use elevate::musical::{midi::MidiMessage, model::InstrumentId};
impl DawApp {
    pub(in crate::ui) fn is_sampler_id(&self, id: InstrumentId) -> bool {
        id == self.active_sampler_id || self.sampler_instances.contains_key(&id)
    }
    pub(in crate::ui) fn activate_sampler(&mut self, id: InstrumentId) {
        if id == self.active_sampler_id {
            return;
        }
        let Some(next) = self.sampler_instances.remove(&id) else {
            return;
        };
        let previous = std::mem::replace(&mut self.sampler, next);
        self.sampler_instances
            .insert(self.active_sampler_id, previous);
        self.active_sampler_id = id;
    }
    pub(in crate::ui) fn sampler_snapshot(
        &self,
        id: InstrumentId,
    ) -> Result<crate::audio::sampler::SamplerSnapshot, String> {
        if id == self.active_sampler_id {
            self.sampler.snapshot()
        } else {
            self.sampler_instances
                .get(&id)
                .ok_or("Sampler sin instancia")?
                .snapshot()
        }
    }
    pub(in crate::ui) fn active_instrument_id(&self) -> InstrumentId {
        if self.vst_selected() {
            return self.tracks[self.cursor_row]
                .instrument
                .unwrap_or(self.vst.id);
        }
        if self.sampler_selected() {
            self.tracks[self.cursor_row]
                .instrument
                .unwrap_or(SAMPLER_ID)
        } else {
            self.instrument.id()
        }
    }
    pub(in crate::ui) fn active_instrument_name(&self) -> &str {
        if self.vst_selected() {
            return self.vst_name(
                self.tracks[self.cursor_row]
                    .instrument
                    .unwrap_or(self.vst.id),
            );
        }
        if self.sampler_selected() {
            self.sampler.name()
        } else {
            self.instrument.active_name()
        }
    }
    pub(in crate::ui) fn sampler_track(&self) -> Option<usize> {
        self.tracks
            .iter()
            .position(|t| t.instrument == Some(self.active_sampler_id))
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
        let id = self.active_sampler_id;
        if self
            .player
            .as_ref()
            .is_some_and(|player| player.has_instrument(id))
        {
            if self.sampler.note_on_shared(note) {
                let sent = self.player.as_ref().is_some_and(|player| {
                    player.send_live_midi(id, MidiMessage::NoteOn { note, velocity })
                });
                if sent {
                    self.midi_capture.record(
                        MidiMessage::NoteOn { note, velocity },
                        self.session_epoch.elapsed().as_nanos() as u64,
                        self.timeline.tempo.bpm(),
                    );
                } else {
                    self.sampler.note_off_shared(note);
                    self.status_message =
                        "La cola MIDI de Play está llena; vuelve a tocar la nota".into();
                }
            }
            return;
        }
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
                if let Some(player) = &self.player
                    && player.has_instrument(self.active_sampler_id)
                {
                    player.send_live_midi(self.active_sampler_id, message);
                    self.sampler.note_off_shared(note);
                } else {
                    self.sampler.note_off(note);
                }
                self.midi_capture.record(
                    MidiMessage::NoteOff { note },
                    self.session_epoch.elapsed().as_nanos() as u64,
                    self.timeline.tempo.bpm(),
                );
            }
            MidiMessage::AllNotesOff => {
                if let Some(player) = &self.player
                    && player.has_instrument(self.active_sampler_id)
                {
                    player.send_live_midi(self.active_sampler_id, message);
                }
                self.sampler.all_off();
            }
            MidiMessage::ControlChange { cc, value } => {
                if let Some(player) = &self.player
                    && player.has_instrument(self.active_sampler_id)
                {
                    player.send_live_midi(self.active_sampler_id, message);
                } else {
                    self.sampler.midi(0xb0, cc, value);
                }
            }
            MidiMessage::PitchBend { bend } => {
                if let Some(player) = &self.player
                    && player.has_instrument(self.active_sampler_id)
                {
                    player.send_live_midi(self.active_sampler_id, message);
                } else {
                    let value = (bend as i32 + 8192).clamp(0, 16383) as u16;
                    self.sampler
                        .midi(0xe0, (value & 127) as u8, (value >> 7) as u8);
                }
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
