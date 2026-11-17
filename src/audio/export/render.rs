//! Mezcla por bloques y rango común. Pre-roll desde cero para conservar estado MIDI/FX.
use super::{Snapshot, sources::Sources};
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

pub(super) fn range(snapshot: &Snapshot, options: &ExportOptions) -> Result<RenderRange, String> {
    if !snapshot.bpm.is_finite() || snapshot.bpm <= 0.0 {
        return Err("Tempo inválido".into());
    }
    let end_beat = snapshot
        .audio
        .iter()
        .filter(|c| options.includes_track(c.track))
        .map(|c| c.start_beat + c.length_seconds() * snapshot.bpm / 60.0)
        .chain(
            snapshot
                .midi
                .iter()
                .filter(|c| {
                    options.includes_track(c.track)
                        || snapshot.hilo.iter().any(|branch| {
                            branch.group_index == c.track
                                && options.includes_track(branch.track_index)
                        })
                })
                .map(|c| c.end_beat()),
        )
        .fold(0.0, f64::max);
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
    snapshot: &Snapshot,
    options: &ExportOptions,
    tracks: &[usize],
    range: RenderRange,
    cancel: &AtomicBool,
    mut consume: impl FnMut(&[StereoFrame], u64) -> Result<(), String>,
) -> Result<(), String> {
    let mut sources = Sources::prepare(snapshot, tracks, options.sample_rate, cancel)?;
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
    let RenderRange {
        start,
        content_end,
        end,
    } = range;
    let mut position = 0;
    let mut block = [[0.0; 2]; 512];
    while position < end {
        if cancel.load(Ordering::Relaxed) {
            return Err("Exportación cancelada".into());
        }
        if position == content_end {
            sources.finish_input();
        }
        let boundary = if position < content_end {
            content_end
        } else {
            end
        };
        let count = (boundary - position).min(block.len() as u64) as usize;
        let block = &mut block[..count];
        sources.render(position, block, position >= content_end)?;
        utility.process(block);
        if block.iter().flatten().any(|sample| !sample.is_finite()) {
            return Err("El render produjo muestras no finitas".into());
        }
        let skip = start.saturating_sub(position).min(count as u64) as usize;
        consume(&block[skip..], position + count as u64)?;
        position += count as u64;
    }
    Ok(())
}
