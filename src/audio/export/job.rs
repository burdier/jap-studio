//! Worker dueño del snapshot, instancias C y archivos. Memoria de salida acotada por bloque.
use super::{
    ExportJob, Report, Snapshot, Stem, StemBuffers,
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
    let mut overs = 0;
    render::render(
        snapshot,
        &options,
        &mut taps,
        cancel,
        |master, taps, range, percent| {
            for ((_, id), encoders) in targets.iter().zip(&mut outputs) {
                let bus = match id {
                    Some(id) => &taps[id][..],
                    None => master,
                };
                let frames = &bus[range.clone()];
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
            }
            progress.store(percent, Ordering::Relaxed);
            Ok(())
        },
    )?;
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
