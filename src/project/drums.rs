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
            map.insert("swing", i64::from(pattern.swing)).map_err(err)?;
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
                swing: if map.get("swing").is_some() {
                    parse_id::<u8>(&map, "swing")?.min(DrumPattern::MAX_SWING)
                } else {
                    0
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

const HAYKIT_SCHEMA: i64 = 1;

/// Guarda un preset completo de kit de batería (`.haykit`) usando el mismo esquema validado de Loro.
pub fn save_kit_preset(
    kit: &crate::musical::drums::DrumKit,
    path: &std::path::Path,
) -> Result<(), String> {
    kit.validate()?;
    let doc = loro::LoroDoc::new();
    let meta = doc.get_map("haykit");
    meta.insert("version", HAYKIT_SCHEMA).map_err(err)?;
    meta.insert("kit_id", i64::from(kit.id.0)).map_err(err)?;
    kit::write(&doc, kit)?;
    let bytes = doc
        .export(loro::ExportMode::Snapshot)
        .map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("haykit.tmp");
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Carga y valida un preset completo de kit de batería (`.haykit`).
pub fn load_kit_preset(path: &std::path::Path) -> Result<crate::musical::drums::DrumKit, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("No se pudo leer el kit: {e}"))?;
    let doc = loro::LoroDoc::new();
    doc.import(&bytes)
        .map_err(|e| format!("Archivo de kit inválido: {e}"))?;
    let meta = doc.get_map("haykit");
    if meta.get("version").is_none() || read_i64(&meta, "version")? != HAYKIT_SCHEMA {
        return Err("Formato de preset .haykit no compatible".into());
    }
    let kit_id = DrumKitId(parse_id(&meta, "kit_id")?);
    let kit = kit::read(&doc, kit_id)?;
    kit.validate()?;
    Ok(kit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musical::{
        drums::{DrumKit, DrumPlaybackSource, DrumSongCell, OffsetMode},
        time::{PPQ, Tick},
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn drum_swing_shifts_odd_steps_and_persists_through_loro_and_haykit() {
        static ID: AtomicU64 = AtomicU64::new(1);
        let dir = std::env::temp_dir().join(format!(
            "hay-drum-kit-test-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let mut session = DrumSession::default();
        let kick = session.kit.add_sample("/tmp/kick_test.wav".into()).unwrap();
        let hat = session.kit.add_sample("/tmp/hat_test.wav".into()).unwrap();
        session.kit.pieces[1].choke_group = Some(2);
        session.kit.pieces[1].timing_offset = -12;
        session.kit.pieces[1].timing_mode = OffsetMode::Random;
        session.kit.bus.fader.level = 82;

        // Dos pasos seguidos en semicorcheas (step 0 = par en 0, step 1 = impar en 240).
        let step_ticks = session.active_pattern().grid.ticks().0;
        assert_eq!(step_ticks, PPQ / 4);
        let pat = session.active_pattern_mut();
        pat.notes
            .add_note(kick, Tick(0), Tick(step_ticks), 36, 110)
            .unwrap();
        pat.notes
            .add_note(hat, Tick(step_ticks), Tick(step_ticks), 42, 90)
            .unwrap();
        pat.swing = 50;

        // 1. Swing retrasa exactamente el paso impar (+50% de 240 = +120 ticks) y deja intacto el par.
        let playback = session.playback_pattern().unwrap().unwrap();
        assert_eq!(playback.notes()[0].start, Tick(0));
        assert_eq!(playback.notes()[1].start, Tick(step_ticks + step_ticks / 2));

        // 2. En modo Canción también aplica el swing del patrón.
        let pat_id = session.active;
        session
            .set_song_cell(
                DrumSongCell {
                    at: Tick(0),
                    pattern: pat_id,
                },
                true,
            )
            .unwrap();
        session.source = DrumPlaybackSource::Song;
        let song = session.playback_pattern().unwrap().unwrap();
        assert_eq!(song.notes()[0].start, Tick(0));
        assert_eq!(song.notes()[1].start, Tick(step_ticks + step_ticks / 2));

        // 3. Round-trip de DrumSession (incluido swing) en ProjectDoc Loro.
        let doc = ProjectDoc::new("DrumsTest").unwrap();
        doc.set_drum_session(&session).unwrap();
        let loaded = doc.drum_session().unwrap().unwrap();
        assert_eq!(loaded.patterns[0].swing, 50);
        assert_eq!(loaded, session);

        // 4. Round-trip completo de preset .haykit en disco.
        let kit_path = dir.join("boom_bap.haykit");
        save_kit_preset(&session.kit, &kit_path).unwrap();
        let restored_kit: DrumKit = load_kit_preset(&kit_path).unwrap();
        assert_eq!(restored_kit, session.kit);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
