//! Estado editable del piano. Los motores y buffers nunca entran en el documento.
use super::{ProjectDoc, read_i64, read_string};
use crate::musical::model::InstrumentId;

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectPiano {
    pub id: InstrumentId,
    pub source: String,
    pub preset: u32,
    pub parameters: Vec<f32>,
}

impl ProjectDoc {
    pub fn set_pianos(&self, pianos: &[ProjectPiano]) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for piano in pianos {
            if !ids.insert(piano.id) || piano.id.0 == u32::MAX {
                return Err("ID de piano duplicado o agotado".into());
            }
            if piano.parameters.is_empty()
                || piano.parameters.len() > 256
                || piano.parameters.iter().any(|v| !v.is_finite())
            {
                return Err("Parámetros de piano inválidos".into());
            }
        }
        let order = self.doc.get_list("piano_ids");
        order.delete(0, order.len()).map_err(|e| e.to_string())?;
        for piano in pianos {
            order
                .push(i64::from(piano.id.0))
                .map_err(|e| e.to_string())?;
            let map = self.doc.get_map(format!("piano:{}", piano.id.0));
            map.insert("source", piano.source.clone())
                .map_err(|e| e.to_string())?;
            map.insert("preset", i64::from(piano.preset))
                .map_err(|e| e.to_string())?;
            map.insert("version", 1_i64).map_err(|e| e.to_string())?;
            map.insert("parameter_count", piano.parameters.len() as i64)
                .map_err(|e| e.to_string())?;
            for (index, value) in piano.parameters.iter().enumerate() {
                map.insert(&format!("parameter:{index}"), f64::from(*value))
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn pianos(&self) -> Result<Vec<ProjectPiano>, String> {
        let order = self.doc.get_list("piano_ids");
        let mut result = Vec::new();
        for index in 0..order.len() {
            let Some(loro::ValueOrContainer::Value(loro::LoroValue::I64(id))) = order.get(index)
            else {
                return Err("ID de piano inválido".into());
            };
            let id = InstrumentId(u32::try_from(id).map_err(|_| "ID de piano inválido")?);
            if result.iter().any(|p: &ProjectPiano| p.id == id) {
                return Err("Piano duplicado".into());
            }
            let map = self.doc.get_map(format!("piano:{}", id.0));
            if read_i64(&map, "version")? != 1 {
                return Err("Versión de piano no compatible".into());
            }
            let count = read_i64(&map, "parameter_count")?;
            if !(1..=256).contains(&count) {
                return Err("Cantidad de parámetros inválida".into());
            }
            let mut parameters = Vec::new();
            for index in 0..count {
                let Some(loro::ValueOrContainer::Value(loro::LoroValue::Double(value))) =
                    map.get(&format!("parameter:{index}"))
                else {
                    return Err("Parámetro de piano inválido".into());
                };
                let value = value as f32;
                if !value.is_finite() {
                    return Err("Parámetro de piano no finito".into());
                }
                parameters.push(value);
            }
            result.push(ProjectPiano {
                id,
                source: read_string(&map, "source")?,
                preset: u32::try_from(read_i64(&map, "preset")?).map_err(|_| "Preset inválido")?,
                parameters,
            });
        }
        Ok(result)
    }
}
