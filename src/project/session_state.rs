//! Tiempo y mezcla editables, separados por campo para fusionar cambios independientes.
use super::{ProjectDoc, order_id, read_i64, read_string};
use crate::musical::{
    time::{Meter, Tempo},
    track::{TrackId, TrackMix},
};

const DEFAULT_LEVEL: i32 = 75;
const DEFAULT_PAN: i32 = 50;

impl ProjectDoc {
    pub fn set_timing(&self, tempo: Tempo, meter: Meter) -> Result<(), String> {
        self.set_tempo(tempo)?;
        self.set_meter(meter)
    }

    pub fn set_tempo(&self, tempo: Tempo) -> Result<(), String> {
        let micro_bpm = (tempo.bpm() * 1_000_000.0).round() as i64;
        self.doc
            .get_map("project")
            .insert("tempo_micro_bpm", micro_bpm)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn set_meter(&self, meter: Meter) -> Result<(), String> {
        self.doc
            .get_map("project")
            .insert("meter", meter.label())
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn timing(&self) -> Result<(Tempo, Meter), String> {
        let meta = self.doc.get_map("project");
        let tempo = if meta.get("tempo_micro_bpm").is_some() {
            let bpm = read_i64(&meta, "tempo_micro_bpm")? as f64 / 1_000_000.0;
            Tempo::new(bpm).map_err(|error| error.to_string())?
        } else {
            Tempo::default()
        };
        let meter = if meta.get("meter").is_some() {
            let label = read_string(&meta, "meter")?;
            let (numerator, denominator) =
                label.split_once('/').ok_or("Compás de sesión inválido")?;
            let numerator = numerator
                .parse::<u8>()
                .map_err(|_| "Numerador de compás inválido")?;
            let denominator = denominator
                .parse::<u8>()
                .map_err(|_| "Denominador de compás inválido")?;
            Meter::new(numerator, denominator).map_err(|error| error.to_string())?
        } else {
            Meter::default()
        };
        Ok((tempo, meter))
    }

    pub fn set_track_mix(&self, id: TrackId, mix: TrackMix) -> Result<(), String> {
        if !(0..=100).contains(&mix.level) || !(0..=100).contains(&mix.pan) {
            return Err("Nivel o panorama fuera de rango".into());
        }
        self.require_track(id)?;
        self.set_track_level(id, mix.level)?;
        self.set_track_pan(id, mix.pan)
    }

    pub fn set_track_level(&self, id: TrackId, level: i32) -> Result<(), String> {
        if !(0..=100).contains(&level) {
            return Err("Nivel de pista fuera de rango".into());
        }
        self.require_track(id)?;
        self.doc
            .get_map(format!("track:{}", id.0))
            .insert("level", i64::from(level))
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn set_track_pan(&self, id: TrackId, pan: i32) -> Result<(), String> {
        if !(0..=100).contains(&pan) {
            return Err("Panorama de pista fuera de rango".into());
        }
        self.require_track(id)?;
        self.doc
            .get_map(format!("track:{}", id.0))
            .insert("pan", i64::from(pan))
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn track_mix(&self, id: TrackId) -> Result<TrackMix, String> {
        self.require_track(id)?;
        let map = self.doc.get_map(format!("track:{}", id.0));
        let level = if map.get("level").is_some() {
            i32::try_from(read_i64(&map, "level")?).map_err(|_| "Nivel de pista fuera de rango")?
        } else {
            DEFAULT_LEVEL
        };
        let pan = if map.get("pan").is_some() {
            i32::try_from(read_i64(&map, "pan")?).map_err(|_| "Panorama de pista fuera de rango")?
        } else {
            DEFAULT_PAN
        };
        if !(0..=100).contains(&level) || !(0..=100).contains(&pan) {
            return Err("Nivel o panorama fuera de rango".into());
        }
        Ok(TrackMix { level, pan })
    }

    fn require_track(&self, id: TrackId) -> Result<(), String> {
        let order = self.doc.get_movable_list("track_order");
        if (0..order.len()).any(|index| order_id(&order, index) == Some(id)) {
            Ok(())
        } else {
            Err("Pista inexistente en el documento".into())
        }
    }
}
