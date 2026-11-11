use super::{ActionParameter, ActionSpec, Argument, ParameterKind};
pub(super) static LOAD: ActionSpec = ActionSpec {
    command: "instrument.vst3.load",
    description: "Carga un instrumento VST3 en la pista seleccionada (una instancia por sesión).",
    parameters: &[ActionParameter {
        key: "bundle",
        label: "Bundle VST3",
        hint: "Ruta Linux .vst3; para Windows selecciona el wrapper de yabridge",
        kind: ParameterKind::Text,
        default: "",
        empty_value: None,
        argument: Argument::Positional,
    }],
};

pub(super) static EFFECT: ActionSpec = ActionSpec {
    command: "fx.vst3.load",
    description: "Inserta un efecto VST3 en la cadena de la pista seleccionada.",
    parameters: &[ActionParameter {
        key: "bundle",
        label: "Bundle VST3",
        hint: "Ruta .vst3 Linux o wrapper de yabridge",
        kind: ParameterKind::Text,
        default: "",
        empty_value: None,
        argument: Argument::Positional,
    }],
};
