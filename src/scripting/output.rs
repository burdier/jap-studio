//! Validación del resultado y expansión acotada de cortes/notas solicitados por Lua.
use crate::musical::{
    clip::MidiClip,
    model::{NoteEvent, NoteId, Pattern},
    time::Tick,
};
use mlua::Table;
use std::collections::HashSet;

pub const MAX_NOTES: usize = 16_384;

pub struct MidiEdit {
    pub pattern: Pattern,
    /// (nuevo ID, nota de origen), para cortes y notas generadas.
    descendants: Vec<(NoteId, NoteId)>,
    unselected_descendants: HashSet<NoteId>,
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
            if selected.contains(&original) && !self.unselected_descendants.contains(&new) {
                selection.push(new);
            }
        }
        clip.replace_pattern(self.pattern);
    }
}

pub(super) fn decode(notes: Table, pattern: &Pattern) -> Result<MidiEdit, String> {
    if super::arrays::length(&notes, MAX_NOTES)? != pattern.notes().len() {
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
        let count = cuts
            .as_ref()
            .map(|cuts| super::arrays::length(cuts, MAX_NOTES))
            .transpose()?
            .unwrap_or(0);
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
        unselected_descendants: HashSet::new(),
    })
}

/// Añade filas solicitadas por paquetes. Lua proporciona valores musicales y un
/// origen seleccionado; el host conserva instrumento/expresión y asigna el ID.
pub(super) fn append_generated(
    rows: Table,
    original: &Pattern,
    selected: &[NoteId],
    edit: &mut MidiEdit,
) -> Result<(), String> {
    let count = super::arrays::length(&rows, MAX_NOTES)?;
    if edit.pattern.notes().len().saturating_add(count) > MAX_NOTES {
        return Err("El resultado supera 16384 notas".into());
    }
    if count == 0 {
        return Ok(());
    }

    let mut notes = edit.pattern.notes().to_vec();
    let mut next_id = edit.pattern.next_id();
    for index in 1..=count {
        let row: Table = rows.raw_get(index).map_err(|e| e.to_string())?;
        let source_id = NoteId(row.raw_get("source_id").map_err(|e| e.to_string())?);
        let _source = original
            .notes()
            .iter()
            .find(|note| note.id == source_id)
            .ok_or("source_id debe referirse a una nota existente")?;
        if !selected.is_empty() && !selected.contains(&source_id) {
            return Err("Solo se pueden generar notas desde la selección".into());
        }

        let mut note: NoteEvent = edit
            .pattern
            .notes()
            .iter()
            .find(|note| note.id == source_id)
            .copied()
            .ok_or("No se encontró la nota de origen en el resultado")?;
        note.id = NoteId(next_id);
        next_id = next_id.checked_add(1).ok_or("No quedan IDs de nota")?;
        note.start = Tick(row.raw_get("start").map_err(|e| e.to_string())?);
        note.duration = Tick(row.raw_get("duration").map_err(|e| e.to_string())?);
        note.pitch = row.raw_get("pitch").map_err(|e| e.to_string())?;
        let end = note
            .start
            .0
            .checked_add(note.duration.0)
            .ok_or("Fin de nota inválido")?;
        if note.start.0 < 0 || note.duration.0 <= 0 || end > original.length().0 || note.pitch > 127
        {
            return Err("Posición, duración o pitch de nota generada inválida".into());
        }
        let select: Option<bool> = row.raw_get("selected").map_err(|e| e.to_string())?;
        if !select.unwrap_or(false) {
            edit.unselected_descendants.insert(note.id);
        }
        edit.descendants.push((note.id, source_id));
        notes.push(note);
    }
    edit.pattern = edit
        .pattern
        .with_notes(notes, next_id)
        .map_err(|e| e.to_string())?;
    Ok(())
}
