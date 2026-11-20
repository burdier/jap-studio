//! Ajustes de la pieza activa; el sampler de pistas conserva su propio estado.
use super::{PatternAction, PatternEditor};
use crate::ui::DawApp;
use elevate::{
    devices::sampler::PARAMS,
    musical::{drums::DrumLayer, model::InstrumentId},
};
use gpui::{Context, PathPromptOptions};

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
            | RemoveSample { piece, .. }
            | LayerVelocity { piece, .. }
            | Filter(piece) => *piece,
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
        piece: InstrumentId,
        cx: &mut Context<Self>,
    ) {
        if self.pattern_editor.sample_picker.is_some() {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Añadir muestra a la pieza".into()),
        });
        self.pattern_editor.sample_picker = Some(cx.spawn(async move |this, cx| {
            let result = prompt.await;
            let _ = this.update(cx, |this, cx| {
                this.pattern_editor.sample_picker = None;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            if let Some(path) = path.to_str() {
                                this.apply_pattern_action(PatternAction::AddSample {
                                    piece,
                                    path: path.to_owned(),
                                });
                            } else {
                                this.status_message = "La ruta de la muestra debe ser UTF-8".into();
                            }
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => this.status_message = "No se pudo abrir el selector de muestras".into(),
                }
                cx.notify();
            });
        }));
    }
}
