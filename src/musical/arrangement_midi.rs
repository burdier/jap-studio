//! Plan absoluto de clips MIDI de varias pistas; no conoce dispositivos ni UI.
use super::{
    clip::MidiClip,
    model::InstrumentId,
    scheduler::{PreparedPattern, ScheduledEvent},
    time::{FrameWindow, TimeBase},
    track::{Track, TrackId},
};

#[derive(Debug, Clone, Copy)]
pub struct ArrangementEvent {
    pub track: TrackId,
    pub instrument: InstrumentId,
    pub channel: u8,
    pub event: ScheduledEvent,
}

#[derive(Debug, Clone, Copy)]
struct AtFrame {
    frame: u64,
    routed: ArrangementEvent,
}

/// Todos los eventos ordenados una vez fuera del callback. La consulta de un bloque
/// solo busca e itera; el transporte común aporta la posición absoluta.
pub struct PreparedMidiArrangement {
    events: Vec<AtFrame>,
    end_frame: u64,
}

impl PreparedMidiArrangement {
    pub fn compile(
        clips: &[MidiClip],
        tracks: &[Track],
        rate: u32,
        bpm: f64,
    ) -> Result<Self, String> {
        if clips.len() > 4096 || tracks.len() > 256 {
            return Err("Plan MIDI excede 256 pistas o 4096 clips".into());
        }
        let base = TimeBase::new(rate, bpm).map_err(|e| e.to_string())?;
        let mut events = Vec::new();
        let mut end_frame = 0;
        for clip in clips {
            if clip.props.muted {
                continue;
            }
            let track = tracks
                .get(clip.track)
                .ok_or("Clip MIDI en pista inexistente")?;
            let Some(instrument) = track.instrument else {
                continue;
            };
            let start = beat_frame(clip.start_beat, rate, bpm)?;
            let pattern = PreparedPattern::compile(&clip.playback_pattern(), base)
                .map_err(|e| e.to_string())?;
            let end = start
                .checked_add(pattern.length_frames())
                .ok_or("Clip MIDI fuera de rango")?;
            end_frame = end_frame.max(end.saturating_add(1));
            for (local, mut event) in pattern.all_events() {
                if events.len() >= 65_536 {
                    return Err("Plan MIDI excede 65536 eventos".into());
                }
                let frame = start
                    .checked_add(local)
                    .ok_or("Evento MIDI fuera de rango")?;
                event.pitch = (i16::from(event.pitch) + i16::from(clip.props.pitch_semitones))
                    .clamp(0, 127) as u8;
                events.push(AtFrame {
                    frame,
                    routed: ArrangementEvent {
                        track: track.id,
                        instrument,
                        channel: clip.channel.saturating_sub(1).min(15),
                        event,
                    },
                });
            }
        }
        events.sort_by_key(|item| {
            (
                item.frame,
                item.routed.event.kind,
                item.routed.track,
                item.routed.event.note,
            )
        });
        Ok(Self { events, end_frame })
    }

    pub fn end_frame(&self) -> u64 {
        self.end_frame
    }

    /// Offset relativo al bloque, siempre menor que su longitud.
    pub fn events_in(&self, window: FrameWindow) -> impl Iterator<Item = ArrangementEvent> + '_ {
        let start = self
            .events
            .partition_point(|item| item.frame < window.start());
        self.events[start..]
            .iter()
            .take_while(move |item| item.frame < window.end())
            .map(move |item| {
                let mut routed = item.routed;
                routed.event.sample_offset = (item.frame - window.start()) as u32;
                routed
            })
    }
}

fn beat_frame(beat: f64, rate: u32, bpm: f64) -> Result<u64, String> {
    let frame = beat * f64::from(rate) * 60.0 / bpm;
    if !beat.is_finite() || beat < 0.0 || !frame.is_finite() || frame >= u64::MAX as f64 {
        return Err("Posición MIDI fuera de rango".into());
    }
    Ok(frame.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musical::{
        model::Pattern,
        scheduler::EventKind,
        time::{PPQ, Tick},
        track::DEMO_TRACKS,
    };

    #[test]
    fn two_tracks_share_absolute_frames_across_block_partitions() {
        let mut tracks = DEMO_TRACKS;
        tracks[0].instrument = Some(InstrumentId(41));
        tracks[1].instrument = Some(InstrumentId(42));
        let mut a = Pattern::new(Tick(2 * PPQ)).unwrap();
        a.add_note(InstrumentId(41), Tick(PPQ), Tick(PPQ / 2), 60, 100)
            .unwrap();
        let mut b = Pattern::new(Tick(2 * PPQ)).unwrap();
        b.add_note(InstrumentId(42), Tick(PPQ / 2), Tick(PPQ / 2), 67, 90)
            .unwrap();
        let clips = [MidiClip::new("A", 0, 0.0, a), MidiClip::new("B", 1, 0.5, b)];
        let plan = PreparedMidiArrangement::compile(&clips, &tracks, 48_000, 120.0).unwrap();
        let whole: Vec<_> = plan
            .events_in(FrameWindow::new(0, 48_000).unwrap())
            .map(|r| {
                (
                    r.track,
                    r.instrument,
                    r.event.sample_offset as u64,
                    r.event.kind,
                )
            })
            .collect();
        let mut split = Vec::new();
        for at in (0..48_000).step_by(137) {
            let len = (48_000 - at).min(137);
            split.extend(
                plan.events_in(FrameWindow::new(at, len as u32).unwrap())
                    .map(|r| {
                        (
                            r.track,
                            r.instrument,
                            at + r.event.sample_offset as u64,
                            r.event.kind,
                        )
                    }),
            );
        }
        assert_eq!(whole.len(), 4);
        assert_eq!(whole.iter().filter(|(_, _, at, kind)|
            *at == 24_000 && matches!(kind, EventKind::NoteOn { .. })).count(), 2);
        assert_eq!(whole, split);
        assert_eq!(plan.end_frame(), 60_001);
    }
}
