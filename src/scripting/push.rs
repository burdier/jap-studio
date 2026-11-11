//! Sesión geométrica de empuje: Lua compilado una vez por gesto, sin controles ni DSP.
use crate::musical::{
    model::{NoteEvent, NoteId, Pattern},
    time::Tick,
};
use mlua::{Function, HookTriggers, Lua, LuaOptions, StdLib, Table, VmState};
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct PushEdit {
    pub delta: f64,
    pub neighbors: HashMap<NoteId, (f64, f64)>,
}

pub struct NotePush {
    lua: Lua,
    project: Function,
    deadline: Rc<Cell<Instant>>,
    notes: Vec<NoteEvent>,
    selected: HashSet<NoteId>,
    lanes: HashSet<u8>,
    neighbors: Vec<Vec<usize>>,
    loaded_pitch: Option<i32>,
    last: Option<(f64, i32, bool)>,
    result: PushEdit,
}

impl NotePush {
    pub fn new(pattern: &Pattern, selected: &[NoteId]) -> Result<Self, String> {
        if pattern.notes().len() > super::output::MAX_NOTES || selected.is_empty() {
            return Err("Empuje: selecciona notas (máximo 16384 en el clip)".into());
        }
        let lua = Lua::new_with(StdLib::MATH | StdLib::TABLE, LuaOptions::default())
            .map_err(|e| e.to_string())?;
        for key in ["dofile", "loadfile", "load", "pcall", "xpcall"] {
            lua.globals()
                .set(key, mlua::Nil)
                .map_err(|e| e.to_string())?;
        }
        lua.set_memory_limit(8 * 1024 * 1024)
            .map_err(|e| e.to_string())?;
        let deadline = Rc::new(Cell::new(Instant::now() + Duration::from_millis(12)));
        let budget = deadline.clone();
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(1000),
            move |_, _| {
                if Instant::now() >= budget.get() {
                    Err(mlua::Error::runtime(
                        "Empuje Lua excedió el presupuesto del gesto",
                    ))
                } else {
                    Ok(VmState::Continue)
                }
            },
        )
        .map_err(|e| e.to_string())?;
        let selected: HashSet<_> = selected.iter().copied().collect();
        let rows = lua
            .create_table_with_capacity(selected.len(), 0)
            .map_err(|e| e.to_string())?;
        let mut neighbors = vec![Vec::new(); 128];
        let mut lanes = HashSet::new();
        let mut count = 0;
        for (index, note) in pattern.notes().iter().enumerate() {
            if selected.contains(&note.id) {
                count += 1;
                lanes.insert(note.pitch);
                rows.raw_set(count, geometry(&lua, index, note)?)
                    .map_err(|e| e.to_string())?;
            } else {
                neighbors[usize::from(note.pitch)].push(index);
            }
        }
        deadline.set(Instant::now() + Duration::from_millis(12));
        let project = lua
            .load(include_str!("../../assets/scripts/midi/push.lua"))
            .call(rows)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            lua,
            project,
            deadline,
            notes: pattern.notes().to_vec(),
            selected,
            lanes,
            neighbors,
            loaded_pitch: None,
            last: None,
            result: PushEdit::default(),
        })
    }

    pub fn project(&mut self, delta: f64, pitch: i32, resize: bool) -> Result<&PushEdit, String> {
        if !delta.is_finite() || !(-127..=127).contains(&pitch) {
            return Err("Empuje: geometría inválida".into());
        }
        if self.last == Some((delta, pitch, resize)) {
            return Ok(&self.result);
        }
        // Solo cargar vecinas al entrar en otro tono; CC y demás filas nunca cruzan a Lua.
        let rows = if self.loaded_pitch != Some(pitch) {
            let indices: Vec<_> = self
                .lanes
                .iter()
                .filter_map(|lane| {
                    usize::try_from(i32::from(*lane) + pitch)
                        .ok()
                        .and_then(|p| self.neighbors.get(p))
                })
                .flatten()
                .copied()
                .collect();
            let rows = self
                .lua
                .create_table_with_capacity(indices.len(), 0)
                .map_err(|e| e.to_string())?;
            for (row, index) in indices.into_iter().enumerate() {
                rows.raw_set(row + 1, geometry(&self.lua, index, &self.notes[index])?)
                    .map_err(|e| e.to_string())?;
            }
            Some(rows)
        } else {
            None
        };
        self.deadline
            .set(Instant::now() + Duration::from_millis(12));
        let (allowed, changes): (f64, Table) = self
            .project
            .call((delta, pitch, resize, rows))
            .map_err(|e| e.to_string())?;
        if !allowed.is_finite()
            || allowed < delta.min(0.0)
            || allowed > delta.max(0.0)
            || changes.raw_len() > self.notes.len()
        {
            return Err("Empuje Lua devolvió un desplazamiento inválido".into());
        }
        let mut result = PushEdit {
            delta: allowed,
            ..Default::default()
        };
        for row in changes.sequence_values::<Table>() {
            let row = row.map_err(|e| e.to_string())?;
            let index: usize = row.raw_get(1).map_err(|e| e.to_string())?;
            let start: f64 = row.raw_get(2).map_err(|e| e.to_string())?;
            let end: f64 = row.raw_get(3).map_err(|e| e.to_string())?;
            let note = index
                .checked_sub(1)
                .and_then(|i| self.notes.get(i))
                .ok_or("Índice de empuje inválido")?;
            if self.selected.contains(&note.id)
                || !start.is_finite()
                || !end.is_finite()
                || start < note.start.0 as f64
                || end > (note.start.0 + note.duration.0) as f64
                || end - start < 1.0
                || result.neighbors.insert(note.id, (start, end)).is_some()
            {
                return Err("Empuje Lua devolvió un recorte inválido".into());
            }
        }
        self.last = Some((delta, pitch, resize));
        self.loaded_pitch = Some(pitch);
        self.result = result;
        Ok(&self.result)
    }

    /// Confirmación única. Conserva expresión, IDs, controles y selección del clip.
    pub fn commit(
        &mut self,
        original: &Pattern,
        delta: f64,
        pitch: i32,
        resize: bool,
    ) -> Result<Pattern, String> {
        if original.notes() != self.notes {
            return Err("El clip cambió durante el empuje".into());
        }
        self.project(delta.round(), pitch, resize)?;
        let mut notes = self.notes.clone();
        for note in &mut notes {
            if self.selected.contains(&note.id) {
                if resize {
                    note.duration.0 += self.result.delta.round() as i64;
                } else {
                    note.start.0 += self.result.delta.round() as i64;
                    note.pitch =
                        u8::try_from(i32::from(note.pitch) + pitch).map_err(|e| e.to_string())?;
                }
            } else if let Some((start, end)) = self.result.neighbors.get(&note.id) {
                note.start = Tick(start.round() as i64);
                note.duration = Tick(end.round() as i64 - note.start.0);
            }
        }
        original
            .with_notes(notes, original.next_id())
            .map_err(|e| e.to_string())
    }
}

fn geometry(lua: &Lua, index: usize, note: &NoteEvent) -> Result<Table, String> {
    let row = lua
        .create_table_with_capacity(4, 0)
        .map_err(|e| e.to_string())?;
    row.raw_set(1, index + 1).map_err(|e| e.to_string())?;
    row.raw_set(2, note.start.0).map_err(|e| e.to_string())?;
    row.raw_set(3, note.start.0 + note.duration.0)
        .map_err(|e| e.to_string())?;
    row.raw_set(4, note.pitch).map_err(|e| e.to_string())?;
    Ok(row)
}
