//! Adaptación del Transport musical existente a seek y segmentos de loop.
use elevate::{StereoFrame, musical::time::Transport};
#[derive(Clone, Copy, Default)]
pub(super) struct Request {
    pub revision: u64,
    pub seek: u64,
    pub generation: u64,
    pub loop_range: Option<(u64, u64)>,
}
pub(super) struct Cursor {
    clock: Transport,
    generation: u64,
    end: u64,
}
impl Cursor {
    pub fn new(start: u64, end: u64) -> Self {
        let mut clock = Transport::default();
        clock.seek(start);
        clock.play();
        Self {
            clock,
            generation: 0,
            end,
        }
    }
    pub fn frame(&self) -> u64 {
        self.clock.frame()
    }
    pub fn finished(&self) -> bool {
        !self.clock.is_playing()
    }
    pub fn render(
        &mut self,
        request: Request,
        output: &mut [StereoFrame],
        mut source: impl FnMut(u64, &mut [StereoFrame]),
    ) {
        if request.generation != self.generation {
            self.clock.seek(request.seek);
            self.clock.play();
            self.generation = request.generation;
        }
        let mut written = 0;
        while written < output.len() {
            let limit = if let Some((start, end)) = request.loop_range {
                if self.clock.frame() >= end {
                    self.clock.seek(start);
                    self.clock.play();
                }
                end
            } else {
                self.end
            };
            if self.clock.frame() >= limit {
                self.clock.stop();
                output[written..].fill([0.; 2]);
                break;
            }
            let count = (limit - self.clock.frame())
                .min((output.len() - written) as u64)
                .min(u32::MAX as u64) as usize;
            self.clock.play();
            let window = self
                .clock
                .advance(count as u32)
                .expect("Segmento validado dentro del límite")
                .unwrap();
            source(window.start(), &mut output[written..written + count]);
            written += count;
            if self.clock.frame() == limit {
                if let Some((start, _)) = request.loop_range {
                    self.clock.seek(start);
                } else {
                    self.clock.stop();
                }
            }
        }
    }
}
pub(crate) fn beat_frame(beat: f64, rate: u32, beats_per_second: f64) -> Result<u64, String> {
    let frame = beat / beats_per_second * rate as f64;
    if !beat.is_finite() || beat < 0. || !frame.is_finite() || frame >= u64::MAX as f64 {
        return Err("Posición fuera de rango".into());
    }
    Ok(frame.round() as u64)
}
pub(super) fn loop_frames(
    range: Option<(f64, f64)>,
    rate: u32,
    beats: f64,
) -> Result<Option<(u64, u64)>, String> {
    range
        .map(|(a, b)| {
            let start = beat_frame(a, rate, beats)?;
            let end = beat_frame(b, rate, beats)?;
            if start >= end {
                return Err("Loop requiere inicio < fin y al menos un frame".into());
            }
            Ok((start, end))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn paint(at: u64, frames: &mut [StereoFrame]) {
        for (i, f) in frames.iter_mut().enumerate() {
            *f = [(at + i as u64) as f32, 0.];
        }
    }
    #[test]
    fn loop_segments_and_position_are_exact_for_any_callback_partition() {
        let request = Request {
            loop_range: Some((3, 8)),
            ..Default::default()
        };
        let mut one = Cursor::new(0, 20);
        let mut many = Cursor::new(0, 20);
        let mut a = vec![[0.; 2]; 101];
        let mut b = a.clone();
        one.render(request, &mut a, paint);
        for part in b.chunks_mut(7) {
            many.render(request, part, paint);
        }
        assert_eq!(a, b);
        assert_eq!(one.frame(), many.frame());
        assert!(!one.finished());
        for (i, f) in a.iter().enumerate() {
            assert_eq!(
                f[0],
                if i < 8 {
                    i as f32
                } else {
                    (3 + (i - 8) % 5) as f32
                }
            );
        }
        // One-frame loops terminate and fill every output frame.
        let tiny = Request {
            loop_range: Some((5, 6)),
            ..Default::default()
        };
        one.render(tiny, &mut a, paint);
        assert!(a.iter().all(|f| f[0] == 5.));
    }
    #[test]
    fn seek_is_consumed_once_and_loop_changes_do_not_replay_the_last_seek() {
        let mut cursor = Cursor::new(0, 30);
        let mut output = [[0.; 2]; 4];
        let mut r = Request {
            seek: 12,
            generation: 1,
            loop_range: None,
            ..Default::default()
        };
        cursor.render(r, &mut output, paint);
        assert_eq!(output[0][0], 12.);
        cursor.render(r, &mut output, paint);
        assert_eq!(output[0][0], 16.);
        r.loop_range = Some((2, 8));
        cursor.render(r, &mut output, paint);
        assert_eq!(output[0][0], 2.);
        r.seek = 4;
        r.generation += 1;
        cursor.render(r, &mut output, paint);
        assert_eq!(output[0][0], 4.);
        assert_eq!(cursor.frame(), 2);
        r.loop_range = None;
        cursor.render(r, &mut output, paint);
        assert_eq!(output[0][0], 2.);
        r.seek = 29;
        r.generation += 1;
        cursor.render(r, &mut output, paint);
        assert_eq!(output, [[29., 0.], [0.; 2], [0.; 2], [0.; 2]]);
        assert!(cursor.finished());
        r.seek = 0;
        r.generation += 1;
        cursor.render(r, &mut output, paint);
        assert!(!cursor.finished());
        assert_eq!(cursor.frame(), 4);
    }
    #[test]
    fn musical_loop_conversion_rejects_invalid_or_subframe_ranges() {
        assert_eq!(
            loop_frames(Some((1., 3.)), 48000, 2.).unwrap(),
            Some((24000, 72000))
        );
        for range in [(2., 1.), (0., 0.), (-1., 2.), (0., f64::NAN), (0., 1e-20)] {
            assert!(loop_frames(Some(range), 48000, 2.).is_err());
        }
        assert!(beat_frame(f64::INFINITY, 48000, 2.).is_err());
    }
}
