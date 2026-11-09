//! Parámetros FX reutilizados por los formularios y las invocaciones de texto.
use super::{ActionParameter, ActionSpec, Argument, ParameterKind};
pub(super) static RENAME: ActionSpec = ActionSpec {
    command: "fx.rename",
    description: "Nombre del efecto o contenedor seleccionado.",
    parameters: &[ActionParameter {
        key: "name",
        label: "Nombre",
        hint: "Conserva la identidad y posición del nodo",
        kind: ParameterKind::Text,
        default: "FX",
        empty_value: None,
        argument: Argument::Positional,
    }],
};
pub(super) static PARAMETERS: ActionSpec = ActionSpec {
    command: "fx.parameters",
    description: "Parámetros de esta instancia de Hay Utility en la pista.",
    parameters: &[ActionParameter {
        key: "gain",
        label: "Ganancia",
        hint: "0× silencia · 1× conserva el nivel · 2× duplica amplitud",
        kind: ParameterKind::Number {
            unit: "×",
            min: 0.0,
            min_exclusive: false,
            max: 2.0,
            step: 0.1,
            whole: false,
        },
        default: "1",
        empty_value: None,
        argument: Argument::Positional,
    }],
};
