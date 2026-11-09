//! Proyección de la topología validada al mecanismo atómico ya usado por los streams.
use crate::ui::DawApp;
impl DawApp {
    pub(in crate::ui) fn track_fx_gain(&self, index: usize) -> f32 {
        let Some(track) = self.tracks.get(index) else {
            return 1.0;
        };
        let own = elevate::devices::fx_gain::compile(&track.chain).unwrap_or(1.0);
        let parent = track
            .parent
            .and_then(|id| self.tracks.iter().find(|track| track.id == id))
            .map_or(1.0, |track| {
                elevate::devices::fx_gain::compile(&track.chain).unwrap_or(1.0)
            });
        own * parent
    }
    pub(in crate::ui) fn publish_track_fx(&mut self) {
        if let Some(track) = self.sampler_track() {
            self.sampler.mix(
                self.sampler_gain(track),
                self.track_pan(track),
                self.track_fx_gain(track),
            );
        }
        if let (Some(player), Some(clip)) = (&self.player, &self.timeline.clip) {
            player.set_fx_gain(self.track_fx_gain(clip.track));
        }
        let track = if self.instrument.clip_playing() {
            self.timeline.midi_clip.as_ref().map(|clip| clip.track)
        } else {
            self.tracks
                .iter()
                .position(|track| track.instrument == Some(self.instrument.id()))
                .or_else(|| self.timeline.midi_clip.as_ref().map(|clip| clip.track))
        };
        let gain = track.map_or(1.0, |index| self.track_fx_gain(index));
        self.instrument.set_fx_gain(gain);
    }
}
