//! Adaptador reutilizable entre el contrato de acciones y los controles de GPUI Kit.
use elevate::actions::{ActionParameter, ParameterKind, ParameterValue};
use gpui::{
    App, AppContext, Entity, FocusHandle, Window,
    component::input::{InputState, StepAction},
};

pub(in crate::ui) enum ParameterField {
    Input(Entity<InputState>),
    Bool { checked: bool, focus: FocusHandle },
}

impl ParameterField {
    pub fn new<S: AsRef<str>>(
        parameter: &ActionParameter<S>,
        initial: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        if matches!(parameter.kind, ParameterKind::Bool) {
            Self::Bool {
                checked: initial == "true",
                focus: cx.focus_handle(),
            }
        } else {
            Self::Input(cx.new(|cx| InputState::new(window, cx).default_value(initial)))
        }
    }

    pub fn value<S: AsRef<str>>(
        &self,
        parameter: &ActionParameter<S>,
        cx: &App,
    ) -> Result<ParameterValue, String> {
        match self {
            Self::Input(input) => parameter.parse(input.read(cx).value().as_ref()),
            Self::Bool { checked, .. } => Ok(ParameterValue::Bool(*checked)),
        }
    }

    /// Kit administra la edición; el contrato musical decide unidad, incremento y límites.
    /// Se usa el evento Step para conservar mínimos exclusivos y comas decimales.
    pub fn step<S: AsRef<str>>(
        &self,
        parameter: &ActionParameter<S>,
        action: StepAction,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (
            Self::Input(input),
            ParameterKind::Number {
                min,
                min_exclusive,
                max,
                step,
                ..
            },
        ) = (self, &parameter.kind)
        else {
            return;
        };
        let (min, min_exclusive, max, step) = (*min, *min_exclusive, *max, *step);
        let current = input
            .read(cx)
            .value()
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .unwrap_or(min);
        let direction = match action {
            StepAction::Increment => 1.0,
            StepAction::Decrement => -1.0,
        };
        let next = (current + direction * step).clamp(min, max);
        if min_exclusive && next <= min {
            return;
        }
        let value = format!("{next:.9}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned();
        input.update(cx, |input, cx| {
            input.replace_all(value, window, cx);
            input.select_all(window, cx);
        });
    }
}
