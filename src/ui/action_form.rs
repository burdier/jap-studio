//! Formulario declarativo: estado de controles Kit y validación del contrato headless.
mod input;

use super::{DawApp, components::parameter_field::ParameterField};
use elevate::{
    actions::{ActionSpec, ParameterValues},
    catalog::CatalogEntry,
    export::ExportOptions,
};
use gpui::{App, Context, FocusHandle, ScrollHandle, Subscription, Window, WindowId};
use std::collections::BTreeMap;

pub(super) struct ActionForm {
    pub title: &'static str,
    pub spec: &'static ActionSpec,
    pub fields: Vec<ParameterField>,
    pub error: Option<String>,
    pub scroll: ScrollHandle,
    pub focus: FocusHandle,
    pub window_id: WindowId,
    fx_target: Option<super::fx::FxTarget>,
    return_focus: Option<FocusHandle>,
    _subscriptions: Vec<Subscription>,
}

impl ActionForm {
    fn new(
        title: &'static str,
        spec: &'static ActionSpec,
        defaults: BTreeMap<&str, String>,
        window: &mut Window,
        cx: &mut Context<DawApp>,
    ) -> Self {
        let fields = spec
            .parameters
            .iter()
            .map(|parameter| {
                let initial = defaults
                    .get(parameter.key)
                    .cloned()
                    .unwrap_or_else(|| parameter.default.into());
                ParameterField::new(parameter, initial, window, cx)
            })
            .collect::<Vec<_>>();
        let subscriptions = input::subscribe_fields(&fields, window, cx);
        Self {
            title,
            spec,
            fields,
            error: None,
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            window_id: window.window_handle().window_id(),
            fx_target: None,
            return_focus: window.focused(cx),
            _subscriptions: subscriptions,
        }
    }

    fn command(&self, cx: &App) -> Result<String, String> {
        let values: ParameterValues = self
            .spec
            .parameters
            .iter()
            .zip(&self.fields)
            .map(|(parameter, field)| {
                field
                    .value(parameter, cx)
                    .map(|value| (parameter.key, value))
            })
            .collect::<Result<_, _>>()?;
        self.spec.command_with(&values)
    }
}

impl DawApp {
    pub(in crate::ui) fn open_action_form(
        &mut self,
        entry: &'static CatalogEntry,
        spec: &'static ActionSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let fx_target = if spec.command.starts_with("fx.") {
            self.fx
                .form_target
                .take()
                .or_else(|| self.selected_fx_target())
        } else {
            None
        };
        let defaults = match spec.command {
            "fx.rename" => BTreeMap::from([(
                "name",
                fx_target
                    .and_then(|target| self.fx_node(target))
                    .map_or(String::new(), |node| node.name.clone()),
            )]),
            "fx.parameters" => {
                let gain = fx_target
                    .and_then(|target| self.fx_node(target))
                    .and_then(|node| match &node.kind {
                        elevate::musical::fx::FxKind::Effect { parameters, .. } => {
                            parameters.get(&0).copied()
                        }
                        _ => None,
                    })
                    .unwrap_or(1.0);
                BTreeMap::from([("gain", gain.to_string())])
            }
            "export" => self.settings.export.saved.action_defaults(),
            "track.height" => {
                BTreeMap::from([("height", self.track_height(self.cursor_row).to_string())])
            }
            "track.rename" => BTreeMap::from([(
                "name",
                self.tracks
                    .get(self.cursor_row)
                    .map_or(String::new(), |t| t.name.to_string()),
            )]),
            "utility.gain" => BTreeMap::from([("gain", self.utility.state().gain.to_string())]),
            _ => BTreeMap::new(),
        };
        self.track_delete_pending = false;
        let released = self.controller.release_all();
        self.apply_midi(released);
        let mut form = ActionForm::new(entry.name, spec, defaults, window, cx);
        form.fx_target = fx_target;
        window.focus(&form.focus, cx);
        let first_input = match form.fields.first() {
            Some(ParameterField::Input(input)) => Some(input.clone()),
            _ => None,
        };
        self.action_form = Some(form);
        if let Some(input) = first_input {
            input.update(cx, |input, cx| {
                input.focus(window, cx);
                input.select_all(window, cx);
            });
        } else {
            // Kit crea el foco de la casilla/botón al montarlo; enfocar tras ese primer frame.
            let focus = self
                .action_form
                .as_ref()
                .expect("formulario creado")
                .focus
                .clone();
            cx.on_next_frame(window, move |this, window, cx| {
                if this
                    .action_form
                    .as_ref()
                    .is_some_and(|form| form.focus == focus)
                    && focus.is_focused(window)
                {
                    window.focus_next(cx);
                }
            });
        }
        cx.notify();
    }

    pub(in crate::ui) fn close_action_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(form) = self.action_form.take() {
            window.focus(form.return_focus.as_ref().unwrap_or(&self.focus_handle), cx);
            cx.notify();
        }
    }

    /// OK y Enter reciben los valores reales de Kit y pasan por el dispatcher compartido.
    pub(in crate::ui) fn submit_action_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = self.action_form.as_ref() else {
            return;
        };
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
            } else {
                Ok(command)
            }
        });
        match result {
            Ok(command) => {
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
                    self.execute_command(&command);
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
