//! Acciones comunes de seek/loop de la mezcla de audio, invocables desde UI y comandos.
use crate::ui::DawApp;
impl DawApp {
    pub(in crate::ui) fn seek_audio(&mut self, beat: f64) -> Result<(), String> {
        if !beat.is_finite() || beat < 0. {
            return Err("Seek requiere una posición positiva o cero en negras".into());
        }
        if let Some(player) = self.player.as_mut() {
            player.seek(beat)?;
        } else {
            self.stop_all();
        }
        if beat == 0. {
            self.timeline.go_to_start();
        } else {
            self.timeline.set_edit_beat(beat);
        }
        self.status_message = format!("Posición solicitada: {beat:.2} negras");
        Ok(())
    }
    fn set_audio_loop(&mut self, range: Option<(f64, f64)>) -> Result<(), String> {
        if let Some((start, end)) = range {
            if !start.is_finite() || !end.is_finite() || start < 0. || start >= end {
                return Err("Loop requiere 0 ≤ inicio < fin (negras)".into());
            }
        }
        if let Some(player) = self.player.as_mut() {
            player.set_loop(range)?;
        }
        self.audio_loop = range;
        self.status_message = match range {
            Some((a, b)) => format!("Loop de audio: {a:.2}–{b:.2} negras"),
            None => "Loop de audio desactivado".into(),
        };
        Ok(())
    }
    pub(in crate::ui) fn toggle_audio_loop(&mut self) {
        let range = if self.audio_loop.is_some() {
            None
        } else {
            let end = self
                .timeline
                .clip
                .iter()
                .chain(&self.timeline.other_audio_clips)
                .map(|c| c.end_beat(self.timeline.tempo.bpm()))
                .fold(0_f64, f64::max);
            Some((0., end))
        };
        if let Err(e) = self.set_audio_loop(range) {
            self.status_message = e;
        }
    }
    pub(in crate::ui) fn audio_transport_command(&mut self, command: &str) -> bool {
        let parts: Vec<_> = command.split_whitespace().collect();
        let result = match parts.as_slice() {
            ["transport.seek", beat] => beat
                .parse::<f64>()
                .map_err(|_| "Posición inválida".to_string())
                .and_then(|b| self.seek_audio(b)),
            ["transport.seek"] => self.seek_audio(self.timeline.edit_beat()),
            ["transport.loop", "off"] => self.set_audio_loop(None),
            ["transport.loop", start, end] => start
                .parse::<f64>()
                .and_then(|a| end.parse::<f64>().map(|b| (a, b)))
                .map_err(|_| "Rango inválido".to_string())
                .and_then(|range| self.set_audio_loop(Some(range))),
            ["transport.loop"] => {
                self.toggle_audio_loop();
                Ok(())
            }
            ["transport.seek", ..] | ["transport.loop", ..] => {
                Err("Uso: transport.seek <negras> · transport.loop <inicio> <fin> | off".into())
            }
            _ => return false,
        };
        if let Err(e) = result {
            self.status_message = e;
        }
        true
    }
}
