//! Interpolación visual; nunca escribe posición, eventos ni tiempo del motor.
use std::time::Instant;

pub(super) struct VisualTransport {
    frame_at: Instant,
    sample_at: Instant,
    sample: Option<f64>,
    playhead: Option<f64>,
    cursor: f64,
}
impl Default for VisualTransport {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            frame_at: now,
            sample_at: now,
            sample: None,
            playhead: None,
            cursor: 0.0,
        }
    }
}
impl VisualTransport {
    pub fn advance(
        &mut self,
        raw: Option<f64>,
        cursor: f64,
        beats_per_second: f64,
        pixels_per_beat: f64,
    ) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.frame_at).as_secs_f64();
        self.frame_at = now;
        // La posición elegida es inmediata para Play; solo el trazo converge.
        let distance = cursor - self.cursor;
        let pending = distance.abs() * pixels_per_beat > 0.2;
        if pending {
            let dt = elapsed.min(1.0 / 30.0);
            self.cursor += distance * (1.0 - (-dt / 0.025).exp());
        } else {
            self.cursor = cursor;
        }
        let Some(raw) = raw.filter(|beat| beat.is_finite()) else {
            self.sample = None;
            self.playhead = None;
            return pending;
        };
        let discontinuity = self.sample.is_none_or(|previous| {
            raw < previous || raw - previous > beats_per_second * elapsed.max(0.1) + 0.25
        }) || elapsed > 0.25;
        if self.sample != Some(raw) {
            self.sample = Some(raw);
            self.sample_at = now;
        }
        // Acotado: si el audio deja de publicar frames, el dibujo también se detiene.
        let predicted =
            raw + now.duration_since(self.sample_at).as_secs_f64().min(0.08) * beats_per_second;
        self.playhead = Some(if discontinuity {
            raw
        } else {
            let advanced = self.playhead.unwrap_or(raw) + elapsed.min(0.05) * beats_per_second;
            let corrected = advanced + (predicted - advanced) * (1.0 - (-elapsed / 0.045).exp());
            corrected.clamp(raw, raw + beats_per_second * 0.08)
        });
        pending
    }
    pub fn playhead(&self, raw: Option<f64>) -> Option<f64> {
        raw.and(self.playhead).or(raw)
    }
    pub fn cursor(&self) -> f64 {
        self.cursor
    }
}

impl super::Timeline {
    pub(crate) fn advance_visual_transport(&mut self) -> bool {
        self.visual_transport.advance(
            self.playhead,
            self.edit_beat,
            self.tempo.beats_per_second(),
            self.viewport.pixels_per_beat,
        )
    }
    pub(crate) fn visual_playhead(&self) -> Option<f64> {
        self.visual_transport.playhead(self.playhead)
    }
    pub(crate) fn visual_edit_beat(&self) -> f64 {
        self.visual_transport.cursor()
    }
}
