//! Estado inmutable del bloque de percusión y picos observables desde el mixer.
use elevate::{
    StereoFrame,
    devices::sampler::{PARAMS, SamplerParams, live_control},
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

pub(crate) struct DrumBlock {
    pub(super) voice_revision: u64,
    pub(super) plan: Option<Arc<PreparedPattern>>,
    pieces: Vec<PieceControl>,
    pub(super) bus_gain: f32,
    pub(super) bus_pan: f32,
    pub(super) bus_peak: Arc<Peak>,
}

impl DrumBlock {
    pub(super) fn new(
        kit: &DrumKit,
        plan: Option<Arc<PreparedPattern>>,
        previous: Option<&Self>,
        voice_revision: u64,
    ) -> Self {
        let solo = kit.pieces.iter().any(|piece| piece.mix.solo);
        let pieces = kit
            .pieces
            .iter()
            .map(|piece| PieceControl {
                id: piece.id,
                gain: gain(piece.mix, !solo || piece.mix.solo || kit.bus.solo),
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
            bus_gain: gain(kit.bus, true),
            bus_pan: pan(kit.bus),
            bus_peak: previous.map_or_else(Arc::default, |block| block.bus_peak.clone()),
        }
    }

    pub fn loops(&self) -> bool {
        self.plan.is_some()
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

/// Las diferencias en CC, corte, mezcla o nombre no requieren volver a cargar un SFZ.
pub(super) fn same_voice_config(a: &DrumKit, b: &DrumKit) -> bool {
    a.id == b.id
        && a.pieces.len() == b.pieces.len()
        && a.pieces.iter().zip(&b.pieces).all(|(a, b)| {
            if a.id != b.id || a.layers != b.layers {
                return false;
            }
            let mut params = a.sampler.clone();
            for id in 0..PARAMS.len() {
                if live_control(&params, id).is_some() {
                    params.set(id, b.sampler.get(id));
                }
            }
            params == b.sampler
        })
}
