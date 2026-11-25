//! Resultado preparado fuera del callback para las cadenas lineales de Utility actuales.
//! Publicación escalar atómica: sin árboles, locks, reservas ni destrucciones en RT.
use std::sync::atomic::{AtomicU32, Ordering};
pub(crate) struct FxGain(AtomicU32, super::effects::Routing);
impl FxGain {
    pub fn new(gain: f32) -> Self {
        Self(
            AtomicU32::new(gain.to_bits()),
            super::effects::Routing::new(),
        )
    }
    pub fn set_rack(&self, rack: Option<std::sync::Arc<super::effects::Rack>>, rate: u32) {
        self.1.set(rack, rate);
    }
    pub fn tail_frames(&self) -> u64 {
        self.1.tail_frames()
    }
    pub fn has_track_sidechain(&self) -> bool {
        self.1.has_track_sidechain()
    }
    pub fn set(&self, gain: f32) {
        self.0.store(gain.to_bits(), Ordering::Relaxed);
    }
    pub fn prepare(&self) -> PreparedFxGain {
        let gain = self.get();
        PreparedFxGain {
            history: super::effects::CrossoverHistory::new(),
            current: gain,
            target: gain,
            step: 0.0,
            remaining: 0,
        }
    }
    fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}
pub(crate) struct PreparedFxGain {
    history: super::effects::CrossoverHistory,
    current: f32,
    target: f32,
    step: f32,
    remaining: usize,
}
impl PreparedFxGain {
    pub fn process(&mut self, state: &FxGain, frames: &mut [[f32; 2]]) {
        self.process_timed(state, frames, None);
    }
    pub fn process_timed(
        &mut self,
        state: &FxGain,
        frames: &mut [[f32; 2]],
        time: Option<super::vst3::TransportContext>,
    ) {
        self.process_timed_automated(state, frames, time, None);
    }
    pub fn process_timed_automated(
        &mut self,
        state: &FxGain,
        frames: &mut [[f32; 2]],
        time: Option<super::vst3::TransportContext>,
        automation: Option<(&elevate::musical::track_automation::TrackAutomation, f64)>,
    ) {
        self.process_timed_automated_sidechain(state, frames, time, automation, None);
    }
    pub fn process_timed_automated_sidechain(
        &mut self,
        state: &FxGain,
        frames: &mut [[f32; 2]],
        time: Option<super::vst3::TransportContext>,
        automation: Option<(&elevate::musical::track_automation::TrackAutomation, f64)>,
        sidechain: Option<super::effects::SidechainInputs<'_>>,
    ) {
        let target = state.get();
        if self.target != target {
            self.target = target;
            self.remaining = 256;
            self.step = (target - self.current) / 256.0;
        }
        state.1.process_timed_automated_sidechain(
            frames,
            time,
            &mut self.history,
            automation,
            sidechain,
        );
        for frame in frames {
            if self.remaining > 0 {
                self.current += self.step;
                self.remaining -= 1;
                if self.remaining == 0 {
                    self.current = self.target;
                }
            }
            frame[0] *= self.current;
            frame[1] *= self.current;
        }
    }
}
