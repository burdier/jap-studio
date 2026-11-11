//! Copias de metadatos para formularios con vida útil independiente del registro.
use super::{ActionParameter, ActionSpec, OwnedActionSpec, ParameterKind};

impl<S: AsRef<str>> ActionParameter<S> {
    pub fn to_owned(&self) -> ActionParameter<String> {
        let kind = match &self.kind {
            ParameterKind::Text => ParameterKind::Text,
            ParameterKind::Bool => ParameterKind::Bool,
            ParameterKind::Number {
                unit,
                min,
                min_exclusive,
                max,
                step,
                whole,
            } => ParameterKind::Number {
                unit: unit.as_ref().into(),
                min: *min,
                min_exclusive: *min_exclusive,
                max: *max,
                step: *step,
                whole: *whole,
            },
        };
        ActionParameter {
            key: self.key.as_ref().into(),
            label: self.label.as_ref().into(),
            hint: self.hint.as_ref().into(),
            default: self.default.as_ref().into(),
            empty_value: self.empty_value.as_ref().map(|value| value.as_ref().into()),
            argument: self.argument,
            kind,
        }
    }
}

impl<S: AsRef<str>, P: AsRef<[ActionParameter<S>]>> ActionSpec<S, P> {
    pub fn to_owned(&self) -> OwnedActionSpec {
        ActionSpec {
            command: self.command.as_ref().into(),
            description: self.description.as_ref().into(),
            parameters: self
                .parameters
                .as_ref()
                .iter()
                .map(ActionParameter::to_owned)
                .collect(),
        }
    }
}
