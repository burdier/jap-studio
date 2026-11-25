//! Una voz de expresión usa un miembro MPE hasta su NoteOff.
use super::VstEngine;
use elevate::musical::{
    clip::ClipId,
    model::NoteId,
    pitch_curve::PITCH_RANGE_CENTS,
    scheduler::{EventKind, ScheduledEvent},
};

#[derive(Clone, Copy)]
pub(super) struct MpeVoice {
    clip: Option<ClipId>,
    note: NoteId,
    pitch: u8,
    ramp: Option<PitchRamp>,
}

#[derive(Clone, Copy)]
struct PitchRamp {
    from: i16,
    to: i16,
    elapsed: u64,
    duration: u64,
}

pub(super) fn supports_mpe_pitch(mapping: &[[Option<u32>; 130]; 16]) -> bool {
    mapping.iter().enumerate().skip(1).all(|(index, row)| {
        row[129].is_some()
            && !mapping[..index]
                .iter()
                .any(|previous| previous[129] == row[129])
    })
}

impl VstEngine {
    pub fn note_pitch_event(&mut self, event: ScheduledEvent, reserved: u16) -> bool {
        let Some(initial) = event.initial_pitch_cents.filter(|_| self.mpe_pitch) else {
            return false;
        };
        let active = self.mpe_voices.iter().position(|voice| {
            voice.is_some_and(|voice| voice.clip == event.clip && voice.note == event.note)
        });
        match event.kind {
            EventKind::NoteOn { velocity } => {
                let slot = active.or_else(|| {
                    self.mpe_voices
                        .iter()
                        .enumerate()
                        .position(|(index, voice)| {
                            voice.is_none()
                                && reserved & (1 << (index + 1)) == 0
                                && !self.held[index + 1].contains(&true)
                        })
                });
                if let Some(index) = slot {
                    let channel = index + 1;
                    self.set_note_pitch(channel, f64::from(initial));
                    self.midi(0x90 | channel as u8, event.pitch, velocity);
                    self.mpe_voices[index] = Some(MpeVoice {
                        clip: event.clip,
                        note: event.note,
                        pitch: event.pitch,
                        ramp: None,
                    });
                }
            }
            EventKind::NoteOff => {
                if let Some(index) = active {
                    if let Some(voice) = self.mpe_voices[index].take() {
                        self.midi(
                            0x80 | (index + 1) as u8,
                            voice.pitch,
                            event.expression.release_velocity,
                        );
                    }
                }
            }
            EventKind::PitchRamp {
                from_cents,
                to_cents,
                frames,
            } => {
                if let Some(index) = active {
                    if let Some(voice) = &mut self.mpe_voices[index] {
                        voice.ramp = Some(PitchRamp {
                            from: from_cents,
                            to: to_cents,
                            elapsed: 0,
                            duration: frames,
                        });
                    }
                }
            }
            EventKind::Control { .. } => return false,
        }
        true
    }

    fn set_note_pitch(&self, channel: usize, cents: f64) {
        self.cc(
            channel,
            129,
            0.5 + cents / (2.0 * f64::from(PITCH_RANGE_CENTS)),
        );
    }

    pub(super) fn pitch_slice_len(&self, remaining: usize) -> usize {
        if self
            .mpe_voices
            .iter()
            .flatten()
            .any(|voice| voice.ramp.is_some_and(|ramp| ramp.elapsed < ramp.duration))
        {
            remaining.min((self.rate / 320.0).max(1.0) as usize)
        } else {
            remaining
        }
    }

    pub(super) fn prepare_note_pitch(&mut self, span: usize) {
        for index in 0..self.mpe_voices.len() {
            let Some(voice) = self.mpe_voices[index].as_mut() else {
                continue;
            };
            let Some(mut ramp) = voice.ramp else {
                continue;
            };
            let fraction = ramp.elapsed as f64 / ramp.duration as f64;
            let cents = f64::from(ramp.from) + fraction * f64::from(ramp.to - ramp.from);
            voice.ramp = if ramp.elapsed < ramp.duration {
                ramp.elapsed = (ramp.elapsed + span as u64).min(ramp.duration);
                Some(ramp)
            } else {
                None
            };
            self.set_note_pitch(index + 1, cents);
        }
    }
}
