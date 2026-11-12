//! Captura y publicación de lotes: la validación termina antes de tocar la sesión.
use super::DawApp;
use elevate::{
    musical::track::{TrackBatch, TrackBatchError, TrackSnapshot, apply_track_batch},
    project::ProjectDoc,
};

impl DawApp {
    pub(in crate::ui) fn track_snapshot(&self) -> Result<TrackSnapshot, String> {
        TrackSnapshot::capture(
            &self.tracks,
            &self.track_levels,
            &self.track_pans,
            self.track_revision,
        )
    }

    pub(in crate::ui) fn apply_track_script_batch(
        &mut self,
        batch: TrackBatch,
    ) -> Result<std::collections::BTreeMap<String, elevate::musical::track::TrackId>, String> {
        let current = self.track_snapshot()?;
        let result = apply_track_batch(&current, &batch)
            .map_err(|error: TrackBatchError| error.to_string())?;
        let tracks: Vec<_> = result
            .snapshot
            .entries
            .iter()
            .map(|entry| entry.track.clone())
            .collect();

        // FX que necesitan instancias o tomas de análisis exigen preparación del
        // host; esta versión del lote publica únicamente cambios de cadena Utility.
        for entry in &result.snapshot.entries {
            let changed = current
                .get(entry.track.id)
                .is_none_or(|old| old.track.chain != entry.track.chain);
            if changed {
                if crate::audio::effects::needs_rack(&entry.track.chain) {
                    return Err("Lote: la edición FX requiere una instancia/toma preparada; usa Hay Utility en esta versión".into());
                }
                elevate::devices::fx_gain::compile(&entry.track.chain)?;
            }
        }

        // El documento Loro conserva IDs/orden/FX por entidad. Se valida una
        // proyección candidata de pistas y MIDI sin reemplazar ningún estado UI.
        let mut original = vec![None; self.tracks.len()];
        for (new, old) in result.sources.iter().enumerate() {
            if let Some(old) = old {
                original[*old] = Some(new);
            }
        }
        let mut midi = Vec::new();
        for clip in self
            .timeline
            .other_midi_clips
            .iter()
            .chain(self.timeline.midi_clip.iter())
        {
            let mut clip = clip.clone();
            clip.track = original
                .get(clip.track)
                .copied()
                .flatten()
                .ok_or("Lote: un clip MIDI perdió su pista")?;
            midi.push(clip);
        }
        ProjectDoc::from_session("Sesión", &tracks, &midi)?.snapshot()?;

        let levels = result
            .snapshot
            .entries
            .iter()
            .map(|entry| entry.mix.level)
            .collect();
        let pans = result
            .snapshot
            .entries
            .iter()
            .map(|entry| entry.mix.pan)
            .collect();
        let handles = result.handles;
        let selected = self.selected_tracks.clone();
        self.commit_track_plan(
            tracks,
            result.sources,
            levels,
            pans,
            selected,
            true,
            false,
            Some(result.snapshot.revision),
        );
        self.publish_track_fx();
        Ok(handles)
    }
}
