//! Diagnóstico en memoria del recorrido Lua; no equivale al callback CPAL completo.
use elevate::{
    musical::midi::MidiMessage,
    scripting::{live::LiveEvent, live_notes::LiveNoteProcessor},
};
use std::{hint::black_box, time::Instant};

const RATE: u64 = 48_000;
const SAMPLES: usize = 2_000;
const SCRIPT: &str = include_str!("../assets/scripts/components/midi/transposer.lua");

fn burst(frames: u32) -> [LiveEvent; 8] {
    let event = |offset, message| LiveEvent {
        offset,
        channel: 0,
        message,
        note_id: None,
    };
    [
        event(
            0,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 90,
            },
        ),
        event(
            1,
            MidiMessage::NoteOn {
                note: 64,
                velocity: 90,
            },
        ),
        event(2, MidiMessage::ControlChange { cc: 1, value: 40 }),
        event(3, MidiMessage::ControlChange { cc: 1, value: 80 }),
        event(frames - 4, MidiMessage::ControlChange { cc: 74, value: 64 }),
        event(frames - 3, MidiMessage::ControlChange { cc: 74, value: 96 }),
        event(frames - 2, MidiMessage::NoteOff { note: 60 }),
        event(frames - 1, MidiMessage::NoteOff { note: 64 }),
    ]
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    sorted[((sorted.len() - 1) * percentile) / 1000]
}

fn measure(
    instances: usize,
    frames: u32,
    lua: bool,
) -> Result<(u64, u64, u64, u64, usize), String> {
    let input = burst(frames);
    let mut processors = if lua {
        (0..instances)
            .map(|_| LiveNoteProcessor::prepare(SCRIPT))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let mut plain: Vec<Vec<(u8, u8)>> = (0..instances).map(|_| Vec::with_capacity(8)).collect();
    let mut samples = Vec::with_capacity(SAMPLES);
    let deadline_ns = u64::from(frames) * 1_000_000_000 / RATE;
    let mut missed = 0;
    for _ in 0..SAMPLES {
        let start = Instant::now();
        if lua {
            for processor in &mut processors {
                let result = processor.process_or_disable(&input, frames);
                if let Some(error) = result.error {
                    return Err(error.into());
                }
                if result.events.len() != input.len() {
                    return Err("Lua: salida incompleta".into());
                }
                black_box(result.events);
            }
        } else {
            for active in &mut plain {
                for event in &input {
                    match event.message {
                        MidiMessage::NoteOn { note, .. } => active.push((event.channel, note)),
                        MidiMessage::NoteOff { note } => {
                            let index = active
                                .iter()
                                .rposition(|&key| key == (event.channel, note))
                                .ok_or("Base: NoteOff sin NoteOn")?;
                            active.remove(index);
                        }
                        _ => {}
                    }
                    black_box(event);
                }
            }
        }
        let elapsed = start.elapsed().as_nanos() as u64;
        missed += usize::from(elapsed > deadline_ns);
        samples.push(elapsed);
    }
    samples.sort_unstable();
    Ok((
        percentile(&samples, 500),
        percentile(&samples, 990),
        percentile(&samples, 999),
        *samples.last().unwrap(),
        missed,
    ))
}

fn main() -> Result<(), String> {
    println!("rate_hz,frames,instances,mode,samples,p50_ns,p99_ns,p999_ns,max_ns,missed");
    for frames in [64, 128, 256, 512] {
        for instances in [1, 8, 32] {
            for (mode, lua) in [("base", false), ("lua", true)] {
                let (p50, p99, p999, max, missed) = measure(instances, frames, lua)?;
                println!(
                    "{RATE},{frames},{instances},{mode},{SAMPLES},{p50},{p99},{p999},{max},{missed}"
                );
            }
        }
    }
    Ok(())
}
