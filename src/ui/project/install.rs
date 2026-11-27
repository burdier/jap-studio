use super::*;

impl DawApp {
    pub(super) fn install_prepared_samplers(&mut self, samplers: Vec<snapshot::PreparedSampler>) {
        use crate::instruments::sampler::Sampler;
        for (id, params, asset, engine) in samplers {
            let mut sampler = Sampler::default();
            sampler.params = params;
            sampler.configure_audio(self.settings.audio.saved.clone());
            sampler.install(asset, engine);
            if id == self.active_sampler_id {
                self.sampler = sampler;
            } else {
                self.sampler_instances.insert(id, sampler);
            }
        }
    }

    pub(super) fn install_project(
        &mut self,
        snapshot: Snapshot,
        audio: Vec<crate::audio::AudioClip>,
        path: PathBuf,
        document: Vec<u8>,
        effects: effects::PreparedEffects,
        components: std::collections::BTreeMap<
            elevate::musical::fx::FxId,
            std::sync::Arc<crate::audio::effects::ComponentEffect>,
        >,
        samplers: Vec<snapshot::PreparedSampler>,
        vsts: Vec<snapshot::PreparedVst>,
    ) -> Result<(), String> {
        let mut pianos = std::collections::HashMap::new();
        for state in &snapshot.pianos {
            let mut piano = crate::instruments::Piano::restore_project(state)?;
            piano.configure_audio(self.settings.audio.saved.clone());
            pianos.insert(state.id, piano);
        }
        #[cfg(feature = "p2p")]
        self.leave_shared_project();
        self.import_task = None;
        self.stop_all();
        self.audio_loop = None;
        self.pattern_editor = crate::ui::pattern_editor::PatternEditor::new();
        self.pattern_editor.session = snapshot.drums;
        self.utility = Default::default();
        let _ = self.utility.restore(snapshot.utility);
        self.master_chain = snapshot.master_chain;
        self.fx.master_focus = false;
        self.vst.close_editor();
        for vst in self.vst_instances.values() {
            vst.close_editor();
        }
        self.vst = Default::default();
        self.vst_instances.clear();
        self.vst_reloads.clear();
        self.vst_effects = Default::default();
        self.fx.current = None;
        self.fx.editor_track = None;
        self.fx.tejer = None;
        self.fx.cut_draft = None;
        self.piano_roll = Default::default();
        self.timeline = crate::ui::timeline::Timeline::new(self.ds.timeline_pixels_per_beat);
        self.piano_instances.clear();
        self.instrument = crate::instruments::Piano::new();
        self.instrument
            .configure_audio(self.settings.audio.saved.clone());
        if let Some(id) = snapshot.pianos.first().map(|p| p.id) {
            self.instrument = pianos.remove(&id).unwrap();
        }
        self.piano_instances = pianos;
        self.sampler_instances.clear();
        self.sampler = Default::default();
        self.active_sampler_id = crate::instruments::sampler::SAMPLER_ID;
        self.sampler
            .configure_audio(self.settings.audio.saved.clone());
        self.synth_instances.clear();
        self.synth = Default::default();
        self.active_synth_id = crate::instruments::synth::SYNTH_ID;
        self.synth
            .configure_audio(self.settings.audio.saved.clone());
        for (i, state) in snapshot.synths.iter().enumerate() {
            let mut synth = crate::instruments::synth::Synth::from_params(state.params.clone());
            synth.configure_audio(self.settings.audio.saved.clone());
            if i == 0 {
                self.active_synth_id = state.id;
                self.synth = synth;
            } else {
                self.synth_instances.insert(state.id, synth);
            }
        }
        self.midi_mapper =
            elevate::musical::midi_map::MidiMapper::from_bindings(snapshot.midi_bindings);
        self.midi_armed_tracks.clear();
        self.frozen_tracks.clear();
        self.freeze_pending = None;
        self.track_presentation.clear();
        let count = snapshot.tracks.len();
        self.vst_effects.instances = effects;
        self.lua_components.instances = components;
        self.commit_track_plan(
            snapshot.tracks,
            vec![None; count],
            snapshot.levels,
            snapshot.pans,
            vec![],
            false,
            false,
            None,
        );
        self.install_prepared_samplers(samplers);
        for (i, (id, prepared, name, bundle, class)) in vsts.into_iter().enumerate() {
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
            if i == 0 {
                self.vst = inst;
            } else {
                self.vst_instances.insert(id, inst);
            }
        }
        self.timeline.tempo = snapshot.tempo;
        self.timeline.meter = snapshot.meter;
        self.timeline.navigation = snapshot.navigation;
        self.timeline.automation = snapshot.automation;
        for clip in snapshot.midi {
            self.timeline.insert_midi_clip(clip);
        }
        for clip in audio {
            self.timeline.insert_audio_clip(clip);
        }
        self.sync_vst_effects();
        self.publish_track_fx();
        let doc_for_history = document.clone();
        self.project.recent = recent::record(&path);
        self.project.document = Some(document);
        self.project.path = Some(path);
        self.project.saved = Some(self.session_signature()?);
        self.project.dirty = false;
        self.load_loro_history(&doc_for_history, "Proyecto abierto");
        Ok(())
    }
}
