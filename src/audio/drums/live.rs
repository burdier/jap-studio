//! El hilo de control publica notas y mezcla. Las voces nuevas se entregan a RT por cola.
use super::{
    controls::{DrumBlock, DrumPlan, same_voice_config},
    kit::PreparedKit,
    mix::DrumScore,
};
use crate::audio::{publication::Publication, sampler::SampleAsset};
use elevate::musical::{
    drums::DrumKit, model::InstrumentId, scheduler::PreparedPattern, time::TimeBase,
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
    score: Option<DrumScore>,
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
        score: Option<DrumScore>,
        asset_bank: Arc<HashMap<String, Arc<SampleAsset>>>,
        rate: u32,
        bpm: f64,
    ) -> Result<(Self, VoiceHandoff), String> {
        let time = TimeBase::new(rate, bpm).map_err(|error| error.to_string())?;
        let plan = compile(score.as_ref(), time)?;
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
                score,
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

    pub fn edit(&mut self, kit: &DrumKit, score: Option<&DrumScore>) -> Result<(), String> {
        if self.score.as_ref() != score {
            let plan = compile(score, self.time)?;
            self.score = score.cloned();
            self.publish(kit, plan);
        } else {
            self.edit_kit(kit);
        }
        Ok(())
    }

    pub fn edit_kit(&mut self, kit: &DrumKit) {
        if self.kit != *kit {
            let plan = self.state.read(|block| block.plan.clone());
            self.publish(kit, plan);
        }
    }

    fn publish(&mut self, kit: &DrumKit, plan: Option<Arc<DrumPlan>>) {
        if !same_voice_config(&self.kit, kit) {
            self.requested = self.requested.wrapping_add(1);
            self.revision.store(self.requested, Ordering::Release);
            self.ready = None;
        }
        let block = self
            .state
            .read(|previous| DrumBlock::new(kit, plan, Some(previous), self.requested));
        self.state.publish(block);
        if self.kit != *kit {
            self.kit = kit.clone();
        }
        self.start_worker();
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

fn compile(score: Option<&DrumScore>, time: TimeBase) -> Result<Option<Arc<DrumPlan>>, String> {
    score
        .map(|score| {
            let pattern = PreparedPattern::compile(&score.pattern, time)
                .map_err(|error| error.to_string())?;
            // El Arrangement termina después de la última ventana visible o la cola
            // acotada del último golpe. El loop conserva la longitud del patrón.
            let mut end = pattern.length_frames().saturating_add(1);
            if !score.repeat
                && let Some((last, _)) = pattern.all_events().last()
            {
                let tail = u64::from(time.sample_rate()) * 2;
                end = end.max(last.saturating_add(tail));
            }
            Ok(Arc::new(DrumPlan {
                pattern,
                repeat: score.repeat,
                end,
            }))
        })
        .transpose()
}
