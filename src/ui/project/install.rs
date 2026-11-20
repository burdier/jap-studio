use super::*;

impl DawApp {
    pub(super) fn install_project(
        &mut self,
        snapshot: Snapshot,
        audio: Vec<crate::audio::AudioClip>,
        path: PathBuf,
        document: Vec<u8>,
    ) -> Result<(), String> {
        let mut pianos = std::collections::HashMap::new();
        for state in &snapshot.pianos {
            let mut piano = crate::instruments::Piano::restore_project(state)?;
            piano.configure_audio(self.settings.audio.saved.clone());
            pianos.insert(state.id, piano);
        }
        self.stop_all();
        self.audio_loop = None;
        self.pattern_editor.session = snapshot.drums;
        self.cursor_row = self
            .cursor_row
            .min(self.pattern_editor.session.kit.pieces.len() - 1);
        self.cursor_col = self.cursor_col.min(self.pattern_editor.step_count() - 1);
        self.utility = Default::default();
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
        self.midi_mapper = Default::default();
        self.midi_armed_tracks.clear();
        self.track_presentation.clear();
        let count = snapshot.tracks.len();
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
        self.timeline.tempo = snapshot.tempo;
        self.timeline.meter = snapshot.meter;
        for clip in snapshot.midi {
            self.timeline.insert_midi_clip(clip);
        }
        for clip in audio {
            self.timeline.insert_audio_clip(clip);
        }
        self.publish_track_fx();
        self.project.document = Some(document);
        self.project.path = Some(path);
        self.project.saved = Some(self.session_signature()?);
        self.project.dirty = false;
        Ok(())
    }
}
