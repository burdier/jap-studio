//! Persistencia opcional de Drums Latex en el documento de proyecto.
mod kit;
use super::{
    ProjectDoc,
    midi::{order_id, reconcile_order},
    track::{read_bool, read_i64, read_string},
};
use crate::musical::{
    drums::{DrumKitId, DrumPattern, DrumPatternId, DrumPlaybackSource, DrumSession, DrumSongCell},
    grid::NoteGrid,
    time::{Meter, Tick},
};
use loro::LoroMap;
use std::collections::HashSet;

const DRUMS_SCHEMA: i64 = 1;
const PATTERN_PREFIX: &str = "drums-pattern";

impl ProjectDoc {
    pub fn set_drum_session(&self, state: &DrumSession) -> Result<(), String> {
        state.validate()?;
        let meta = self.doc.get_map("drums");
        meta.insert("version", DRUMS_SCHEMA).map_err(err)?;
        meta.insert("kit_id", i64::from(state.kit.id.0))
            .map_err(err)?;
        meta.insert("active", i64::from(state.active.0))
            .map_err(err)?;
        meta.insert("next_pattern_id", i64::from(state.next_pattern_id))
            .map_err(err)?;
        meta.insert(
            "source",
            match state.source {
                DrumPlaybackSource::Pattern => "pattern",
                DrumPlaybackSource::Song => "song",
                DrumPlaybackSource::Arrangement => "arrangement",
            },
        )
        .map_err(err)?;
        kit::write(&self.doc, &state.kit)?;

        let order = self.doc.get_movable_list("drums-patterns");
        reconcile_order(
            &order,
            &state
                .patterns
                .iter()
                .map(|pattern| u64::from(pattern.id.0))
                .collect::<Vec<_>>(),
        )?;
        let muted = HashSet::new();
        for pattern in &state.patterns {
            let id = u64::from(pattern.id.0);
            let map = self.doc.get_map(format!("{PATTERN_PREFIX}:{id}"));
            map.insert("name", pattern.name.as_str()).map_err(err)?;
            map.insert("meter_numerator", i64::from(pattern.meter.numerator()))
                .map_err(err)?;
            map.insert("meter_denominator", i64::from(pattern.meter.denominator()))
                .map_err(err)?;
            map.insert("grid", i64::from(pattern.grid.denominator()))
                .map_err(err)?;
            super::pattern::write(&self.doc, PATTERN_PREFIX, id, &map, &pattern.notes, &muted)?;
        }
        meta.insert("song_count", state.song.len() as i64)
            .map_err(err)?;
        for (index, cell) in state.song.iter().enumerate() {
            let map = self.doc.get_map(format!("drums-song:{index}"));
            map.insert("tick", cell.at.0).map_err(err)?;
            map.insert("pattern", i64::from(cell.pattern.0))
                .map_err(err)?;
        }
        Ok(())
    }

    pub fn drum_session(&self) -> Result<Option<DrumSession>, String> {
        let meta = self.doc.get_map("drums");
        if meta.get("version").is_none() {
            return Ok(None);
        }
        if read_i64(&meta, "version")? != DRUMS_SCHEMA {
            return Err("Versión de Drums Latex no compatible".into());
        }
        let kit = kit::read(&self.doc, DrumKitId(parse_id(&meta, "kit_id")?))?;
        let order = self.doc.get_movable_list("drums-patterns");
        let mut patterns = Vec::new();
        let (_, session_meter) = self.timing()?;
        for index in 0..order.len() {
            let id = order_id(&order, index).ok_or("ID de patrón inválido")?;
            let map = self.doc.get_map(format!("{PATTERN_PREFIX}:{id}"));
            let (notes, _) = super::pattern::read(&self.doc, PATTERN_PREFIX, id, &map)?;
            patterns.push(DrumPattern {
                id: DrumPatternId(u32::try_from(id).map_err(|_| "ID de patrón inválido")?),
                name: read_string(&map, "name")?,
                notes,
                meter: if map.get("meter_numerator").is_some() {
                    Meter::new(
                        parse_id(&map, "meter_numerator")?,
                        parse_id(&map, "meter_denominator")?,
                    )
                    .map_err(|error| error.to_string())?
                } else {
                    session_meter
                },
                grid: if map.get("grid").is_some() {
                    NoteGrid::new(parse_id(&map, "grid")?)?
                } else {
                    NoteGrid::default()
                },
            });
        }
        let count = parse_id::<usize>(&meta, "song_count")?;
        let mut song = Vec::new();
        for index in 0..count {
            let map = self.doc.get_map(format!("drums-song:{index}"));
            song.push(DrumSongCell {
                at: Tick(read_i64(&map, "tick")?),
                pattern: DrumPatternId(parse_id(&map, "pattern")?),
            });
        }
        let session = DrumSession {
            kit,
            patterns,
            active: DrumPatternId(parse_id(&meta, "active")?),
            next_pattern_id: parse_id(&meta, "next_pattern_id")?,
            song,
            source: match read_string(&meta, "source")?.as_str() {
                "pattern" => DrumPlaybackSource::Pattern,
                "song" => DrumPlaybackSource::Song,
                "arrangement" => DrumPlaybackSource::Arrangement,
                _ => return Err("Fuente de Drums Latex inválida".into()),
            },
        };
        session.validate()?;
        Ok(Some(session))
    }
}

fn parse_id<T: TryFrom<i64>>(map: &LoroMap, key: &str) -> Result<T, String> {
    T::try_from(read_i64(map, key)?).map_err(|_| format!("Campo de batería inválido ({key})"))
}

fn err(error: loro::LoroError) -> String {
    error.to_string()
}
