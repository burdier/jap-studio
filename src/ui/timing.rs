//! Edición del tempo y compás compartidos por transporte y rejillas.
use super::DawApp;
use elevate::musical::time::{Meter, Tempo};

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
                match (parts.next().and_then(|v| v.replace(',', ".").parse().ok()), parts.next()) {
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
                } else { None };
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
