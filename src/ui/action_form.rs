//! Formulario declarativo: estado de controles Kit y validación del contrato headless.
mod defaults;
mod export;
mod input;
mod submit;

use super::{DawApp, components::parameter_field::ParameterField};
use elevate::{
    actions::{ActionSpec, OwnedActionSpec, ParameterValues},
    catalog::CatalogEntry,
    scripting::packages::PackageForm,
};
use gpui::{App, Context, FocusHandle, ScrollHandle, Subscription, Window, WindowId};
use std::collections::BTreeMap;

pub(super) struct ActionForm {
    pub title: String,
    pub spec: OwnedActionSpec,
    pub fields: Vec<ParameterField>,
    pub error: Option<String>,
    pub scroll: ScrollHandle,
    pub focus: FocusHandle,
    pub window_id: WindowId,
    pub export: Option<export::ExportForm>,
    fx_target: Option<super::fx::FxTarget>,
    tool: Option<PackageForm>,
    return_focus: Option<FocusHandle>,
    _subscriptions: Vec<Subscription>,
}

impl ActionForm {
    fn new(
        title: String,
        spec: OwnedActionSpec,
        defaults: BTreeMap<&str, String>,
        window: &mut Window,
        cx: &mut Context<DawApp>,
    ) -> Self {
        let fields = spec
            .parameters
            .iter()
            .map(|parameter| {
                let initial = defaults
                    .get(parameter.key.as_str())
                    .cloned()
                    .unwrap_or_else(|| parameter.default.clone());
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
            export: None,
            fx_target: None,
            tool: None,
            return_focus: window.focused(cx),
            _subscriptions: subscriptions,
        }
    }

    fn values(&self, cx: &App) -> Result<ParameterValues<String>, String> {
        self.spec
            .parameters
            .iter()
            .zip(&self.fields)
            .map(|(parameter, field)| {
                field
                    .value(parameter, cx)
                    .map(|value| (parameter.key.clone(), value))
            })
            .collect()
    }

    fn command(&self, cx: &App) -> Result<String, String> {
        let mut values = self.values(cx)?;
        if let Some(export) = &self.export {
            export.add_formats(&mut values)?;
        }
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
        let mut defaults = self.action_defaults(spec.command, fx_target);
        let export = (spec.command == "export").then(|| self.init_export_form(&mut defaults));
        let mut form = ActionForm::new(entry.name.into(), spec.to_owned(), defaults, window, cx);
        form.fx_target = fx_target;
        form.export = export;
        self.show_action_form(form, window, cx);
    }

    pub(in crate::ui) fn open_tool_action_form(
        &mut self,
        tool: PackageForm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut form = ActionForm::new(
            tool.package().descriptor().name.clone(),
            tool.package().action_spec(),
            BTreeMap::new(),
            window,
            cx,
        );
        form.tool = Some(tool);
        self.show_action_form(form, window, cx);
    }

    fn show_action_form(&mut self, form: ActionForm, window: &mut Window, cx: &mut Context<Self>) {
        let released = self.controller.release_all();
        self.apply_midi(released);
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
}
