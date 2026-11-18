//! Reparto de eventos de un clip del Hilo entre pistas hijas, preparado fuera del audio.
use super::{ArrangementEvent, AtFrame, PreparedMidiArrangement, beat_frame};
use crate::musical::{
    clip::MidiClip,
    model::{InstrumentId, NoteId},
    scheduler::{EventKind, PreparedPattern, ScheduledEvent},
    time::TimeBase,
    track::{HiloMode, Track},
    weave::WeaveRule,
};
use std::collections::HashMap;

struct SourceEvent {
    frame: u64,
    clip: u64,
    channel: u8,
    event: ScheduledEvent,
}

impl PreparedMidiArrangement {
    /// Compila la rama indicada del Hilo. El orden de NoteOn se decide globalmente
    /// por frame, clip y NoteId; NoteOff conserva siempre el instrumento elegido.
    pub fn compile_hilo_branch(
        clips: &[MidiClip],
        group_index: usize,
        branch: &Track,
        branch_index: usize,
        instruments: &[InstrumentId],
        mode: HiloMode,
        weave: Option<&WeaveRule>,
        rate: u32,
        bpm: f64,
    ) -> Result<Self, String> {
        if instruments.is_empty()
            || branch_index >= instruments.len()
            || branch.instrument != Some(instruments[branch_index])
        {
            return Err("Rama de Hilo inválida".into());
        }
        if weave.is_some_and(|rule| !rule.valid_for(instruments.len())) {
            return Err("Tejer: la regla menciona una rama inexistente".into());
        }
        let group = branch.parent.ok_or("Rama de Hilo sin grupo")?;
        let base = TimeBase::new(rate, bpm).map_err(|error| error.to_string())?;
        let mut source = Vec::new();
        let mut end_frame = 0;
        for clip in clips
            .iter()
            .filter(|clip| clip.targets_hilo(group_index, group) && !clip.props.muted)
        {
            let start = beat_frame(clip.start_beat, rate, bpm)?;
            let pattern = PreparedPattern::compile(&clip.playback_pattern(), base)
                .map_err(|error| error.to_string())?;
            end_frame = end_frame.max(
                start
                    .saturating_add(pattern.length_frames())
                    .saturating_add(1),
            );
            for (local, mut event) in pattern.all_events() {
                if source.len() >= 65_536 {
                    return Err("Plan MIDI excede 65536 eventos".into());
                }
                event.pitch = (i16::from(event.pitch) + i16::from(clip.props.pitch_semitones))
                    .clamp(0, 127) as u8;
                source.push(SourceEvent {
                    frame: start
                        .checked_add(local)
                        .ok_or("Evento MIDI fuera de rango")?,
                    clip: clip.id.value(),
                    channel: clip.channel.saturating_sub(1).min(15),
                    event,
                });
            }
        }
        source.sort_by_key(|item| (item.frame, item.event.kind, item.clip, item.event.note));
        let mut owners: HashMap<(u64, NoteId), usize> = HashMap::new();
        let mut next = 0;
        let mut events = Vec::new();
        for item in source {
            let selected = match item.event.kind {
                EventKind::NoteOn { .. } => {
                    let selected = weave
                        .map(|rule| rule.branch_at(next))
                        .unwrap_or(next % instruments.len());
                    next += 1;
                    owners.insert((item.clip, item.event.note), selected);
                    selected
                }
                EventKind::NoteOff => owners
                    .remove(&(item.clip, item.event.note))
                    .ok_or("NoteOff de Hilo sin NoteOn")?,
                EventKind::Control { .. } => branch_index,
            };
            if mode == HiloMode::Layer || selected == branch_index {
                let mut event = item.event;
                event.instrument = instruments[branch_index];
                events.push(AtFrame {
                    frame: item.frame,
                    routed: ArrangementEvent {
                        track: branch.id,
                        instrument: instruments[branch_index],
                        channel: item.channel,
                        event,
                    },
                });
            }
        }
        Ok(Self { events, end_frame })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musical::{
        model::Pattern,
        time::{FrameWindow, PPQ, Tick},
        track::{TrackId, TrackKind},
        weave::WeaveRule,
    };

    #[test]
    fn prepared_weave_routes_six_notes_across_three_branches() {
        let instruments = [InstrumentId(51), InstrumentId(52), InstrumentId(53)];
        let mut branches = [
            Track::new("A", TrackKind::Synth),
            Track::new("B", TrackKind::Synth),
            Track::new("C", TrackKind::Synth),
        ];
        for (branch, &id) in branches.iter_mut().zip(&instruments) {
            branch.instrument = Some(id);
            branch.parent = Some(TrackId(900));
        }
        let mut pattern = Pattern::new(Tick(PPQ * 4)).unwrap();
        for i in 0..6 {
            pattern
                .add_note(instruments[0], Tick(i * 120), Tick(60), 60 + i as u8, 100)
                .unwrap();
        }
        let clips = [MidiClip::new("Tejer", 0, 0.0, pattern)];
        let rule = WeaveRule::new(vec![0, 2, 1]).unwrap();
        let notes: Vec<Vec<u8>> = branches
            .iter()
            .enumerate()
            .map(|(index, branch)| {
                PreparedMidiArrangement::compile_hilo_branch(
                    &clips,
                    0,
                    branch,
                    index,
                    &instruments,
                    HiloMode::RoundRobin,
                    Some(&rule),
                    48_000,
                    120.0,
                )
                .unwrap()
                .events_in(FrameWindow::new(0, 96_000).unwrap())
                .filter_map(|event| {
                    matches!(event.event.kind, EventKind::NoteOn { .. })
                        .then_some(event.event.pitch)
                })
                .collect()
            })
            .collect();
        assert_eq!(notes, vec![vec![60, 63], vec![62, 65], vec![61, 64]]);
    }

    #[test]
    fn round_robin_keeps_each_release_on_its_note_on_branch() {
        let mut a = Track::new("A", TrackKind::Synth);
        let mut b = Track::new("B", TrackKind::Synth);
        a.instrument = Some(InstrumentId(41));
        b.instrument = Some(InstrumentId(42));
        a.parent = Some(TrackId(900));
        b.parent = Some(TrackId(900));
        let instruments = [InstrumentId(41), InstrumentId(42)];
        let mut pattern = Pattern::new(Tick(4 * PPQ)).unwrap();
        pattern
            .add_note(instruments[0], Tick(0), Tick(PPQ * 3), 60, 100)
            .unwrap();
        pattern
            .add_note(instruments[0], Tick(PPQ), Tick(PPQ), 62, 100)
            .unwrap();
        pattern
            .add_note(instruments[0], Tick(PPQ * 2), Tick(PPQ), 64, 100)
            .unwrap();
        let clips = [MidiClip::new("Hilo", 0, 0.0, pattern)];
        let branch_a = PreparedMidiArrangement::compile_hilo_branch(
            &clips,
            0,
            &a,
            0,
            &instruments,
            HiloMode::RoundRobin,
            None,
            48_000,
            120.0,
        )
        .unwrap();
        let branch_b = PreparedMidiArrangement::compile_hilo_branch(
            &clips,
            0,
            &b,
            1,
            &instruments,
            HiloMode::RoundRobin,
            None,
            48_000,
            120.0,
        )
        .unwrap();
        let window = FrameWindow::new(0, 96_000).unwrap();
        let a_events: Vec<_> = branch_a
            .events_in(window)
            .map(|e| (e.event.pitch, e.event.kind))
            .collect();
        let b_events: Vec<_> = branch_b
            .events_in(window)
            .map(|e| (e.event.pitch, e.event.kind))
            .collect();
        assert_eq!(a_events.len(), 4);
        assert_eq!(b_events.len(), 2);
        assert_eq!(a_events.iter().filter(|(pitch, _)| *pitch == 60).count(), 2);
        assert_eq!(a_events.iter().filter(|(pitch, _)| *pitch == 64).count(), 2);
        assert_eq!(b_events.iter().filter(|(pitch, _)| *pitch == 62).count(), 2);
    }

    #[test]
    fn routed_clip_stays_on_its_lane_and_only_sounds_in_selected_hilo() {
        let mut branch_a = Track::new("A", TrackKind::Synth);
        branch_a.parent = Some(TrackId(900));
        branch_a.instrument = Some(InstrumentId(41));
        let mut branch_b = Track::new("B", TrackKind::Synth);
        branch_b.parent = Some(TrackId(901));
        branch_b.instrument = Some(InstrumentId(42));
        let mut pattern = Pattern::new(Tick(PPQ)).unwrap();
        pattern
            .add_note(InstrumentId(41), Tick(0), Tick(PPQ / 2), 60, 100)
            .unwrap();
        let mut clip = MidiClip::new("Enviado", 5, 0.0, pattern);
        clip.hilo_target = Some(TrackId(901));
        let clips = [clip];
        let a = PreparedMidiArrangement::compile_hilo_branch(
            &clips,
            0,
            &branch_a,
            0,
            &[InstrumentId(41)],
            HiloMode::Layer,
            None,
            48_000,
            120.0,
        )
        .unwrap();
        let b = PreparedMidiArrangement::compile_hilo_branch(
            &clips,
            1,
            &branch_b,
            0,
            &[InstrumentId(42)],
            HiloMode::Layer,
            None,
            48_000,
            120.0,
        )
        .unwrap();
        assert_eq!(clips[0].track, 5);
        assert_eq!(a.events_in(FrameWindow::new(0, 48_000).unwrap()).count(), 0);
        assert_eq!(b.events_in(FrameWindow::new(0, 48_000).unwrap()).count(), 2);
    }
}
