//! Controlador de modulación LFO (rutas sin bloqueos hacia Play, Export y Loro).
use super::DawApp;
use elevate::musical::{
    fx::FxId,
    modulation::{LfoDivision, LfoPolarity, LfoRate, LfoShape, ModSourceId},
    track::TrackId,
    track_automation::{AutomationParam, TrackAutomation},
};

impl DawApp {
    /// Compila el `TrackAutomation` efectivo de `track_id` incluyendo sus rutas LFO activas.
    pub(in crate::ui) fn compiled_track_automation(&self, track_id: TrackId) -> TrackAutomation {
        let mut auto = self.timeline.automation.track_or_default(track_id);
        auto.modulation = self.timeline.modulation.compile_for_track(track_id);
        auto
    }

    pub(in crate::ui) fn track_lfo_count(&self, track_id: TrackId) -> usize {
        self.timeline.modulation.track_route_count(track_id)
    }

    pub(in crate::ui) fn publish_all_modulation(&mut self) {
        self.publish_all_automation();
    }

    /// Comando `:lfo.add [target] [rate] [depth] [shape]`
    /// Ejemplos: `:lfo.add pan 1/4 0.6 sine`, `:lfo.add vol 1/8 0.35`, `:lfo.add fx:0 1bar 0.5 tri`
    pub(in crate::ui) fn run_lfo_add(&mut self, args: &str) {
        let Some(track) = self.tracks.get(self.cursor_row) else {
            self.status_message = "No hay pista seleccionada para asignar LFO".into();
            return;
        };
        let track_id = track.id;
        let track_name = track.name.clone();
        let parts: Vec<&str> = args.split_whitespace().collect();
        let target_token = parts.first().copied().unwrap_or("pan");
        let Some(param) = self.resolve_lfo_target(self.cursor_row, target_token) else {
            self.status_message =
                "Destino LFO inválido · usa: vol | pan | fx:<param> | fx:<fx_id>:<param>".into();
            return;
        };
        let rate = parts
            .get(1)
            .and_then(|s| LfoRate::parse(s))
            .unwrap_or(LfoRate::Sync(LfoDivision::Quarter));
        let depth = parts
            .get(2)
            .and_then(|s| s.parse::<f32>().ok())
            .map(|v| {
                if v.abs() > 1.0 && v.abs() <= 100.0 {
                    v / 100.0
                } else {
                    v
                }
            })
            .unwrap_or(0.5)
            .clamp(-1.0, 1.0);
        let shape = parts
            .get(3)
            .and_then(|s| LfoShape::parse(s))
            .unwrap_or(LfoShape::Sine);

        let lfo_id = match self.timeline.modulation.add_lfo(None, shape, rate) {
            Ok(id) => id,
            Err(err) => {
                self.status_message = err;
                return;
            }
        };
        if let Err(err) = self
            .timeline
            .modulation
            .add_route(lfo_id, track_id, param, depth)
        {
            self.timeline.modulation.remove_lfo(lfo_id);
            self.status_message = err;
            return;
        }
        self.publish_all_modulation();
        self.status_message = format!(
            "〜 LFO #{} ({} · {}) → «{track_name}» {} (profundidad {:+.0}%)",
            lfo_id.0,
            shape.label(),
            rate.label(),
            param.short_label(),
            depth * 100.0
        );
    }

    pub(in crate::ui) fn run_lfo_rate(&mut self, args: &str) {
        let parts: Vec<&str> = args.split_whitespace().collect();
        let (lfo_id, rate_token) = match parts.as_slice() {
            [rate] => match self.timeline.modulation.lfos.last() {
                Some(lfo) => (lfo.id, *rate),
                None => {
                    self.status_message = "No hay LFOs creados (usa :lfo.add)".into();
                    return;
                }
            },
            [id_str, rate, ..] => match id_str.parse::<u32>() {
                Ok(id) => (ModSourceId(id), *rate),
                Err(_) => {
                    self.status_message = "Uso: :lfo.rate [id] <1/4 | 1/8 | 1bar | 2hz>".into();
                    return;
                }
            },
            [] => {
                self.status_message = "Uso: :lfo.rate [id] <1/4 | 1/8 | 1bar | 2hz>".into();
                return;
            }
        };
        let Some(rate) = LfoRate::parse(rate_token) else {
            self.status_message = format!("Velocidad LFO no reconocida: {rate_token}");
            return;
        };
        let Some(lfo) = self.timeline.modulation.lfo_mut(lfo_id) else {
            self.status_message = format!("LFO #{} no encontrado", lfo_id.0);
            return;
        };
        lfo.rate = rate;
        self.publish_all_modulation();
        self.status_message = format!("〜 LFO #{} velocidad → {}", lfo_id.0, rate.label());
    }

    pub(in crate::ui) fn run_lfo_shape(&mut self, args: &str) {
        let parts: Vec<&str> = args.split_whitespace().collect();
        let (lfo_id, shape_token) = match parts.as_slice() {
            [shape] => match self.timeline.modulation.lfos.last() {
                Some(lfo) => (lfo.id, *shape),
                None => {
                    self.status_message = "No hay LFOs creados (usa :lfo.add)".into();
                    return;
                }
            },
            [id_str, shape, ..] => match id_str.parse::<u32>() {
                Ok(id) => (ModSourceId(id), *shape),
                Err(_) => {
                    self.status_message =
                        "Uso: :lfo.shape [id] <sine | tri | saw | saw_down | square | sh>".into();
                    return;
                }
            },
            [] => {
                self.status_message =
                    "Uso: :lfo.shape [id] <sine | tri | saw | saw_down | square | sh>".into();
                return;
            }
        };
        let Some(shape) = LfoShape::parse(shape_token) else {
            self.status_message = format!("Forma LFO no reconocida: {shape_token}");
            return;
        };
        let Some(lfo) = self.timeline.modulation.lfo_mut(lfo_id) else {
            self.status_message = format!("LFO #{} no encontrado", lfo_id.0);
            return;
        };
        lfo.shape = shape;
        self.publish_all_modulation();
        self.status_message = format!("〜 LFO #{} forma → {}", lfo_id.0, shape.label());
    }

    pub(in crate::ui) fn run_lfo_depth(&mut self, args: &str) {
        let parts: Vec<&str> = args.split_whitespace().collect();
        let (route_id, depth_str) = match parts.as_slice() {
            [depth] => match self.timeline.modulation.routes.last() {
                Some(route) => (route.id, *depth),
                None => {
                    self.status_message = "No hay rutas LFO activas".into();
                    return;
                }
            },
            [id_str, depth, ..] => match id_str.parse::<u32>() {
                Ok(id) => (id, *depth),
                Err(_) => {
                    self.status_message =
                        "Uso: :lfo.depth [id_ruta] <-1.0..1.0 o -100..100>".into();
                    return;
                }
            },
            [] => {
                self.status_message = "Uso: :lfo.depth [id_ruta] <-1.0..1.0 o -100..100>".into();
                return;
            }
        };
        let Ok(raw) = depth_str.parse::<f32>() else {
            self.status_message = format!("Profundidad LFO inválida: {depth_str}");
            return;
        };
        let depth = if raw.abs() > 1.0 && raw.abs() <= 100.0 {
            raw / 100.0
        } else {
            raw
        }
        .clamp(-1.0, 1.0);
        let Some(route) = self
            .timeline
            .modulation
            .routes
            .iter_mut()
            .find(|r| r.id == route_id)
        else {
            self.status_message = format!("Ruta LFO #{route_id} no encontrada");
            return;
        };
        route.depth = depth;
        self.publish_all_modulation();
        self.status_message = format!(
            "〜 Ruta LFO #{route_id} profundidad → {:+.0}%",
            depth * 100.0
        );
    }

    pub(in crate::ui) fn run_lfo_polarity(&mut self, args: &str) {
        let parts: Vec<&str> = args.split_whitespace().collect();
        let (lfo_id, pol_token) = match parts.as_slice() {
            [pol] => match self.timeline.modulation.lfos.last() {
                Some(lfo) => (lfo.id, *pol),
                None => {
                    self.status_message = "No hay LFOs creados".into();
                    return;
                }
            },
            [id_str, pol, ..] => match id_str.parse::<u32>() {
                Ok(id) => (ModSourceId(id), *pol),
                Err(_) => {
                    self.status_message = "Uso: :lfo.polarity [id] <bipolar | unipolar>".into();
                    return;
                }
            },
            [] => {
                self.status_message = "Uso: :lfo.polarity [id] <bipolar | unipolar>".into();
                return;
            }
        };
        let Some(pol) = LfoPolarity::parse(pol_token) else {
            self.status_message = format!("Polaridad inválida: {pol_token}");
            return;
        };
        let Some(lfo) = self.timeline.modulation.lfo_mut(lfo_id) else {
            self.status_message = format!("LFO #{} no encontrado", lfo_id.0);
            return;
        };
        lfo.polarity = pol;
        self.publish_all_modulation();
        self.status_message = format!("〜 LFO #{} polaridad → {}", lfo_id.0, pol.key());
    }

    pub(in crate::ui) fn run_lfo_remove(&mut self, args: &str) {
        let trimmed = args.trim();
        if trimmed.is_empty() {
            if let Some(track) = self.tracks.get(self.cursor_row) {
                let id = track.id;
                let before = self.timeline.modulation.routes.len();
                self.timeline.modulation.remove_track(id);
                let removed = before.saturating_sub(self.timeline.modulation.routes.len());
                self.publish_all_modulation();
                self.status_message = format!("〜 {removed} ruta(s) LFO eliminadas de la pista");
            }
            return;
        }
        let Ok(id) = trimmed.parse::<u32>() else {
            self.status_message =
                "Uso: :lfo.remove [id_lfo] (sin id limpia la pista actual)".into();
            return;
        };
        if self.timeline.modulation.remove_lfo(ModSourceId(id)) {
            self.publish_all_modulation();
            self.status_message = format!("〜 LFO #{id} y sus rutas eliminados");
        } else {
            self.status_message = format!("LFO #{id} no encontrado");
        }
    }

    pub(in crate::ui) fn run_lfo_list(&mut self) {
        if self.timeline.modulation.routes.is_empty() {
            self.status_message =
                "Sin rutas LFO activas · ejemplo: :lfo.add pan 1/4 0.6 sine".into();
            return;
        }
        let summary: Vec<String> = self
            .timeline
            .modulation
            .routes
            .iter()
            .take(4)
            .filter_map(|r| {
                let lfo = self.timeline.modulation.lfo(r.source)?;
                let track_name: &str = self
                    .tracks
                    .iter()
                    .find(|t| t.id == r.track)
                    .map(|t| t.name.as_ref())
                    .unwrap_or("Pista");
                Some(format!(
                    "#{} {} ({} {}) → {} {} {:+.0}%",
                    lfo.id.0,
                    lfo.name,
                    lfo.shape.label(),
                    lfo.rate.label(),
                    track_name,
                    r.param.short_label(),
                    r.depth * 100.0
                ))
            })
            .collect();
        self.status_message = format!("〜 LFO: {}", summary.join(" · "));
    }

    fn resolve_lfo_target(&self, track_index: usize, token: &str) -> Option<AutomationParam> {
        let lower = token.trim().to_ascii_lowercase();
        match lower.as_str() {
            "vol" | "volume" | "volumen" => return Some(AutomationParam::Volume),
            "pan" | "paneo" => return Some(AutomationParam::Pan),
            _ => {}
        }
        if let Some(rest) = lower.strip_prefix("fx:") {
            let track = self.tracks.get(track_index)?;
            if let Some((raw_fx, raw_param)) = rest.split_once(':') {
                let fx = FxId(raw_fx.parse::<u64>().ok()?);
                let param = raw_param.parse::<u32>().ok()?;
                return Some(AutomationParam::FxParam { fx, param });
            }
            let param = rest.parse::<u32>().ok()?;
            // Apunta al primer efecto del rack de la pista si no se especificó FxId.
            let fx = track
                .chain
                .nodes()
                .iter()
                .find(|n| matches!(n.kind, elevate::musical::fx::FxKind::Effect { .. }))
                .map(|n| n.id)?;
            return Some(AutomationParam::FxParam { fx, param });
        }
        if let Some(rest) = lower.strip_prefix("inst:") {
            return rest
                .parse::<u32>()
                .ok()
                .map(AutomationParam::InstrumentParam);
        }
        None
    }
}
