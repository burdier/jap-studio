//! Preparación de mezcla cancelable; fuentes y remuestreo fuera del callback.

use super::DawApp;
use crate::audio::drums::{DrumInput, DrumPreview, DrumScore};
use crate::audio::piano::VoiceSource;
use crate::audio::{AudioClip, HiloBranchInput, MidiTrackInput, PreparedPlayback, TrackInput};
use elevate::musical::clip::MidiClip;
use elevate::musical::{
    drums::{DrumPlaybackSource, arrangement_pattern},
    hilo::HiloPlan,
    model::InstrumentId,
    time::PPQ,
    track::effective_mix,
};
use std::sync::mpsc::{self, Receiver, TryRecvError};
pub(super) struct PendingPlayback {
    result: Receiver<Result<PreparedPlayback, String>>,
    pub(super) beat: f64,
    bpm: f64,
    meter: elevate::musical::time::Meter,
    clips: Vec<AudioClip>,
    midi_clips: Vec<MidiClip>,
    revision: u64,
    audio: crate::audio::device::AudioConfig,
    lua_preplay: bool,
    drums_enabled: bool,
    count_in_bars: u32,
}
impl DawApp {
    pub(super) fn audio_snapshot(&self) -> Vec<AudioClip> {
        let mut clips: Vec<AudioClip> = self
            .timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
            .cloned()
            .collect();
        clips.sort_unstable_by_key(|clip| clip.id.value());
        clips
    }
    pub(super) fn midi_snapshot(&self) -> Vec<MidiClip> {
        let mut clips: Vec<MidiClip> = self
            .timeline
            .midi_clip
            .iter()
            .chain(&self.timeline.other_midi_clips)
            .filter(|clip| clip.drum_target.is_none())
            .cloned()
            .collect();
        clips.sort_unstable_by_key(|clip| clip.id.value());
        clips
    }
    pub(super) fn has_drum_clips(&self) -> bool {
        self.timeline
            .midi_clip
            .iter()
            .chain(&self.timeline.other_midi_clips)
            .any(|clip| clip.drum_target.is_some())
    }

    fn drum_score(&self) -> Result<Option<DrumScore>, String> {
        self.drum_score_for_tracks(|_| true)
    }

    pub(super) fn drum_score_for_tracks(
        &self,
        includes: impl Fn(usize) -> bool,
    ) -> Result<Option<DrumScore>, String> {
        let session = &self.pattern_editor.session;
        if session.source == DrumPlaybackSource::Arrangement {
            let clips = self
                .timeline
                .midi_clip
                .iter()
                .chain(&self.timeline.other_midi_clips)
                .filter(|clip| {
                    includes(clip.track)
                        && clip
                            .drum_target
                            .is_some_and(|target| target.kit == session.kit.id)
                });
            let pattern = arrangement_pattern(clips, |clip| self.drum_track_audible(clip.track))?;
            return Ok(pattern.map(|pattern| DrumScore {
                pattern,
                repeat: false,
            }));
        }
        if session.source == DrumPlaybackSource::Song && session.song.is_empty() {
            return Ok(None);
        }
        Ok(session.playback_pattern()?.map(|pattern| DrumScore {
            pattern: pattern.into_owned(),
            repeat: true,
        }))
    }

    fn drum_track_audible(&self, index: usize) -> bool {
        if self.pattern_editor.session.kit.bus.solo {
            return self.tracks.get(index).is_some_and(|track| {
                !track.muted
                    && track
                        .parent
                        .and_then(|id| self.tracks.iter().find(|item| item.id == id))
                        .is_none_or(|parent| !parent.muted)
            });
        }
        effective_mix(&self.tracks, &self.track_levels, &self.track_pans, index).audible
    }

    pub(super) fn arrangement_inputs(
        &self,
        clips: &[AudioClip],
        midi_clips: &[MidiClip],
        selected: Option<&elevate::export::ExportOptions>,
    ) -> Result<Vec<TrackInput>, String> {
        let mut hilo_plans = Vec::new();
        for (index, group) in self.tracks.iter().enumerate() {
            if let Some(mode) = group.hilo {
                match HiloPlan::compile(&self.tracks, group.id, mode) {
                    Ok(plan) => {
                        let weave_rule = if mode == elevate::musical::track::HiloMode::RoundRobin {
                            match self.rule_for_hilo(group.id) {
                                Ok(rule) => rule,
                                Err(error) => {
                                    return Err(error);
                                }
                            }
                        } else {
                            None
                        };
                        if let Some(rule) = &weave_rule
                            && mode == elevate::musical::track::HiloMode::RoundRobin
                            && !rule.valid_for(plan.instruments().count())
                        {
                            return Err("Tejer: la regla menciona una rama inexistente".into());
                        }
                        hilo_plans.push((index, plan, weave_rule));
                    }
                    Err(error) => {
                        return Err(error);
                    }
                }
            }
        }
        let mut inputs = Vec::new();
        for (index, track) in self.tracks.iter().enumerate() {
            let include_sources = selected.is_none_or(|s| s.includes_track(index));
            let needed_as_return = track.is_return()
                && self.tracks.iter().enumerate().any(|(src_index, src)| {
                    selected.is_some_and(|s| s.includes_track(src_index))
                        && src.send_amount(track.id) > 0
                });
            if !include_sources
                && !needed_as_return
                && !self.tracks.iter().enumerate().any(|(child_index, child)| {
                    child.parent == Some(track.id)
                        && selected.is_some_and(|s| s.includes_track(child_index))
                })
            {
                continue;
            }
            if let Some(frozen) = self.frozen_tracks.get(&track.id) {
                let (gain, pan, source_audible, audible) = self.arrangement_track_mix(index);
                let is_return = track.is_return();
                let sends = if is_return {
                    Vec::new()
                } else {
                    track
                        .sends
                        .iter()
                        .filter(|send| send.amount > 0)
                        .map(|send| (send.target, send.gain()))
                        .collect()
                };
                let sources = if include_sources {
                    let mut clip = frozen.clip.clone();
                    clip.track = index;
                    vec![clip]
                } else {
                    Vec::new()
                };
                inputs.push(TrackInput {
                    id: track.id,
                    parent: track.parent,
                    is_return,
                    sends,
                    source_audible,
                    audible,
                    clips: sources,
                    gain,
                    pan,
                    fx_gain: 1.0,
                    rack: None,
                    midi: None,
                    automation: self.timeline.automation.track_or_default(track.id),
                });
                continue;
            }
            let sources: Vec<_> = clips
                .iter()
                .filter(|c| include_sources && c.track == index && !c.props.muted)
                .cloned()
                .collect();
            let notes: Vec<_> = midi_clips
                .iter()
                .filter(|clip| {
                    include_sources
                        && clip.track == index
                        && clip.hilo_target.is_none()
                        && !clip.props.muted
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
                && (selected.is_some() || (!live_vst && !live_internal))
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
                        return Err("Falta el instrumento de una rama del Hilo".into());
                    }
                    None => None,
                    Some(Err(_)) if notes.is_empty() && !has_hilo_notes => None,
                    Some(Err(error)) => {
                        return Err(error);
                    }
                }
            };
            let (gain, pan, source_audible, audible) = self.arrangement_track_mix(index);
            let is_return = track.is_return();
            let sends = if is_return {
                Vec::new()
            } else {
                track
                    .sends
                    .iter()
                    .filter(|send| send.amount > 0)
                    .map(|send| (send.target, send.gain()))
                    .collect()
            };
            inputs.push(TrackInput {
                id: track.id,
                parent: track.parent,
                is_return,
                sends,
                source_audible,
                audible,
                clips: sources,
                gain,
                pan,
                fx_gain: self.own_track_fx_gain(index),
                rack: if selected.is_none() {
                    self.effect_rack(index)
                } else {
                    None
                },
                midi,
                automation: self.timeline.automation.track_or_default(track.id),
            });
        }
        Ok(inputs)
    }

    pub(in crate::ui) fn publish_clip_edit(&mut self) {
        if self.player.is_none() {
            if let Some(pending) = &self.file_playback {
                let beat = pending.beat;
                self.request_file_playback(beat);
            }
            return;
        }
        self.publish_drum_edit();
        let inputs = self.arrangement_inputs(&self.audio_snapshot(), &self.midi_snapshot(), None);
        match inputs {
            Ok(inputs) => self.player.as_mut().unwrap().queue_arrangement(inputs),
            Err(error) => self.status_message = error,
        }
    }

    pub(super) fn finish_arrangement_drag(&mut self) -> bool {
        let moved = self.timeline.clip_moved();
        let finished = self.timeline.end_clip_drag();
        if moved {
            self.publish_clip_edit();
        }
        if let Some(kind) = self.timeline.sample_editor.pending.take()
            && !moved
        {
            self.show_selected_editor(kind);
            return true;
        }
        moved || finished
    }
    pub(super) fn request_file_playback(&mut self, beat: f64) {
        self.request_file_playback_with_count_in(beat, 0);
    }

    pub(super) fn request_file_playback_with_count_in(&mut self, beat: f64, count_in_bars: u32) {
        self.sync_vst_effects();
        let clips = self.audio_snapshot();
        let midi_clips = self.midi_snapshot();
        let bpm = self.timeline.tempo.bpm();
        let meter = self.timeline.meter;
        let drums_enabled = self.pattern_loop_beats.is_some() || self.has_drum_clips();
        let drums = if drums_enabled {
            match self.drum_score() {
                Ok(score) => Some(DrumInput {
                    score,
                    kit: self.pattern_editor.session.kit.clone(),
                }),
                Err(error) => {
                    self.status_message = error;
                    self.pattern_loop_beats = None;
                    return;
                }
            }
        } else {
            None
        };
        self.pattern_loop_beats = loop_beats(drums.as_ref().and_then(|input| input.score.as_ref()));

        let inputs = match self.arrangement_inputs(&clips, &midi_clips, None) {
            Ok(inputs) => inputs,
            Err(error) => {
                self.status_message = error;
                return;
            }
        };
        let metro_or_rec =
            self.metronome.is_enabled() || count_in_bars > 0 || self.is_any_recording();
        if !metro_or_rec
            && inputs.iter().all(|input| {
                input.clips.is_empty()
                    && input.midi.as_ref().is_none_or(|midi| {
                        midi.clips.is_empty()
                            && midi.hilo.as_ref().is_none_or(|hilo| hilo.clips.is_empty())
                    })
            })
            && drums.as_ref().is_none_or(|input| input.score.is_none())
        {
            self.status_message =
                "No hay clips para reproducir (activa ♩ Clic para ensayar o grabar con metrónomo)"
                    .into();
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
            revision: self.track_revision,
            audio: audio.clone(),
            lua_preplay: lua_script.is_some(),
            drums_enabled,
            count_in_bars,
        });
        self.status_message = if count_in_bars > 0 {
            format!("Preparando pre-cuenta ({count_in_bars} comp.)…")
        } else {
            "Preparando mezcla de clips…".into()
        };
        std::thread::spawn(move || {
            let result = (|| {
                let mut inputs = inputs;
                crate::audio::apply_preplay(&mut inputs)?;
                PreparedPlayback::load(inputs, drums, bpm, meter, audio, vsts)
            })();
            let _ = tx.send(result);
        });
    }
    pub(super) fn poll_file_playback(&mut self) {
        if let Some(player) = &mut self.player
            && let Err(error) = player.poll_edits()
        {
            self.status_message = error;
        }
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
        {
            self.pattern_loop_beats = None;
            self.status_message =
                "Preparación cancelada: cambió la mezcla o la salida; pulsa Play de nuevo".into();
            return;
        }
        match result.and_then(|mut prepared| {
            if pending.drums_enabled {
                let score = self.drum_score()?;
                prepared.update_drums(&self.pattern_editor.session.kit, score.as_ref())?;
                self.pattern_loop_beats = loop_beats(score.as_ref());
            }
            self.metronome.set_keep_running(self.is_any_recording());
            prepared.start(
                pending.beat,
                pending.bpm,
                pending.meter,
                self.utility.clone(),
                self.master_fx_gain(),
                self.master_effect_rack(),
                self.audio_loop,
                self.metronome.clone(),
                pending.count_in_bars,
            )
        }) {
            Ok(player) => {
                let counting = player.is_counting_in();
                self.player = Some(player);
                self.status_message = if counting {
                    format!(
                        "● Pre-cuenta ({} comp.) · prepárate para entrar…",
                        pending.count_in_bars
                    )
                } else if let Some(active) = &self.audio_recorder {
                    let mode = if active.recorder.stereo {
                        "Estéreo"
                    } else {
                        "Mono"
                    };
                    format!(
                        "🎙 Grabando audio en «{}» ({mode}) · pulsa 🎙 o ■ para guardar",
                        active.track_name
                    )
                } else if self.midi_capture.is_recording() {
                    format!(
                        "● Grabando MIDI en {} · pulsa ● o Espacio para finalizar",
                        self.tracks
                            .get(self.cursor_row)
                            .map_or("pista", |t| t.name.as_ref())
                    )
                } else if self.pattern_loop_beats.is_some() {
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
    /// Los editores publican una vez por acción confirmada. El callback solo ve el plan compilado.
    pub(in crate::ui) fn publish_drum_edit(&mut self) {
        if !self
            .player
            .as_ref()
            .is_some_and(|player| player.has_drum_kit())
        {
            return;
        }
        let score = match self.drum_score() {
            Ok(score) => score,
            Err(error) => {
                self.status_message = error;
                return;
            }
        };
        self.pattern_loop_beats = loop_beats(score.as_ref());
        if let Some(player) = &mut self.player
            && let Err(error) =
                player.update_drums(&self.pattern_editor.session.kit, score.as_ref())
        {
            self.status_message = error;
        }
    }

    pub(in crate::ui) fn publish_drum_mix(&mut self) {
        let session = &self.pattern_editor.session;
        if session.source == DrumPlaybackSource::Arrangement
            && let Some(player) = &self.player
            && player.has_drum_kit()
            && player.drum_bus_solo() != session.kit.bus.solo
        {
            self.publish_drum_edit();
        } else if let Some(player) = &mut self.player {
            player.update_drum_kit(&session.kit);
        }
    }

    pub(in crate::ui) fn preview_drum_piece(
        &mut self,
        id: InstrumentId,
        pitch: u8,
        velocity: u8,
    ) -> bool {
        self.player.as_mut().is_some_and(|player| {
            player.preview_drum(DrumPreview {
                piece: id,
                pitch,
                velocity,
            })
        })
    }
}
fn loop_beats(score: Option<&DrumScore>) -> Option<f64> {
    score
        .filter(|score| score.repeat)
        .map(|score| score.pattern.length().0 as f64 / PPQ as f64)
}

fn same_midi_sources(a: &[MidiClip], b: &[MidiClip]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.id == b.id
                && a.track == b.track
                && a.hilo_target == b.hilo_target
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
