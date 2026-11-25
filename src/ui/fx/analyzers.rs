//! Estado por FxId y puente de captura → vistas; no conoce instancias VST3.
use crate::{
    audio::analyzer::{
        SpectrumAnalyzer,
        tap::{Samples, Tap, TapPoint},
    },
    ui::{CenterView, DawApp, components::spectrum::SpectrumMode},
};
use TapPoint::{Input, Output};
use elevate::musical::fx::{FxChain, FxId, FxKind, FxNode};
use elevate::musical::track::TrackDevice;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

/// Altura mínima de cabecera TCP para incrustar Hay Spectrum en la pista.
pub(in crate::ui) const EMBEDDED_SPECTRUM_MIN_HEIGHT: f32 = 96.0;

pub(in crate::ui) fn is_analyzer(node: &FxNode) -> bool {
    matches!(&node.kind, FxKind::Effect { plugin_id: TrackDevice::Hay { catalog_id }, .. } if catalog_id == elevate::devices::spectrum::ID)
}
/// Orden de señal en el árbol; incluye bypass heredado sin confundirlo con selección.
pub(in crate::ui) fn nodes(chain: &FxChain) -> Vec<(&FxNode, bool)> {
    fn visit<'a>(node: &'a FxNode, enabled: bool, out: &mut Vec<(&'a FxNode, bool)>) {
        let active = enabled && node.enabled;
        if is_analyzer(node) {
            out.push((node, active));
        }
        if let Some(children) = node.children() {
            for child in children {
                visit(child, active, out);
            }
        }
    }
    let mut out = Vec::new();
    for node in chain.nodes() {
        visit(node, true, &mut out);
    }
    out
}

pub(in crate::ui) struct AnalyzerView {
    consumer: rtrb::Consumer<Samples>,
    pub analyzer: SpectrumAnalyzer,
    pub mode: SpectrumMode,
    rate: u32,
    visible: bool,
    last_received: Option<Instant>,
    dropped: u64,
}
impl AnalyzerView {
    pub fn signal_status(&self) -> &'static str {
        match self.last_received {
            Some(time) if time.elapsed() <= Duration::from_millis(250) => "En vivo",
            Some(_) => "Sin señal · historial detenido",
            None => "Esperando señal",
        }
    }
}
#[derive(Default)]
pub(in crate::ui) struct Analyzers {
    pub taps: BTreeMap<TapPoint, Arc<Tap>>,
    pub views: BTreeMap<TapPoint, AnalyzerView>,
}
impl Analyzers {
    pub fn prepare(
        &mut self,
        chain: &FxChain,
        components: &BTreeMap<FxId, Arc<crate::audio::effects::ComponentEffect>>,
    ) {
        let devices = nodes(chain).into_iter().map(|(n, _)| Output(n.id));
        let bands = chain
            .bands()
            .into_iter()
            .flat_map(|b| [Input(b.input_crossover), Output(b.chain)]);
        let filters = components
            .iter()
            .filter_map(|(&id, component)| {
                (component.descriptor().response_node.is_some() && chain.find(id).is_some())
                    .then_some([Input(id), Output(id)])
            })
            .flatten();
        for id in devices.chain(bands).chain(filters) {
            if self.taps.contains_key(&id) {
                continue;
            }
            let (tap, consumer) = Tap::new();
            self.taps.insert(id, tap);
            self.views.insert(
                id,
                AnalyzerView {
                    consumer,
                    analyzer: SpectrumAnalyzer::new(48000),
                    mode: SpectrumMode::Spectrum,
                    rate: 48000,
                    visible: false,
                    last_received: None,
                    dropped: 0,
                },
            );
        }
    }
    pub fn visible(&self) -> bool {
        self.views.values().any(|v| v.visible)
    }

    pub fn comparison(
        &self,
        input: FxId,
        output: FxId,
        ds: &crate::ui::DesignSystem,
    ) -> Vec<crate::ui::components::curve_overlay::CurvePath> {
        [
            self.spectrum_curve(Input(input), ds.muted),
            self.spectrum_curve(Output(output), ds.cyan),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    fn spectrum_curve(
        &self,
        id: TapPoint,
        color: u32,
    ) -> Option<crate::ui::components::curve_overlay::CurvePath> {
        let view = self.views.get(&id)?;
        let levels = view.analyzer.bands();
        if !view.visible || !levels.iter().any(|&level| level > 0.005) {
            return None;
        }
        // Los bins son logarítmicos hasta Nyquist; ambos editores llegan a 20 kHz.
        let span = (view.analyzer.upper_hz() / 20.).log10() / 3.;
        let points = levels
            .iter()
            .enumerate()
            .map(|(i, &level)| ((i as f32 + 0.5) * span / levels.len() as f32, level))
            .collect();
        Some(crate::ui::components::curve_overlay::CurvePath {
            points,
            color,
            stroke_px: if matches!(id, Input(_)) { 2. } else { 1. },
            fill_alpha: if matches!(id, Input(_)) { 0 } else { 58 },
        })
    }

    pub fn reset(&mut self, id: FxId) {
        let id = Output(id);
        if let Some(tap) = self.taps.get(&id) {
            tap.invalidate();
        }
        if let Some(view) = self.views.get_mut(&id) {
            view.analyzer.clear();
            view.last_received = None;
        }
    }
    pub fn changed(&mut self, chain: &FxChain) {
        for (node, _) in nodes(chain) {
            self.reset(node.id);
        }
    }
}
impl DawApp {
    pub(in crate::ui) fn poll_fx_analyzers(&mut self) {
        let shown = self.fx.width > 0.
            && self.center_view == CenterView::Arrangement
            && matches!(
                self.workspace_focus.expanded,
                None | Some(crate::ui::workspace_focus::Panel::Fx)
            );
        let mut active: Vec<_> = self
            .active_fx_track_id()
            .and_then(|id| self.fx_chain(id))
            .map(|chain| {
                nodes(chain)
                    .into_iter()
                    .filter(|(_, enabled)| shown && *enabled)
                    .map(|(n, _)| Output(n.id))
                    .collect()
            })
            .unwrap_or_default();
        let mixer_visible = self.center_view == CenterView::Mixer
            || (self.center_view == CenterView::Arrangement
                && self.piano_roll_window.is_some()
                && self.piano_roll.open
                && self.timeline.midi_clip.is_some());
        if mixer_visible
            || (self.center_view == CenterView::Arrangement
                && matches!(
                    self.workspace_focus.expanded,
                    None | Some(crate::ui::workspace_focus::Panel::Arrangement)
                ))
        {
            for (index, track) in self.tracks.iter().enumerate() {
                if (mixer_visible || self.track_height(index) >= EMBEDDED_SPECTRUM_MIN_HEIGHT)
                    && let Some((node, _)) = nodes(&track.chain)
                        .into_iter()
                        .find(|(_, enabled)| *enabled)
                {
                    let tap_id = Output(node.id);
                    if !active.contains(&tap_id) {
                        active.push(tap_id);
                    }
                }
            }
        }
        if shown && let Some(id) = self.selected_lua_component().and_then(|t| t.node) {
            active.extend([Input(id), Output(id)]);
        }
        if self.center_view == CenterView::Arrangement
            && self.workspace_focus.expanded.is_none()
            && let Some(target) = self.fx.tejer
            && let Some(track) = self
                .tracks
                .get(self.cursor_row)
                .filter(|t| t.id == target.track)
        {
            let bands = track.chain.bands();
            if let Some(band) = bands
                .iter()
                .find(|b| Some(b.chain) == target.node)
                .or(bands.first())
            {
                active.extend([Input(band.input_crossover), Output(band.chain)]);
            }
        }
        self.fx.analyzers.taps.retain(|id, _| {
            self.master_chain.find(id.node()).is_some()
                || self
                    .tracks
                    .iter()
                    .any(|t| t.chain.find(id.node()).is_some())
        });
        self.fx
            .analyzers
            .views
            .retain(|id, _| self.fx.analyzers.taps.contains_key(id));
        for (id, view) in &mut self.fx.analyzers.views {
            let tap = &self.fx.analyzers.taps[id];
            let visible = active.contains(id);
            if visible != view.visible {
                tap.invalidate();
                view.analyzer.clear();
                view.last_received = None;
                view.visible = visible;
            }
            let dropped = tap.dropped.load(std::sync::atomic::Ordering::Relaxed);
            if dropped != view.dropped {
                // No representar bloques discontinuos como una historia temporal continua.
                view.dropped = dropped;
                tap.invalidate();
                view.analyzer.clear();
                view.last_received = None;
            }
            tap.activate(visible);
            for _ in 0..64 {
                let Ok(samples) = view.consumer.pop() else {
                    break;
                };
                if !visible || samples.epoch != tap.epoch() {
                    continue;
                }
                if samples.rate != view.rate {
                    view.rate = samples.rate;
                    view.analyzer = SpectrumAnalyzer::new(samples.rate);
                }
                for frame in &samples.frames[..samples.len] {
                    view.analyzer.push_stereo(*frame);
                }
                view.last_received = Some(Instant::now());
            }
            if view
                .last_received
                .is_some_and(|time| time.elapsed() > Duration::from_millis(250))
            {
                view.analyzer.silence();
            }
        }
    }
}
