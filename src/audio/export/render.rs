//! Mezcla por bloques y rango común. Pre-roll desde cero para conservar estado MIDI/FX.
use super::{Snapshot, StemBuffers};
use crate::audio::playback::Mix;
use elevate::devices::{Device, utility::Utility};
use elevate::export::ExportOptions;
use elevate::{ProcessSpec, StereoFrame};
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) fn frame_at(seconds: f64, rate: u32) -> Result<u64, String> {
    let frames = (seconds * rate as f64).round();
    if !frames.is_finite() || frames < 0.0 || frames >= u64::MAX as f64 {
        return Err("Tiempo de exportación fuera de rango".into());
    }
    Ok(frames as u64)
}

#[derive(Clone, Copy)]
pub(super) struct RenderRange {
    pub start: u64,
    pub content_end: u64,
    pub end: u64,
}

fn range(
    snapshot: &Snapshot,
    options: &ExportOptions,
    natural_end: u64,
) -> Result<RenderRange, String> {
    let bpm = snapshot.bpm;
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err("Tempo inválido".into());
    }
    let end_beat = natural_end as f64 * bpm / (60.0 * f64::from(options.sample_rate));
    let end = options
        .length_beats
        .map_or(end_beat, |length| options.start_beat + length);
    if end <= options.start_beat {
        return Err("El rango no contiene material; usa -start y -length".into());
    }
    let start = frame_at(
        options.start_beat * 60.0 / snapshot.bpm,
        options.sample_rate,
    )?;
    let content_end = frame_at(end * 60.0 / snapshot.bpm, options.sample_rate)?;
    let end = content_end
        .checked_add(frame_at(options.tail_seconds, options.sample_rate)?)
        .ok_or("Cola fuera de rango")?;
    if content_end <= start {
        return Err("Rango menor que una muestra".into());
    }
    Ok(RenderRange {
        start,
        content_end,
        end,
    })
}

pub(super) fn render(
    mut snapshot: Snapshot,
    options: &ExportOptions,
    taps: &mut StemBuffers,
    cancel: &AtomicBool,
    mut consume: impl FnMut(
        &[StereoFrame],
        &StemBuffers,
        std::ops::Range<usize>,
        u32,
    ) -> Result<(), String>,
) -> Result<(), String> {
    let drum_beats = snapshot
        .drums
        .as_ref()
        .and_then(|d| d.score.as_ref())
        .map_or(0.0, |s| {
            s.pattern.length().0 as f64 / elevate::musical::time::PPQ as f64
        });
    let racks: Vec<_> = snapshot
        .inputs
        .iter()
        .filter_map(|input| input.rack.clone())
        .collect();
    let mut mix = Mix::prepare(
        std::mem::take(&mut snapshot.inputs),
        snapshot.drums.take(),
        options.sample_rate,
        snapshot.bpm,
        snapshot.meter,
    )?;
    let drum_io = mix.take_drum_io();
    let finite_drum_end = drum_io.as_ref().map_or(0, |(live, _)| {
        live.state
            .read(|d| if d.loops() { 0 } else { d.arrangement_end() })
    });
    let natural_end = mix.end.max(finite_drum_end).max(frame_at(
        drum_beats * 60.0 / snapshot.bpm,
        options.sample_rate,
    )?);
    let range = range(&snapshot, options, natural_end)?;
    let drain = options.length_beats.is_none() && options.tail_seconds == 0.0 && drum_io.is_some();
    let spec = ProcessSpec::new(options.sample_rate as f64).map_err(|e| e.to_string())?;
    let mut utility = Utility::new(spec).map_err(|e| e.to_string())?;
    utility
        .set_parameter(Utility::GAIN, snapshot.utility.gain)
        .map_err(|e| e.to_string())?;
    utility
        .set_bypassed(
            !options.includes_master_fx()
                || !snapshot.utility.inserted
                || snapshot.utility.bypassed,
        )
        .map_err(|e| e.to_string())?;
    utility.reset();
    let master_rack = options
        .includes_master_fx()
        .then(|| snapshot.master_rack.clone())
        .flatten();
    let master_fx_ctl = crate::audio::fx_gain::FxGain::new(if options.includes_master_fx() {
        snapshot.master_fx_gain
    } else {
        1.0
    });
    master_fx_ctl.set_rack(master_rack.clone(), options.sample_rate);
    let mut master_fx = master_fx_ctl.prepare();
    let RenderRange {
        start,
        content_end,
        end,
    } = range;
    let mut position = 0;
    let mut block = [[0.0; 2]; 256];
    while position < end || (drain && mix.has_drum_one_shot_tail()) {
        if cancel.load(Ordering::Relaxed) {
            return Err("Exportación cancelada".into());
        }
        if position == content_end {
            mix.finish_input();
        }
        let boundary = if position < content_end {
            content_end
        } else {
            if position >= end {
                position.saturating_add(256)
            } else {
                end
            }
        };
        let count = (boundary - position).min(block.len() as u64) as usize;
        let block = &mut block[..count];
        let stems = (!taps.is_empty()).then_some(&mut *taps);
        if let Some((live, _)) = &drum_io {
            live.state.read(|d| {
                mix.render_block(
                    position,
                    block,
                    None,
                    Some(d),
                    position >= content_end,
                    stems,
                )
            });
        } else {
            mix.render_block(position, block, None, None, position >= content_end, stems);
        }
        // Cadena FX del Master solo sobre la mezcla Master (los stems individuales permanecen limpios).
        if options.includes_master_fx() {
            let ctx = crate::audio::vst3::TransportContext::new(
                position,
                options.sample_rate,
                snapshot.bpm,
                snapshot.meter,
                None,
            )
            .ok()
            .map(|mut c| {
                c.playing = position < content_end;
                c
            });
            master_fx.process_timed(&master_fx_ctl, block, ctx);
        }
        // Utility es una ganancia estéreo lineal. El mismo coeficiente se aplica a todas las salidas.
        let mut gains = [[1.; 2]; 256];
        utility.process(&mut gains[..count]);
        for bus in std::iter::once(&mut *block).chain(taps.values_mut().map(|s| &mut s[..count])) {
            for (frame, gain) in bus.iter_mut().zip(&gains) {
                frame[0] *= gain[0];
                frame[1] *= gain[1];
            }
        }
        if block.iter().flatten().any(|sample| !sample.is_finite()) {
            return Err("El render produjo muestras no finitas".into());
        }
        if mix.vsts.iter().any(|plugin| plugin.failed())
            || racks.iter().any(|rack| rack.failed())
            || master_rack.as_ref().is_some_and(|rack| rack.failed())
        {
            return Err("Un VST3 falló durante la exportación".into());
        }
        let skip = start.saturating_sub(position).min(count as u64) as usize;
        let progress = (100.0 * (position + count as u64) as f64 / end as f64).min(100.) as u32;
        consume(block, taps, skip..count, progress)?;
        position += count as u64;
    }
    Ok(())
}
