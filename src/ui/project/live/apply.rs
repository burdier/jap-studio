//! Publicación de la revisión recibida en la sesión abierta.
use super::{DawApp, Snapshot};
use crate::{audio::AudioClip, instruments::Piano};

impl DawApp {
    pub(super) fn install_shared_snapshot(
        &mut self,
        before: &Snapshot,
        next: Snapshot,
        audio: Vec<AudioClip>,
        effects: super::super::effects::PreparedEffects,
        components: std::collections::BTreeMap<
            elevate::musical::fx::FxId,
            std::sync::Arc<crate::audio::effects::ComponentEffect>,
        >,
        samplers: Vec<super::super::snapshot::PreparedSampler>,
        vsts: Vec<super::super::snapshot::PreparedVst>,
    ) -> Result<(), String> {
        let mut restore = Vec::new();
        for state in next.pianos.iter().filter(|p| !before.pianos.contains(p)) {
            let mut piano = Piano::restore_project(state)?;
            piano.configure_audio(self.settings.audio.saved.clone());
            restore.push((state.id, piano));
        }

        let roll_clip = self.timeline.midi_clip.as_ref().map(|clip| clip.id);
        let tracks_changed = before.tracks != next.tracks;
        let timing_changed = before.tempo != next.tempo || before.meter != next.meter;
        let drums_changed = before.drums != next.drums;
        let arrangement_changed = before.midi != next.midi || before.audio != next.audio;
        let layout_changed = tracks_changed
            && (before.tracks.len() != next.tracks.len()
                || !before
                    .tracks
                    .iter()
                    .zip(&next.tracks)
                    .all(|(a, b)| super::super::snapshot::same_track_settings(a, b)));
        self.vst_effects.instances.extend(effects);
        self.lua_components.instances.extend(components);
        if tracks_changed {
            self.invalidate_changed_fx(&next.tracks);
        }
        if layout_changed {
            let sources = next
                .tracks
                .iter()
                .map(|track| self.tracks.iter().position(|old| old.id == track.id))
                .collect();
            self.commit_track_plan(
                next.tracks,
                sources,
                next.levels,
                next.pans,
                self.selected_tracks.clone(),
                true,
                false,
                None,
            );
        } else {
            if tracks_changed {
                // Un cambio de bandas se publica en el rack que ya está sonando.
                self.tracks = next.tracks;
                self.refresh_track_layout();
            }
            let mix_changed = before.levels != next.levels || before.pans != next.pans;
            if mix_changed {
                self.track_levels = next.levels;
                self.track_pans = next.pans;
                self.apply_track_gains();
            }
            if tracks_changed || mix_changed {
                self.track_revision = self.track_revision.wrapping_add(1);
            }
        }
        self.install_prepared_samplers(samplers);
        for (id, prepared, name, bundle, class) in vsts {
            let track_idx = self.tracks.iter().position(|t| t.instrument == Some(id));
            let gain = track_idx.map_or(1.0, |idx| self.track_gain(idx));
            let pan = track_idx.map_or(0.0, |idx| self.track_pan(idx));
            let fx = track_idx.map_or(1.0, |idx| self.track_fx_gain(idx));
            let mut inst = crate::instruments::vst::VstInstrument::new(id);
            inst.install(
                prepared,
                name,
                bundle,
                class,
                gain,
                pan,
                fx,
                self.utility.clone(),
            )?;
            if self.vst.id == id || !self.vst.ready() {
                self.vst = inst;
            } else {
                self.vst_instances.insert(id, inst);
            }
        }
        for (id, piano) in restore {
            if self.instrument.id() == id {
                self.instrument.stop();
                self.instrument = piano;
            } else {
                self.piano_instances.insert(id, piano);
            }
        }
        if tracks_changed || before.pianos != next.pianos || before.samplers != next.samplers {
            self.sync_vst_effects();
            self.publish_track_fx();
            self.sync_midi_focus();
        }
        if timing_changed {
            self.stop_audio();
            self.timeline.tempo = next.tempo;
            self.timeline.meter = next.meter;
        }
        if self.audio_loop.is_some() && before.navigation.loop_region != next.navigation.loop_region
        {
            self.set_audio_loop(
                next.navigation
                    .loop_region
                    .map(crate::ui::timeline::loop_beats),
            )?;
        }
        self.timeline.navigation = next.navigation;
        if drums_changed {
            self.pattern_editor.install_shared_drums(next.drums);
        }
        if layout_changed || arrangement_changed {
            if let Some(id) = self.timeline.sample_editor.id
                && before.audio.iter().find(|c| c.id == id)
                    != next.audio.iter().find(|c| c.id == id)
            {
                self.timeline.sample_editor.silence();
                self.timeline.sample_editor.draft = None;
                self.timeline.sample_editor.tuning = None;
            }
            self.timeline.replace_project_clips(next.midi, audio);
            let current = self.timeline.midi_clip.as_ref();
            if current.map(|clip| clip.id) != roll_clip {
                let roll = &mut self.piano_roll;
                roll.open = false;
                roll.opened_clip = None;
                roll.selected = None;
                roll.selected_notes.clear();
                roll.selected_control = None;
            } else if let Some(clip) = current {
                let has_note = |id| clip.notes().iter().any(|note| note.id == id);
                self.piano_roll.selected_notes.retain(|id| has_note(*id));
                self.piano_roll.selected = self.piano_roll.selected.filter(|id| has_note(*id));
            }
        }
        if arrangement_changed {
            self.publish_clip_edit();
        } else if drums_changed {
            self.publish_drum_edit();
        }
        self.project.checked = None;
        Ok(())
    }
}
