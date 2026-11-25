//! Herramientas de notas integradas: mismo Lua, entrada reducida y controles intactos.
use super::{
    MidiEdit,
    actions::{MidiOptions, MidiTool},
    edit_midi,
    output::MAX_NOTES,
};
use crate::musical::model::{NoteId, Pattern};
use std::collections::HashSet;

/// Los scripts integrados solo editan notas seleccionadas y no consultan controles.
/// Los scripts de archivo conservan la API completa mediante `edit_midi`.
pub fn edit_builtin(
    tool: MidiTool,
    pattern: &Pattern,
    selected: &[NoteId],
    options: MidiOptions,
) -> Result<MidiEdit, String> {
    if pattern.notes().len() > MAX_NOTES {
        return Err("Demasiadas notas".into());
    }
    let ids: HashSet<_> = selected.iter().copied().collect();
    let affected = |id: &NoteId| ids.is_empty() || ids.contains(id);
    let notes = pattern
        .notes()
        .iter()
        .filter(|n| affected(&n.id))
        .cloned()
        .collect();
    // Mantener el contador del patrón completo: los cortes nunca reutilizan IDs omitidos.
    let input = Pattern::restore_with_next_id(pattern.length(), notes, pattern.next_id())
        .map_err(|e| e.to_string())?;
    // Sin controles y sin notas omitidas no hay contexto que reconstruir.
    if input.notes().len() == pattern.notes().len() && pattern.controls().points().is_empty() {
        return edit_midi(tool.source(), pattern, &[], options);
    }
    let mut edit = edit_midi(tool.source(), &input, &[], options)?;
    edit.restore_context(pattern)?;
    Ok(edit)
}

/// Solo el script conocido de subdivisión puede ejecutarse dentro del evento UI.
/// El coste se acota ANTES de Lua por tamaño de entrada, controles y cortes producidos.
/// No aplica esta vía rápida a código de usuario ni a otras herramientas.
pub fn inline_subdivision(pattern: &Pattern, selected: &[NoteId], options: MidiOptions) -> bool {
    if selected.is_empty()
        || selected.len() > 16
        || pattern.notes().len() > 4096
        || pattern.controls().points().len() > 8192
    {
        return false;
    }
    let step = options.grid.ticks().0;
    let mut segments = 0;
    for note in pattern
        .notes()
        .iter()
        .filter(|note| selected.contains(&note.id))
    {
        let end = note.start.0 + note.duration.0;
        // Cantidad de fronteras interiores. No iterar sobre todas ellas para decidir.
        segments += (end - 1) / step - note.start.0 / step + 1;
        if segments > 128 {
            return false;
        }
    }
    segments > 0
}
