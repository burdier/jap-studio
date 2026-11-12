//! Adaptador UI de scripts: captura una copia, ejecuta en background y aplica por ID.
use super::DawApp;
use elevate::scripting::actions::{MidiOptions, MidiTool};
use elevate::scripting::{
    MidiScriptTarget,
    packages::{PackagePhase, ToolInvocation},
};
use gpui::{AppContext, Context};
use std::{io::Read, path::PathBuf};
mod commands;

enum ScriptSource {
    Builtin(MidiTool),
    File(PathBuf),
    Tool(ToolInvocation),
}

impl DawApp {
    pub(super) fn start_tool_invocation(
        &mut self,
        invocation: ToolInvocation,
        cx: &mut Context<Self>,
    ) {
        let options = MidiOptions {
            grid: self.piano_roll.resolution,
            ..Default::default()
        };
        self.start_midi_script(ScriptSource::Tool(invocation), options, cx);
    }

    fn start_midi_script(
        &mut self,
        source: ScriptSource,
        options: MidiOptions,
        cx: &mut Context<Self>,
    ) {
        let mut source = source;
        let package_alias = match &source {
            ScriptSource::Builtin(MidiTool::Accent) => {
                Some(("hay.midi.accent", vec!["12".to_owned()]))
            }
            ScriptSource::Builtin(MidiTool::Subdivide) => Some(("hay.midi.subdivide", vec![])),
            _ => None,
        };
        if let Some((id, arguments)) = package_alias
            && let Some(package) = self.lua_tools.registry.get(id)
        {
            match package.invoke(&arguments) {
                Ok(invocation) => source = ScriptSource::Tool(invocation),
                Err(error) => {
                    self.status_message = error.to_string();
                    return;
                }
            }
        }
        let require_selection = matches!(&source, ScriptSource::Builtin(MidiTool::Subdivide))
            || matches!(
                &source,
                ScriptSource::Tool(invocation)
                    if invocation.package().descriptor().id == "hay.midi.subdivide"
            );
        let package = match &source {
            ScriptSource::Tool(invocation) => Some(invocation.package().clone()),
            _ => None,
        };
        let input_error = |reason: &str| {
            package.as_ref().map_or_else(
                || reason.to_string(),
                |p| {
                    p.error(PackagePhase::Arguments, "context", reason)
                        .to_string()
                },
            )
        };
        if self.script_task.is_some() {
            self.status_message = input_error("Ya hay un script Lua en curso");
            return;
        }
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            self.status_message = input_error("Abre un clip MIDI para ejecutar el script");
            return;
        };
        if clip.props.locked {
            self.status_message = input_error("Clip MIDI bloqueado");
            return;
        }
        if self.piano_roll.drag.is_some() {
            self.status_message = input_error("Termina el arrastre antes de editar con Lua");
            return;
        }
        if require_selection
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
        let target = MidiScriptTarget::capture(clip);
        let original = clip.pattern.clone();
        let selected = self.piano_roll.selected_notes.clone();
        self.status_message = "Ejecutando Lua sobre el clip MIDI…".into();
        let work = cx.background_spawn(async move {
            let source = match source {
                ScriptSource::Tool(invocation) => {
                    return invocation
                        .edit_midi(&original, &selected, options)
                        .map_err(|e| e.to_string());
                }
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
        self.script_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.script_task = None;
                let current = this.timeline.midi_clip.as_ref();
                if let Err(error) = target.validate(current, this.piano_roll.drag.is_some()) {
                    this.status_message = package.as_ref().map_or_else(
                        || format!("Lua: {error}"),
                        |p| p.error(PackagePhase::Stale, "clip", &error).to_string(),
                    );
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
