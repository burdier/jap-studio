//! Eje temporal común para bloques del Arrangement y notas del piano roll.
//! Solo transforma beats y píxeles; no conoce clips, notas ni transporte.

#[derive(Clone, Copy)]
pub(crate) struct TimeGrid {
    pub first_beat: f64,
    pub pixels_per_beat: f64,
}

#[derive(Clone, Copy)]
pub(crate) struct TimeSpan {
    pub start: f64,
    pub end: f64,
}

impl TimeSpan {
    pub fn length(self) -> f64 {
        self.end - self.start
    }
}

impl TimeGrid {
    /// Compases visibles, incluido el que queda parcialmente a la izquierda.
    /// El número absoluto conserva la alternancia al desplazar o hacer zoom.
    pub fn visible_bars(self, width: f64, beats_per_bar: f64) -> std::ops::Range<u64> {
        if !width.is_finite()
            || width <= 0.0
            || !beats_per_bar.is_finite()
            || beats_per_bar <= 0.0
            || !self.pixels_per_beat.is_finite()
            || self.pixels_per_beat <= 0.0
            || !self.first_beat.is_finite()
        {
            return 0..0;
        }
        let first = (self.first_beat.max(0.0) / beats_per_bar).floor() as u64;
        let end = (self.beat_at(width) / beats_per_bar).ceil() as u64;
        first..end.min(first.saturating_add(4096))
    }

    pub fn beat_at(self, x: f64) -> f64 {
        self.first_beat + x / self.pixels_per_beat
    }

    pub fn x_at(self, beat: f64) -> f64 {
        (beat - self.first_beat) * self.pixels_per_beat
    }

    pub fn snap(self, beat: f64, step: f64) -> f64 {
        (beat / step).round() * step
    }

    pub fn edge_beats(self, width_pixels: f64, duration: f64) -> f64 {
        (width_pixels / self.pixels_per_beat).min(duration / 3.0)
    }

    pub fn interval(self, start: f64, end: f64) -> (f64, f64) {
        (self.x_at(start), self.x_at(end))
    }

    /// Desplaza un bloque completo, conservando su duración y su posición relativa al snap.
    pub fn move_span(
        self,
        span: TimeSpan,
        delta: f64,
        min_start: f64,
        max_end: f64,
        step: f64,
    ) -> TimeSpan {
        let delta = if step > 0.0 {
            self.snap(delta, step)
        } else {
            delta
        };
        let shift = delta.clamp(min_start - span.start, max_end - span.end);
        TimeSpan {
            start: span.start + shift,
            end: span.end + shift,
        }
    }

    /// Mismo cálculo de borde izquierdo para recortar un clip o redimensionar una nota.
    pub fn resize_start(
        self,
        span: TimeSpan,
        delta: f64,
        min_start: f64,
        min_length: f64,
        step: f64,
    ) -> TimeSpan {
        TimeSpan {
            start: self
                .snap(span.start + delta, step)
                .clamp(min_start, (span.end - min_length).max(min_start)),
            end: span.end,
        }
    }

    /// Mismo cálculo de borde derecho para recortar un clip o redimensionar una nota.
    pub fn resize_end(
        self,
        span: TimeSpan,
        delta: f64,
        max_end: f64,
        min_length: f64,
        step: f64,
    ) -> TimeSpan {
        let end = self.snap(span.end + delta, step).clamp(
            span.start + min_length,
            max_end.max(span.start + min_length),
        );
        TimeSpan {
            start: span.start,
            end,
        }
    }

    pub fn grid_step(self) -> f64 {
        if self.pixels_per_beat >= 48.0 {
            0.25
        } else if self.pixels_per_beat >= 24.0 {
            0.5
        } else if self.pixels_per_beat >= 12.0 {
            1.0
        } else {
            4.0
        }
    }
}
