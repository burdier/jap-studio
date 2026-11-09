//! Worker dueño del snapshot, instancias C y archivos. Memoria de salida acotada por bloque.
use super::{
    ExportJob, Report, Snapshot,
    files::{Batch, track_name},
    lame, render, sndfile,
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

pub(crate) fn start(snapshot: Snapshot, options: ExportOptions) -> Result<ExportJob, String> {
    options.validate()?;
    let (sender, result) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(AtomicU32::new(0));
    let (worker_cancel, worker_progress) = (cancel.clone(), progress.clone());
    std::thread::Builder::new()
        .name("hay-export".into())
        .spawn(move || {
            let result = run(snapshot, options, &worker_cancel, &worker_progress);
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
    snapshot: Snapshot,
    options: ExportOptions,
    cancel: &AtomicBool,
    progress: &AtomicU32,
) -> Result<Report, String> {
    options.validate()?;
    if options.tracks.iter().any(|&i| i >= snapshot.tracks.len()) {
        return Err("Selección de pista fuera de rango".into());
    }
    let tracks: Vec<_> = (0..snapshot.tracks.len())
        .filter(|&i| {
            options.includes_track(i)
                && (snapshot.audio.iter().any(|c| c.track == i)
                    || snapshot.midi.iter().any(|c| c.track == i))
        })
        .collect();
    if tracks.is_empty() {
        return Err("No hay clips en las pistas elegidas del Arrangement".into());
    }
    let range = render::range(&snapshot, &options)?;
    let targets = if options.target == Target::Master {
        vec![(options.name.clone(), tracks)]
    } else {
        tracks
            .into_iter()
            .map(|i| {
                (
                    format!(
                        "{}-{:02}-{}",
                        options.name,
                        i + 1,
                        track_name(&snapshot.tracks[i].name)
                    ),
                    vec![i],
                )
            })
            .collect()
    };
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
    let mut overs = 0;
    for (i, ((_, tracks), encoders)) in targets.iter().zip(outputs.iter_mut()).enumerate() {
        render::render(
            &snapshot,
            &options,
            tracks,
            range,
            cancel,
            |frames, position| {
                let mut samples = [0.0; 1024];
                let mut size = 0;
                for &[left, right] in frames {
                    if options.mono {
                        samples[size] = left * 0.5 + right * 0.5;
                        size += 1;
                    } else {
                        samples[size] = left;
                        samples[size + 1] = right;
                        size += 2;
                    }
                }
                overs += samples[..size].iter().filter(|x| x.abs() > 1.0).count() as u64;
                if size > 0 {
                    for encoder in encoders.iter_mut() {
                        encoder.write(&samples[..size])?;
                    }
                }
                progress.store(
                    ((i as f64 + position as f64 / range.end as f64) * 100.0 / targets.len() as f64)
                        as u32,
                    Ordering::Relaxed,
                );
                Ok(())
            },
        )?;
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
    Ok(Report { paths, overs })
}
