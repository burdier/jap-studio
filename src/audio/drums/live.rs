//! El hilo de control publica notas y mezcla. Las voces nuevas se entregan a RT por cola.
use super::{
    controls::{DrumBlock, same_voice_config},
    kit::PreparedKit,
};
use crate::audio::{publication::Publication, sampler::SampleAsset};
use elevate::musical::{
    drums::DrumKit,
    model::{InstrumentId, Pattern},
    scheduler::PreparedPattern,
    time::TimeBase,
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
};

type Voices = (u64, PreparedKit);
type Work = Result<PreparedKit, String>;

pub(crate) struct LiveDrums {
    pub state: Arc<Publication<DrumBlock>>,
    time: TimeBase,
    rate: u32,
    kit: DrumKit,
    pattern: Option<Pattern>,
    revision: Arc<AtomicU64>,
    requested: u64,
    started: u64,
    assets: Arc<HashMap<String, Arc<SampleAsset>>>,
    pending: Option<(u64, Receiver<Work>)>,
    ready: Option<Voices>,
    incoming: Producer<Voices>,
    retired: Consumer<PreparedKit>,
}

pub(super) struct VoiceHandoff {
    wanted: Arc<AtomicU64>,
    incoming: Consumer<Voices>,
    retired: Producer<PreparedKit>,
}

impl VoiceHandoff {
    pub fn install(&mut self, active: &mut PreparedKit, block_revision: u64) {
        if self.retired.is_full() || self.wanted.load(Ordering::Acquire) != block_revision {
            return;
        }
        let Ok((revision, kit)) = self.incoming.pop() else {
            return;
        };
        let retired = if block_revision == revision {
            std::mem::replace(active, kit)
        } else {
            kit
        };
        // Un solo escritor y había espacio antes de sacar la voz entrante.
        assert!(
            self.retired.push(retired).is_ok(),
            "cola de retiro de batería"
        );
    }
}

impl LiveDrums {
    pub(super) fn prepare(
        kit: DrumKit,
        pattern: Option<Pattern>,
        asset_bank: Arc<HashMap<String, Arc<SampleAsset>>>,
        rate: u32,
        bpm: f64,
    ) -> Result<(Self, VoiceHandoff), String> {
        let time = TimeBase::new(rate, bpm).map_err(|error| error.to_string())?;
        let plan = compile(pattern.as_ref(), time)?;
        let state = Arc::new(Publication::new(DrumBlock::new(&kit, plan, None, 0)));
        let (incoming, next) = RingBuffer::new(1);
        let (garbage, retired) = RingBuffer::new(1);
        let revision = Arc::new(AtomicU64::new(0));
        let handoff = VoiceHandoff {
            wanted: revision.clone(),
            incoming: next,
            retired: garbage,
        };
        Ok((
            Self {
                state,
                time,
                rate,
                kit,
                pattern,
                revision,
                requested: 0,
                started: 0,
                assets: asset_bank,
                pending: None,
                ready: None,
                incoming,
                retired,
            },
            handoff,
        ))
    }

    pub fn edit(&mut self, kit: &DrumKit, pattern: Option<&Pattern>) -> Result<(), String> {
        let pattern_changed = self.pattern.as_ref() != pattern;
        if !pattern_changed && self.kit == *kit {
            return Ok(());
        }
        let plan = if pattern_changed {
            compile(pattern, self.time)?
        } else {
            self.state.read(|block| block.plan.clone())
        };
        if !same_voice_config(&self.kit, kit) {
            self.requested = self.requested.wrapping_add(1);
            self.revision.store(self.requested, Ordering::Release);
            self.ready = None;
        }
        let block = self
            .state
            .read(|previous| DrumBlock::new(kit, plan, Some(previous), self.requested));
        self.state.publish(block);
        self.kit = kit.clone();
        if pattern_changed {
            self.pattern = pattern.cloned();
        }
        self.start_worker();
        Ok(())
    }

    pub fn poll(&mut self) -> Result<(), String> {
        if let Ok(kit) = self.retired.pop() {
            drop(kit);
        }
        let mut error = None;
        if let Some((revision, pending)) = &self.pending {
            let result = match pending.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    Some(Err("Preparación de batería interrumpida".into()))
                }
            };
            if let Some(result) = result {
                let revision = *revision;
                self.pending = None;
                if revision == self.requested {
                    match result {
                        Ok(kit) => self.ready = Some((revision, kit)),
                        Err(message) => error = Some(message),
                    }
                }
            }
        }
        if !self.incoming.is_full()
            && let Some((revision, kit)) = self.ready.take()
        {
            self.assets = kit.assets.clone();
            assert!(
                self.incoming.push((revision, kit)).is_ok(),
                "cola de voces de batería"
            );
        }
        self.start_worker();
        error.map_or(Ok(()), Err)
    }

    fn start_worker(&mut self) {
        if self.pending.is_some() || self.started == self.requested {
            return;
        }
        self.started = self.requested;
        let kit = self.kit.clone();
        let rate = self.rate;
        let assets = self.assets.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.pending = Some((self.started, rx));
        std::thread::spawn(move || {
            let _ = tx.send(PreparedKit::prepare(&kit, &assets, rate));
        });
    }

    pub fn peak(&self, piece: Option<InstrumentId>) -> f32 {
        self.state.read(|block| match piece {
            Some(id) => block.piece(id).map_or(0.0, |piece| piece.peak.read()),
            None => block.bus_peak.read(),
        })
    }
}

fn compile(
    pattern: Option<&Pattern>,
    time: TimeBase,
) -> Result<Option<Arc<PreparedPattern>>, String> {
    pattern
        .map(|pattern| {
            PreparedPattern::compile(pattern, time)
                .map(Arc::new)
                .map_err(|error| error.to_string())
        })
        .transpose()
}
