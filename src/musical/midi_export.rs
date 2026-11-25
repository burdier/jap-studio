//! Exportación headless a Standard MIDI File (`.mid`, Formato 1) usando `midly`.
//! Sin dependencias de GPUI ni CPAL.
use super::{
    clip::MidiClip,
    time::{Meter, PPQ},
    track::Track,
};
use midly::{
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
    num::{u4, u7, u15, u24, u28},
};

struct RawMidiEvent {
    tick: u64,
    priority: u8, // 0 = NoteOff, 1 = NoteOn (evitar colisiones cuando dos notas se tocan)
    channel: u8,
    note: u8,
    velocity: u8,
    is_on: bool,
}

fn tempo_meta(bpm: f64) -> MetaMessage<'static> {
    let us_per_quarter = (60_000_000.0 / bpm.clamp(1.0, 1000.0)).round() as u32;
    MetaMessage::Tempo(u24::new(us_per_quarter.min(0x00FF_FFFF)))
}

fn meter_meta(meter: Meter) -> MetaMessage<'static> {
    let denom_pow = match meter.denominator() {
        1 => 0,
        2 => 1,
        4 => 2,
        8 => 3,
        16 => 4,
        _ => 2,
    };
    MetaMessage::TimeSignature(meter.numerator(), denom_pow, 24, 8)
}

fn build_smf(
    named_tracks: Vec<(String, Vec<RawMidiEvent>)>,
    bpm: f64,
    meter: Meter,
) -> Result<Vec<u8>, String> {
    let mut smf = Smf::new(Header::new(
        Format::Parallel,
        Timing::Metrical(u15::new(PPQ as u16)),
    ));

    // Pista 0: mapa de tempo y compás
    let meta_track = vec![
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(tempo_meta(bpm)),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(meter_meta(meter)),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];
    smf.tracks.push(meta_track);

    let names: Vec<Vec<u8>> = named_tracks
        .iter()
        .map(|(name, _)| name.as_bytes().to_vec())
        .collect();

    for (idx, (_, mut events)) in named_tracks.into_iter().enumerate() {
        events.sort_by_key(|e| (e.tick, e.priority, e.channel, e.note));
        let mut track: Vec<TrackEvent<'_>> = Vec::with_capacity(events.len() + 2);
        track.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(names[idx].as_slice())),
        });
        let mut prev_tick = 0u64;
        for ev in events {
            let delta = ev.tick.saturating_sub(prev_tick).min(0x0FFF_FFFF) as u32;
            prev_tick = ev.tick;
            let ch = u4::new(ev.channel.min(15));
            let key = u7::new(ev.note.min(127));
            let vel = u7::new(ev.velocity.min(127));
            let msg = if ev.is_on {
                MidiMessage::NoteOn { key, vel }
            } else {
                MidiMessage::NoteOff { key, vel }
            };
            track.push(TrackEvent {
                delta: u28::new(delta),
                kind: TrackEventKind::Midi {
                    channel: ch,
                    message: msg,
                },
            });
        }
        track.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        });
        smf.tracks.push(track);
    }

    let mut out = Vec::new();
    smf.write(&mut out).map_err(|e| format!("SMF write: {e}"))?;
    Ok(out)
}

fn collect_clip_events(clip: &MidiClip, base_tick: u64, out: &mut Vec<RawMidiEvent>) {
    let channel = clip.channel.saturating_sub(1).min(15);
    let pattern = clip.playback_pattern();
    for note in pattern.notes() {
        let start = base_tick.saturating_add(note.start.0.max(0) as u64);
        let end = start.saturating_add(note.duration.0.max(1) as u64);
        out.push(RawMidiEvent {
            tick: start,
            priority: 1,
            channel,
            note: note.pitch.min(127),
            velocity: note.velocity.clamp(1, 127),
            is_on: true,
        });
        out.push(RawMidiEvent {
            tick: end,
            priority: 0,
            channel,
            note: note.pitch.min(127),
            velocity: 64,
            is_on: false,
        });
    }
}

/// Exporta un único `MidiClip` a bytes SMF `.mid` (comenzando en el tick 0).
pub fn export_clip_to_smf(clip: &MidiClip, bpm: f64, meter: Meter) -> Result<Vec<u8>, String> {
    let mut events = Vec::new();
    collect_clip_events(clip, 0, &mut events);
    if events.is_empty() {
        return Err("El clip MIDI no tiene notas para exportar".into());
    }
    build_smf(vec![(clip.name.clone(), events)], bpm, meter)
}

/// Exporta todos los clips MIDI activos del Arrangement en pistas SMF separadas.
pub fn export_arrangement_to_smf(
    tracks: &[Track],
    clips: &[MidiClip],
    bpm: f64,
    meter: Meter,
) -> Result<Vec<u8>, String> {
    let mut by_track: std::collections::BTreeMap<usize, Vec<RawMidiEvent>> =
        std::collections::BTreeMap::new();
    for clip in clips {
        if clip.props.muted {
            continue;
        }
        let base_tick = (clip.start_beat.max(0.0) * PPQ as f64).round() as u64;
        let bucket = by_track.entry(clip.track).or_default();
        collect_clip_events(clip, base_tick, bucket);
    }
    let named: Vec<(String, Vec<RawMidiEvent>)> = by_track
        .into_iter()
        .filter(|(_, evs)| !evs.is_empty())
        .map(|(track_idx, evs)| {
            let name = tracks
                .get(track_idx)
                .map(|t| t.name.to_string())
                .unwrap_or_else(|| format!("Track {}", track_idx + 1));
            (name, evs)
        })
        .collect();
    if named.is_empty() {
        return Err("No hay notas MIDI activas en el Arrangement para exportar".into());
    }
    build_smf(named, bpm, meter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musical::{
        model::{InstrumentId, Pattern},
        time::Tick,
    };

    #[test]
    fn exported_smf_round_trips_tempo_and_polyphonic_notes() {
        let mut pattern = Pattern::new(Tick(PPQ * 4)).unwrap();
        pattern
            .add_note(InstrumentId(1), Tick(0), Tick(PPQ), 60, 100)
            .unwrap();
        pattern
            .add_note(InstrumentId(1), Tick(0), Tick(PPQ), 64, 92)
            .unwrap();
        pattern
            .add_note(InstrumentId(1), Tick(PPQ), Tick(PPQ / 2), 67, 110)
            .unwrap();
        let clip = MidiClip::new("Acorde", 0, 0.0, pattern);
        let bytes = export_clip_to_smf(&clip, 128.0, Meter::default()).unwrap();

        let parsed = Smf::parse(&bytes).expect("el archivo SMF debe ser válido");
        assert_eq!(parsed.tracks.len(), 2);
        let mut notes_on = Vec::new();
        for ev in &parsed.tracks[1] {
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, vel },
                ..
            } = ev.kind
            {
                notes_on.push((key.as_int(), vel.as_int()));
            }
        }
        assert_eq!(notes_on, vec![(60, 100), (64, 92), (67, 110)]);
    }
}
