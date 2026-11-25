//! Worker dueño del snapshot, instancias C y archivos. Memoria de salida acotada por bloque.
use super::{
    ExportJob, LoudnessSummary, Report, Snapshot, Stem, StemBuffers,
    files::{Batch, track_name},
    lame,
    mastering::{self, ExportMeter, StageFile, TruePeakLimiter},
    render, sndfile,
};
use elevate::export::{ExportOptions, Format, Target};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
    mpsc,
};

enum Encoder {
    Pcm(sndfile::Writer),
    Mp3(lame::Writer),
}
impl Encoder {
    fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        match self {
            Self::Pcm(w) => w.write(samples),
            Self::Mp3(w) => w.write(samples),
        }
    }
    fn finish(self) -> Result<(), String> {
        match self {
            Self::Pcm(w) => w.finish(),
            Self::Mp3(w) => w.finish(),
        }
    }
}

pub(crate) fn start(
    mut snapshot: Snapshot,
    options: ExportOptions,
    plugins: super::Plugins,
) -> Result<ExportJob, String> {
    options.validate()?;
    let (sender, result) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(AtomicU32::new(0));
    let (worker_cancel, worker_progress) = (cancel.clone(), progress.clone());
    std::thread::Builder::new()
        .name("hay-export".into())
        .spawn(move || {
            let result = plugins
                .prepare(
                    &mut snapshot.inputs,
                    &mut snapshot.master_rack,
                    options.sample_rate,
                    &worker_cancel,
                )
                .and_then(|()| run(snapshot, options, &worker_cancel, &worker_progress));
            let _ = sender.send(result);
        })
        .map_err(|e| e.to_string())?;
    Ok(ExportJob {
        result,
        cancel,
        progress,
    })
}

pub(super) fn run(
    mut snapshot: Snapshot,
    options: ExportOptions,
    cancel: &AtomicBool,
    progress: &AtomicU32,
) -> Result<Report, String> {
    options.validate()?;
    if options.tracks.iter().any(|&i| i >= snapshot.tracks.len()) {
        return Err("Selección de pista fuera de rango".into());
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Exportación cancelada".into());
    }
    let includes = |id| {
        snapshot
            .tracks
            .iter()
            .position(|(track, _)| *track == id)
            .is_some_and(|i| options.includes_track(i))
    };
    let buses: std::collections::HashSet<_> = snapshot
        .inputs
        .iter()
        .filter(|input| includes(input.id))
        .filter_map(|input| input.parent)
        .collect();
    snapshot
        .inputs
        .retain(|input| includes(input.id) || buses.contains(&input.id));
    crate::audio::apply_preplay(&mut snapshot.inputs)?;
    let mut targets = Vec::new();
    for (i, (id, name)) in snapshot.tracks.iter().enumerate() {
        if snapshot.inputs.iter().any(|t| {
            (t.id == *id || (options.tracks.contains(&i) && t.parent == Some(*id)))
                && (!t.clips.is_empty()
                    || t.midi.as_ref().is_some_and(|m| {
                        !m.clips.is_empty() || m.hilo.as_ref().is_some_and(|h| !h.clips.is_empty())
                    }))
        }) {
            targets.push((
                format!("{}-{:02}-{}", options.name, i + 1, track_name(name)),
                Some(Stem::Track(*id)),
            ));
        }
    }
    if let Some(drums) = &snapshot.drums
        && let Some(score) = &drums.score
    {
        for (i, piece) in drums.kit.pieces.iter().enumerate() {
            if score
                .pattern
                .notes()
                .iter()
                .any(|n| n.instrument == piece.id)
            {
                targets.push((
                    format!(
                        "{}-Drums-{:02}-{}",
                        options.name,
                        i + 1,
                        track_name(&piece.name)
                    ),
                    Some(Stem::Drum(piece.id)),
                ));
            }
        }
    }
    if targets.is_empty() {
        return Err("No hay material en las pistas elegidas".into());
    }
    if options.target == Target::Master {
        targets.clear();
        targets.push((options.name.clone(), None));
    }
    let mut taps: StemBuffers = targets
        .iter()
        .filter_map(|(_, id)| id.map(|id| (id, [[0.; 2]; 256])))
        .collect();
    let mut batch = Batch::new();
    let mut outputs = Vec::new();
    // Validar todos los destinos/codecs antes de renderizar y publicar cualquiera.
    for (name, _) in &targets {
        let mut encoders = Vec::new();
        for &format in &options.formats {
            let path = options
                .directory
                .join(format!("{name}.{}", format.extension()));
            let file = batch.create(&path)?;
            encoders.push(if format == Format::Mp3 {
                Encoder::Mp3(lame::Writer::new(file, &options)?)
            } else {
                Encoder::Pcm(sndfile::Writer::new(file, format, &options)?)
            });
        }
        outputs.push(encoders);
    }
    let channels = if options.mono { 1 } else { 2 };
    let mut overs = 0u64;
    let mut loudness = LoudnessSummary::default();

    if let Some(target_lufs) = options.normalize_lufs {
        // Dos pasadas con archivo temporal raw f32: memoria acotada incluso en sesiones largas.
        let mut stages: Vec<(StageFile, ExportMeter)> = targets
            .iter()
            .map(|_| {
                Ok((
                    StageFile::create()?,
                    ExportMeter::new(channels, options.sample_rate)?,
                ))
            })
            .collect::<Result<_, String>>()?;
        render::render(
            snapshot,
            &options,
            &mut taps,
            cancel,
            |master, taps, range, percent| {
                for ((_, id), (stage, meter)) in targets.iter().zip(&mut stages) {
                    let bus = match id {
                        Some(id) => &taps[id][..],
                        None => master,
                    };
                    let mut samples = [0.0; 1024];
                    let size = pack_frames(&bus[range.clone()], options.mono, &mut samples);
                    if size > 0 {
                        meter.push_interleaved(&samples[..size]);
                        stage.write(&samples[..size])?;
                    }
                }
                progress.store((percent * 70) / 100, Ordering::Relaxed);
                Ok(())
            },
        )?;
        let total_samples: u64 = stages
            .iter()
            .map(|(s, _)| s.total_samples())
            .sum::<u64>()
            .max(1);
        let mut processed_samples = 0u64;
        let mut byte_buf = [0u8; 4096];
        let mut chunk = Vec::with_capacity(1024);
        let mut processed = Vec::with_capacity(1024);
        for (idx, ((stage, pre_meter), encoders)) in stages.iter_mut().zip(&mut outputs).enumerate()
        {
            let gain_db =
                mastering::normalization_gain_db(pre_meter.integrated_lufs(), target_lufs);
            let pre_gain = 10.0_f32.powf((gain_db / 20.0) as f32);
            let mut post_meter = ExportMeter::new(channels, options.sample_rate)?;
            let mut limiter = options
                .true_peak_limit_db
                .map(|tp| TruePeakLimiter::new(channels, options.sample_rate, tp));
            let mut reader = stage.rewind_reader()?;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Exportación cancelada".into());
                }
                let read = mastering::read_f32_chunk(&mut reader, &mut byte_buf, &mut chunk)?;
                if read == 0 {
                    break;
                }
                let out_slice: &[f32] = if let Some(lim) = &mut limiter {
                    lim.process_interleaved(&chunk, pre_gain, &mut processed);
                    &processed
                } else {
                    for s in &mut chunk {
                        *s *= pre_gain;
                    }
                    &chunk
                };
                emit_samples(out_slice, &mut post_meter, &mut overs, encoders)?;
                processed_samples += read as u64;
                let p2 = 70 + ((processed_samples * 30) / total_samples).min(30) as u32;
                progress.store(p2, Ordering::Relaxed);
            }
            let max_gr = if let Some(lim) = &mut limiter {
                lim.flush(&mut processed);
                emit_samples(&processed, &mut post_meter, &mut overs, encoders)?;
                lim.max_gain_reduction_db()
            } else {
                0.0
            };
            let summary = post_meter.summary(gain_db, max_gr);
            if idx == 0 {
                loudness = summary;
            } else {
                loudness.merge_stem(&summary);
            }
        }
    } else {
        // Una sola pasada en streaming: medición EBU R128 en vivo y limitador True Peak opcional.
        let mut pipelines: Vec<(ExportMeter, Option<TruePeakLimiter>)> = targets
            .iter()
            .map(|_| {
                Ok((
                    ExportMeter::new(channels, options.sample_rate)?,
                    options
                        .true_peak_limit_db
                        .map(|tp| TruePeakLimiter::new(channels, options.sample_rate, tp)),
                ))
            })
            .collect::<Result<_, String>>()?;
        let mut limited = Vec::with_capacity(1024);
        render::render(
            snapshot,
            &options,
            &mut taps,
            cancel,
            |master, taps, range, percent| {
                for (((_, id), encoders), (meter, limiter)) in
                    targets.iter().zip(&mut outputs).zip(&mut pipelines)
                {
                    let bus = match id {
                        Some(id) => &taps[id][..],
                        None => master,
                    };
                    let mut samples = [0.0; 1024];
                    let size = pack_frames(&bus[range.clone()], options.mono, &mut samples);
                    if size > 0 {
                        let out_slice: &[f32] = if let Some(lim) = limiter {
                            lim.process_interleaved(&samples[..size], 1.0, &mut limited);
                            &limited
                        } else {
                            &samples[..size]
                        };
                        emit_samples(out_slice, meter, &mut overs, encoders)?;
                    }
                }
                progress.store(percent, Ordering::Relaxed);
                Ok(())
            },
        )?;
        for (idx, (encoders, (meter, limiter))) in
            outputs.iter_mut().zip(&mut pipelines).enumerate()
        {
            let max_gr = if let Some(lim) = limiter {
                lim.flush(&mut limited);
                emit_samples(&limited, meter, &mut overs, encoders)?;
                lim.max_gain_reduction_db()
            } else {
                0.0
            };
            let summary = meter.summary(0.0, max_gr);
            if idx == 0 {
                loudness = summary;
            } else {
                loudness.merge_stem(&summary);
            }
        }
    }
    for encoders in outputs {
        for encoder in encoders {
            encoder.finish()?;
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Exportación cancelada".into());
    }
    let paths = batch.commit()?;
    Ok(Report {
        paths,
        overs,
        loudness,
    })
}

fn pack_frames(frames: &[[f32; 2]], mono: bool, samples: &mut [f32; 1024]) -> usize {
    let mut size = 0;
    for &[left, right] in frames {
        if mono {
            samples[size] = left * 0.5 + right * 0.5;
            size += 1;
        } else {
            samples[size] = left;
            samples[size + 1] = right;
            size += 2;
        }
    }
    size
}

fn emit_samples(
    samples: &[f32],
    meter: &mut ExportMeter,
    overs: &mut u64,
    encoders: &mut [Encoder],
) -> Result<(), String> {
    if samples.is_empty() {
        return Ok(());
    }
    meter.push_interleaved(samples);
    *overs += samples.iter().filter(|x| x.abs() > 1.0).count() as u64;
    for encoder in encoders.iter_mut() {
        encoder.write(samples)?;
    }
    Ok(())
}
