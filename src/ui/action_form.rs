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
            "midi.cc" | "midi.pitch" | "midi.pressure" => {
                let mut values = BTreeMap::from([("beat", (self.piano_roll.cursor.0 as f64 / elevate::musical::time::PPQ as f64).to_string())]);
                if spec.command == "midi.cc" {
                    let cc = match self.piano_roll.active_lane.controller() { Some(elevate::musical::automation::ControlLane::Cc(cc)) => cc, _ => 1 };
                    values.insert("controller", cc.to_string());
                }
                values
            }
            "midi.clip.channel" => BTreeMap::from([("channel", self.timeline.midi_clip.as_ref().map_or(1, |clip| clip.channel).to_string())]),
            "midi.clip.rename" => BTreeMap::from([("name", self.timeline.midi_clip.as_ref().map_or(String::new(), |clip| clip.name.clone()))]),
            "midi.clip.length" => BTreeMap::from([("bars", self.timeline.midi_clip.as_ref().map_or(4.0, |clip| clip.pattern_beats() / self.timeline.meter.beats_per_bar()).to_string())]),
            "midi.draw.length" => BTreeMap::from([("division", self.piano_roll.draw_length.denominator().to_string())]),
            "midi.grid" => BTreeMap::from([("division", self.piano_roll.resolution.denominator().to_string())]),
            "midi.scale.root" => BTreeMap::from([("root", ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"][usize::from(self.piano_roll.scale_root)].to_string())]),
            "midi.scale.mode" => BTreeMap::from([("mode", if self.piano_roll.scale_minor { "minor" } else { "major" }.to_string())]),
            "midi.humanize.amount" => BTreeMap::from([("amount", self.piano_roll.humanize_amount.to_string())]),
            "midi.quantize" => BTreeMap::from([
                ("grid", self.piano_roll.resolution.denominator().to_string()),
                ("strength", "100".into()),
            ]),
            "midi.subdivide" => BTreeMap::from([("grid", self.piano_roll.resolution.denominator().to_string())]),
            "midi.humanize" => BTreeMap::from([("amount", self.piano_roll.humanize_amount.to_string())]),
            "midi.gate" => BTreeMap::from([("amount", "100".into())]),
            "midi.swing" => BTreeMap::from([("amount", "0".into())]),
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
            "tempo.set" => BTreeMap::from([("bpm", self.timeline.tempo.bpm().to_string())]),
            "meter.set" => BTreeMap::from([
                ("numerator", self.timeline.meter.numerator().to_string()),
                ("denominator", self.timeline.meter.denominator().to_string()),
            ]),
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
            } else if let Some(tool) = elevate::scripting::actions::MidiTool::from_command(form.spec.command) {
                let parts = elevate::actions::tokenize(&command)?;
                tool.options(&parts[1..], elevate::scripting::actions::MidiOptions::default())?;
                Ok(command)
            } else if form.spec.command == "meter.set" {
                let mut parts = command.split_whitespace().skip(1);
                let numerator = parts.next().unwrap_or_default().parse().map_err(|_| "Numerador inválido")?;
                let denominator = parts.next().unwrap_or_default().parse().map_err(|_| "Denominador inválido")?;
                elevate::musical::time::Meter::new(numerator, denominator)
                    .map_err(|error| error.to_string())?;
                Ok(command)
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
