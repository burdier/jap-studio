//! Diferencias de notas preparadas por el worker para un cambio de arreglo en Play.
use super::{ArrangementEvent, PreparedMidiArrangement};
use crate::musical::{model::NoteId, scheduler::EventKind};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq)]
struct Held {
    start: u64,
    end: u64,
    on: ArrangementEvent,
}

pub struct NoteChange {
    old: Option<Held>,
    new: Option<Held>,
}
impl NoteChange {
    /// En el límite aún se debe liberar una nota cuyo NoteOff estaba justo en ese frame.
    pub fn event(&self, at: u64, release: bool) -> Option<ArrangementEvent> {
        let old = self.old.filter(|n| n.start < at && at <= n.end);
        let new = self.new.filter(|n| n.start < at && at <= n.end);
        if let (Some(a), Some(b)) = (old, new)
            && a.on == b.on
        {
            return None;
        }
        if release {
            let mut routed = old?.on;
            routed.event.kind = EventKind::NoteOff;
            Some(routed)
        } else {
            new.filter(|n| at < n.end).map(|n| n.on)
        }
    }
}
impl PreparedMidiArrangement {
    fn held_notes(&self) -> BTreeMap<(u64, NoteId), Held> {
        let mut notes = BTreeMap::new();
        for e in &self.events {
            let routed = e.routed;
            let Some(clip) = routed.event.clip else {
                continue;
            };
            let key = (clip.value(), routed.event.note);
            match routed.event.kind {
                EventKind::NoteOn { .. } => {
                    notes.insert(
                        key,
                        Held {
                            on: routed,
                            start: e.frame,
                            end: e.frame,
                        },
                    );
                }
                EventKind::NoteOff => {
                    if let Some(note) = notes.get_mut(&key) {
                        note.end = e.frame;
                    }
                }
                _ => {}
            }
        }
        notes
    }
    pub fn note_changes(old: Option<&Self>, new: Option<&Self>) -> Vec<NoteChange> {
        if old.map(|p| &p.events) == new.map(|p| &p.events) {
            return Vec::new();
        }
        let mut next = new.map(Self::held_notes).unwrap_or_default();
        let mut changes = Vec::new();
        for (key, note) in old.map(Self::held_notes).unwrap_or_default() {
            let new = next.remove(&key);
            if new != Some(note) {
                changes.push(NoteChange {
                    old: Some(note),
                    new,
                });
            }
        }
        changes.extend(next.into_values().map(|note| NoteChange {
            old: None,
            new: Some(note),
        }));
        changes
    }
}
