//! Estado inmutable del bloque de percusión y picos observables desde el mixer.
use elevate::{
    StereoFrame,
    devices::sampler::SamplerParams,
    musical::{
        drums::{DrumKit, DrumMix},
        model::InstrumentId,
        scheduler::PreparedPattern,
        track::UNITY_LEVEL,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

#[derive(Default)]
pub(super) struct Peak(AtomicU32);

impl Peak {
    pub fn read(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    pub fn measure(&self, block: &[StereoFrame], decay: f32) {
        let peak = block
            .iter()
            .fold(0.0_f32, |peak, &[l, r]| peak.max(l.abs()).max(r.abs()));
        self.0
            .store(peak.max(self.read() * decay).to_bits(), Ordering::Relaxed);
    }
}

pub(super) struct PieceControl {
    id: InstrumentId,
    pub gain: f32,
    pub pan: f32,
    pub base_note: u8,
    pub params: SamplerParams,
    pub choke_group: Option<u16>,
    pub peak: Arc<Peak>,
}

pub(super) struct DrumPlan {
    pub pattern: PreparedPattern,
    pub repeat: bool,
    pub end: u64,
}

pub(crate) struct DrumBlock {
    pub(super) voice_revision: u64,
    pub(super) plan: Option<Arc<DrumPlan>>,
    pieces: Vec<PieceControl>,
    pub bus_solo: bool,
    pub(super) bus_gain: f32,
    pub(super) bus_pan: f32,
    pub(super) bus_peak: Arc<Peak>,
}

impl DrumBlock {
    pub(super) fn new(
        kit: &DrumKit,
        plan: Option<Arc<DrumPlan>>,
        previous: Option<&Self>,
        voice_revision: u64,
    ) -> Self {
        let solo = kit.pieces.iter().any(|piece| piece.mix.solo);
        let pieces = kit
            .pieces
            .iter()
            .map(|piece| PieceControl {
                id: piece.id,
                gain: gain(piece.mix, !solo || piece.mix.solo),
                pan: pan(piece.mix),
                base_note: piece.base_note(),
                params: piece.sampler.clone(),
                choke_group: piece.choke_group,
                peak: previous
                    .and_then(|block| block.piece(piece.id))
                    .map_or_else(Arc::default, |channel| channel.peak.clone()),
            })
            .collect();
        Self {
            voice_revision,
            plan,
            pieces,
            bus_solo: kit.bus.solo,
            bus_gain: gain(kit.bus, true),
            bus_pan: pan(kit.bus),
            bus_peak: previous.map_or_else(Arc::default, |block| block.bus_peak.clone()),
        }
    }

    pub fn loops(&self) -> bool {
        self.plan.as_ref().is_some_and(|plan| plan.repeat)
    }

    pub fn arrangement_end(&self) -> u64 {
        self.plan.as_ref().map_or(0, |plan| plan.end)
    }

    pub(super) fn piece(&self, id: InstrumentId) -> Option<&PieceControl> {
        self.pieces.iter().find(|piece| piece.id == id)
    }
}

fn gain(mix: DrumMix, solo_passes: bool) -> f32 {
    if mix.muted || !solo_passes {
        0.0
    } else {
        mix.fader.level as f32 / UNITY_LEVEL as f32
    }
}

fn pan(mix: DrumMix) -> f32 {
    (mix.fader.pan as f32 - 50.0) / 50.0
}

/// El sampler nativo actualiza parámetros, filtro y modo en vivo; solo recarga al cambiar capas.
pub(super) fn same_voice_config(a: &DrumKit, b: &DrumKit) -> bool {
    a.id == b.id
        && a.pieces.len() == b.pieces.len()
        && a.pieces
            .iter()
            .zip(&b.pieces)
            .all(|(a, b)| a.id == b.id && a.layers == b.layers)
}
