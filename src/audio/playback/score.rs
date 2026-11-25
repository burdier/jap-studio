//! Contenido musical inmutable. Las voces, los FX y el reloj permanecen en el stream.
use super::{ClipSource, TrackInput, mix::MidiTrackInput};
use crate::audio::export::resample;
use elevate::{
    StereoFrame,
    musical::{
        arrangement_midi::{NoteChange, PreparedMidiArrangement},
        track::TrackId,
    },
};
use std::{
    collections::HashMap,
    sync::{Arc, mpsc},
};

pub(super) struct Lane {
    pub sources: Vec<(ClipSource, f32)>,
    pub midi: Option<PreparedMidiArrangement>,
    pub changes: Vec<NoteChange>,
}
pub(super) struct Score {
    pub lanes: HashMap<TrackId, Lane>,
    converted: HashMap<usize, Arc<Vec<StereoFrame>>>,
    pub end: u64,
    pub latency: u64,
}

impl MidiTrackInput {
    fn compile(&self, rate: u32, bpm: f64) -> Result<PreparedMidiArrangement, String> {
        let mut plan = PreparedMidiArrangement::compile_track(
            &self.clips,
            &self.track,
            self.index,
            rate,
            bpm,
        )?;
        if let Some(h) = &self.hilo {
            plan.append(PreparedMidiArrangement::compile_hilo_branch(
                &h.clips,
                h.group_index,
                &self.track,
                h.branch_index,
                &h.instruments,
                h.mode,
                h.weave.as_ref(),
                rate,
                bpm,
            )?)?;
        }
        Ok(plan)
    }
}

impl Score {
    pub fn prepare(
        inputs: &[TrackInput],
        rate: u32,
        bpm: f64,
        previous: Option<&Score>,
    ) -> Result<Self, String> {
        if inputs.len() > super::MAX_TRACKS
            || inputs.iter().map(|t| t.clips.len()).sum::<usize>() > 4096
        {
            return Err("La mezcla admite hasta 256 pistas y 4096 clips".into());
        }
        let mut result = Self {
            lanes: HashMap::new(),
            converted: HashMap::new(),
            end: 0,
            latency: 0,
        };
        let parents = super::mix::track_parents(inputs)?;
        let latencies: Vec<_> = inputs.iter().map(TrackInput::rack_latency).collect();
        let (longest, _, _) = super::mix::compensation(&latencies, &parents);
        result.latency = longest;
        for (input, parent) in inputs.iter().zip(parents) {
            let mut lane_end = 0u64;
            let mut sources = Vec::with_capacity(input.clips.len());
            let mut fades = crate::audio::crossfade::prepare(input.clips.iter(), rate, bpm);
            for clip in input.clips.iter().filter(|c| !c.props.muted) {
                let converted = if rate == clip.file.sample_rate {
                    None
                } else {
                    let key = Arc::as_ptr(&clip.file) as usize;
                    let samples = match result.converted.entry(key) {
                        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            let frames = match previous.and_then(|p| p.converted.get(&key)) {
                                Some(frames) => frames.clone(),
                                None => Arc::new(resample(
                                    &clip.file.frames,
                                    clip.file.sample_rate,
                                    rate,
                                )?),
                            };
                            entry.insert(frames)
                        }
                    };
                    Some(samples.clone())
                };
                let mut source = ClipSource::new(clip, converted, rate, bpm / 60.);
                source.fades = fades.remove(&clip.id).unwrap_or_default();
                lane_end = lane_end.max(source.end_frame());
                sources.push((source, clip.props.gain_linear()));
            }
            let midi = input
                .midi
                .as_ref()
                .map(|m| m.compile(rate, bpm))
                .transpose()?;
            if let Some(plan) = &midi
                && plan.end_frame() > 0
            {
                let tail = match &input.midi.as_ref().unwrap().source {
                    crate::audio::piano::VoiceSource::Vst(vst) => vst.tail_frames,
                    _ => 0,
                };
                lane_end = lane_end.max(plan.end_frame().saturating_add(tail));
            }
            if lane_end > 0 {
                // También hay que vaciar las últimas muestras que aún atraviesan el rack.
                let drain = longest
                    .saturating_add(input.rack.as_ref().map_or(0, |rack| rack.tail_frames()))
                    .saturating_add(
                        parent
                            .and_then(|p| inputs[p].rack.as_ref())
                            .map_or(0, |rack| rack.tail_frames()),
                    );
                result.end = result.end.max(lane_end.saturating_add(drain));
            }
            let changes = previous
                .and_then(|p| p.lanes.get(&input.id))
                .map_or_else(Vec::new, |old| {
                    PreparedMidiArrangement::note_changes(old.midi.as_ref(), midi.as_ref())
                });
            result.lanes.insert(
                input.id,
                Lane {
                    sources,
                    midi,
                    changes,
                },
            );
        }
        Ok(result)
    }
}

type ResultScore = Result<Arc<Score>, String>;
pub(super) struct Edits {
    current: Arc<Score>,
    send: rtrb::Producer<Arc<Score>>,
    retired: rtrb::Consumer<Arc<Score>>,
    task: Option<mpsc::Receiver<ResultScore>>,
    queued: Option<Vec<TrackInput>>,
}
pub(super) struct AudioEdits {
    next: rtrb::Consumer<Arc<Score>>,
    retired: rtrb::Producer<Arc<Score>>,
}
impl AudioEdits {
    pub fn apply(&mut self, mix: &mut super::Mix, at: u64) {
        if self.retired.is_full() {
            return;
        }
        if let Ok(next) = self.next.pop() {
            let old = mix.replace_score(next, at);
            // Único productor. El consumidor solo puede liberar más espacio.
            let _ = self.retired.push(old);
        }
    }
}
impl Edits {
    pub fn new(current: Arc<Score>) -> (Self, AudioEdits) {
        let (send, next) = rtrb::RingBuffer::new(1);
        let (retire, retired) = rtrb::RingBuffer::new(1);
        (
            Self {
                current,
                send,
                retired,
                task: None,
                queued: None,
            },
            AudioEdits {
                next,
                retired: retire,
            },
        )
    }
    pub fn queue(&mut self, inputs: Vec<TrackInput>) {
        self.queued = Some(inputs);
    }
    pub fn poll(&mut self, rate: u32, bpm: f64) -> Result<(), String> {
        while self.retired.pop().is_ok() {}
        if self.send.is_full() {
            return Ok(());
        }
        if let Some(task) = &self.task {
            let result = match task.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return Ok(()),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err("Se interrumpió la preparación del arreglo".into())
                }
            };
            self.task = None;
            if self.queued.is_none() {
                let score = result?;
                self.send
                    .push(score.clone())
                    .map_err(|_| "Cola de arreglos llena")?;
                self.current = score;
            }
        }
        if let Some(mut inputs) = self.queued.take() {
            let previous = self.current.clone();
            let (tx, rx) = mpsc::sync_channel(1);
            self.task = Some(rx);
            std::thread::spawn(move || {
                let result = (|| {
                    crate::audio::playback::apply_preplay(&mut inputs)?;
                    Score::prepare(&inputs, rate, bpm, Some(&previous)).map(Arc::new)
                })();
                let _ = tx.send(result);
            });
        }
        Ok(())
    }
}
