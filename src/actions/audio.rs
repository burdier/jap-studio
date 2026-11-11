//! Acciones de dispositivos; contratos independientes de CPAL y GPUI.
use super::{ActionParameter, ActionSpec, Argument, ParameterKind};
const fn text(label: &'static str, hint: &'static str) -> ActionParameter {
    ActionParameter {
        key: "device",
        label,
        hint,
        kind: ParameterKind::Text,
        default: "default",
        empty_value: None,
        argument: Argument::Positional,
    }
}
const fn number(
    label: &'static str,
    unit: &'static str,
    min: f64,
    max: f64,
    default: &'static str,
) -> ActionParameter {
    ActionParameter {
        key: "value",
        label,
        hint: unit,
        kind: ParameterKind::Number {
            unit,
            min,
            min_exclusive: false,
            max,
            step: 1.0,
            whole: true,
        },
        default,
        empty_value: None,
        argument: Argument::Positional,
    }
}
pub static SPECS: &[ActionSpec] = &[
    ActionSpec {
        command: "audio.output",
        description: "Seleccionar y guardar la salida común. Detiene la reproducción al aplicar.",
        parameters: &[text(
            "Interfaz de salida",
            "Número de :list device, ID, nombre exacto o default",
        )],
    },
    ActionSpec {
        command: "audio.input",
        description: "Seleccionar entrada para el medidor. La grabación de audio a pistas aún no está implementada.",
        parameters: &[text(
            "Interfaz de entrada",
            "Número de :list device, ID, nombre exacto o default",
        )],
    },
    ActionSpec {
        command: "audio.driver",
        description: "Cambiar el driver del borrador y consultar sus dispositivos. Usa audio.apply para guardar.",
        parameters: &[text("Driver", "Nombre de un backend disponible o default")],
    },
    ActionSpec {
        command: "audio.rate",
        description: "Frecuencia común de reproducción. Se valida contra la interfaz.",
        parameters: &[number("Frecuencia", "Hz · 0 = Auto", 0., 384000., "0")],
    },
    ActionSpec {
        command: "audio.buffer",
        description: "Tamaño solicitado al driver. Un valor pequeño reduce latencia y aumenta carga; 0 usa Auto.",
        parameters: &[number("Buffer", "samples", 0., 8192., "0")],
    },
    ActionSpec {
        command: "audio.output.channel",
        description: "Primer canal físico de la pareja estéreo (1 selecciona 1/2, 3 selecciona 3/4).",
        parameters: &[number("Primer canal", "canal físico", 1., 255., "1")],
    },
    ActionSpec {
        command: "audio.input.channel",
        description: "Canal físico mono para el medidor de entrada.",
        parameters: &[number("Canal", "canal físico", 1., 255., "1")],
    },
];
pub fn find(command: &str) -> Option<&'static ActionSpec> {
    SPECS.iter().find(|s| s.command == command)
}
