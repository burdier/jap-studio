//! La cola se cuenta en frames de salida, incluso cuando el transporte vuelve al inicio del loop.
const MAX_TAIL_SECONDS: u64 = 10;
const FADE_FRAMES: u64 = 256;

#[derive(Default)]
pub(super) struct Retirement {
    remaining: Option<u64>,
}

impl Retirement {
    pub fn begin(&mut self, sample_rate: u32) -> bool {
        if self.remaining.is_some() {
            return false;
        }
        self.remaining = Some(u64::from(sample_rate) * MAX_TAIL_SECONDS);
        true
    }

    pub fn remaining(&self) -> Option<u64> {
        self.remaining
    }

    pub fn finish_block(&mut self, frames: &mut [[f32; 2]]) {
        let Some(remaining) = &mut self.remaining else {
            return;
        };
        for frame in frames {
            let gain = (*remaining).min(FADE_FRAMES) as f32 / FADE_FRAMES as f32;
            frame[0] *= gain;
            frame[1] *= gain;
            *remaining = remaining.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_fades_to_silence_on_the_same_frame_for_different_block_sizes() {
        let render = |size| {
            let mut state = Retirement::default();
            assert!(state.begin(48));
            let mut output = [[1.0; 2]; 512];
            for window in output.chunks_mut(size) {
                assert!(!state.begin(48), "no reinicia una cola ya solicitada");
                state.finish_block(window);
            }
            assert_eq!(state.remaining(), Some(0));
            output
        };
        let output = render(256);
        assert_eq!(output, render(39));
        assert_eq!(output[0], [1.0; 2]);
        assert!(output[479][0] > 0.0 && output[479][0] < 0.01);
        assert!(output[480..].iter().all(|frame| *frame == [0.0; 2]));
    }
}
