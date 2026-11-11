//! Diagnóstico de latencia sin UI ni dispositivo; no es una suite de tests.
use elevate::{
    musical::{
        automation::{ControlLane, ControlPoint},
        model::{InstrumentId, NoteEvent, NoteId, Pattern},
        time::Tick,
    },
    scripting::{
        actions::{MidiOptions, MidiTool},
        edit_builtin, edit_midi, inline_subdivision,
    },
};
use std::{hint::black_box, time::Instant};
fn main() {
    for (notes, controls, selected_count) in [
        (1, 0, 1),
        (38, 0, 1),
        (4096, 0, 1),
        (38, 8192, 1),
        (4096, 0, 4096),
    ] {
        let length = Tick((notes.max(controls) as i64 + 4) * 960);
        let events = (0..notes)
            .map(|i| NoteEvent {
                id: NoteId(i as u64 + 1),
                instrument: InstrumentId(1),
                start: Tick((i / 3) as i64 * 960),
                duration: Tick(960),
                pitch: 60,
                velocity: 100,
                expression: Default::default(),
            })
            .collect();
        let mut pattern = Pattern::restore(length, events).unwrap();
        pattern
            .restore_controls(
                (0..controls)
                    .map(|i| ControlPoint {
                        id: i as u64 + 1,
                        at: Tick(i as i64 * 120),
                        lane: ControlLane::Cc(1),
                        value: 64,
                    })
                    .collect(),
                controls as u64 + 1,
            )
            .unwrap();
        let selected: Vec<_> = pattern
            .notes()
            .iter()
            .skip(usize::from(selected_count == 1 && notes > 1))
            .take(selected_count)
            .map(|n| n.id)
            .collect();
        let baseline = edit_midi(
            MidiTool::Subdivide.source(),
            &pattern,
            &selected,
            MidiOptions::default(),
        )
        .unwrap();
        for compact in [false, true] {
            let mut times = Vec::new();
            for _ in 0..12 {
                let start = Instant::now();
                let result = if compact {
                    edit_builtin(
                        MidiTool::Subdivide,
                        &pattern,
                        &selected,
                        MidiOptions::default(),
                    )
                } else {
                    edit_midi(
                        MidiTool::Subdivide.source(),
                        &pattern,
                        &selected,
                        MidiOptions::default(),
                    )
                };
                match result {
                    Ok(edit) => {
                        times.push(start.elapsed().as_secs_f64() * 1000.);
                        if edit.pattern != baseline.pattern {
                            panic!("El diagnóstico detectó un cambio de resultado");
                        }
                        black_box(edit);
                    }
                    Err(error) => {
                        eprintln!("{notes} notas / {controls} CC: {error}");
                        break;
                    }
                }
            }
            times.sort_by(f64::total_cmp);
            if !times.is_empty() {
                println!(
                    "{notes} notas, {controls} CC, {selected_count} seleccionadas: {} p50={:.3} ms, max={:.3} ms (inmediato={})",
                    if compact {
                        "Lua compacto"
                    } else {
                        "Lua completo"
                    },
                    times[times.len() / 2],
                    times[times.len() - 1],
                    inline_subdivision(&pattern, &selected, MidiOptions::default())
                );
            }
        }
    }
}
