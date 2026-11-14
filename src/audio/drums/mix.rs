//! Percusión provisional en el reloj y la salida del mezclador común.
use super::voice::DrumKit;
use elevate::musical::{model::Pattern, scheduler::PreparedPattern, time::TimeBase};
use elevate::musical::{scheduler::EventKind, time::FrameWindow};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

pub(crate) struct DrumInput {
    pub pattern: Pattern,
    pub gain: f32,
}

pub(crate) struct PreparedDrums {
    plan: PreparedPattern,
    kit: DrumKit,
    dt: f64,
    pub control: Arc<AtomicU32>,
    last_end: Option<u64>,
}

impl PreparedDrums {
    pub fn prepare(input: DrumInput, rate: u32, bpm: f64) -> Result<Self, String> {
        let base = TimeBase::new(rate, bpm).map_err(|e| e.to_string())?;
        let plan = PreparedPattern::compile(&input.pattern, base).map_err(|e| e.to_string())?;
        Ok(Self {
            plan,
            kit: DrumKit::new(),
            dt: 1.0 / rate as f64,
            control: Arc::new(AtomicU32::new(input.gain.to_bits())),
            last_end: None,
        })
    }

    pub fn render(&mut self, at: u64, output: &mut [[f32; 2]]) {
        if self.last_end.is_some_and(|end| end != at) {
            self.kit = DrumKit::new();
        }
        for (index, block) in output.chunks_mut(256).enumerate() {
            let mut scratch = [[0.0; 2]; 256];
            render_window(
                &self.plan,
                &mut self.kit,
                &mut scratch[..block.len()],
                at.saturating_add((index * 256) as u64),
                self.plan.length_frames(),
                self.dt,
            );
            let gain = f32::from_bits(self.control.load(Ordering::Relaxed));
            for (sum, drum) in block.iter_mut().zip(&scratch) {
                sum[0] += drum[0] * gain;
                sum[1] += drum[1] * gain;
            }
        }
        self.last_end = Some(at.saturating_add(output.len() as u64));
    }
}

/// Los NoteOff de la percusión se ignoran: cada voz tiene su propia envolvente.
fn render_window(
    plan: &PreparedPattern,
    kit: &mut DrumKit,
    frames: &mut [[f32; 2]],
    start_frame: u64,
    loop_frames: u64,
    dt: f64,
) {
    let mut local = start_frame % loop_frames;
    let mut written = 0;
    let mut remaining = frames.len();
    while remaining > 0 {
        let segment = (loop_frames - local).min(remaining as u64) as usize;
        let Ok(window) = FrameWindow::new(local, segment as u32) else {
            return;
        };
        let mut cursor = 0;
        for event in plan.events_in(window) {
            let offset = event.sample_offset as usize;
            if offset > cursor {
                kit.render(&mut frames[written + cursor..written + offset], dt);
                cursor = offset;
            }
            if let EventKind::NoteOn { velocity } = event.kind
                && event.expression.plays(
                    event.note,
                    start_frame.saturating_add(written as u64) / loop_frames,
                )
            {
                kit.trigger(event.instrument, velocity, event.note.0);
            }
        }
        if cursor < segment {
            kit.render(&mut frames[written + cursor..written + segment], dt);
        }
        written += segment;
        remaining -= segment;
        local = (local + segment as u64) % loop_frames;
    }
}
