//! Ajustes de la pieza activa; el sampler de pistas conserva su propio estado.
use super::{PatternAction, PatternEditor};
use crate::{
    audio::AudioFile,
    ui::{DawApp, workspace_focus::Panel},
};
use elevate::{
    devices::sampler::PARAMS,
    musical::{drums::DrumLayer, model::InstrumentId},
};
use gpui::{Context, PathPromptOptions, Window};
use std::{
    future::{Future, ready},
    path::PathBuf,
};

pub(in crate::ui) enum DrumSampleTarget {
    NewPiece,
    AddLayer(InstrumentId),
    ReplaceLayer(InstrumentId, usize, String),
}

impl PatternEditor {
    pub(super) fn apply_inspector(
        &mut self,
        action: PatternAction,
    ) -> Result<&'static str, String> {
        use PatternAction::*;
        let id = match &action {
            SampleParam { piece, .. }
            | Choke { piece, .. }
            | AddSample { piece, .. }
            | ReplaceSample { piece, .. }
            | RemoveSample { piece, .. }
            | LayerVelocity { piece, .. }
            | Filter(piece)
            | FullSample(piece)
            | RetriggerChoke(piece) => *piece,
            _ => unreachable!("acción ajena al inspector"),
        };
        let piece = self
            .session
            .kit
            .pieces
            .iter_mut()
            .find(|piece| piece.id == id)
            .ok_or("La pieza ya no existe")?;
        match action {
            SampleParam { param, value, .. } if param < PARAMS.len() => {
                piece.sampler.set(param, value);
                Ok("Instrumento ajustado")
            }
            Filter(_) => {
                piece.sampler.filter = !piece.sampler.filter;
                Ok("Filtro actualizado")
            }
            FullSample(_) => {
                piece.sampler.play_full_sample();
                Ok("Cola completa activada · Sustain 100 %")
            }
            RetriggerChoke(_) => {
                piece.sampler.choke = !piece.sampler.choke;
                Ok(if piece.sampler.choke {
                    "Cada golpe corta la cola anterior de esta pieza"
                } else {
                    "Los golpes de esta pieza pueden superponerse"
                })
            }
            Choke { delta, .. } => {
                let group = i32::from(piece.choke_group.unwrap_or(0)) + delta;
                piece.choke_group = u16::try_from(group.clamp(0, i32::from(u16::MAX)))
                    .ok()
                    .filter(|&value| value != 0);
                Ok("Grupo de corte actualizado")
            }
            AddSample { path, .. } => {
                let layers = &mut piece.layers;
                let (min, max) = if layers.is_empty() {
                    (1, 127)
                } else {
                    let previous = layers
                        .iter_mut()
                        .max_by_key(|layer| layer.velocity_max - layer.velocity_min)
                        .filter(|layer| layer.velocity_max > layer.velocity_min)
                        .ok_or("Amplía un rango de velocidad para añadir otra capa")?;
                    let max = previous.velocity_max;
                    previous.velocity_max =
                        previous.velocity_min + (max - previous.velocity_min) / 2;
                    (previous.velocity_max + 1, max)
                };
                layers.push(DrumLayer {
                    source: path,
                    velocity_min: min,
                    velocity_max: max,
                });
                layers.sort_by_key(|layer| layer.velocity_min);
                Ok("Capa añadida")
            }
            ReplaceSample {
                layer,
                previous,
                path,
                ..
            } => {
                let layer = piece
                    .layers
                    .get_mut(layer)
                    .filter(|layer| layer.source == previous)
                    .ok_or("La capa cambió mientras se abría la muestra")?;
                layer.source = path;
                Ok("Muestra reemplazada")
            }
            RemoveSample { layer, .. } if layer < piece.layers.len() => {
                piece.layers.remove(layer);
                Ok("Capa eliminada")
            }
            LayerVelocity {
                layer,
                upper,
                delta,
                ..
            } => {
                let layer = piece.layers.get_mut(layer).ok_or("La capa ya no existe")?;
                if upper {
                    layer.velocity_max = (i16::from(layer.velocity_max) + delta)
                        .clamp(i16::from(layer.velocity_min), 127)
                        as u8;
                } else {
                    layer.velocity_min = (i16::from(layer.velocity_min) + delta)
                        .clamp(1, i16::from(layer.velocity_max))
                        as u8;
                }
                Ok("Rango de velocidad ajustado")
            }
            _ => Err("Control de instrumento inválido".into()),
        }
    }
}

impl DawApp {
    pub(in crate::ui) fn choose_drum_sample(
        &mut self,
        target: DrumSampleTarget,
        cx: &mut Context<Self>,
    ) {
        if self.pattern_editor.sample_task.is_some() {
            self.status_message = "Espera a que termine la carga de la muestra".into();
            return;
        }
        let title = match target {
            DrumSampleTarget::NewPiece => "Añadir una muestra al kit",
            DrumSampleTarget::AddLayer(_) => "Añadir una muestra a la pieza",
            DrumSampleTarget::ReplaceLayer(..) => "Cambiar muestra de la capa",
        };
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(title.into()),
        });
        self.prepare_drum_sample(
            target,
            async move {
                match prompt.await {
                    Ok(Ok(paths)) => Ok(paths.and_then(|paths| paths.into_iter().next())),
                    _ => Err("No se pudo abrir el selector de muestras".into()),
                }
            },
            cx,
        );
    }

    pub(in crate::ui) fn drop_drum_sample(
        &mut self,
        path: PathBuf,
        piece: Option<InstrumentId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pattern_editor.sample_task.is_some() {
            self.status_message = "Espera a que termine la carga de la muestra".into();
            cx.notify();
            return;
        }
        let target = match piece {
            Some(id) => {
                let Some(piece) = self
                    .pattern_editor
                    .session
                    .kit
                    .pieces
                    .iter()
                    .find(|piece| piece.id == id)
                else {
                    self.status_message = "La pieza ya no existe".into();
                    cx.notify();
                    return;
                };
                match piece.layers.first() {
                    Some(layer) => DrumSampleTarget::ReplaceLayer(id, 0, layer.source.clone()),
                    None => DrumSampleTarget::AddLayer(id),
                }
            }
            None => DrumSampleTarget::NewPiece,
        };
        self.workspace_focus.active = Panel::Center;
        window.focus(&self.focus_handle, cx);
        self.sync_midi_focus();
        self.status_message = format!("Cargando {}…", path.display());
        self.prepare_drum_sample(target, ready(Ok(Some(path))), cx);
        cx.notify();
    }

    fn prepare_drum_sample(
        &mut self,
        target: DrumSampleTarget,
        selection: impl Future<Output = Result<Option<PathBuf>, String>> + 'static,
        cx: &mut Context<Self>,
    ) {
        let background = cx.background_executor().clone();
        self.pattern_editor.sample_task = Some(cx.spawn(async move |this, cx| {
            let result = match selection.await {
                Ok(Some(path)) => {
                    background
                        .spawn(async move {
                            let source = path
                                .to_str()
                                .ok_or("La ruta de la muestra debe ser UTF-8")?;
                            AudioFile::decode(&path).map_err(|error| {
                                format!("No se pudo cargar {} · {error}", path.display())
                            })?;
                            Ok(Some(source.to_owned()))
                        })
                        .await
                }
                Ok(None) => Ok(None),
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.pattern_editor.sample_task = None;
                match result {
                    Ok(Some(path)) => {
                        let action = match target {
                            DrumSampleTarget::NewPiece => PatternAction::AddPiece(path),
                            DrumSampleTarget::AddLayer(piece) => {
                                PatternAction::AddSample { piece, path }
                            }
                            DrumSampleTarget::ReplaceLayer(piece, layer, previous) => {
                                PatternAction::ReplaceSample {
                                    piece,
                                    layer,
                                    previous,
                                    path,
                                }
                            }
                        };
                        this.apply_pattern_action(action);
                    }
                    Ok(None) => {}
                    Err(error) => this.status_message = error,
                }
                cx.notify();
            });
        }));
    }
}
