//! Puntos de control por identidad, sin reemplazar notas al cambiar una curva.
use super::{
    midi::{order_id, reconcile_order},
    track::{read_i64, read_string},
};
use crate::musical::{
    automation::{ControlLane, ControlPoint},
    model::Pattern,
    time::Tick,
};
use loro::{LoroDoc, LoroMap};

pub(super) fn write(
    doc: &LoroDoc,
    prefix: &str,
    id: u64,
    map: &LoroMap,
    pattern: &Pattern,
) -> Result<(), String> {
    let controls = pattern.controls();
    map.insert("control_next_id", controls.next_id() as i64)
        .map_err(|e| e.to_string())?;
    let order = doc.get_movable_list(format!("{prefix}-controls:{id}"));
    reconcile_order(
        &order,
        &controls.points().iter().map(|p| p.id).collect::<Vec<_>>(),
    )?;
    for point in controls.points() {
        let map = doc.get_map(format!("{prefix}-control:{id}:{}", point.id));
        map.insert("at", point.at.0).map_err(|e| e.to_string())?;
        map.insert("lane", point.lane.key())
            .map_err(|e| e.to_string())?;
        map.insert("value", i64::from(point.value))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(super) fn read(
    doc: &LoroDoc,
    prefix: &str,
    id: u64,
    map: &LoroMap,
    pattern: &mut Pattern,
) -> Result<(), String> {
    // Proyectos anteriores al soporte CC no contienen esta clave.
    if map.get("control_next_id").is_none() {
        return Ok(());
    }
    let next_id =
        u64::try_from(read_i64(map, "control_next_id")?).map_err(|_| "ID de control inválido")?;
    let order = doc.get_movable_list(format!("{prefix}-controls:{id}"));
    if order.len() > 65_536 {
        return Err("Demasiados puntos de control".into());
    }
    let mut points = Vec::with_capacity(order.len());
    for index in 0..order.len() {
        let point_id = order_id(&order, index).ok_or("ID de control inválido")?;
        let map = doc.get_map(format!("{prefix}-control:{id}:{point_id}"));
        points.push(ControlPoint {
            id: point_id,
            at: Tick(read_i64(&map, "at")?),
            lane: ControlLane::parse(&read_string(&map, "lane")?)
                .ok_or("Carril de control inválido")?,
            value: i16::try_from(read_i64(&map, "value")?)
                .map_err(|_| "Valor de control inválido")?,
        });
    }
    pattern.restore_controls(points, next_id)
}
