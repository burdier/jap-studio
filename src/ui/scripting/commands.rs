//! Comandos de scripts de archivo e integrados; las herramientas usan lua_tools.
use super::{DawApp, ScriptSource};
use elevate::scripting::actions::{MidiOptions, MidiTool};
use gpui::Context;
use std::path::PathBuf;

impl DawApp {
    pub(in crate::ui) fn execute_script_command(
        &mut self,
        command: &str,
        cx: &mut Context<Self>,
    ) -> bool {
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
}
