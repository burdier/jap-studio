//! Edición del tempo y compás compartidos por transporte y rejillas.
use super::DawApp;
use elevate::musical::time::{DEFAULT_BPM, Meter, Tempo};
use gpui::{Context, MouseMoveEvent, Pixels, Window};

const STUDIO_METERS: [(u8, u8); 7] = [(4, 4), (3, 4), (2, 4), (6, 8), (5, 4), (7, 8), (12, 8)];

#[derive(Clone, Copy, Debug)]
pub(crate) struct TempoDrag {
    pub start_y: Pixels,
    pub initial_bpm: f64,
    pub moved: bool,
}

impl DawApp {
    pub(super) fn set_tempo(&mut self, bpm: f64) {
        if self.midi_capture.is_recording() {
            self.status_message = "Detén la grabación MIDI antes de cambiar el tempo".into();
            return;
        }
        match Tempo::new(bpm) {
            Ok(tempo) => {
                self.stop_all();
                self.timeline.tempo = tempo;
                self.status_message = format!("Tempo: {bpm:.2} BPM");
            }
            Err(error) => self.status_message = error.to_string(),
        }
    }

    pub(super) fn step_tempo(&mut self, delta: f64) {
        let next = ((self.timeline.tempo.bpm() + delta).clamp(1.0, 1000.0) * 100.0).round() / 100.0;
        self.set_tempo(next);
    }

    pub(super) fn reset_tempo(&mut self) {
        self.set_tempo(DEFAULT_BPM);
    }

    pub(super) fn begin_tempo_drag(&mut self, start_y: Pixels) {
        self.timeline.tempo_drag = Some(TempoDrag {
            start_y,
            initial_bpm: self.timeline.tempo.bpm(),
            moved: false,
        });
    }

    pub(super) fn update_tempo_drag(&mut self, event: &MouseMoveEvent) -> bool {
        let Some(mut drag) = self.timeline.tempo_drag else {
            return false;
        };
        if !event.dragging() {
            self.timeline.tempo_drag = None;
            return true;
        }
        let dy = f32::from(drag.start_y - event.position.y);
        if !drag.moved && dy.abs() < 3.0 {
            return false;
        }
        drag.moved = true;
        self.timeline.tempo_drag = Some(drag);
        let delta = if event.modifiers.shift {
            f64::from(dy) * 0.02
        } else {
            f64::from((dy / 4.0).round())
        };
        let next = ((drag.initial_bpm + delta).clamp(1.0, 1000.0) * 100.0).round() / 100.0;
        if (next - self.timeline.tempo.bpm()).abs() >= 0.005 {
            self.set_tempo(next);
        }
        true
    }

    pub(super) fn end_tempo_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.timeline.tempo_drag.take() else {
            return false;
        };
        if !drag.moved {
            self.open_tempo_form(window, cx);
        }
        true
    }

    pub(super) fn open_tempo_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = elevate::catalog::find("action.tempo.set")
            && let Some(spec) = entry.action.parameters()
        {
            self.open_action_form(entry, spec, window, cx);
        }
    }

    pub(super) fn open_meter_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = elevate::catalog::find("action.meter.set")
            && let Some(spec) = entry.action.parameters()
        {
            self.open_action_form(entry, spec, window, cx);
        }
    }

    pub(super) fn cycle_meter(&mut self, forward: bool) {
        let current = (
            self.timeline.meter.numerator(),
            self.timeline.meter.denominator(),
        );
        let idx = STUDIO_METERS
            .iter()
            .position(|&pair| pair == current)
            .unwrap_or(0);
        let next_idx = if forward {
            (idx + 1) % STUDIO_METERS.len()
        } else {
            (idx + STUDIO_METERS.len() - 1) % STUDIO_METERS.len()
        };
        let (n, d) = STUDIO_METERS[next_idx];
        self.set_meter(n, d);
    }

    pub(super) fn set_meter(&mut self, numerator: u8, denominator: u8) {
        if self.midi_capture.is_recording() {
            self.status_message = "Detén la grabación MIDI antes de cambiar el compás".into();
            return;
        }
        match Meter::new(numerator, denominator) {
            Ok(meter) => {
                self.timeline.meter = meter;
                self.status_message = format!("Compás: {}", meter.label());
            }
            Err(error) => self.status_message = error.to_string(),
        }
    }

    pub(super) fn execute_timing_command(&mut self, cmd: &str) -> bool {
        let mut parts = cmd.split_whitespace();
        match parts.next().map(str::to_ascii_lowercase).as_deref() {
            Some("tempo" | "tempo.set") => {
                match (
                    parts.next().and_then(|v| v.replace(',', ".").parse().ok()),
                    parts.next(),
                ) {
                    (Some(bpm), None) => self.set_tempo(bpm),
                    _ => self.status_message = "Uso: tempo 120".into(),
                }
                true
            }
            Some("meter" | "meter.set") => {
                let first = parts.next();
                let pair = if let Some((n, d)) = first.and_then(|v| v.split_once('/')) {
                    Some((n.to_owned(), d.to_owned()))
                } else if let Some(n) = first {
                    parts.next().map(|d| (n.to_owned(), d.to_owned()))
                } else {
                    None
                };
                match (pair, parts.next()) {
                    (Some((n, d)), None) => match (n.parse(), d.parse()) {
                        (Ok(n), Ok(d)) => self.set_meter(n, d),
                        _ => self.status_message = "Uso: meter 6/8".into(),
                    },
                    _ => self.status_message = "Uso: meter 6/8".into(),
                }
                true
            }
            _ => false,
        }
    }
}
