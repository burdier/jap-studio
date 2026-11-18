//! Preparación de mezcla cancelable; fuentes y remuestreo fuera del callback.
mod lua;
use super::DawApp;
use crate::audio::drums::DrumInput;
use crate::audio::piano::VoiceSource;
use crate::audio::{AudioClip, HiloBranchInput, MidiTrackInput, PreparedPlayback, TrackInput};
use elevate::musical::clip::MidiClip;
use elevate::musical::hilo::HiloPlan;
use std::sync::mpsc::{self, Receiver, TryRecvError};
pub(super) struct PendingPlayback {
    result: Receiver<Result<PreparedPlayback, String>>,
    beat: f64,
    bpm: f64,
    meter: elevate::musical::time::Meter,
    clips: Vec<AudioClip>,
    midi_clips: Vec<MidiClip>,
    pattern: Option<elevate::musical::model::Pattern>,
    revision: u64,
    audio: crate::audio::device::AudioConfig,
    lua_preplay: bool,
}
impl DawApp {
    fn audio_snapshot(&self) -> Vec<AudioClip> {
        self.timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
            .cloned()
            .collect()
    }
    fn midi_snapshot(&self) -> Vec<MidiClip> {
        self.timeline
            .midi_clip
            .iter()
            .chain(&self.timeline.other_midi_clips)
            .cloned()
            .collect()
    }
    pub(super) fn request_file_playback(&mut self, beat: f64) {
        self.sync_vst_effects();
        let clips = self.audio_snapshot();
        let midi_clips = self.midi_snapshot();
        let bpm = self.timeline.tempo.bpm();
        let meter = self.timeline.meter;
        let pattern = self
            .pattern_loop_beats
            .map(|_| self.pattern_editor.pattern.clone());
        let drum_gain = self.drum_gain();
        let mut hilo_plans = Vec::new();
        for (index, group) in self.tracks.iter().enumerate() {
            if let Some(mode) = group.hilo {
                match HiloPlan::compile(&self.tracks, group.id, mode) {
                    Ok(plan) => {
                        let weave_rule = if mode == elevate::musical::track::HiloMode::RoundRobin {
                            match self.rule_for_hilo(group.id) {
                                Ok(rule) => rule,
                                Err(error) => {
                                    self.status_message = error;
                                    return;
                                }
                            }
                        } else {
                            None
                        };
                        if let Some(rule) = &weave_rule
                            && mode == elevate::musical::track::HiloMode::RoundRobin
                            && !rule.valid_for(plan.instruments().count())
                        {
                            self.status_message =
                                "Tejer: la regla menciona una rama inexistente".into();
                            return;
                        }
                        hilo_plans.push((index, plan, weave_rule));
                    }
                    Err(error) => {
                        self.status_message = error;
                        return;
                    }
                }
            }
        }
        let mut inputs = Vec::new();
        for (index, track) in self.tracks.iter().enumerate() {
            let sources: Vec<_> = clips
                .iter()
                .filter(|c| c.track == index && !c.props.muted)
                .cloned()
                .collect();
            let notes: Vec<_> = midi_clips
                .iter()
                .filter(|clip| {
                    clip.track == index && clip.hilo_target.is_none() && !clip.props.muted
                })
                .cloned()
                .collect();
            let hilo = track.parent.and_then(|parent| {
                let (group_index, plan, weave_rule) = hilo_plans
                    .iter()
                    .find(|(_, plan, _)| plan.group == parent)?;
                let branch_index = plan.branch_index(track.id)?;
                let clips: Vec<_> = midi_clips
                    .iter()
                    .filter(|clip| clip.targets_hilo(*group_index, plan.group) && !clip.props.muted)
                    .cloned()
                    .collect();
                Some(HiloBranchInput {
                    clips,
                    group_index: *group_index,
                    branch_index,
                    instruments: plan.instruments().collect(),
                    mode: plan.mode,
                    weave: if plan.mode == elevate::musical::track::HiloMode::RoundRobin {
                        weave_rule.clone()
                    } else {
                        None
                    },
                })
            });
            let has_hilo_notes = hilo.as_ref().is_some_and(|hilo| !hilo.clips.is_empty());
            let live_vst = track.instrument.is_some_and(|id| {
                if id == self.vst.id {
                    self.vst.ready()
                } else {
                    self.vst_instances.get(&id).is_some_and(|vst| vst.ready())
                }
            });
            let live_internal = track
                .instrument
                .is_some_and(|id| self.is_piano_id(id) || self.is_sampler_id(id));
            let midi = if notes.is_empty()
                && hilo.as_ref().is_none_or(|hilo| hilo.clips.is_empty())
                && !live_vst
                && !live_internal
            {
                None
            } else {
                let source = if track.instrument.is_some_and(|id| self.is_piano_id(id)) {
                    Some(
                        self.piano_snapshot(track.instrument.unwrap())
                            .map(VoiceSource::Piano),
                    )
                } else if track.instrument.is_some_and(|id| self.is_sampler_id(id)) {
                    Some(
                        self.sampler_snapshot(track.instrument.unwrap())
                            .map(VoiceSource::Sampler),
                    )
                } else if let Some(id) = track.instrument.filter(|id| self.is_vst_id(*id)) {
                    let vst = if id == self.vst.id {
                        &self.vst
                    } else {
                        &self.vst_instances[&id]
                    };
                    Some(vst.shared().map(VoiceSource::Vst))
                } else {
                    None
                };
                match source {
                    Some(Ok(source)) => Some(MidiTrackInput {
                        index,
                        track: track.clone(),
                        instrument: track.instrument.unwrap(),
                        clips: notes,
                        source,
                        hilo,
                    }),
                    None if has_hilo_notes => {
                        self.status_message = "Falta el instrumento de una rama del Hilo".into();
                        return;
                    }
                    None => None,
                    Some(Err(_)) if notes.is_empty() && !has_hilo_notes => None,
                    Some(Err(error)) => {
                        self.status_message = error;
                        return;
                    }
                }
            };
            if sources.is_empty() && midi.is_none() {
                continue;
            }
            inputs.push(TrackInput {
                id: track.id,
                clips: sources,
                gain: self.track_gain(index),
                pan: self.track_pan(index),
                fx_gain: self.track_fx_gain(index),
                rack: self.effect_rack(index),
                midi,
            });
        }
        if inputs.iter().all(|input| {
            input.clips.is_empty()
                && input.midi.as_ref().is_none_or(|midi| {
                    midi.clips.is_empty()
                        && midi.hilo.as_ref().is_none_or(|hilo| hilo.clips.is_empty())
                })
        }) && pattern.is_none()
        {
            self.status_message = "No hay clips de audio o MIDI para reproducir".into();
            return;
        }
        let (tx, rx) = mpsc::sync_channel(1);
        let audio = self.settings.audio.saved.clone();
        let lua_script = std::env::var_os("HAY_LUA_PREPLAY_SCRIPT").map(std::path::PathBuf::from);
        let vsts = std::iter::once(&self.vst)
            .chain(self.vst_instances.values())
            .filter_map(|vst| vst.shared().ok())
            .collect();
        self.file_playback = Some(PendingPlayback {
            result: rx,
            beat,
            bpm,
            meter,
            clips,
            midi_clips,
            pattern: pattern.clone(),
            revision: self.track_revision,
            audio: audio.clone(),
            lua_preplay: lua_script.is_some(),
        });
        self.status_message = "Preparando mezcla de clips…".into();
        std::thread::spawn(move || {
            let drums = pattern.map(|pattern| DrumInput {
                pattern,
                gain: drum_gain,
            });
            let result = (|| {
                let mut inputs = inputs;
                if let Some(path) = lua_script {
                    let source = std::fs::read_to_string(&path)
                        .map_err(|error| format!("Lua pre-Play ({}): {error}", path.display()))?;
                    lua::transform_inputs(&mut inputs, &source)?;
                }
                PreparedPlayback::load(inputs, drums, bpm, meter, audio, vsts)
            })();
            let _ = tx.send(result);
        });
    }
    pub(super) fn poll_file_playback(&mut self) {
        let Some(pending) = self.file_playback.as_ref() else {
            return;
        };
        let result = match pending.result.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                Err("La preparación de la mezcla se interrumpió".into())
            }
        };
        let pending = self.file_playback.take().unwrap();
        if pending.revision != self.track_revision
            || pending.audio != self.settings.audio.saved
            || pending.bpm != self.timeline.tempo.bpm()
            || pending.meter != self.timeline.meter
            || !same_sources(&pending.clips, &self.audio_snapshot())
            || !same_midi_sources(&pending.midi_clips, &self.midi_snapshot())
            || pending.pattern.as_ref()
                != self
                    .pattern_loop_beats
                    .map(|_| &self.pattern_editor.pattern)
        {
            self.status_message =
                "Preparación cancelada: cambió la mezcla o la salida; pulsa Play de nuevo".into();
            return;
        }
        match result.and_then(|prepared| {
            prepared.start(
                pending.beat,
                pending.bpm,
                self.utility.clone(),
                self.audio_loop,
            )
        }) {
            Ok(player) => {
                player.set_drum_gain(self.drum_gain());
                self.player = Some(player);
                self.status_message = if self.pattern_loop_beats.is_some() {
                    if pending.lua_preplay {
                        "Patrón y MIDI · Lua aplicado antes de Play".into()
                    } else {
                        "Patrón, MIDI y audio · una salida y un reloj compartidos".into()
                    }
                } else if pending.lua_preplay {
                    "Reproduciendo MIDI transformado por Lua antes de Play".into()
                } else {
                    "Reproduciendo mezcla de audio y MIDI · una salida compartida".into()
                };
            }
            Err(error) => {
                self.pattern_loop_beats = None;
                self.status_message = if error.starts_with("Lua pre-Play") {
                    error
                } else {
                    format!("Salida de audio: {error}")
                };
            }
        }
    }
}
fn same_midi_sources(a: &[MidiClip], b: &[MidiClip]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.id == b.id
                && a.track == b.track
                && a.start_beat == b.start_beat
                && a.pattern == b.pattern
                && a.props == b.props
                && a.channel == b.channel
                && a.trim_start == b.trim_start
                && a.trim_len == b.trim_len
                && a.muted_notes == b.muted_notes
        })
}
fn same_sources(a: &[AudioClip], b: &[AudioClip]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.id == b.id
                && std::sync::Arc::ptr_eq(&a.file, &b.file)
                && a.track == b.track
                && a.start_beat == b.start_beat
                && a.trim_start == b.trim_start
                && a.trim_len == b.trim_len
                && a.props == b.props
        })
}
