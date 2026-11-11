//! Guarda la identidad y el contenido contra los que se preparó una edición Lua.
use crate::musical::{
    clip::{ClipId, MidiClip},
    model::Pattern,
};

pub struct MidiScriptTarget {
    id: ClipId,
    pattern: Pattern,
}

impl MidiScriptTarget {
    pub fn capture(clip: &MidiClip) -> Self {
        Self {
            id: clip.id,
            pattern: clip.pattern.clone(),
        }
    }

    pub fn validate(&self, current: Option<&MidiClip>, dragging: bool) -> Result<(), String> {
        if dragging
            || !current.is_some_and(|clip| {
                clip.id == self.id && !clip.props.locked && clip.pattern == self.pattern
            })
        {
            return Err("Clip cambiado, bloqueado o en arrastre; resultado descartado".into());
        }
        Ok(())
    }
}
