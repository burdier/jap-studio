//! Importador de archivos Standard MIDI File (`.mid` / `.midi`) mediante `midly`.
//! Sin GPUI ni CPAL: convierte cualquier pista/archivo `.mid` en un `MidiClip` con ticks `PPQ=960`.
use super::{
    automation::ControlLane,
    clip::{CapturedNote, MidiClip},
    model::{InstrumentId, Pattern},
    time::{Meter, PPQ, Tick},
};
use midly::{MidiMessage as SmfMsg, Smf, Timing, TrackEventKind};
use std::collections::HashMap;

const MIN_NOTE_TICKS: i64 = 60;

/// Parsea los bytes de un archivo `.mid` y construye un `MidiClip` normalizado en `PPQ = 960`.
pub fn parse_smf_to_clip(
    bytes: &[u8],
    name: impl Into<String>,
    track_index: usize,
    start_beat: f64,
    instrument: InstrumentId,
) -> Result<MidiClip, String> {
    parse_smf_to_clip_with_meter(
        bytes,
        name,
        track_index,
        start_beat,
        instrument,
        Meter::default(),
    )
}

pub fn parse_smf_to_clip_with_meter(
    bytes: &[u8],
    name: impl Into<String>,
    track_index: usize,
    start_beat: f64,
    instrument: InstrumentId,
    meter: Meter,
) -> Result<MidiClip, String> {
    let smf = Smf::parse(bytes).map_err(|err| format!("Archivo MIDI inválido: {err}"))?;
    let file_ppq = match smf.header.timing {
        Timing::Metrical(ticks_per_beat) => i64::from(ticks_per_beat.as_int()).max(1),
        Timing::Timecode(fps, sub) => (i64::from(fps.as_int()) * i64::from(sub)).max(1),
    };

    let mut collected: Vec<CapturedNote> = Vec::new();
    let mut controls = Vec::new();
    for track in &smf.tracks {
        let mut cursor_raw: u64 = 0;
        let mut active: HashMap<(u8, u8), (Tick, u8)> = HashMap::new();
        for event in track {
            cursor_raw = cursor_raw.saturating_add(u64::from(event.delta.as_int()));
            let tick = Tick(((cursor_raw as i128 * i128::from(PPQ)) / i128::from(file_ppq)) as i64);
            if let TrackEventKind::Midi { channel, message } = event.kind {
                let ch = channel.as_int();
                match message {
                    SmfMsg::NoteOn { key, vel } => {
                        let pitch = key.as_int();
                        let velocity = vel.as_int();
                        if velocity == 0 {
                            if let Some((start, on_vel)) = active.remove(&(ch, pitch)) {
                                let duration = Tick((tick.0 - start.0).max(MIN_NOTE_TICKS));
                                collected.push(CapturedNote {
                                    start,
                                    duration,
                                    pitch,
                                    velocity: on_vel,
                                    release_velocity: 64,
                                });
                            }
                        } else {
                            active.insert((ch, pitch), (tick, velocity));
                        }
                    }
                    SmfMsg::NoteOff { key, vel } => {
                        let pitch = key.as_int();
                        if let Some((start, on_vel)) = active.remove(&(ch, pitch)) {
                            let duration = Tick((tick.0 - start.0).max(MIN_NOTE_TICKS));
                            collected.push(CapturedNote {
                                start,
                                duration,
                                pitch,
                                velocity: on_vel,
                                release_velocity: vel.as_int(),
                            });
                        }
                    }
                    SmfMsg::Controller { controller, value } => controls.push((
                        tick,
                        ControlLane::Cc(controller.as_int()),
                        i16::from(value.as_int()),
                    )),
                    SmfMsg::PitchBend { bend } => {
                        controls.push((tick, ControlLane::PitchBend, bend.as_int()))
                    }
                    SmfMsg::ChannelAftertouch { vel } => {
                        controls.push((tick, ControlLane::Pressure, i16::from(vel.as_int())))
                    }
                    _ => {}
                }
            }
        }
    }

    if collected.is_empty() && controls.is_empty() {
        return Err("El archivo MIDI no contiene notas ni controles".into());
    }

    let min_start = collected
        .iter()
        .map(|n| n.start.0)
        .min()
        .or_else(|| controls.iter().map(|(at, _, _)| at.0).min())
        .unwrap_or(0);
    let max_end = collected
        .iter()
        .map(|n| n.start.0 - min_start + n.duration.0)
        .chain(
            controls
                .iter()
                .map(|(at, _, _)| (at.0 - min_start).max(0).saturating_add(1)),
        )
        .max()
        .unwrap_or(PPQ);
    let bar_ticks = meter.ticks_per_bar();
    let length_ticks = ((max_end + bar_ticks - 1) / bar_ticks).max(1) * bar_ticks;
    let mut pattern = Pattern::new(Tick(length_ticks)).map_err(|e| e.to_string())?;
    collected.sort_by_key(|n| (n.start.0, n.pitch));
    for note in collected {
        let start = Tick((note.start.0 - min_start).max(0));
        let duration = Tick(note.duration.0.min(length_ticks - start.0).max(1));
        let id = pattern
            .add_note(instrument, start, duration, note.pitch, note.velocity)
            .map_err(|e| e.to_string())?;
        pattern
            .set_note_property(
                id,
                super::note_expression::NoteProperty::ReleaseVelocity,
                i16::from(note.release_velocity),
            )
            .map_err(|e| e.to_string())?;
    }
    controls.sort_by_key(|(tick, _, _)| *tick);
    for (at, lane, value) in controls {
        let at = Tick((at.0 - min_start).max(0));
        if at < pattern.length() {
            pattern.set_control(lane, at, value)?;
        }
    }
    Ok(MidiClip::new(
        name,
        track_index,
        start_beat.max(0.0),
        pattern,
    ))
}
