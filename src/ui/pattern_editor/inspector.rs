//! Ajustes de la pieza activa; el sampler de pistas conserva su propio estado.
use super::{PatternAction, PatternEditor};
use crate::{
    audio::AudioFile,
    ui::{DawApp, workspace_focus::Panel},
};
use elevate::{
    devices::sampler::PARAMS,
    explorer::EntryKind,
    musical::{drums::DrumLayer, model::InstrumentId},
};
use gpui::{Context, MouseMoveEvent, PathPromptOptions, Pixels, Window};
use std::{
    future::{Future, ready},
    path::PathBuf,
};

pub(in crate::ui) enum DrumSampleTarget {
    NewPiece,
    AddLayer(InstrumentId),
    ReplaceLayer(InstrumentId, usize, String),
}

pub(super) struct DrumOffsetDrag {
    piece: InstrumentId,
    layer: Option<usize>,
    y: Pixels,
    original: i16,
    draft: f32,
}

impl PatternEditor {
    pub(in crate::ui) fn layer_offset(
        &self,
        piece: InstrumentId,
        layer: usize,
        stored: u16,
    ) -> u16 {
        self.offset_drag
            .as_ref()
            .filter(|drag| drag.piece == piece && drag.layer == Some(layer))
            .map_or(stored, |drag| drag.draft.round().max(0.0) as u16)
    }

    pub(in crate::ui) fn piece_timing_offset(&self, piece: InstrumentId, stored: i16) -> i16 {
        self.offset_drag
            .as_ref()
            .filter(|drag| drag.piece == piece && drag.layer.is_none())
            .map_or(stored, |drag| drag.draft.round() as i16)
    }

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
            | LayerOffset { piece, .. }
            | CycleLayerOffsetMode { piece, .. }
            | PieceTimingOffset { piece, .. }
            | CyclePieceTimingMode { piece, .. }
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
                    start_offset: 0,
                    offset_mode: elevate::musical::drums::OffsetMode::Fixed,
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
            LayerOffset { layer, value, .. } => {
                let layer = piece.layers.get_mut(layer).ok_or("La capa ya no existe")?;
                layer.start_offset = value.min(DrumLayer::MAX_START_OFFSET);
                Ok("Offset de la muestra ajustado")
            }
            CycleLayerOffsetMode { layer, .. } => {
                let layer = piece.layers.get_mut(layer).ok_or("La capa ya no existe")?;
                layer.offset_mode = layer.offset_mode.next();
                Ok(match layer.offset_mode {
                    elevate::musical::drums::OffsetMode::Fixed => "Offset: Fijo",
                    elevate::musical::drums::OffsetMode::EachNote => "Offset: Cada nota",
                    elevate::musical::drums::OffsetMode::Random => "Offset: Random",
                })
            }
            PieceTimingOffset { value, .. } => {
                let max = elevate::musical::drums::DrumPiece::MAX_TIMING_OFFSET;
                piece.timing_offset = value.clamp(-max, max);
                Ok("Offset de tiempo ajustado")
            }
            CyclePieceTimingMode { .. } => {
                piece.timing_mode = piece.timing_mode.next();
                Ok(match piece.timing_mode {
                    elevate::musical::drums::OffsetMode::Fixed => "Timing: Fijo",
                    elevate::musical::drums::OffsetMode::EachNote => "Timing: Cada nota",
                    elevate::musical::drums::OffsetMode::Random => "Timing: Random",
                })
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
    /// Picos de la muestra para dibujarla; decodifica fuera del hilo de UI la primera vez.
    pub(in crate::ui) fn drum_wave(
        &self,
        source: &str,
        cx: &mut Context<Self>,
    ) -> std::sync::Arc<Vec<[f32; 2]>> {
        let cache = self.pattern_editor.waveforms.clone();
        if let Some(entry) = cache.borrow().get(source) {
            return entry.clone().unwrap_or_default();
        }
        cache.borrow_mut().insert(source.to_owned(), None);
        let key = source.to_owned();
        let background = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let decode_key = key.clone();
            let peaks = background
                .spawn(async move {
                    let file = AudioFile::decode(std::path::Path::new(&decode_key)).ok()?;
                    let scale = file.max_peak.max(0.01);
                    let group = file.peaks.len().div_ceil(480).max(1);
                    Some(
                        file.peaks
                            .chunks(group)
                            .map(|chunk| {
                                let [lo, hi] = chunk
                                    .iter()
                                    .fold([0f32, 0f32], |p, a| [p[0].min(a[0]), p[1].max(a[1])]);
                                [lo / scale, hi / scale]
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(peaks) = peaks {
                    this.pattern_editor
                        .waveforms
                        .borrow_mut()
                        .insert(key, Some(std::sync::Arc::new(peaks)));
                }
                cx.notify();
            });
        })
        .detach();
        Default::default()
    }

    pub(in crate::ui) fn begin_drum_offset_drag(
        &mut self,
        piece: InstrumentId,
        layer: usize,
        value: u16,
        y: Pixels,
    ) {
        self.pattern_editor.offset_drag = Some(DrumOffsetDrag {
            piece,
            layer: Some(layer),
            y,
            original: value as i16,
            draft: f32::from(value),
        });
    }

    pub(in crate::ui) fn begin_drum_timing_drag(
        &mut self,
        piece: InstrumentId,
        value: i16,
        x: Pixels,
    ) {
        self.pattern_editor.offset_drag = Some(DrumOffsetDrag {
            piece,
            layer: None,
            y: x,
            original: value,
            draft: f32::from(value),
        });
    }

    pub(in crate::ui) fn update_drum_offset_drag(&mut self, event: &MouseMoveEvent) -> bool {
        let Some(drag) = &mut self.pattern_editor.offset_drag else {
            return false;
        };
        if !event.dragging() {
            self.finish_drum_offset_drag();
            return true;
        }
        if drag.layer.is_some() {
            let scale = if event.modifiers.shift { 0.5 } else { 5.0 };
            drag.draft = (drag.draft + f32::from(drag.y - event.position.y) * scale)
                .clamp(0.0, f32::from(DrumLayer::MAX_START_OFFSET));
            drag.y = event.position.y;
        } else {
            let max = f32::from(elevate::musical::drums::DrumPiece::MAX_TIMING_OFFSET);
            let scale = if event.modifiers.shift { 0.5 } else { 3.0 };
            drag.draft =
                (drag.draft + f32::from(event.position.x - drag.y) * scale).clamp(-max, max);
            drag.y = event.position.x;
            let piece = drag.piece;
            let live = drag.draft.round() as i16;
            self.apply_pattern_action(PatternAction::PieceTimingOffset { piece, value: live });
        }
        true
    }

    pub(in crate::ui) fn finish_drum_offset_drag(&mut self) -> bool {
        let Some(drag) = self.pattern_editor.offset_drag.take() else {
            return false;
        };
        let value = drag.draft.round() as i16;
        if value != drag.original {
            if let Some(layer) = drag.layer {
                self.apply_pattern_action(PatternAction::LayerOffset {
                    piece: drag.piece,
                    layer,
                    value: value.max(0) as u16,
                });
            } else {
                self.apply_pattern_action(PatternAction::PieceTimingOffset {
                    piece: drag.piece,
                    value,
                });
            }
        }
        true
    }

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
        if EntryKind::for_file(&path) == EntryKind::Hydrogen {
            self.import_hydrogen(path, None, cx);
            cx.notify();
            return;
        }
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

    pub(in crate::ui) fn drop_drum_files(
        &mut self,
        paths: &[PathBuf],
        piece: Option<InstrumentId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let [path] = paths {
            self.drop_drum_sample(path.clone(), piece, window, cx);
        } else {
            self.status_message = "Suelta una muestra o un proyecto de Hydrogen a la vez".into();
            cx.notify();
        }
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
                Ok(Some(path)) if EntryKind::for_file(&path) == EntryKind::Hydrogen => {
                    let _ = this.update(cx, |this, cx| {
                        this.pattern_editor.sample_task = None;
                        this.import_hydrogen(path, None, cx);
                        cx.notify();
                    });
                    return;
                }
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
