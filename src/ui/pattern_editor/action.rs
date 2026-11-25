//! Acciones de la superficie de batería. Pasos, Canción y roll editan la sesión compartida.
use super::PatternEditor;
use elevate::musical::{
    drums::{DrumPatternId, DrumPlaybackSource, DrumSongCell},
    grid::NoteGrid,
    model::{EditError, InstrumentId, NoteEvent, NoteId},
    time::{Meter, Tick},
};

#[derive(Clone)]
pub(in crate::ui) enum PatternAction {
    Toggle {
        lane: usize,
        step: usize,
    },
    SplitStep {
        lane: usize,
        step: usize,
    },
    Velocity {
        lane: usize,
        step: usize,
        delta: i16,
    },
    Select(DrumPatternId),
    NewPattern,
    Grid(NoteGrid),
    Bars(i64),
    Duplicate,
    SongCell(DrumSongCell),
    InsertNote {
        piece: InstrumentId,
        at: Tick,
        duration: Tick,
        pitch: u8,
    },
    MoveNote(NoteEvent),
    DeleteNote(NoteId),
    NoteVelocity {
        id: NoteId,
        value: u8,
    },
    SampleParam {
        piece: InstrumentId,
        param: usize,
        value: f32,
    },
    Filter(InstrumentId),
    FullSample(InstrumentId),
    RetriggerChoke(InstrumentId),
    Choke {
        piece: InstrumentId,
        delta: i32,
    },
    AddPiece(String),
    AddSample {
        piece: InstrumentId,
        path: String,
    },
    ReplaceSample {
        piece: InstrumentId,
        layer: usize,
        previous: String,
        path: String,
    },
    RemoveSample {
        piece: InstrumentId,
        layer: usize,
    },
    LayerOffset {
        piece: InstrumentId,
        layer: usize,
        value: u16,
    },
    CycleLayerOffsetMode {
        piece: InstrumentId,
        layer: usize,
    },
    PieceTimingOffset {
        piece: InstrumentId,
        value: i16,
    },
    CyclePieceTimingMode {
        piece: InstrumentId,
    },
    LayerVelocity {
        piece: InstrumentId,
        layer: usize,
        upper: bool,
        delta: i16,
    },
    UseSource(DrumPlaybackSource),
    FillLane {
        lane: usize,
        every: usize,
    },
    ClearLane(usize),
    InvertLane(usize),
    ShiftLane {
        lane: usize,
        steps: i64,
    },
    CopyLaneMidi(usize),
    PasteLaneMidi(usize),
    CloneLane {
        lane: usize,
        keep_layers: bool,
        copy_midi: bool,
    },
}

impl PatternEditor {
    #[cfg(feature = "p2p")]
    pub(super) fn action_step(
        &self,
        action: &PatternAction,
    ) -> Option<crate::collaboration::live::DrumStepPresence> {
        let (piece, at, pitch) = match action {
            PatternAction::Toggle { lane, step }
            | PatternAction::SplitStep { lane, step }
            | PatternAction::Velocity { lane, step, .. } => {
                let piece = self.session.kit.pieces.get(*lane)?;
                if *step >= self.step_count() {
                    return None;
                }
                (
                    piece.id,
                    Tick(*step as i64 * self.ticks_per_step()),
                    piece.base_note(),
                )
            }
            PatternAction::InsertNote {
                piece, at, pitch, ..
            } => (*piece, *at, *pitch),
            PatternAction::MoveNote(note) => (note.instrument, note.start, note.pitch),
            PatternAction::DeleteNote(id) | PatternAction::NoteVelocity { id, .. } => {
                let note = self.pattern().notes().iter().find(|n| n.id == *id)?;
                (note.instrument, note.start, note.pitch)
            }
            _ => return None,
        };
        Some(crate::collaboration::live::DrumStepPresence {
            pattern: self.session.active.0,
            piece: piece.0,
            tick: at.0,
            pitch,
        })
    }

    fn toggle(&mut self, lane: usize, step: usize) -> Result<&'static str, EditError> {
        let piece = self
            .session
            .kit
            .pieces
            .get(lane)
            .ok_or(EditError::InvalidNote)?;
        if step >= self.step_count() {
            return Err(EditError::InvalidNote);
        }
        let id = piece.id;
        let pitch = piece.base_note();
        let step_ticks = self.ticks_per_step();
        let start = Tick(step as i64 * step_ticks);
        let step_notes: Vec<_> = self.step_notes(lane, step).cloned().collect();
        let pattern = &mut self.session.active_pattern_mut().notes;
        let step_span = step_ticks.min(pattern.length().0 - start.0);
        if step_notes.is_empty() {
            pattern.add_note(id, start, Tick(step_span), pitch, 100)?;
            Ok("Nota añadida · velocity 100")
        } else if step_notes.len() == 1 {
            for note in step_notes {
                pattern.remove_note(note.id)?;
            }
            Ok("Paso borrado")
        } else {
            // Retrocede la subdivisión del roll (8 → 4 → 2 → 1) antes de borrar.
            let velocity = step_notes[0].velocity;
            let note_pitch = step_notes[0].pitch;
            let next_count = (step_notes.len() / 2).max(1);
            for note in step_notes {
                pattern.remove_note(note.id)?;
            }
            let slice = (step_span / next_count as i64).max(1);
            for idx in 0..next_count {
                let at = Tick(start.0 + idx as i64 * slice);
                let dur = Tick(slice.min(pattern.length().0 - at.0).max(1));
                pattern.add_note(id, at, dur, note_pitch, velocity)?;
            }
            Ok("Subdivisión reducida")
        }
    }

    fn split_step(&mut self, lane: usize, step: usize) -> Result<&'static str, String> {
        const MAX_HITS: usize = 8;
        let notes: Vec<_> = self.step_notes(lane, step).collect();
        let mut pending = notes.as_slice();
        let end_of_step = (step.min(self.step_count()) as i64 + 1) * self.ticks_per_step();
        let mut cuts = Vec::new();
        // Todas las notas simultáneas comparten la misma frontera del siguiente golpe.
        let mut hits = 0;
        let mut added_hits = 0;
        while let Some(first) = pending.first() {
            let start = first.start.0;
            let next = pending.partition_point(|note| note.start.0 == start);
            let (simultaneous, rest) = pending.split_at(next);
            let end = rest.first().map_or(end_of_step, |note| note.start.0);
            let span = simultaneous
                .iter()
                .map(|note| note.duration.0)
                .fold(end - start, i64::min);
            hits += 1;
            if span >= 2 {
                let at = Tick(start + span / 2);
                cuts.extend(simultaneous.iter().map(|note| (note.id, at)));
                added_hits += 1;
            }
            pending = rest;
        }
        if hits + added_hits > MAX_HITS {
            return Err("Este paso admite un roll de hasta 8 golpes".into());
        }
        if cuts.is_empty() {
            return Err("No hay un golpe divisible en este paso".into());
        }
        let pattern = &mut self.session.active_pattern_mut().notes;
        let mut updated = pattern.clone();
        for (id, at) in cuts {
            updated
                .split_note(id, at)
                .map_err(|error| error.to_string())?;
        }
        *pattern = updated;
        Ok("Golpe subdividido")
    }

    pub(in crate::ui) fn apply(
        &mut self,
        action: PatternAction,
        meter: Meter,
    ) -> Result<&'static str, String> {
        match action {
            PatternAction::Toggle { lane, step } => {
                self.toggle(lane, step).map_err(|error| error.to_string())
            }
            PatternAction::SplitStep { lane, step } => self.split_step(lane, step),
            PatternAction::Velocity { lane, step, delta } => {
                let notes: Vec<_> = self
                    .step_notes(lane, step)
                    .map(|note| (note.id, note.velocity))
                    .collect();
                if notes.is_empty() {
                    return Err(EditError::UnknownNote.to_string());
                }
                let pattern = &mut self.session.active_pattern_mut().notes;
                for (id, velocity) in notes {
                    let velocity = (i32::from(velocity) + i32::from(delta)).clamp(1, 127) as u8;
                    pattern
                        .set_velocity(id, velocity)
                        .map_err(|error| error.to_string())?;
                }
                Ok("Velocity actualizada")
            }
            PatternAction::Select(id) => {
                self.session.select(id)?;
                Ok("Patrón seleccionado")
            }
            PatternAction::NewPattern => {
                self.session.set_active_meter(meter);
                self.session.new_pattern()?;
                self.cursor_step = 0;
                Ok("Patrón vacío creado")
            }
            PatternAction::Grid(grid) => {
                let cursor = self.cursor_step as i64 * self.ticks_per_step();
                self.session.active_pattern_mut().grid = grid;
                self.cursor_step = (cursor / grid.ticks().0) as usize;
                self.roll.snap = grid;
                Ok("Subdivisión actualizada")
            }
            PatternAction::Bars(delta) => {
                self.session.set_active_meter(meter);
                let pattern = self.session.active_pattern();
                let length = pattern.notes.length().0 + delta * meter.ticks_per_bar();
                if length <= 0 {
                    return Err("La duración del patrón debe ser mayor que cero".into());
                }
                self.session.resize_active(Tick(length))?;
                Ok("Duración del patrón actualizada")
            }
            PatternAction::Duplicate => {
                self.session.set_active_meter(meter);
                self.session.duplicate_active()?;
                Ok("Patrón duplicado")
            }
            PatternAction::SongCell(cell) => {
                let present = !self.session.song.contains(&cell);
                self.session.set_song_cell(cell, present)?;
                if present {
                    self.session.select(cell.pattern)?;
                }
                Ok(if present {
                    "Patrón añadido a Canción"
                } else {
                    "Patrón retirado de Canción"
                })
            }
            PatternAction::InsertNote {
                piece,
                at,
                duration,
                pitch,
            } => {
                let notes = &mut self.session.active_pattern_mut().notes;
                let id = notes
                    .add_note(piece, at, duration, pitch, 100)
                    .map_err(|error| error.to_string())?;
                self.roll.selected = Some(id);
                Ok("Nota insertada")
            }
            PatternAction::MoveNote(note) => {
                let pattern = &mut self.session.active_pattern_mut().notes;
                let mut notes = pattern.notes().to_vec();
                let current = notes
                    .iter_mut()
                    .find(|current| current.id == note.id)
                    .ok_or("La nota ya no existe")?;
                *current = note;
                *pattern = pattern
                    .with_notes(notes, pattern.next_id())
                    .map_err(|error| error.to_string())?;
                Ok("Nota editada")
            }
            PatternAction::DeleteNote(id) => {
                self.session
                    .active_pattern_mut()
                    .notes
                    .remove_note(id)
                    .map_err(|error| error.to_string())?;
                self.roll.selected = None;
                Ok("Nota eliminada")
            }
            PatternAction::NoteVelocity { id, value } => {
                self.session
                    .active_pattern_mut()
                    .notes
                    .set_velocity(id, value)
                    .map_err(|error| error.to_string())?;
                Ok("Velocity actualizada")
            }
            PatternAction::UseSource(source) => {
                self.session.source = source;
                Ok("Fuente de batería seleccionada")
            }
            PatternAction::AddPiece(path) => {
                self.session.kit.add_sample(path)?;
                self.cursor_lane = self.session.kit.pieces.len() - 1;
                self.roll.piece = None;
                self.roll.reset_selection();
                self.mixer_open = false;
                self.steps_scroll.scroll_to_item(self.cursor_lane);
                Ok("Muestra añadida al kit")
            }
            PatternAction::FillLane { lane, every } => self.fill_lane(lane, every),
            PatternAction::ClearLane(lane) => self.clear_lane(lane),
            PatternAction::InvertLane(lane) => self.invert_lane(lane),
            PatternAction::ShiftLane { lane, steps } => self.shift_lane(lane, steps),
            PatternAction::CopyLaneMidi(lane) => self.copy_lane_midi(lane),
            PatternAction::PasteLaneMidi(lane) => self.paste_lane_midi(lane),
            PatternAction::CloneLane {
                lane,
                keep_layers,
                copy_midi,
            } => self.clone_lane(lane, keep_layers, copy_midi),
            action => self.apply_inspector(action),
        }
    }

    fn fill_lane(&mut self, lane: usize, every: usize) -> Result<&'static str, String> {
        let step_stride = every.max(1);
        let piece = self
            .session
            .kit
            .pieces
            .get(lane)
            .ok_or("La pieza ya no existe")?;
        let id = piece.id;
        let pitch = piece.base_note();
        let steps = self.step_count();
        let step_ticks = self.ticks_per_step();
        let pattern = &mut self.session.active_pattern_mut().notes;
        let length = pattern.length().0;
        let mut notes: Vec<_> = pattern
            .notes()
            .iter()
            .filter(|note| note.instrument != id)
            .cloned()
            .collect();
        let mut next_id = pattern.next_id();
        for step in (0..steps).step_by(step_stride) {
            let start = step as i64 * step_ticks;
            if start >= length {
                break;
            }
            let duration = step_ticks.min(length - start);
            notes.push(NoteEvent {
                id: NoteId(next_id),
                instrument: id,
                start: Tick(start),
                duration: Tick(duration),
                pitch,
                velocity: 100,
                expression: Default::default(),
                pitch_curve: Default::default(),
            });
            next_id += 1;
        }
        *pattern = pattern
            .with_notes(notes, next_id)
            .map_err(|error| error.to_string())?;
        Ok("Fila rellenada")
    }

    fn clear_lane(&mut self, lane: usize) -> Result<&'static str, String> {
        let id = self
            .session
            .kit
            .pieces
            .get(lane)
            .ok_or("La pieza ya no existe")?
            .id;
        let pattern = &mut self.session.active_pattern_mut().notes;
        let notes: Vec<_> = pattern
            .notes()
            .iter()
            .filter(|note| note.instrument != id)
            .cloned()
            .collect();
        *pattern = pattern
            .with_notes(notes, pattern.next_id())
            .map_err(|error| error.to_string())?;
        Ok("Fila vaciada")
    }

    fn invert_lane(&mut self, lane: usize) -> Result<&'static str, String> {
        let piece = self
            .session
            .kit
            .pieces
            .get(lane)
            .ok_or("La pieza ya no existe")?;
        let id = piece.id;
        let pitch = piece.base_note();
        let steps = self.step_count();
        let step_ticks = self.ticks_per_step();
        let occupied: Vec<bool> = (0..steps)
            .map(|step| self.step_notes(lane, step).next().is_some())
            .collect();
        let pattern = &mut self.session.active_pattern_mut().notes;
        let length = pattern.length().0;
        let mut notes: Vec<_> = pattern
            .notes()
            .iter()
            .filter(|note| note.instrument != id)
            .cloned()
            .collect();
        let mut next_id = pattern.next_id();
        for (step, has_note) in occupied.into_iter().enumerate() {
            if has_note {
                continue;
            }
            let start = step as i64 * step_ticks;
            if start >= length {
                break;
            }
            let duration = step_ticks.min(length - start);
            notes.push(NoteEvent {
                id: NoteId(next_id),
                instrument: id,
                start: Tick(start),
                duration: Tick(duration),
                pitch,
                velocity: 100,
                expression: Default::default(),
                pitch_curve: Default::default(),
            });
            next_id += 1;
        }
        *pattern = pattern
            .with_notes(notes, next_id)
            .map_err(|error| error.to_string())?;
        Ok("Pasos invertidos")
    }

    fn shift_lane(&mut self, lane: usize, steps: i64) -> Result<&'static str, String> {
        let id = self
            .session
            .kit
            .pieces
            .get(lane)
            .ok_or("La pieza ya no existe")?
            .id;
        let offset = steps * self.ticks_per_step();
        let pattern = &mut self.session.active_pattern_mut().notes;
        let length = pattern.length().0;
        let mut notes = pattern.notes().to_vec();
        for note in &mut notes {
            if note.instrument == id {
                let start = (note.start.0 + offset).rem_euclid(length);
                note.start = Tick(start);
                note.duration = Tick(note.duration.0.min(length - start).max(1));
            }
        }
        *pattern = pattern
            .with_notes(notes, pattern.next_id())
            .map_err(|error| error.to_string())?;
        Ok("Fila desplazada")
    }

    fn copy_lane_midi(&mut self, lane: usize) -> Result<&'static str, String> {
        let piece = self
            .session
            .kit
            .pieces
            .get(lane)
            .ok_or("La pieza ya no existe")?;
        let id = piece.id;
        let base_note = piece.base_note();
        let notes: Vec<_> = self
            .pattern()
            .notes()
            .iter()
            .filter(|note| note.instrument == id)
            .cloned()
            .collect();
        self.copied_lane = Some(super::CopiedLane { base_note, notes });
        Ok("MIDI de la fila copiado")
    }

    fn paste_lane_midi(&mut self, lane: usize) -> Result<&'static str, String> {
        let copied = self
            .copied_lane
            .as_ref()
            .ok_or("Primero copia el MIDI de una fila")?;
        let piece = self
            .session
            .kit
            .pieces
            .get(lane)
            .ok_or("La pieza ya no existe")?;
        let target_id = piece.id;
        let pitch_shift = i16::from(piece.base_note()) - i16::from(copied.base_note);
        let source_notes = copied.notes.clone();
        let pattern = &mut self.session.active_pattern_mut().notes;
        let length = pattern.length().0;
        let mut notes: Vec<_> = pattern
            .notes()
            .iter()
            .filter(|note| note.instrument != target_id)
            .cloned()
            .collect();
        let mut next_id = pattern.next_id();
        for mut note in source_notes {
            if note.start.0 >= length {
                continue;
            }
            note.id = NoteId(next_id);
            note.instrument = target_id;
            note.pitch = (i16::from(note.pitch) + pitch_shift).clamp(0, 127) as u8;
            note.duration = Tick(note.duration.0.min(length - note.start.0).max(1));
            notes.push(note);
            next_id += 1;
        }
        *pattern = pattern
            .with_notes(notes, next_id)
            .map_err(|error| error.to_string())?;
        Ok("MIDI pegado en la fila")
    }

    fn clone_lane(
        &mut self,
        lane: usize,
        keep_layers: bool,
        copy_midi: bool,
    ) -> Result<&'static str, String> {
        let source_id = self
            .session
            .kit
            .pieces
            .get(lane)
            .ok_or("La pieza ya no existe")?
            .id;
        let new_id = self.session.kit.clone_piece(lane, keep_layers)?;
        if copy_midi {
            let pattern = &mut self.session.active_pattern_mut().notes;
            let mut notes = pattern.notes().to_vec();
            let mut next_id = pattern.next_id();
            let cloned: Vec<_> = notes
                .iter()
                .filter(|note| note.instrument == source_id)
                .cloned()
                .collect();
            for mut note in cloned {
                note.id = NoteId(next_id);
                note.instrument = new_id;
                notes.push(note);
                next_id += 1;
            }
            *pattern = pattern
                .with_notes(notes, next_id)
                .map_err(|error| error.to_string())?;
        }
        self.cursor_lane = lane + 1;
        self.steps_scroll.scroll_to_item(self.cursor_lane);
        Ok(match (keep_layers, copy_midi) {
            (false, true) => "Shadow clone creado (solo MIDI)",
            (true, true) => "Pieza y patrón clonados",
            _ => "Instrumento clonado",
        })
    }
}
