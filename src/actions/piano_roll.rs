//! Parámetros musicales compartidos por comandos, catálogo y formulario del piano roll.
use super::{ActionParameter, ActionSpec, Argument, ParameterKind};

const fn number(
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    min: f64,
    max: f64,
    default: &'static str,
    whole: bool,
) -> ActionParameter {
    ActionParameter {
        key,
        label,
        hint: unit,
        kind: ParameterKind::Number {
            unit,
            min,
            min_exclusive: false,
            max,
            step: 1.0,
            whole,
        },
        default,
        empty_value: None,
        argument: Argument::Positional,
    }
}
const fn text(
    key: &'static str,
    label: &'static str,
    hint: &'static str,
    default: &'static str,
) -> ActionParameter {
    ActionParameter {
        key,
        label,
        hint,
        kind: ParameterKind::Text,
        default,
        empty_value: None,
        argument: Argument::Positional,
    }
}

pub static SPECS: &[ActionSpec] = &[
    ActionSpec {
        command: "midi.lane.select",
        description: "Seleccionar un carril por su nombre estable; no modifica el contenido musical.",
        parameters: &[text(
            "lane",
            "Carril",
            "velocity, release_velocity, pan, probability, pitch, pressure, cc1, cc2, cc4, cc7, cc10, cc11, cc64, cc71, cc74",
            "velocity",
        )],
    },
    ActionSpec {
        command: "midi.cc",
        description: "Inserta o actualiza un controlador MIDI en una posición musical del patrón (origen 0).",
        parameters: &[
            number("controller", "Controlador CC", "CC", 0.0, 127.0, "1", true),
            number("value", "Valor", "MIDI", 0.0, 127.0, "0", true),
            number(
                "beat",
                "Posición",
                "negras desde inicio del patrón",
                0.0,
                131072.0,
                "0",
                false,
            ),
        ],
    },
    ActionSpec {
        command: "midi.pitch",
        description: "Pitch bend del canal; 0 centra el tono. El rango audible depende del instrumento.",
        parameters: &[
            number(
                "value",
                "Pitch bend",
                "MIDI firmado",
                -8192.0,
                8191.0,
                "0",
                true,
            ),
            number(
                "beat",
                "Posición",
                "negras desde inicio del patrón",
                0.0,
                131072.0,
                "0",
                false,
            ),
        ],
    },
    ActionSpec {
        command: "midi.pressure",
        description: "Presión de canal; el instrumento decide cómo responde al aftertouch.",
        parameters: &[
            number("value", "Presión", "MIDI", 0.0, 127.0, "0", true),
            number(
                "beat",
                "Posición",
                "negras desde inicio del patrón",
                0.0,
                131072.0,
                "0",
                false,
            ),
        ],
    },
    ActionSpec {
        command: "midi.clip.channel",
        description: "Canal de salida MIDI del clip (1–16). El sampler sfizz recibe todos los canales en su misma instancia.",
        parameters: &[number(
            "channel",
            "Canal MIDI",
            "canal",
            1.0,
            16.0,
            "1",
            true,
        )],
    },
    ActionSpec {
        command: "midi.clip.rename",
        description: "Nombre del clip MIDI activo.",
        parameters: &[text(
            "name",
            "Nombre",
            "Nombre mostrado en Arrangement y piano roll",
            "Clip MIDI",
        )],
    },
    ActionSpec {
        command: "midi.clip.length",
        description: "Longitud del patrón en compases del proyecto. No elimina notas: rechaza un final anterior a las notas existentes.",
        parameters: &[number(
            "bars", "Longitud", "compases", 0.0625, 4096.0, "4", false,
        )],
    },
    ActionSpec {
        command: "midi.draw.length",
        description: "Duración de las nuevas notas; independiente de la rejilla.",
        parameters: &[number(
            "division",
            "Figura 1/",
            "denominador musical: 1, 2, 4, 8, 16, 32, 64, 128",
            1.0,
            128.0,
            "8",
            true,
        )],
    },
    ActionSpec {
        command: "midi.grid",
        description: "Resolución del snap y del cursor; no modifica las notas existentes.",
        parameters: &[number(
            "division",
            "Rejilla 1/",
            "denominador musical: 1, 2, 4, 8, 16, 32, 64, 128",
            1.0,
            128.0,
            "16",
            true,
        )],
    },
    ActionSpec {
        command: "midi.scale.root",
        description: "Tónica para el resaltado y el ajuste de notas a escala.",
        parameters: &[text(
            "root",
            "Tónica",
            "C, C#, D, D#, E, F, F#, G, G#, A, A#, B (también bemoles)",
            "C",
        )],
    },
    ActionSpec {
        command: "midi.scale.mode",
        description: "Escala para el resaltado y el ajuste de notas.",
        parameters: &[text(
            "mode",
            "Escala",
            "major o minor (mayor o menor natural)",
            "minor",
        )],
    },
    ActionSpec {
        command: "midi.humanize.amount",
        description: "Cantidad usada por el botón H. Cambiarla no transforma notas hasta ejecutar Humanize.",
        parameters: &[number(
            "amount",
            "Humanización",
            "%",
            0.0,
            100.0,
            "50",
            true,
        )],
    },
];

pub fn find(command: &str) -> Option<&'static ActionSpec> {
    SPECS.iter().find(|spec| spec.command == command)
}
