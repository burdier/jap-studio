//! Instancias de Hay Piano por InstrumentId; la vista edita la instancia activa.
use super::DawApp;
use crate::instruments::Piano;
use elevate::musical::model::InstrumentId;

impl DawApp {
    pub(in crate::ui) fn is_piano_id(&self, id: InstrumentId) -> bool {
        id == self.instrument.id() || self.piano_instances.contains_key(&id)
    }

    pub(in crate::ui) fn activate_piano(&mut self, id: InstrumentId) {
        if id == self.instrument.id() {
            return;
        }
        let Some(next) = self.piano_instances.remove(&id) else {
            return;
        };
        let previous = std::mem::replace(&mut self.instrument, next);
        self.piano_instances.insert(previous.id(), previous);
    }

    pub(in crate::ui) fn piano_snapshot(
        &self,
        id: InstrumentId,
    ) -> Result<crate::audio::piano::PianoSnapshot, String> {
        if id == self.instrument.id() {
            self.instrument.export_snapshot()
        } else {
            self.piano_instances
                .get(&id)
                .ok_or("Piano sin instancia")?
                .export_snapshot()
        }
    }

    pub(in crate::ui) fn assign_piano_instance(&mut self, track: usize) -> InstrumentId {
        let id = self.tracks[track]
            .instrument
            .filter(|id| self.is_piano_id(*id))
            .unwrap_or_else(|| {
                if self
                    .tracks
                    .iter()
                    .any(|t| t.instrument == Some(self.instrument.id()))
                {
                    InstrumentId::fresh()
                } else {
                    self.instrument.id()
                }
            });
        if !self.is_piano_id(id) {
            let mut piano = Piano::new_with_id(id);
            piano.configure_audio(self.settings.audio.saved.clone());
            self.piano_instances.insert(id, piano);
        }
        self.tracks[track].instrument = Some(id);
        id
    }

    pub(in crate::ui) fn add_piano(&mut self, track: usize) {
        self.stop_all();
        let id = self.assign_piano_instance(track);
        self.activate_piano(id);
        self.track_revision = self.track_revision.wrapping_add(1);
        self.mix_vst();
        self.cursor_row = track;
        self.open_track_instrument(self.tracks[track].id);
        self.sync_midi_focus();
        self.status_message = format!("Hay Piano asignado a {}", self.tracks[track].name);
    }
}
