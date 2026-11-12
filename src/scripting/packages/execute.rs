//! Cada invocación usa su fuente capturada y una VM nueva, siempre fuera del audio.
use super::{PackageError, PackagePhase, ToolInvocation, descriptor};
use crate::{
    actions::ParameterValue,
    musical::{
        model::{NoteId, Pattern},
        time::PPQ,
    },
    scripting::{MidiEdit, actions::MidiOptions, controls, notes, output, sandbox},
};
use mlua::{MultiValue, Value};

impl ToolInvocation {
    pub fn edit_midi(
        &self,
        pattern: &Pattern,
        selected: &[NoteId],
        options: MidiOptions,
    ) -> Result<MidiEdit, PackageError> {
        let error = |phase, field, message| self.package.error(phase, field, message);
        if pattern.notes().len() > output::MAX_NOTES
            || pattern.controls().points().len() > controls::MAX_CONTROLS
        {
            return Err(error(
                PackagePhase::Arguments,
                "context",
                "Demasiadas notas o controles".into(),
            ));
        }
        let execution_error = |e: mlua::Error| error(PackagePhase::Execute, "run", e.to_string());
        let lua = sandbox::new().map_err(execution_error)?;
        let (metadata, run) =
            descriptor::evaluate(&lua, self.package.source(), self.package.path(), true).map_err(
                |mut e| {
                    if e.phase == PackagePhase::Load {
                        e.phase = PackagePhase::Execute;
                    }
                    if e.id.is_none() {
                        e.id = Some(self.package.descriptor.id.clone());
                    }
                    e
                },
            )?;
        if metadata != self.package.descriptor {
            return Err(error(
                PackagePhase::Descriptor,
                "descriptor",
                "Los metadatos cambiaron al evaluar la fuente capturada".into(),
            ));
        }
        let context = lua.create_table().map_err(execution_error)?;
        let notes = notes::encode(&lua, pattern, selected)
            .map_err(|e| error(PackagePhase::Execute, "context.notes", e))?;
        let controls = controls::encode(&lua, pattern)
            .map_err(|e| error(PackagePhase::Execute, "context.controls", e))?;
        let new_notes = lua.create_table().map_err(execution_error)?;
        let scale = lua.create_table().map_err(execution_error)?;
        scale
            .raw_set("root", options.scale_root)
            .map_err(execution_error)?;
        scale
            .raw_set("intervals", options.scale_intervals)
            .map_err(execution_error)?;
        context
            .raw_set("notes", notes.clone())
            .map_err(execution_error)?;
        context
            .raw_set("controls", controls.clone())
            .map_err(execution_error)?;
        context
            .raw_set("new_notes", new_notes.clone())
            .map_err(execution_error)?;
        context.raw_set("scale", scale).map_err(execution_error)?;
        context
            .raw_set("length", pattern.length().0)
            .map_err(execution_error)?;
        context.raw_set("ppq", PPQ).map_err(execution_error)?;
        context
            .raw_set("grid_ticks", options.grid.ticks().0)
            .map_err(execution_error)?;
        let params = lua.create_table().map_err(execution_error)?;
        for (key, value) in &self.values {
            match value {
                ParameterValue::Text(value) => params.raw_set(key.as_str(), value.as_str()),
                ParameterValue::Number(value) => params.raw_set(key.as_str(), *value),
                ParameterValue::Bool(value) => params.raw_set(key.as_str(), *value),
                ParameterValue::Empty => unreachable!("los paquetes exigen valores"),
            }
            .map_err(execution_error)?;
        }
        let result: MultiValue = run
            .call((context.clone(), params))
            .map_err(execution_error)?;
        if result.len() > 1 || result.iter().any(|value| !matches!(value, Value::Nil)) {
            return Err(error(
                PackagePhase::Result,
                "return",
                "run debe devolver nil".into(),
            ));
        }
        for (key, original) in [
            ("notes", &notes),
            ("controls", &controls),
            ("new_notes", &new_notes),
        ] {
            let value: Value = context.raw_get(key).map_err(execution_error)?;
            if !matches!(value, Value::Table(table) if table.to_pointer() == original.to_pointer())
            {
                return Err(error(
                    PackagePhase::Result,
                    key,
                    "No se puede sustituir el arreglo del contexto".into(),
                ));
            }
        }
        let mut edit =
            output::decode(notes, pattern).map_err(|e| error(PackagePhase::Result, "notes", e))?;
        controls::decode(controls, pattern, &mut edit.pattern)
            .map_err(|e| error(PackagePhase::Result, "controls", e))?;
        output::append_generated(new_notes, pattern, selected, &mut edit)
            .map_err(|e| error(PackagePhase::Result, "new_notes", e))?;
        Ok(edit)
    }
}
