//! Acción de exportación: captura estado propio y observa el resultado del worker.
mod hilo;
use super::DawApp;
use crate::audio::export::{self, Snapshot, TrackMix};
use elevate::export::ExportOptions;
use std::sync::{atomic::Ordering, mpsc::TryRecvError};

impl DawApp {
    pub(super) fn start_export(&mut self, command: &str) {
        if self.export_job.is_some() {
            self.status_message =
                "Ya hay una exportación activa · :export.cancel para cancelarla".into();
            return;
        }
        let result = (|| {
            let mut options =
                ExportOptions::parse_with_defaults(command, &self.settings.export.saved)?;
            // Elegir una cabecera de grupo incluye también sus pistas hijas.
            if !options.tracks.is_empty() {
                let selected = options.tracks.clone();
                for index in selected {
                    if let Some(group) = self.tracks.get(index)
                        && group.kind == elevate::musical::track::TrackKind::Group
                    {
                        for (child, track) in self.tracks.iter().enumerate() {
                            if track.parent == Some(group.id) && !options.tracks.contains(&child) {
                                options.tracks.push(child);
                            }
                        }
                    }
                }
            }
            if let Ok(rest) = options.directory.strip_prefix("~") {
                options.directory = dirs::home_dir()
                    .ok_or("No se encontró la carpeta personal")?
                    .join(rest);
            }
            if options.directory.is_relative() {
                options.directory = std::env::current_dir()
                    .map_err(|e| e.to_string())?
                    .join(&options.directory);
            }
            if self.tracks.iter().enumerate().any(|(index, t)| {
                (options.includes_track(index)
                    || self.tracks.iter().enumerate().any(|(child_index, child)| {
                        options.includes_track(child_index) && child.parent == Some(t.id)
                    }))
                    && crate::audio::effects::contains_vst(&t.chain)
            }) {
                return Err("La exportación con efectos VST3 todavía requiere render offline de sus estados; no se exportará sin ellos".into());
            }
            if self.tracks.iter().enumerate().any(|(index, t)| {
                (options.includes_track(index)
                    || self.tracks.iter().enumerate().any(|(child_index, child)| {
                        options.includes_track(child_index) && child.parent == Some(t.id)
                    }))
                    && crate::audio::effects::contains_component(
                        &t.chain,
                        &self.lua_components.instances,
                    )
            }) {
                return Err("La exportación con componentes de audio Lua todavía no está disponible; no se exportará sin esos efectos".into());
            }
            let audio = self
                .timeline
                .other_audio_clips
                .iter()
                .chain(self.timeline.clip.iter())
                .cloned()
                .collect();
            let midi: Vec<_> = self
                .timeline
                .other_midi_clips
                .iter()
                .chain(self.timeline.midi_clip.iter())
                .cloned()
                .collect();
            let hilo = self.capture_hilo_export(&midi, &options)?;
            let piano_ids: std::collections::HashSet<_> = midi
                .iter()
                .filter(|clip| options.includes_track(clip.track) && !clip.props.muted)
                .filter(|clip| self.tracks[clip.track].hilo.is_none())
                .filter_map(|clip| self.tracks[clip.track].instrument)
                .filter(|id| self.is_piano_id(*id))
                .collect();
            if piano_ids.len() > 1 {
                return Err(
                    "Exportar varias instancias de Hay Piano todavía no está disponible".into(),
                );
            }
            let needs_piano = midi.iter().any(|c| {
                options.includes_track(c.track)
                    && !c.props.muted
                    && self.tracks[c.track].hilo.is_none()
                    && self.track_gain(c.track) > 0.0
                    && !self.tracks[c.track]
                        .instrument
                        .is_some_and(|id| self.is_sampler_id(id))
            });
            if midi.iter().any(|clip| {
                options.includes_track(clip.track)
                    && !clip.props.muted
                    && self.tracks[clip.track].hilo.is_none()
                    && self.track_gain(clip.track) > 0.
                    && self.tracks[clip.track]
                        .instrument
                        .is_some_and(|id| self.is_vst_id(id))
            }) {
                return Err("La exportación de instrumentos VST3 todavía no está disponible; excluye esa pista del rango de exportación".into());
            }
            let piano = if needs_piano {
                Some(
                    self.piano_snapshot(
                        piano_ids
                            .iter()
                            .next()
                            .copied()
                            .unwrap_or(self.instrument.id()),
                    )?,
                )
            } else {
                None
            };
            let sampler_tracks: Vec<usize> = self
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.instrument.is_some_and(|id| self.is_sampler_id(id)))
                .map(|(i, _)| i)
                .collect();
            let needs_sampler = midi.iter().any(|c| {
                sampler_tracks.contains(&c.track)
                    && options.includes_track(c.track)
                    && !c.props.muted
                    && self.track_gain(c.track) > 0.
            });
            let sampler = if needs_sampler {
                if sampler_tracks.len() > 1 {
                    return Err(
                        "Exportar varias instancias de sampler aún no está disponible".into(),
                    );
                }
                let id = self.tracks[sampler_tracks[0]].instrument.unwrap();
                Some((sampler_tracks, self.sampler_snapshot(id)?))
            } else {
                None
            };
            let tracks = self
                .tracks
                .iter()
                .enumerate()
                .map(|(i, track)| TrackMix {
                    name: track.name.to_string(),
                    gain: self.track_gain(i),
                    fx_gain: self.track_fx_gain(i),
                    pan: self.track_pan(i),
                })
                .collect();
            let snapshot = Snapshot {
                audio,
                midi,
                piano,
                sampler,
                hilo,
                tracks,
                utility: self.utility.state(),
                bpm: self.timeline.tempo.bpm(),
            };
            export::start(snapshot, options)
        })();
        match result {
            Ok(job) => {
                self.export_job = Some(job);
                self.status_message = "Preparando exportación del Arrangement…".into();
            }
            Err(error) => self.status_message = format!("Export: {error}"),
        }
    }

    pub(super) fn cancel_export(&mut self) {
        if let Some(job) = &self.export_job {
            job.cancel.store(true, Ordering::Relaxed);
            self.status_message = "Cancelando exportación…".into();
        } else {
            self.status_message = "No hay exportación activa".into();
        }
    }

    pub(super) fn poll_export(&mut self) {
        let Some(job) = &self.export_job else {
            return;
        };
        match job.result.try_recv() {
            Ok(result) => {
                self.export_job = None;
                self.status_message = match result {
                    Ok(report) => {
                        let folder = report
                            .paths
                            .first()
                            .and_then(|p| p.parent())
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        let peak = if report.overs == 0 {
                            String::new()
                        } else {
                            format!(
                                " · {} muestras >0 dBFS (PCM entero/MP3 saturan; float conserva)",
                                report.overs
                            )
                        };
                        format!(
                            "Exportados {} archivos → {folder}{peak}",
                            report.paths.len()
                        )
                    }
                    Err(error) => format!("Export: {error}"),
                };
            }
            Err(TryRecvError::Empty) => {
                if !job.cancel.load(Ordering::Relaxed) {
                    self.status_message = format!(
                        "Exportando Arrangement · {}% · :export.cancel",
                        job.progress.load(Ordering::Relaxed)
                    );
                }
            }
            Err(TryRecvError::Disconnected) => {
                self.export_job = None;
                self.status_message = "Export: el worker terminó sin resultado".into();
            }
        }
    }
}
