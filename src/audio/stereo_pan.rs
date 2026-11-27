use std::sync::atomic::{AtomicU32, Ordering};

pub(crate) struct StereoPan(AtomicU32);

impl StereoPan {
    pub fn new(position: f32) -> Self {
        Self(AtomicU32::new(position.clamp(-1.0, 1.0).to_bits()))
    }

    pub fn set(&self, position: f32) {
        self.0
            .store(position.clamp(-1.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn position(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed)).clamp(-1.0, 1.0)
    }

    pub fn apply(&self, frames: &mut [[f32; 2]]) {
        let position = f32::from_bits(self.0.load(Ordering::Relaxed)).clamp(-1.0, 1.0);
        Self::apply_position(frames, position);
    }

    pub fn apply_position(frames: &mut [[f32; 2]], position: f32) {
        let position = position.clamp(-1.0, 1.0);
        let left_gain = 1.0 - position.max(0.0);
        let right_gain = 1.0 + position.min(0.0);
        for frame in frames {
            frame[0] *= left_gain;
            frame[1] *= right_gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StereoPan;

    #[test]
    fn pan_is_stereo_balancing_and_center_preserves_the_signal() {
        let pan = StereoPan::new(0.0);
        let mut frame = [[0.5, -0.5]];
        pan.apply(&mut frame);
        assert_eq!(frame, [[0.5, -0.5]]);
        pan.set(-1.0);
        pan.apply(&mut frame);
        assert_eq!(frame, [[0.5, 0.0]]);
        pan.set(1.0);
        pan.apply(&mut frame);
        assert_eq!(frame, [[0.0, 0.0]]);
    }
}
