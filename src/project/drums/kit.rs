//! Referencias de muestras, parámetros del sampler y mezcla del kit.
use super::{err, parse_id, read_bool, read_string};
use crate::{
    devices::{
        filter::FilterMode,
        sampler::{PARAMS, PlayMode, SamplerParams},
    },
    musical::{
        drums::{DrumKit, DrumKitId, DrumLayer, DrumMix, DrumPiece},
        model::InstrumentId,
        track::TrackMix,
    },
};
use loro::{LoroDoc, LoroMap};

pub(super) fn write(doc: &LoroDoc, kit: &DrumKit) -> Result<(), String> {
    let order = doc.get_movable_list("drums-pieces");
    super::reconcile_order(
        &order,
        &kit.pieces
            .iter()
            .map(|piece| u64::from(piece.id.0))
            .collect::<Vec<_>>(),
    )?;
    write_mix(&doc.get_map("drums-bus"), kit.bus)?;
    for piece in &kit.pieces {
        let map = doc.get_map(format!("drums-piece:{}", piece.id.0));
        map.insert("name", piece.name.as_str()).map_err(err)?;
        map.insert("choke_group", i64::from(piece.choke_group.unwrap_or(0)))
            .map_err(err)?;
        write_mix(&map, piece.mix)?;
        write_sampler(&map, &piece.sampler)?;
        map.insert("layer_count", piece.layers.len() as i64)
            .map_err(err)?;
        for (index, layer) in piece.layers.iter().enumerate() {
            let map = doc.get_map(format!("drums-layer:{}:{index}", piece.id.0));
            map.insert("source", layer.source.as_str()).map_err(err)?;
            map.insert("velocity_min", i64::from(layer.velocity_min))
                .map_err(err)?;
            map.insert("velocity_max", i64::from(layer.velocity_max))
                .map_err(err)?;
        }
    }
    Ok(())
}

pub(super) fn read(doc: &LoroDoc, id: DrumKitId) -> Result<DrumKit, String> {
    let order = doc.get_movable_list("drums-pieces");
    let mut pieces = Vec::new();
    for index in 0..order.len() {
        let id = super::order_id(&order, index)
            .and_then(|id| u32::try_from(id).ok())
            .map(InstrumentId)
            .ok_or("ID de pieza inválido")?;
        let map = doc.get_map(format!("drums-piece:{}", id.0));
        let choke_group = parse_id::<u16>(&map, "choke_group")?;
        let count = parse_id::<usize>(&map, "layer_count")?;
        let mut layers = Vec::new();
        for index in 0..count {
            let map = doc.get_map(format!("drums-layer:{}:{index}", id.0));
            layers.push(DrumLayer {
                source: read_string(&map, "source")?,
                velocity_min: parse_id(&map, "velocity_min")?,
                velocity_max: parse_id(&map, "velocity_max")?,
            });
        }
        pieces.push(DrumPiece {
            id,
            name: read_string(&map, "name")?,
            sampler: read_sampler(&map)?,
            layers,
            choke_group: (choke_group != 0).then_some(choke_group),
            mix: read_mix(&map)?,
        });
    }
    Ok(DrumKit {
        id,
        pieces,
        bus: read_mix(&doc.get_map("drums-bus"))?,
    })
}

fn write_mix(map: &LoroMap, mix: DrumMix) -> Result<(), String> {
    map.insert("level", i64::from(mix.fader.level))
        .map_err(err)?;
    map.insert("pan", i64::from(mix.fader.pan)).map_err(err)?;
    map.insert("muted", mix.muted).map_err(err)?;
    map.insert("solo", mix.solo).map_err(err)?;
    Ok(())
}

fn read_mix(map: &LoroMap) -> Result<DrumMix, String> {
    Ok(DrumMix {
        fader: TrackMix {
            level: parse_id(map, "level")?,
            pan: parse_id(map, "pan")?,
        },
        muted: read_bool(map, "muted")?,
        solo: read_bool(map, "solo")?,
    })
}

fn write_sampler(map: &LoroMap, params: &SamplerParams) -> Result<(), String> {
    map.insert("one_shot", matches!(params.mode, PlayMode::OneShot))
        .map_err(err)?;
    map.insert("looped", params.looped).map_err(err)?;
    map.insert("filter", params.filter).map_err(err)?;
    map.insert(
        "filter_mode",
        match params.filter_mode {
            FilterMode::LowPass => "low_pass",
            FilterMode::HighPass => "high_pass",
            FilterMode::BandPass => "band_pass",
        },
    )
    .map_err(err)?;
    map.insert("keytrack", params.keytrack).map_err(err)?;
    map.insert("reverse", params.reverse).map_err(err)?;
    map.insert("choke", params.choke).map_err(err)?;
    for (index, spec) in PARAMS.iter().enumerate() {
        map.insert(
            &format!("sample:{}", spec.key),
            f64::from(params.get(index)),
        )
        .map_err(err)?;
    }
    Ok(())
}

fn read_sampler(map: &LoroMap) -> Result<SamplerParams, String> {
    let mut params = SamplerParams::default();
    params.mode = if read_bool(map, "one_shot")? {
        PlayMode::OneShot
    } else {
        PlayMode::Classic
    };
    params.looped = read_bool(map, "looped")?;
    params.filter = read_bool(map, "filter")?;
    params.filter_mode = match read_string(map, "filter_mode")?.as_str() {
        "low_pass" => FilterMode::LowPass,
        "high_pass" => FilterMode::HighPass,
        "band_pass" => FilterMode::BandPass,
        _ => return Err("Modo de filtro de batería inválido".into()),
    };
    params.keytrack = read_bool(map, "keytrack")?;
    params.reverse = read_bool(map, "reverse")?;
    params.choke = read_bool(map, "choke")?;
    let mut values = [0.0; PARAMS.len()];
    for (index, spec) in PARAMS.iter().enumerate() {
        let value = super::super::midi::read_f64(map, &format!("sample:{}", spec.key))? as f32;
        if !value.is_finite() || value < spec.min || value > spec.max {
            return Err(format!("Parámetro de batería inválido ({})", spec.name));
        }
        params.set(index, value);
        values[index] = value;
    }
    if values
        .iter()
        .enumerate()
        .any(|(index, &value)| params.get(index) != value)
    {
        return Err("Parámetros de sampler de batería incompatibles".into());
    }
    Ok(params)
}
