//! Validación del resultado y expansión acotada de cortes solicitados por Lua.
use crate::musical::{
    clip::MidiClip,
    model::{NoteId, Pattern},
    time::Tick,
};
use mlua::Table;

pub const MAX_NOTES: usize = 16_384;

pub struct MidiEdit {
    pub pattern: Pattern,
    /// (nuevo ID, ID original); el primer segmento conserva el ID original.
    descendants: Vec<(NoteId, NoteId)>,
}

impl MidiEdit {
    /// Reincorpora notas omitidas sin cambiar el orden de notas/acordes simultáneos.
    /// Iterar en el orden original conserva también la estabilidad de un no-op.
    pub(super) fn restore_context(&mut self, original: &Pattern) -> Result<(), String> {
        let len = original.notes().len() + self.descendants.len();
        if len > MAX_NOTES {
            return Err("El resultado supera 16384 notas".into());
        }
        let edited: std::collections::HashMap<_, _> =
            self.pattern.notes().iter().map(|n| (n.id, *n)).collect();
        let mut children: std::collections::HashMap<NoteId, Vec<NoteId>> = Default::default();
        for (new, parent) in &self.descendants {
            children.entry(*parent).or_default().push(*new);
        }
        let mut notes = Vec::with_capacity(len);
        for note in original.notes() {
            notes.push(edited.get(&note.id).copied().unwrap_or(*note));
            if let Some(ids) = children.get(&note.id) {
                notes.extend(ids.iter().map(|id| edited[id]));
            }
        }
        self.pattern = original
            .with_notes(notes, self.pattern.next_id())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    /// El host comprueba identidad y snapshot antes de aplicar. El silencio actual
    /// se hereda sin pisar un cambio de mute ocurrido mientras corría el worker.
    pub fn apply(self, clip: &mut MidiClip, selection: &mut Vec<NoteId>) {
        let selected: std::collections::HashSet<_> = selection.iter().copied().collect();
        for (new, original) in self.descendants {
            if clip.muted_notes.contains(&original) {
                clip.muted_notes.insert(new);
            }
            if selected.contains(&original) {
                selection.push(new);
            }
        }
        clip.pattern = self.pattern;
    }
}

pub(super) fn decode(notes: Table, pattern: &Pattern) -> Result<MidiEdit, String> {
    if notes.raw_len() != pattern.notes().len() {
        return Err("Usa note.cuts para subdividir; no añadas ni quites filas de notes".into());
    }
    let mut changed = Vec::with_capacity(pattern.notes().len());
    let mut descendants = Vec::new();
    let mut next_id = pattern.next_id();
    for (index, original) in pattern.notes().iter().enumerate() {
        let row: Table = notes.raw_get(index + 1).map_err(|e| e.to_string())?;
        let id: u64 = row.raw_get("id").map_err(|e| e.to_string())?;
        if id != original.id.0 {
            return Err("El script no puede cambiar la identidad de una nota".into());
        }
        let mut note = *original;
        note.start = Tick(row.raw_get("start").map_err(|e| e.to_string())?);
        note.duration = Tick(row.raw_get("duration").map_err(|e| e.to_string())?);
        note.pitch = row.raw_get("pitch").map_err(|e| e.to_string())?;
        note.velocity = row.raw_get("velocity").map_err(|e| e.to_string())?;
        note.expression.release_velocity = row
            .raw_get::<Option<u8>>("release_velocity")
            .map_err(|e| e.to_string())?
            .unwrap_or(original.expression.release_velocity);
        note.expression.pan = row
            .raw_get::<Option<i8>>("pan")
            .map_err(|e| e.to_string())?
            .unwrap_or(original.expression.pan);
        note.expression.probability = row
            .raw_get::<Option<u8>>("probability")
            .map_err(|e| e.to_string())?
            .unwrap_or(original.expression.probability);
        let end = note
            .start
            .0
            .checked_add(note.duration.0)
            .ok_or("Fin de nota inválido")?;
        if note.start.0 < 0 || note.duration.0 <= 0 || end > pattern.length().0 {
            return Err("Posición o duración de nota inválida".into());
        }
        let cuts: Option<Table> = row.raw_get("cuts").map_err(|e| e.to_string())?;
        let count = cuts.as_ref().map_or(0, Table::raw_len);
        if count >= MAX_NOTES || changed.len() + count + 1 > MAX_NOTES {
            return Err("El resultado supera 16384 notas".into());
        }
        if let Some(cuts) = cuts {
            for index in 1..=count {
                let cut: i64 = cuts.raw_get(index).map_err(|e| e.to_string())?;
                if cut <= note.start.0 || cut >= end {
                    return Err("Los cortes deben ser crecientes e interiores a la nota".into());
                }
                note.duration = Tick(cut - note.start.0);
                changed.push(note);
                let id = NoteId(next_id);
                next_id = next_id.checked_add(1).ok_or("No quedan IDs de nota")?;
                descendants.push((id, original.id));
                note.id = id;
                note.start = Tick(cut);
            }
        }
        note.duration = Tick(end - note.start.0);
        changed.push(note);
    }
    let pattern = pattern
        .with_notes(changed, next_id)
        .map_err(|e| e.to_string())?;
    Ok(MidiEdit {
        pattern,
        descendants,
    })
}
