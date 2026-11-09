//! Contratos headless de argumentos. La UI recoge valores; el comando recibe los mismos datos.
use std::collections::BTreeMap;
mod command;
mod fx;
pub use command::tokenize;

#[derive(Clone, Copy, Debug)]
pub enum ParameterKind {
    Text,
    Number {
        unit: &'static str,
        min: f64,
        min_exclusive: bool,
        max: f64,
        /// Incremento de edición, no cuantiza el valor escrito.
        step: f64,
        whole: bool,
    },
    Bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Argument {
    Positional,
    Flag(&'static str),
    Switch { on: &'static str, off: &'static str },
}

#[derive(Clone, Copy, Debug)]
pub struct ActionParameter {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub kind: ParameterKind,
    pub default: &'static str,
    /// None exige un valor; Some traduce un campo vacío al valor del comando.
    pub empty_value: Option<&'static str>,
    pub argument: Argument,
}

#[derive(Clone, Debug)]
pub enum ParameterValue {
    Text(String),
    Number(f64),
    Bool(bool),
    Empty,
}

pub type ParameterValues = BTreeMap<&'static str, ParameterValue>;

#[derive(Debug)]
pub struct ActionSpec {
    pub command: &'static str,
    pub description: &'static str,
    pub parameters: &'static [ActionParameter],
}

impl ActionParameter {
    pub fn parse(&self, input: &str) -> Result<ParameterValue, String> {
        let fail = |reason: &str| format!("{}: {reason}", self.label);
        if input.trim().is_empty() {
            return if self.empty_value.is_some() {
                Ok(ParameterValue::Empty)
            } else {
                Err(fail("completa este campo"))
            };
        }
        match self.kind {
            ParameterKind::Text => {
                if input.contains(['\n', '\r', '\0']) {
                    return Err(fail("usa una sola línea de texto"));
                }
                Ok(ParameterValue::Text(input.into()))
            }
            ParameterKind::Bool => match input {
                "true" => Ok(ParameterValue::Bool(true)),
                "false" => Ok(ParameterValue::Bool(false)),
                _ => Err(fail("elige Sí o No")),
            },
            ParameterKind::Number {
                unit,
                min,
                min_exclusive,
                max,
                whole,
                ..
            } => {
                let number: f64 = input
                    .trim()
                    .replace(',', ".")
                    .parse()
                    .map_err(|_| fail(&format!("escribe un número en {unit}")))?;
                if !number.is_finite() {
                    return Err(fail("usa una cantidad finita"));
                }
                if number < min || (min_exclusive && number == min) {
                    let relation = if min_exclusive { ">" } else { "≥" };
                    return Err(fail(&format!("debe ser {relation} {min} {unit}")));
                }
                if number > max {
                    return Err(fail(&format!("debe ser ≤ {max} {unit}")));
                }
                if whole && number.fract() != 0.0 {
                    return Err(fail(&format!("usa {unit} sin fracciones")));
                }
                Ok(ParameterValue::Number(number))
            }
        }
    }
}

impl ActionSpec {
    /// Valida también llamadas headless; los consumidores no necesitan construir controles GPUI.
    pub fn command_with(&self, values: &ParameterValues) -> Result<String, String> {
        if values
            .keys()
            .any(|key| !self.parameters.iter().any(|p| p.key == *key))
        {
            return Err("Parámetro desconocido para esta acción".into());
        }
        let mut command = self.command.to_string();
        for parameter in self.parameters {
            let value = values
                .get(parameter.key)
                .ok_or_else(|| format!("Falta {}", parameter.label))?;
            let raw = match (parameter.kind, value) {
                (_, ParameterValue::Empty) => String::new(),
                (ParameterKind::Text, ParameterValue::Text(text)) => text.clone(),
                (ParameterKind::Number { .. }, ParameterValue::Number(number)) => {
                    number.to_string()
                }
                (ParameterKind::Bool, ParameterValue::Bool(boolean)) => boolean.to_string(),
                _ => return Err(format!("{}: tipo de valor incorrecto", parameter.label)),
            };
            let checked = parameter.parse(&raw)?;
            if let Argument::Switch { on, off } = parameter.argument {
                let ParameterValue::Bool(enabled) = checked else {
                    return Err(format!("{}: se esperaba un booleano", parameter.label));
                };
                command.push(' ');
                command.push_str(if enabled { on } else { off });
                continue;
            }
            let encoded = match checked {
                ParameterValue::Text(text) => quote(&text),
                ParameterValue::Empty => quote(parameter.empty_value.unwrap_or_default()),
                _ => raw,
            };
            command.push(' ');
            if let Argument::Flag(flag) = parameter.argument {
                command.push_str(flag);
                command.push(' ');
            }
            command.push_str(&encoded);
        }
        Ok(command)
    }
}

// Citas para el tokenizer de comandos de Hay; nunca se ejecuta mediante un shell.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

/// Registro compartido por catálogo, botones y futuros consumidores headless.
pub fn find(command: &str) -> Option<&'static ActionSpec> {
    match command {
        "fx.rename" => Some(&fx::RENAME),
        "fx.parameters" => Some(&fx::PARAMETERS),
        "export" => Some(&crate::export::action::SPEC),
        "file.import" => Some(&FILE_IMPORT),
        "track.height" => Some(&TRACK_HEIGHT),
        "track.rename" => Some(&TRACK_RENAME),
        "utility.gain" => Some(&UTILITY_GAIN),
        _ => None,
    }
}

static FILE_IMPORT: ActionSpec = ActionSpec {
    command: "file.import",
    description: "Inserta audio o MIDI en la pista activa, en el cursor del Arrangement.",
    parameters: &[ActionParameter {
        key: "path",
        label: "Archivo",
        hint: "Ruta completa de un archivo MIDI, WAV, FLAC, MP3, Ogg, AAC o M4A",
        kind: ParameterKind::Text,
        default: "",
        empty_value: None,
        argument: Argument::Positional,
    }],
};

static UTILITY_GAIN: ActionSpec = ActionSpec {
    command: "utility.gain",
    description: "Ganancia de Hay Utility en la salida de audición. 1× conserva el nivel original.",
    parameters: &[ActionParameter {
        key: "gain",
        label: "Ganancia",
        hint: "0× silencia · 1× nivel original · 2× doble amplitud",
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

static TRACK_RENAME: ActionSpec = ActionSpec {
    command: "track.rename",
    description: "Nombre de la pista o grupo seleccionado.",
    parameters: &[ActionParameter {
        key: "name",
        label: "Nombre",
        hint: "Un nombre para reconocer la pista",
        kind: ParameterKind::Text,
        default: "Pista",
        empty_value: None,
        argument: Argument::Positional,
    }],
};

static TRACK_HEIGHT: ActionSpec = ActionSpec {
    command: "track.height",
    description: "Altura visual de las pistas seleccionadas. No modifica el audio.",
    parameters: &[ActionParameter {
        key: "height",
        label: "Altura",
        hint: "32–240 px · las pistas compactas muestran los controles esenciales",
        kind: ParameterKind::Number {
            unit: "px",
            min: 32.0,
            min_exclusive: false,
            max: 240.0,
            step: 4.0,
            whole: false,
        },
        default: "76",
        empty_value: None,
        argument: Argument::Positional,
    }],
};
