//! Estado por FxId y puente de captura → vistas; no conoce instancias VST3.
use crate::{
    audio::analyzer::{
        SpectrumAnalyzer,
        tap::{Samples, Tap},
    },
    ui::{CenterView, DawApp, components::spectrum::SpectrumMode},
};
use elevate::musical::fx::{FxChain, FxId, FxKind, FxNode};
use elevate::musical::track::TrackDevice;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

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
    pub taps: BTreeMap<FxId, Arc<Tap>>,
    pub views: BTreeMap<FxId, AnalyzerView>,
}
impl Analyzers {
    pub fn prepare(&mut self, chain: &FxChain) {
        for (node, _) in nodes(chain) {
            if self.taps.contains_key(&node.id) {
                continue;
            }
            let (tap, consumer) = Tap::new();
            self.taps.insert(node.id, tap);
            self.views.insert(
                node.id,
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
    pub fn reset(&mut self, id: FxId) {
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
        let active: Vec<_> = self
            .tracks
            .get(self.cursor_row)
            .map(|t| {
                nodes(&t.chain)
                    .into_iter()
                    .filter(|(_, enabled)| shown && *enabled)
                    .map(|(n, _)| n.id)
                    .collect()
            })
            .unwrap_or_default();
        self.fx
            .analyzers
            .taps
            .retain(|id, _| self.tracks.iter().any(|t| t.chain.find(*id).is_some()));
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
