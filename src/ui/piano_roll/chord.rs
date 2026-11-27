//! Estado y controlador de Hay Chord (Smart Pad) sincronizado con la escala del DAW.
use crate::ui::DawApp;
use elevate::{musical::time::Tick, scripting::actions::MidiOptions};
use gpui::Context;

const NOTE_NAMES: [&str; 12] = [
    "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
];
const MINOR_STEPS: [u8; 7] = [0, 2, 3, 5, 7, 8, 10];
const MAJOR_STEPS: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
const MINOR_ROMANS: [&str; 7] = ["i", "ii°", "III", "iv", "v", "VI", "VII"];
const MAJOR_ROMANS: [&str; 7] = ["I", "ii", "iii", "IV", "V", "vi", "vii°"];
const MINOR_MOODS: [&str; 7] = [
    "Reposo", "Puente", "Luz", "Viaje", "Tensión", "Calma", "Impulso",
];
const MAJOR_MOODS: [&str; 7] = [
    "Reposo", "Suave", "Cálido", "Viaje", "Impulso", "Calma", "Tensión",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) struct ChordDegreeConfig {
    pub color: u8,
    pub voicing: u8,
    pub bass_octave: bool,
    pub inversion: i8,
}

impl Default for ChordDegreeConfig {
    fn default() -> Self {
        Self {
            color: 1, // 7ª (Cálido) por defecto
            voicing: 0,
            bass_octave: true,
            inversion: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::ui) struct ChordPadState {
    pub degree: u8,
    pub color: u8,
    pub voicing: u8,
    pub bass_octave: bool,
    pub inversion: i8,
    /// Configuración individual (color, voicing, bajo, inversión) por cada uno de los 7 grados.
    pub pads: [ChordDegreeConfig; 7],
    pub strum_ticks: i16,
    /// Cuando está activo, las notas MIDI entrantes disparan acordes diatónicos de la escala activa.
    pub midi_chord_mode: bool,
}

impl Default for ChordPadState {
    fn default() -> Self {
        let base = ChordDegreeConfig::default();
        Self {
            degree: 0,
            color: base.color,
            voicing: base.voicing,
            bass_octave: base.bass_octave,
            inversion: base.inversion,
            pads: [base; 7],
            strum_ticks: 15,
            midi_chord_mode: false,
        }
    }
}

impl ChordPadState {
    pub fn select_degree(&mut self, deg: u8) {
        let d = deg.min(6);
        self.degree = d;
        let cfg = self.pads[usize::from(d)];
        self.color = cfg.color;
        self.voicing = cfg.voicing;
        self.bass_octave = cfg.bass_octave;
        self.inversion = cfg.inversion;
    }

    pub fn set_color(&mut self, color: u8) {
        let c = color.min(4);
        self.color = c;
        self.pads[usize::from(self.degree.min(6))].color = c;
    }

    pub fn apply_color_to_all(&mut self) {
        let c = self.color;
        for cfg in &mut self.pads {
            cfg.color = c;
        }
    }

    pub fn set_voicing(&mut self, voicing: u8) {
        let v = voicing.min(2);
        self.voicing = v;
        self.pads[usize::from(self.degree.min(6))].voicing = v;
    }

    pub fn toggle_bass(&mut self) {
        self.bass_octave = !self.bass_octave;
        self.pads[usize::from(self.degree.min(6))].bass_octave = self.bass_octave;
    }

    pub fn set_inversion(&mut self, inv: i8) {
        let v = inv.clamp(-2, 3);
        self.inversion = v;
        self.pads[usize::from(self.degree.min(6))].inversion = v;
    }

    pub fn color_for(&self, deg: u8) -> u8 {
        self.pads[usize::from(deg.min(6))].color
    }

    pub fn quality_arg(&self) -> &'static str {
        match self.color {
            0 => "scale",
            1 => "scale7",
            2 => "scale9",
            3 => "scale_sus4",
            _ => "power",
        }
    }

    pub fn voicing_arg(&self) -> &'static str {
        match self.voicing {
            1 => "open",
            2 => "drop2",
            _ => "close",
        }
    }

    pub fn degree_steps(scale_minor: bool) -> &'static [u8; 7] {
        if scale_minor {
            &MINOR_STEPS
        } else {
            &MAJOR_STEPS
        }
    }

    pub fn degree_roman(scale_minor: bool, degree: u8) -> &'static str {
        let idx = usize::from(degree.min(6));
        if scale_minor {
            MINOR_ROMANS[idx]
        } else {
            MAJOR_ROMANS[idx]
        }
    }

    pub fn degree_mood(scale_minor: bool, degree: u8) -> &'static str {
        let idx = usize::from(degree.min(6));
        if scale_minor {
            MINOR_MOODS[idx]
        } else {
            MAJOR_MOODS[idx]
        }
    }

    pub fn degree_root_pitch(&self, scale_root: u8, scale_minor: bool) -> u8 {
        let steps = Self::degree_steps(scale_minor);
        let offset = steps[usize::from(self.degree.min(6))];
        let pc = (scale_root + offset) % 12;
        60 + pc
    }

    pub fn degree_chord_name(scale_root: u8, scale_minor: bool, degree: u8, color: u8) -> String {
        let steps = Self::degree_steps(scale_minor);
        let idx = usize::from(degree.min(6));
        let root_pc = usize::from((scale_root + steps[idx]) % 12);
        let root_name = NOTE_NAMES[root_pc];
        let triad_quality = if scale_minor {
            ["m", "dim", "", "m", "m", "", ""][idx]
        } else {
            ["", "m", "m", "", "", "m", "dim"][idx]
        };
        let seventh_quality = if scale_minor {
            ["m7", "m7b5", "maj7", "m7", "m7", "maj7", "7"][idx]
        } else {
            ["maj7", "m7", "m7", "maj7", "7", "m7", "m7b5"][idx]
        };
        let ninth_quality = if scale_minor {
            ["m9", "m7b5", "maj9", "m9", "m9", "maj9", "9"][idx]
        } else {
            ["maj9", "m9", "m7", "maj9", "9", "m9", "m7b5"][idx]
        };
        let suffix = match color {
            0 => triad_quality,
            1 => seventh_quality,
            2 => ninth_quality,
            3 => "sus4",
            _ => "5",
        };
        format!("{root_name}{suffix}")
    }

    pub fn preview_voices(&self, scale_root: u8, scale_minor: bool) -> (u8, Vec<u8>) {
        let steps = Self::degree_steps(scale_minor);
        let deg_idx = usize::from(self.degree.min(6));
        let root_pitch = self.degree_root_pitch(scale_root, scale_minor);
        let scale_degrees: &[usize] = match self.color {
            0 => &[0, 2, 4],
            1 => &[0, 2, 4, 6],
            2 => &[0, 2, 4, 6, 8],
            3 => &[0, 3, 4],
            _ => &[],
        };
        let mut voices = Vec::with_capacity(6);
        if scale_degrees.is_empty() {
            voices.push(root_pitch);
            voices.push((root_pitch + 7).min(127));
        } else {
            let base_scale_pitch = 60 + u16::from(scale_root);
            let root_abs = base_scale_pitch + u16::from(steps[deg_idx]);
            let shift = if root_abs >= 72 { 12 } else { 0 };
            for &step in scale_degrees {
                let target_deg = deg_idx + step;
                let oct = (target_deg / 7) as u16;
                let rem = target_deg % 7;
                let pitch = base_scale_pitch + oct * 12 + u16::from(steps[rem]) - shift;
                voices.push(pitch.min(127) as u8);
            }
        }
        if self.inversion > 0 {
            for _ in 0..self.inversion {
                let lowest = voices.remove(0);
                voices.push((lowest + 12).min(127));
                voices.sort_unstable();
            }
        } else if self.inversion < 0 {
            for _ in 0..(-self.inversion) {
                if let Some(highest) = voices.pop() {
                    voices.insert(0, highest.saturating_sub(12));
                    voices.sort_unstable();
                }
            }
        }
        if self.voicing == 1 && voices.len() >= 3 {
            voices[1] = (voices[1] + 12).min(127);
        } else if self.voicing == 2 && voices.len() >= 3 {
            let idx = voices.len() - 2;
            voices[idx] = voices[idx].saturating_sub(12);
        }
        voices.sort_unstable();
        if self.bass_octave && root_pitch >= 12 {
            voices.insert(0, root_pitch - 12);
            voices.sort_unstable();
        }
        voices.dedup();
        (root_pitch, voices)
    }

    /// Calcula el grado diatónico y las voces de acorde para una nota MIDI entrante
    /// respetando la escala activa, el voicing elegido y la octava tocada.
    pub fn voices_for_midi_note(
        &self,
        trigger_note: u8,
        scale_root: u8,
        scale_minor: bool,
    ) -> (u8, Vec<u8>) {
        let (degree, octave_shift) =
            elevate::musical::chord::scale_degree_for_note(trigger_note, scale_root, scale_minor);
        let mut pad = *self;
        pad.select_degree(degree);
        let (_, raw_voices) = pad.preview_voices(scale_root, scale_minor);
        let semitone_shift = i16::from(octave_shift) * 12;
        let mut shifted: Vec<u8> = raw_voices
            .into_iter()
            .map(|p| (i16::from(p) + semitone_shift).clamp(0, 127) as u8)
            .collect();
        shifted.sort_unstable();
        shifted.dedup();
        (degree, shifted)
    }

    pub fn format_note_name(pitch: u8) -> String {
        let name = NOTE_NAMES[usize::from(pitch % 12)];
        let octave = i16::from(pitch / 12) - 1;
        format!("{name}{octave}")
    }
}

impl DawApp {
    fn ensure_chord_invocation(
        &mut self,
    ) -> Result<elevate::scripting::packages::ToolInvocation, String> {
        if self
            .lua_tools
            .registry
            .get("hay.midi.chord_utility")
            .is_none()
        {
            let dirs = elevate::scripting::packages::PackageDirectories {
                bundled: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets/scripts/tools/midi"),
                user: None,
            };
            let _ = self.lua_tools.registry.reload(&dirs);
        }
        let package = self
            .lua_tools
            .registry
            .get("hay.midi.chord_utility")
            .cloned()
            .ok_or_else(|| "No se encontró el paquete Lua hay.midi.chord_utility".to_string())?;
        let args = self.chord_tool_arguments();
        package.invoke(&args).map_err(|e| e.to_string())
    }

    pub(super) fn chord_tool_arguments(&self) -> Vec<String> {
        let pad = &self.piano_roll.chord_pad;
        vec![
            pad.quality_arg().to_owned(),
            pad.inversion.to_string(),
            pad.voicing_arg().to_owned(),
            pad.bass_octave.to_string(),
            pad.strum_ticks.to_string(),
        ]
    }

    pub(super) fn apply_chord_to_selection(&mut self, cx: &mut Context<Self>) {
        match self.ensure_chord_invocation() {
            Ok(invocation) => self.start_tool_invocation(invocation, cx),
            Err(error) => {
                self.piano_roll.hint = error;
                self.status_message = self.piano_roll.hint.clone();
            }
        }
    }

    pub(super) fn insert_chord_at_cursor(&mut self, cx: &mut Context<Self>) {
        let invocation = match self.ensure_chord_invocation() {
            Ok(invocation) => invocation,
            Err(error) => {
                self.piano_roll.hint = error;
                self.status_message = self.piano_roll.hint.clone();
                return;
            }
        };
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            self.status_message = "Abre un clip MIDI para insertar acordes".into();
            return;
        };
        if clip.props.locked {
            self.piano_roll.hint = "Clip MIDI bloqueado".into();
            self.status_message = self.piano_roll.hint.clone();
            return;
        }

        let step = self.piano_roll.resolution.ticks().0.max(1);
        let clip_len = clip.pattern.length().0;
        let mut start = (self.piano_roll.cursor.0 / step * step).clamp(0, (clip_len - 1).max(0));
        let draw_ticks = self.piano_roll.draw_length.ticks().0.max(1);
        if start >= clip_len {
            start = 0;
        }
        let duration = draw_ticks.min(clip_len - start).max(1);
        let root_pitch = self
            .piano_roll
            .chord_pad
            .degree_root_pitch(self.piano_roll.scale_root, self.piano_roll.scale_minor);
        let instrument = self
            .tracks
            .get(clip.track)
            .and_then(|track| track.instrument)
            .unwrap_or(self.instrument.id());

        let mut working = clip.pattern.clone();
        let Ok(root_id) =
            working.add_note(instrument, Tick(start), Tick(duration), root_pitch, 100)
        else {
            return;
        };
        let scale = self.piano_roll.pitch_scale();
        let options = MidiOptions {
            grid: self.piano_roll.resolution,
            scale_root: scale.root,
            scale_intervals: scale.intervals,
            ..Default::default()
        };
        match invocation.edit_midi(&working, &[root_id], options) {
            Ok(edit) => {
                self.stop_audio();
                self.piano_roll.selected_notes = vec![root_id];
                if let Some(clip) = self.timeline.midi_clip.as_mut() {
                    edit.apply(clip, &mut self.piano_roll.selected_notes);
                    self.piano_roll.selected = self.piano_roll.selected_notes.first().copied();
                }
                let next_cursor = if start + duration < clip_len {
                    start + duration
                } else {
                    0
                };
                self.piano_roll.cursor = Tick(next_cursor);
                let chord_name = ChordPadState::degree_chord_name(
                    self.piano_roll.scale_root,
                    self.piano_roll.scale_minor,
                    self.piano_roll.chord_pad.degree,
                    self.piano_roll.chord_pad.color,
                );
                self.piano_roll.hint = format!("Hay Chord · {chord_name} insertado en el clip");
                self.status_message = self.piano_roll.hint.clone();
                cx.notify();
            }
            Err(error) => {
                self.piano_roll.hint = error.to_string();
                self.status_message = self.piano_roll.hint.clone();
            }
        }
    }
}
