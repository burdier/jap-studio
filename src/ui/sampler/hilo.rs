//! Añade una rama preparada desde el Explorador y conserva el clip MIDI.
use super::super::DawApp;
use crate::instruments::sampler::{SampleAsset, Sampler};
use elevate::{
    devices::sampler::{ROOT, TUNE},
    musical::{
        model::InstrumentId,
        track::{HiloMode, Track, TrackId, TrackKind, validate_track_structure},
    },
};
use gpui::{AppContext, Context};
use std::path::PathBuf;

impl DawApp {
    pub(in crate::ui) fn add_sample_to_hilo(
        &mut self,
        path: PathBuf,
        note: u8,
        cents: i32,
        cx: &mut Context<Self>,
    ) {
        if self.sampler_ui.busy {
            self.status_message = "Espera a que termine de preparar el sample".into();
            return;
        }
        let Some(clip) = self
            .timeline
            .midi_clip
            .as_ref()
            .filter(|clip| clip.drum_target.is_none())
        else {
            self.status_message = "Selecciona un clip MIDI del Arrangement".into();
            return;
        };
        let Some(track) = self.tracks.get(clip.track) else {
            return;
        };
        let source = track.id;
        let target = clip
            .hilo_target
            .or_else(|| track.hilo.map(|_| track.id))
            .or_else(|| {
                track.parent.filter(|parent| {
                    self.tracks
                        .iter()
                        .any(|t| t.id == *parent && t.hilo.is_some())
                })
            });
        if target.is_none() && (track.instrument.is_none() || track.parent.is_some()) {
            self.status_message = "Usa un clip de un Hilo o una pista raíz con instrumento".into();
            return;
        }
        let clip_id = clip.id;
        let revision = self.track_revision;
        self.sampler_ui.busy = true;
        self.status_message = "Preparando rama del Hilo…".into();
        let job = cx.background_spawn(async move {
            let mut params = elevate::devices::sampler::SamplerParams::default();
            params.set(ROOT, f32::from(note));
            params.set(TUNE, -(cents as f32) / 100.0);
            let asset = SampleAsset::load(&path)?;
            let engine = Sampler::prepare(asset.clone(), params.clone())?;
            Ok::<_, String>((asset, params, engine))
        });
        self.sampler_ui.task = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.sampler_ui.busy = false;
                this.sampler_ui.task = None;
                this.status_message = (|| {
                    if revision != this.track_revision
                        || this
                            .timeline
                            .midi_clip
                            .as_ref()
                            .is_none_or(|clip| clip.id != clip_id)
                    {
                        return Err("La selección o las pistas cambiaron durante la carga".into());
                    }
                    let (asset, params, engine) = result?;
                    let id = InstrumentId::fresh();
                    let (tracks, sources, group) =
                        hilo_branch(&this.tracks, source, target, id, &asset.file.name)?;
                    let levels = sources
                        .iter()
                        .map(|s| s.map_or(75, |i| this.track_levels[i]))
                        .collect();
                    let pans = sources
                        .iter()
                        .map(|s| s.map_or(50, |i| this.track_pans[i]))
                        .collect();
                    let mut sampler = Sampler::default();
                    sampler.params = params;
                    sampler.configure_audio(this.settings.audio.saved.clone());
                    sampler.install(asset, engine);
                    let selected = vec![group];
                    this.commit_track_plan(
                        tracks, sources, levels, pans, selected, true, false, None,
                    );
                    this.sampler_instances.insert(id, sampler);
                    this.route_active_midi_clip(Some(group));
                    Ok("Sample añadido al Hilo · el clip MIDI sigue siendo el mismo".to_owned())
                })()
                .unwrap_or_else(|error: String| error);
                cx.notify();
            });
        }));
    }
}

fn hilo_branch(
    original: &[Track],
    source: TrackId,
    target: Option<TrackId>,
    id: InstrumentId,
    name: &str,
) -> Result<(Vec<Track>, Vec<Option<usize>>, TrackId), String> {
    let mut tracks = original.to_vec();
    let mut sources: Vec<_> = (0..original.len()).map(Some).collect();
    let (group, at) = if let Some(target) = target {
        let group = tracks
            .iter()
            .position(|t| t.id == target && t.hilo.is_some())
            .ok_or("El Hilo ya no existe")?;
        let at = group
            + 1
            + tracks[group + 1..]
                .iter()
                .take_while(|t| t.parent == Some(target))
                .count();
        (target, at)
    } else {
        let row = tracks
            .iter()
            .position(|t| t.id == source)
            .ok_or("La pista ya no existe")?;
        let mut group = Track::new(format!("Hilo · {}", tracks[row].name), TrackKind::Group);
        group.hilo = Some(HiloMode::Layer);
        tracks[row].parent = Some(group.id);
        let id = group.id;
        tracks.insert(row, group);
        sources.insert(row, None);
        (id, row + 2)
    };
    let mut branch = Track::new(name, TrackKind::Synth);
    branch.parent = Some(group);
    branch.instrument = Some(id);
    tracks.insert(at, branch);
    sources.insert(at, None);
    if tracks.len() > 256 {
        return Err("El Arrangement admite hasta 256 pistas".into());
    }
    validate_track_structure(&tracks)?;
    Ok((tracks, sources, group))
}
