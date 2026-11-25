//! Controlador y estado del modo MPC (Pads 4×4, bancos A–D, 16 Levels, Note Repeat, Overdub y Chop).
use super::{PatternAction, PatternEditor};
use crate::{
    audio::{
        AudioFile,
        drums::{DrumAudition, DrumPreview},
        sample_warp,
    },
    ui::{CenterView, DawApp},
};
use elevate::musical::{
    model::InstrumentId,
    time::{PPQ, Tick},
};
use gpui::{Context, PathPromptOptions};
use std::time::Instant;

pub(in crate::ui) const MPC_PAD_COUNT: usize = 16;
pub(in crate::ui) const MPC_BANK_COUNT: usize = 4;

/// Orden clásico de MPC: Pad 1 abajo-izquierda (Z) → Pad 16 arriba-derecha (4).
pub(in crate::ui) const MPC_KEY_LABELS: [&str; MPC_PAD_COUNT] = [
    "Z", "X", "C", "V", "A", "S", "D", "F", "Q", "W", "E", "R", "1", "2", "3", "4",
];

/// Orden visual de filas (fila superior = pads 12..16, fila inferior = pads 0..4).
pub(in crate::ui) const MPC_VISUAL_ROWS: [[usize; 4]; 4] =
    [[12, 13, 14, 15], [8, 9, 10, 11], [4, 5, 6, 7], [0, 1, 2, 3]];

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::ui) enum DrumViewMode {
    #[default]
    Steps,
    Pads,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::ui) enum SixteenLevelsMode {
    #[default]
    Off,
    Velocity,
    Pitch,
}

impl SixteenLevelsMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Velocity => "Vel",
            Self::Pitch => "Pitch",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::ui) enum NoteRepeatRate {
    #[default]
    Off,
    Eighth,
    Sixteenth,
    ThirtySecond,
    Triplet16,
}

impl NoteRepeatRate {
    pub const ALL: [Self; 5] = [
        Self::Off,
        Self::Eighth,
        Self::Sixteenth,
        Self::ThirtySecond,
        Self::Triplet16,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Eighth => "1/8",
            Self::Sixteenth => "1/16",
            Self::ThirtySecond => "1/32",
            Self::Triplet16 => "1/16T",
        }
    }

    pub fn beats(self) -> Option<f64> {
        match self {
            Self::Off => None,
            Self::Eighth => Some(0.5),
            Self::Sixteenth => Some(0.25),
            Self::ThirtySecond => Some(0.125),
            Self::Triplet16 => Some(1.0 / 6.0),
        }
    }

    pub fn interval_frames(self, bpm: f64, sample_rate: u32) -> Option<u64> {
        let beats = self.beats()?;
        let bpm = bpm.clamp(20.0, 300.0);
        let secs = beats * 60.0 / bpm;
        Some(((secs * f64::from(sample_rate.max(8_000))).round() as u64).max(64))
    }
}

#[derive(Clone, Copy)]
pub(in crate::ui) struct HeldPad {
    pub pad: usize,
    pub piece: InstrumentId,
    pub pitch: u8,
    pub velocity: u8,
    pub key: Option<&'static str>,
}

pub(in crate::ui) struct MpcState {
    pub bank: usize,
    pub selected_pad: usize,
    pub sixteen_levels: SixteenLevelsMode,
    pub note_repeat: NoteRepeatRate,
    pub full_level: bool,
    pub overdub: bool,
    pub pad_flash: [Option<Instant>; MPC_PAD_COUNT],
    pub held: Option<HeldPad>,
    pub last_repeat_hits: u64,
    pub chop_slices: usize,
    pub chop_transients: bool,
    pub chop_choke: bool,
}

impl Default for MpcState {
    fn default() -> Self {
        Self {
            bank: 0,
            selected_pad: 0,
            sixteen_levels: SixteenLevelsMode::Off,
            note_repeat: NoteRepeatRate::Off,
            full_level: false,
            overdub: false,
            pad_flash: [None; MPC_PAD_COUNT],
            held: None,
            last_repeat_hits: 0,
            chop_slices: 16,
            chop_transients: true,
            chop_choke: true,
        }
    }
}

impl MpcState {
    pub fn bank_label(bank: usize) -> &'static str {
        match bank {
            0 => "A",
            1 => "B",
            2 => "C",
            _ => "D",
        }
    }

    pub fn lane_for_pad(&self, pad: usize) -> usize {
        self.bank * MPC_PAD_COUNT + pad.min(MPC_PAD_COUNT - 1)
    }

    pub fn pad_flash_alpha(&self, pad: usize) -> f32 {
        let Some(at) = self.pad_flash.get(pad).and_then(|t| *t) else {
            return 0.0;
        };
        let elapsed = at.elapsed().as_secs_f32();
        (1.0 - elapsed / 0.16).clamp(0.0, 1.0)
    }
}

pub(in crate::ui) fn mpc_key_to_pad(key: &str) -> Option<(usize, &'static str)> {
    match key {
        "z" | "Z" => Some((0, "z")),
        "x" | "X" => Some((1, "x")),
        "c" | "C" => Some((2, "c")),
        "v" | "V" => Some((3, "v")),
        "a" | "A" => Some((4, "a")),
        "s" | "S" => Some((5, "s")),
        "d" | "D" => Some((6, "d")),
        "f" | "F" => Some((7, "f")),
        "q" | "Q" => Some((8, "q")),
        "w" | "W" => Some((9, "w")),
        "e" | "E" => Some((10, "e")),
        "r" | "R" => Some((11, "r")),
        "1" => Some((12, "1")),
        "2" => Some((13, "2")),
        "3" => Some((14, "3")),
        "4" => Some((15, "4")),
        _ => None,
    }
}

impl PatternEditor {
    /// Resuelve `(lane, piece_id, pitch, velocity)` para un pad `0..15` respetando 16 Levels y Full Level.
    pub(in crate::ui) fn resolve_mpc_pad(
        &self,
        pad: usize,
        input_velocity: u8,
    ) -> Option<(usize, InstrumentId, u8, u8)> {
        let pad = pad.min(MPC_PAD_COUNT - 1);
        let base_vel = if self.mpc.full_level {
            127
        } else {
            input_velocity.clamp(1, 127)
        };
        match self.mpc.sixteen_levels {
            SixteenLevelsMode::Off => {
                let lane = self.mpc.lane_for_pad(pad);
                let piece = self.session.kit.pieces.get(lane)?;
                Some((lane, piece.id, piece.base_note(), base_vel))
            }
            SixteenLevelsMode::Velocity => {
                let lane = self.mpc.lane_for_pad(self.mpc.selected_pad);
                let piece = self.session.kit.pieces.get(lane)?;
                let vel = (16 + (pad as u32 * 111) / 15).clamp(1, 127) as u8;
                Some((lane, piece.id, piece.base_note(), vel))
            }
            SixteenLevelsMode::Pitch => {
                let lane = self.mpc.lane_for_pad(self.mpc.selected_pad);
                let piece = self.session.kit.pieces.get(lane)?;
                let semi = pad as i16 - 7; // -7..=+8 semitonos alrededor de la nota raíz
                let pitch = (i16::from(piece.base_note()) + semi).clamp(0, 127) as u8;
                Some((lane, piece.id, pitch, base_vel))
            }
        }
    }
}

impl DawApp {
    /// Asegura una salida de audio para los pads incluso cuando el transporte está detenido.
    pub(in crate::ui) fn trigger_drum_audition(
        &mut self,
        piece: InstrumentId,
        pitch: u8,
        velocity: u8,
    ) -> bool {
        if self.player.is_some() {
            return self.preview_drum_piece(piece, pitch, velocity);
        }
        if self.drum_audition.is_none() {
            let kit = self.pattern_editor.session.kit.clone();
            let bpm = self.timeline.tempo.bpm();
            match DrumAudition::open(
                kit,
                &self.settings.audio.saved,
                bpm,
                self.drum_note_repeat.clone(),
            ) {
                Ok(audition) => self.drum_audition = Some(audition),
                Err(error) => {
                    self.status_message = error;
                    return false;
                }
            }
        }
        if let Some(audition) = &mut self.drum_audition {
            audition.trigger(DrumPreview {
                piece,
                pitch,
                velocity,
            })
        } else {
            false
        }
    }

    pub(in crate::ui) fn active_drum_sample_rate(&self) -> u32 {
        if let Some(player) = &self.player {
            player.sample_rate()
        } else if let Some(audition) = &self.drum_audition {
            audition.sample_rate()
        } else {
            self.settings.audio.saved.sample_rate.unwrap_or(48_000)
        }
    }

    pub(in crate::ui) fn sync_drum_audition_kit(&mut self) {
        if let Some(audition) = &mut self.drum_audition {
            audition.update_kit(&self.pattern_editor.session.kit);
        }
    }

    pub(in crate::ui) fn sync_mpc_note_repeat_bpm(&self) {
        if self.drum_note_repeat.is_active()
            && let Some(interval) = self
                .pattern_editor
                .mpc
                .note_repeat
                .interval_frames(self.timeline.tempo.bpm(), self.active_drum_sample_rate())
        {
            self.drum_note_repeat.update_interval(interval);
        }
    }

    pub(in crate::ui) fn mpc_pad_press(
        &mut self,
        pad: usize,
        input_velocity: u8,
        from_key: Option<&'static str>,
    ) {
        let pad = pad.min(MPC_PAD_COUNT - 1);
        if self.pattern_editor.mpc.sixteen_levels == SixteenLevelsMode::Off {
            self.pattern_editor.mpc.selected_pad = pad;
            let lane = self.pattern_editor.mpc.lane_for_pad(pad);
            if lane < self.pattern_editor.session.kit.pieces.len() {
                self.pattern_editor.cursor_lane = lane;
            }
        }
        let Some((_lane, piece, pitch, velocity)) =
            self.pattern_editor.resolve_mpc_pad(pad, input_velocity)
        else {
            return;
        };
        self.pattern_editor.mpc.pad_flash[pad] = Some(Instant::now());
        let _ = self.trigger_drum_audition(piece, pitch, velocity);

        if self.pattern_editor.mpc.overdub {
            self.overdub_mpc_hit(piece, pitch, velocity);
        }

        if let Some(interval) = self
            .pattern_editor
            .mpc
            .note_repeat
            .interval_frames(self.timeline.tempo.bpm(), self.active_drum_sample_rate())
        {
            self.pattern_editor.mpc.last_repeat_hits = self.drum_note_repeat.hits();
            self.drum_note_repeat
                .start(piece, pitch, velocity, interval);
            self.pattern_editor.mpc.held = Some(HeldPad {
                pad,
                piece,
                pitch,
                velocity,
                key: from_key,
            });
        } else {
            self.drum_note_repeat.stop();
            self.pattern_editor.mpc.held = Some(HeldPad {
                pad,
                piece,
                pitch,
                velocity,
                key: from_key,
            });
        }
    }

    pub(in crate::ui) fn mpc_pad_release(&mut self) {
        self.drum_note_repeat.stop();
        self.pattern_editor.mpc.held = None;
    }

    pub(in crate::ui) fn handle_mpc_key_down(&mut self, key: &str, is_repeat: bool) -> bool {
        if self.pattern_editor.view_mode != DrumViewMode::Pads
            || self.pattern_editor.mixer_open
            || self.pattern_editor.roll.piece.is_some()
        {
            return false;
        }
        let Some((pad, canonical)) = mpc_key_to_pad(key) else {
            return false;
        };
        if !is_repeat {
            self.mpc_pad_press(pad, 112, Some(canonical));
        }
        true
    }

    pub(in crate::ui) fn handle_mpc_key_up(&mut self, key: &str) -> bool {
        let Some((_, canonical)) = mpc_key_to_pad(key) else {
            return false;
        };
        if self
            .pattern_editor
            .mpc
            .held
            .is_some_and(|held| held.key == Some(canonical))
        {
            self.mpc_pad_release();
            return true;
        }
        false
    }

    pub(in crate::ui) fn handle_mpc_midi_note(
        &mut self,
        note: u8,
        velocity: u8,
        is_on: bool,
    ) -> bool {
        if !(36..=51).contains(&note) {
            return false;
        }
        let pad = usize::from(note - 36);
        if is_on && velocity > 0 {
            self.mpc_pad_press(pad, velocity, None);
        } else if self
            .pattern_editor
            .mpc
            .held
            .is_some_and(|held| held.pad == pad && held.key.is_none())
        {
            self.mpc_pad_release();
        }
        true
    }

    /// Graba un golpe en vivo al patrón activo (cuantizado a la rejilla del patrón) durante Play.
    fn overdub_mpc_hit(&mut self, piece: InstrumentId, pitch: u8, velocity: u8) {
        let Some(beat) = self.drum_pattern_playhead() else {
            return;
        };
        let step_ticks = self.pattern_editor.ticks_per_step().max(1);
        let length = self.pattern_editor.pattern().length().0.max(step_ticks);
        let raw_tick = (beat * PPQ as f64).round() as i64;
        let quantized = ((raw_tick + step_ticks / 2) / step_ticks) * step_ticks;
        let start = quantized.rem_euclid(length);
        let duration = step_ticks.min(length - start).max(1);

        // Evita apilar notas duplicadas exactamente en el mismo tick/pieza/tono.
        let exists = self
            .pattern_editor
            .pattern()
            .notes()
            .iter()
            .any(|n| n.instrument == piece && n.start.0 == start && n.pitch == pitch);
        if exists {
            return;
        }
        let notes = &mut self.pattern_editor.session.active_pattern_mut().notes;
        if notes
            .add_note(piece, Tick(start), Tick(duration), pitch, velocity)
            .is_ok()
        {
            let active = self.pattern_editor.session.active;
            let _ = self
                .timeline
                .refresh_drum_links(&self.pattern_editor.session, active);
            self.publish_drum_edit();
            self.refresh_project_dirty();
        }
    }

    pub(in crate::ui) fn poll_mpc_live(&mut self) {
        if let Some(audition) = &mut self.drum_audition
            && let Err(error) = audition.poll()
        {
            self.status_message = error;
        }
        let hits = self.drum_note_repeat.hits();
        if hits != self.pattern_editor.mpc.last_repeat_hits {
            self.pattern_editor.mpc.last_repeat_hits = hits;
            if let Some(held) = self.pattern_editor.mpc.held {
                self.pattern_editor.mpc.pad_flash[held.pad] = Some(Instant::now());
                if self.pattern_editor.mpc.overdub {
                    self.overdub_mpc_hit(held.piece, held.pitch, held.velocity);
                }
            }
        }
    }

    /// Trocea un archivo de audio (desde el Sample Editor o diálogo MPC) y lo envía a los Pads MPC.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::ui) fn chop_audio_to_mpc_pads(
        &mut self,
        source: String,
        base_name: &str,
        frames: &[elevate::StereoFrame],
        sample_rate: u32,
        range_start: usize,
        range_end: usize,
        count: usize,
        by_transients: bool,
        choke: bool,
        replace_existing: bool,
    ) {
        let slices = sample_warp::slice_ranges(
            frames,
            sample_rate,
            range_start,
            range_end,
            count,
            by_transients,
        );
        if slices.is_empty() {
            self.status_message = "No se pudieron calcular cortes en esta muestra".into();
            return;
        }
        let choke_group = choke.then_some(1u16);
        let kit_id = self.pattern_editor.session.kit.id;
        let old_pieces: Vec<InstrumentId> = if replace_existing {
            self.pattern_editor
                .session
                .kit
                .pieces
                .iter()
                .map(|p| p.id)
                .collect()
        } else {
            Vec::new()
        };
        match self.pattern_editor.session.kit.add_sliced_sample(
            source,
            base_name,
            &slices,
            choke_group,
            replace_existing,
        ) {
            Ok(created) => {
                if replace_existing {
                    for old_id in old_pieces {
                        self.timeline.detach_removed_drum_piece(kit_id, old_id);
                    }
                    let valid: std::collections::HashSet<_> = self
                        .pattern_editor
                        .session
                        .kit
                        .pieces
                        .iter()
                        .map(|p| p.id)
                        .collect();
                    for pat in &mut self.pattern_editor.session.patterns {
                        let kept: Vec<_> = pat
                            .notes
                            .notes()
                            .iter()
                            .filter(|n| valid.contains(&n.instrument))
                            .cloned()
                            .collect();
                        if kept.len() != pat.notes.notes().len()
                            && let Ok(cleaned) = pat.notes.with_notes(kept, pat.notes.next_id())
                        {
                            pat.notes = cleaned;
                        }
                    }
                    self.pattern_editor.mpc.bank = 0;
                    self.pattern_editor.mpc.selected_pad = 0;
                    self.pattern_editor.cursor_lane = 0;
                } else {
                    let first_lane = self
                        .pattern_editor
                        .session
                        .kit
                        .pieces
                        .len()
                        .saturating_sub(created.len());
                    self.pattern_editor.mpc.bank =
                        (first_lane / MPC_PAD_COUNT).min(MPC_BANK_COUNT - 1);
                    self.pattern_editor.mpc.selected_pad = first_lane % MPC_PAD_COUNT;
                    self.pattern_editor.cursor_lane = first_lane;
                }
                self.pattern_editor.roll.piece = None;
                self.pattern_editor.roll.reset_selection();
                self.pattern_editor.mixer_open = false;
                self.pattern_editor.view_mode = DrumViewMode::Pads;
                self.center_view = CenterView::Session;
                self.publish_drum_edit();
                self.sync_drum_audition_kit();
                self.refresh_project_dirty();
                let mode_lbl = if by_transients {
                    "transientes"
                } else {
                    "rejilla igual"
                };
                self.status_message = format!(
                    "MPC Chop: {} pads creados ({mode_lbl}) · toca con Z-X-C-V / A-S-D-F / Q-W-E-R / 1-2-3-4 o MIDI",
                    created.len()
                );
            }
            Err(error) => {
                self.status_message = error;
            }
        }
    }

    pub(in crate::ui) fn chop_sample_dialog(&mut self, cx: &mut Context<Self>) {
        if self.pattern_editor.sample_task.is_some() {
            self.status_message = "Espera a que termine la carga de la muestra".into();
            return;
        }
        let count = self.pattern_editor.mpc.chop_slices;
        let by_transients = self.pattern_editor.mpc.chop_transients;
        let choke = self.pattern_editor.mpc.chop_choke;
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Trocear sample a Pads MPC".into()),
        });
        let background = cx.background_executor().clone();
        self.pattern_editor.sample_task = Some(cx.spawn(async move |this, cx| {
            let selection = match prompt.await {
                Ok(Ok(paths)) => Ok(paths.and_then(|p| p.into_iter().next())),
                _ => Err("No se pudo abrir el selector de muestras".into()),
            };
            let decoded = match selection {
                Ok(Some(path)) => {
                    background
                        .spawn(async move {
                            let source = path.to_str().ok_or("La ruta debe ser UTF-8")?.to_owned();
                            let name = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Chop")
                                .to_owned();
                            let file = AudioFile::decode(&path)?;
                            Ok(Some((source, name, file)))
                        })
                        .await
                }
                Ok(None) => Ok(None),
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.pattern_editor.sample_task = None;
                match decoded {
                    Ok(Some((source, name, file))) => {
                        let len = file.frames.len();
                        this.chop_audio_to_mpc_pads(
                            source,
                            &name,
                            &file.frames,
                            file.sample_rate,
                            0,
                            len,
                            count,
                            by_transients,
                            choke,
                            false,
                        );
                    }
                    Ok(None) => {}
                    Err(error) => this.status_message = error,
                }
                cx.notify();
            });
        }));
    }

    pub(in crate::ui) fn mpc_piece_peak(&self, id: InstrumentId) -> f32 {
        if let Some(player) = &self.player {
            player.drum_piece_peak(id)
        } else if let Some(audition) = &self.drum_audition {
            audition.piece_peak(id)
        } else {
            0.0
        }
    }

    pub(in crate::ui) fn mpc_bus_peak(&self) -> f32 {
        if let Some(player) = &self.player {
            player.drum_bus_peak()
        } else if let Some(audition) = &self.drum_audition {
            audition.bus_peak()
        } else {
            0.0
        }
    }

    /// Reemplaza o añade muestra a un pad específico (drag & drop sobre pad MPC).
    pub(in crate::ui) fn drop_sample_on_mpc_pad(
        &mut self,
        pad: usize,
        path: std::path::PathBuf,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let lane = self.pattern_editor.mpc.lane_for_pad(pad);
        let piece = self
            .pattern_editor
            .session
            .kit
            .pieces
            .get(lane)
            .map(|p| p.id);
        self.pattern_editor.mpc.selected_pad = pad;
        self.drop_drum_sample(path, piece, window, cx);
    }

    /// Borra la pieza asignada al pad MPC.
    pub(in crate::ui) fn clear_mpc_pad(&mut self, pad: usize) {
        let lane = self.pattern_editor.mpc.lane_for_pad(pad);
        if lane < self.pattern_editor.session.kit.pieces.len() {
            self.apply_pattern_action(PatternAction::RemovePiece(lane));
        }
    }
}
