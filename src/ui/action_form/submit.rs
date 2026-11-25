//! Confirmación por la ruta nativa o por una invocación Lua de la generación capturada.
use crate::ui::DawApp;
use elevate::{export::ExportOptions, scripting::packages::PackagePhase};
use gpui::{Context, Window};

impl DawApp {
    /// OK y Enter reciben los valores reales de Kit y pasan por el dispatcher compartido.
    pub(in crate::ui) fn submit_action_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = self.action_form.as_ref() else {
            return;
        };
        if form
            .export
            .as_ref()
            .is_some_and(|export| export.running || export.choosing_folder)
        {
            return;
        }
        if form
            .export
            .as_ref()
            .is_some_and(|export| export.completed.is_some())
        {
            self.close_action_form(window, cx);
            return;
        }
        if let Some(tool) = &form.tool {
            let result = tool
                .validate_generation(&self.lua_tools.registry)
                .and_then(|()| {
                    form.values(cx).map_err(|e| {
                        tool.package()
                            .error(PackagePhase::Arguments, "parameters", e)
                    })
                })
                .and_then(|values| tool.submit(&self.lua_tools.registry, &values));
            match result {
                Ok(invocation) => {
                    self.close_action_form(window, cx);
                    self.start_tool_invocation(invocation, cx);
                    self.sync_windows(cx);
                }
                Err(error) => {
                    if let Some(form) = self.action_form.as_mut() {
                        form.error = Some(error.to_string());
                    }
                }
            }
            cx.notify();
            return;
        }
        let fx_target = form.fx_target;
        let result = form.command(cx).and_then(|command| {
            if form.spec.command == "export" {
                let options =
                    ExportOptions::parse_with_defaults(&command, &self.settings.export.saved)?;
                if options
                    .tracks
                    .iter()
                    .any(|index| *index >= self.tracks.len())
                {
                    return Err(format!(
                        "Las pistas disponibles son 1–{}",
                        self.tracks.len()
                    ));
                }
                options.to_command()
            } else if let Some(tool) =
                elevate::scripting::actions::MidiTool::from_command(&form.spec.command)
            {
                let parts = elevate::actions::tokenize(&command)?;
                tool.options(
                    &parts[1..],
                    elevate::scripting::actions::MidiOptions::default(),
                )?;
                Ok(command)
            } else if form.spec.command == "meter.set" {
                let mut parts = command.split_whitespace().skip(1);
                let numerator = parts
                    .next()
                    .unwrap_or_default()
                    .parse()
                    .map_err(|_| "Numerador inválido")?;
                let denominator = parts
                    .next()
                    .unwrap_or_default()
                    .parse()
                    .map_err(|_| "Denominador inválido")?;
                elevate::musical::time::Meter::new(numerator, denominator)
                    .map_err(|error| error.to_string())?;
                Ok(command)
            } else if form.spec.command == "hilo.rule" {
                let parts = elevate::actions::tokenize(&command)?;
                let steps = parts.get(1).ok_or("Escribe ramas como 1,3,2 o auto")?;
                let rule = elevate::musical::weave::WeaveRule::parse_steps(steps)?;
                let group = self
                    .tracks
                    .get(self.cursor_row)
                    .ok_or("Selecciona un grupo Hilo")?;
                if group.hilo != Some(elevate::musical::track::HiloMode::RoundRobin) {
                    return Err("Selecciona un grupo Hilo en modo Tejer".into());
                }
                let plan = elevate::musical::hilo::HiloPlan::compile(
                    &self.tracks,
                    group.id,
                    elevate::musical::track::HiloMode::RoundRobin,
                )?;
                if rule
                    .as_ref()
                    .is_some_and(|rule| !rule.valid_for(plan.instruments().count()))
                {
                    return Err("Tejer: la regla menciona una rama inexistente".into());
                }
                Ok(command)
            } else {
                Ok(command)
            }
        });
        match result {
            Ok(command) => {
                if form.export.is_some() {
                    let already_running = self.export_job.is_some();
                    self.start_export(&command);
                    if let Some(form) = &mut self.action_form
                        && let Some(export) = &mut form.export
                    {
                        export.running = !already_running && self.export_job.is_some();
                        form.error = (!export.running).then(|| self.status_message.clone());
                    }
                    if self.export_job.is_some() && !already_running {
                        let focus = self.action_form.as_ref().unwrap().focus.clone();
                        window.focus(&focus, cx);
                    }
                    cx.notify();
                    return;
                }
                if let Some(target) = fx_target {
                    if let Err(error) = self.execute_fx_at(target, &command) {
                        if let Some(form) = self.action_form.as_mut() {
                            form.error = Some(error);
                        }
                        cx.notify();
                        return;
                    }
                }
                self.close_action_form(window, cx);
                if fx_target.is_none() {
                    self.execute_command_with_context(&command, window, cx);
                }
                self.sync_windows(cx);
            }
            Err(error) => {
                if let Some(form) = self.action_form.as_mut() {
                    form.error = Some(error);
                }
            }
        }
        cx.notify();
    }
}
