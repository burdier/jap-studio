//! Parámetros de la acción visual Exportar y valores iniciales derivados de preferencias.
use super::{ExportOptions, Target};
use crate::actions::{ActionParameter, ActionSpec, Argument, ParameterKind};
use std::collections::BTreeMap;

pub static SPEC: ActionSpec = ActionSpec {
    command: "export",
    description: "Valores para esta exportación. Profundidad, calidad MP3 y efectos se toman de Configuración → Exportación.",
    parameters: &[
        ActionParameter {
            key: "name",
            label: "Nombre y formato",
            hint: "Ejemplo: mezcla.wav o mezcla.[wav,mp3]",
            kind: ParameterKind::Text,
            default: "export.wav",
            empty_value: None,
            argument: Argument::Flag("-n"),
        },
        ActionParameter {
            key: "directory",
            label: "Carpeta de destino",
            hint: "Ruta absoluta o relativa al proyecto",
            kind: ParameterKind::Text,
            default: "exports",
            empty_value: None,
            argument: Argument::Flag("-out"),
        },
        ActionParameter {
            key: "tracks",
            label: "Pistas",
            hint: "all para todas, o números separados por coma: 1,3",
            kind: ParameterKind::Text,
            default: "all",
            empty_value: None,
            argument: Argument::Flag("-tracks"),
        },
        ActionParameter {
            key: "start",
            label: "Inicio",
            hint: "Posición desde el comienzo del Arrangement (0)",
            kind: ParameterKind::Number {
                unit: "beats",
                min: 0.0,
                min_exclusive: false,
                max: f64::MAX,
                step: 1.0,
                whole: false,
            },
            default: "0",
            empty_value: None,
            argument: Argument::Flag("-start"),
        },
        ActionParameter {
            key: "length",
            label: "Duración",
            hint: "Vacío: hasta el final · admite fracciones de beat",
            kind: ParameterKind::Number {
                unit: "beats",
                min: 0.0,
                min_exclusive: true,
                max: f64::MAX,
                step: 1.0,
                whole: false,
            },
            default: "",
            empty_value: Some("all"),
            argument: Argument::Flag("-length"),
        },
        ActionParameter {
            key: "tail",
            label: "Cola de audio",
            hint: "Tiempo adicional después del rango musical",
            kind: ParameterKind::Number {
                unit: "s",
                min: 0.0,
                min_exclusive: false,
                max: f64::MAX,
                step: 0.1,
                whole: false,
            },
            default: "0",
            empty_value: None,
            argument: Argument::Flag("-tail"),
        },
        ActionParameter {
            key: "sample_rate",
            label: "Frecuencia de muestreo",
            hint: "8000–384000 Hz · MP3 hasta 48000 Hz",
            kind: ParameterKind::Number {
                unit: "Hz",
                min: 8000.0,
                min_exclusive: false,
                max: 384000.0,
                step: 1000.0,
                whole: true,
            },
            default: "48000",
            empty_value: None,
            argument: Argument::Flag("-sr"),
        },
        ActionParameter {
            key: "mono",
            label: "Exportar en mono",
            hint: "No: estéreo",
            kind: ParameterKind::Bool,
            default: "false",
            empty_value: None,
            argument: Argument::Switch {
                on: "-mono",
                off: "-stereo",
            },
        },
        ActionParameter {
            key: "individuals",
            label: "Un archivo por pista",
            hint: "No: mezcla master · Sí: pistas separadas",
            kind: ParameterKind::Bool,
            default: "false",
            empty_value: None,
            argument: Argument::Switch {
                on: "-individuals",
                off: "-master",
            },
        },
        ActionParameter {
            key: "lufs",
            label: "Normalizar sonoridad",
            hint: "Vacío: desactivado · Streaming -14 · Podcast -16 · Club -9",
            kind: ParameterKind::Number {
                unit: "LUFS",
                min: -40.0,
                min_exclusive: false,
                max: -1.0,
                step: 1.0,
                whole: false,
            },
            default: "",
            empty_value: Some("off"),
            argument: Argument::Flag("-lufs"),
        },
        ActionParameter {
            key: "true_peak",
            label: "Limitador True Peak",
            hint: "Vacío: desactivado · Streaming -1.0 · Mastering -0.3",
            kind: ParameterKind::Number {
                unit: "dBTP",
                min: -12.0,
                min_exclusive: false,
                max: 0.0,
                step: 0.5,
                whole: false,
            },
            default: "",
            empty_value: Some("off"),
            argument: Argument::Flag("-tp"),
        },
    ],
};

impl ExportOptions {
    pub fn action_defaults(&self) -> BTreeMap<&'static str, String> {
        let formats = self
            .formats
            .iter()
            .map(|f| f.extension())
            .collect::<Vec<_>>()
            .join(",");
        BTreeMap::from([
            ("name", format!("{}.[{formats}]", self.name)),
            ("directory", self.directory.to_string_lossy().into_owned()),
            (
                "tracks",
                if self.tracks.is_empty() {
                    "all".into()
                } else {
                    self.tracks
                        .iter()
                        .map(|n| (n + 1).to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                },
            ),
            ("start", self.start_beat.to_string()),
            (
                "length",
                self.length_beats.map_or(String::new(), |n| n.to_string()),
            ),
            ("tail", self.tail_seconds.to_string()),
            ("sample_rate", self.sample_rate.to_string()),
            ("mono", self.mono.to_string()),
            (
                "individuals",
                (self.target == Target::Individuals).to_string(),
            ),
            (
                "lufs",
                self.normalize_lufs.map_or(String::new(), |n| n.to_string()),
            ),
            (
                "true_peak",
                self.true_peak_limit_db
                    .map_or(String::new(), |n| n.to_string()),
            ),
        ])
    }
}
