//! Traducción del reloj del motor a los campos válidos de VST3 ProcessContext.
use elevate::musical::time::{Meter, Tempo};
use vst3::Steinberg::Vst::{ProcessContext, ProcessContext_::StatesAndFlags_::*};

#[derive(Clone, Copy)]
pub(crate) struct TransportContext {
    pub frame: u64,
    pub rate: u32,
    pub bpm: f64,
    pub meter: Meter,
    pub loop_range: Option<(u64, u64)>,
    pub playing: bool,
}

impl TransportContext {
    pub fn new(
        frame: u64,
        rate: u32,
        bpm: f64,
        meter: Meter,
        loop_range: Option<(u64, u64)>,
    ) -> Result<Self, String> {
        if rate == 0 || Tempo::new(bpm).is_err() {
            return Err("Contexto VST3: frecuencia o tempo inválido".into());
        }
        if loop_range.is_some_and(|(start, end)| start >= end) {
            return Err("Contexto VST3: loop inválido".into());
        }
        Ok(Self {
            frame,
            rate,
            bpm,
            meter,
            loop_range,
            playing: true,
        })
    }

    pub fn offset(self, frames: usize) -> Self {
        Self {
            frame: if self.playing {
                self.frame.saturating_add(frames as u64)
            } else {
                self.frame
            },
            ..self
        }
    }

    pub fn vst(self) -> ProcessContext {
        let mut context: ProcessContext = unsafe { std::mem::zeroed() };
        context.sampleRate = f64::from(self.rate);
        context.projectTimeSamples = self.frame.min(i64::MAX as u64) as i64;
        let quarter = |frame: u64| frame as f64 * self.bpm / (60.0 * f64::from(self.rate));
        let beat = quarter(self.frame);
        context.projectTimeMusic = beat;
        context.barPositionMusic =
            (beat / self.meter.beats_per_bar()).floor() * self.meter.beats_per_bar();
        context.tempo = self.bpm;
        context.timeSigNumerator = i32::from(self.meter.numerator());
        context.timeSigDenominator = i32::from(self.meter.denominator());
        context.state =
            (kProjectTimeMusicValid | kBarPositionValid | kTempoValid | kTimeSigValid) as u32;
        if self.playing {
            context.state |= kPlaying as u32;
        }
        if let Some((start, end)) = self.loop_range {
            context.cycleStartMusic = quarter(start);
            context.cycleEndMusic = quarter(end);
            context.state |= (kCycleActive | kCycleValid) as u32;
        }
        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_context_marks_only_known_fields_and_tracks_seek_loop() {
        let meter = Meter::new(3, 4).unwrap();
        let at =
            TransportContext::new(96_000, 48_000, 120., meter, Some((24_000, 144_000))).unwrap();
        let context = at.vst();
        assert_eq!(context.projectTimeSamples, 96_000);
        assert_eq!(context.projectTimeMusic, 4.);
        assert_eq!(context.barPositionMusic, 3.);
        assert_eq!((context.cycleStartMusic, context.cycleEndMusic), (1., 6.));
        assert_eq!(
            (context.timeSigNumerator, context.timeSigDenominator),
            (3, 4)
        );
        assert_eq!(
            context.state & (kContTimeValid | kSystemTimeValid | kRecording | kSmpteValid) as u32,
            0
        );
        assert_ne!(
            context.state & (kPlaying | kTempoValid | kCycleValid) as u32,
            0
        );
        assert_eq!(at.offset(256).vst().projectTimeSamples, 96_256);
        assert_eq!(
            TransportContext::new(0, 48_000, 120., meter, None)
                .unwrap()
                .vst()
                .state
                & kCycleActive as u32,
            0
        );
    }
}
