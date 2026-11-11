//! Adaptador UI de scripts: captura una copia, ejecuta en background y aplica por ID.
use super::DawApp;
use elevate::musical::clip::ClipId;
use elevate::scripting::actions::{MidiOptions, MidiTool};
use gpui::{AppContext, Context};
use std::{io::Read, path::PathBuf};

enum ScriptSource {
    Builtin(MidiTool),
    File(PathBuf),
}

impl DawApp {
    pub(super) fn execute_script_command(&mut self, command: &str, cx: &mut Context<Self>) -> bool {
        let parts = match elevate::actions::tokenize(command) {
            Ok(parts) => parts,
            Err(error) => {
                if command.starts_with("midi.script") {
                    self.status_message = format!("Lua: {error}");
                    return true;
                }
                return false;
            }
        };
        let Some(name) = parts.first() else {
            return false;
        };
        let mut options = MidiOptions {
            grid: self.piano_roll.resolution,
            amount: self.piano_roll.humanize_amount,
            value: 100,
            pan: 0,
            strength: 100,
            scale_root: self.piano_roll.pitch_scale().root,
            scale_intervals: self.piano_roll.pitch_scale().intervals,
        };
        if let Some(tool) = MidiTool::from_command(&name.to_ascii_lowercase()) {
            options.amount = match tool {
                MidiTool::Gate | MidiTool::Probability => 100,
                MidiTool::Swing => 0,
                _ => options.amount,
            };
            if matches!(tool, MidiTool::ReleaseVelocity) {
                options.value = 64;
            }
            match tool.options(&parts[1..], options) {
                Ok(parsed) => options = parsed,
                Err(error) => {
                    self.status_message = error;
                    return true;
                }
            }
            self.start_midi_script(ScriptSource::Builtin(tool), options, cx);
            return true;
        }
        let source = match parts.as_slice() {
            [name, path] if name.eq_ignore_ascii_case("midi.script") && !path.is_empty() => {
                ScriptSource::File(PathBuf::from(path))
            }
            [name, ..] if name.eq_ignore_ascii_case("midi.script") => {
                self.status_message = "Uso: midi.script '/ruta/herramienta.lua'".into();
                return true;
            }
            _ => return false,
        };
        self.start_midi_script(source, options, cx);
        true
    }

    fn start_midi_script(
        &mut self,
        source: ScriptSource,
        options: MidiOptions,
        cx: &mut Context<Self>,
    ) {
        if self.script_task.is_some() {
            self.status_message = "Ya hay un script Lua en curso".into();
            return;
        }
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            self.status_message = "Abre un clip MIDI para ejecutar el script".into();
            return;
        };
        if clip.props.locked {
            self.status_message = "Clip MIDI bloqueado".into();
            return;
        }
        if self.piano_roll.drag.is_some() {
            self.status_message = "Termina el arrastre antes de editar con Lua".into();
            return;
        }
        if matches!(source, ScriptSource::Builtin(MidiTool::Subdivide))
            && !clip
                .notes()
                .iter()
                .any(|note| self.piano_roll.selected_notes.contains(&note.id))
        {
            self.piano_roll.hint = "Selecciona notas para subdividir".into();
            self.status_message = self.piano_roll.hint.clone();
            return;
        }
        if let ScriptSource::Builtin(MidiTool::Subdivide) = source
            && elevate::scripting::inline_subdivision(
                &clip.pattern,
                &self.piano_roll.selected_notes,
                options,
            )
        {
            let result = elevate::scripting::edit_builtin(
                MidiTool::Subdivide,
                &clip.pattern,
                &self.piano_roll.selected_notes,
                options,
            );
            self.apply_midi_script_result(result);
            cx.notify();
            return;
        }
        let clip_id: ClipId = clip.id;
        let original = clip.pattern.clone();
        let selected = self.piano_roll.selected_notes.clone();
        self.status_message = "Ejecutando Lua sobre el clip MIDI…".into();
        let work = cx.background_spawn(async move {
            let source = match source {
                ScriptSource::Builtin(tool) => {
                    return elevate::scripting::edit_builtin(tool, &original, &selected, options);
                }
                ScriptSource::File(path) => {
                    let file = std::fs::File::open(&path).map_err(|error| error.to_string())?;
                    let mut text = String::new();
                    file.take(256 * 1024 + 1)
                        .read_to_string(&mut text)
                        .map_err(|error| error.to_string())?;
                    if text.len() > 256 * 1024 {
                        return Err("El script Lua supera 256 KiB".into());
                    }
                    text
                }
            };
            elevate::scripting::edit_midi(&source, &original, &selected, options)
        });
        let snapshot = clip.pattern.clone();
        self.script_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.script_task = None;
                let current = this.timeline.midi_clip.as_ref();
                if this.piano_roll.drag.is_some()
                    || !current.is_some_and(|clip| {
                        clip.id == clip_id && !clip.props.locked && clip.pattern == snapshot
                    })
                {
                    this.status_message = "Lua: clip cambiado; resultado descartado".into();
                    cx.notify();
                    return;
                }
                this.apply_midi_script_result(result);
                cx.notify();
            });
        }));
        cx.notify();
    }
    /// Commit común a la vía inmediata y al worker, con los mismos IDs/mute/selección.
    fn apply_midi_script_result(&mut self, result: Result<elevate::scripting::MidiEdit, String>) {
        match result {
            Ok(edit) => {
                let count = edit.pattern.notes().len();
                let controls = edit.pattern.controls().points().len();
                if self
                    .timeline
                    .midi_clip
                    .as_ref()
                    .is_some_and(|clip| clip.pattern != edit.pattern)
                {
                    self.stop_audio();
                    if let Some(clip) = self.timeline.midi_clip.as_mut() {
                        edit.apply(clip, &mut self.piano_roll.selected_notes);
                        if !clip
                            .pattern
                            .controls()
                            .points()
                            .iter()
                            .any(|p| Some(p.id) == self.piano_roll.selected_control)
                        {
                            self.piano_roll.selected_control = None;
                        }
                    }
                }
                self.piano_roll.hint =
                    format!("Lua · {count} notas y {controls} controles validados");
            }
            Err(error) => self.piano_roll.hint = format!("Lua: {error}"),
        }
        self.status_message = self.piano_roll.hint.clone();
    }
}
