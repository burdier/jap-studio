//! Herramientas MIDI integradas: la misma implementación para comandos y controles.
use crate::{
    actions::{ActionParameter, ActionSpec, Argument, ParameterKind},
    musical::grid::NoteGrid,
};

#[derive(Clone, Copy)]
pub enum MidiTool {
    Accent,
    Quantize,
    Humanize,
    Subdivide,
    Gate,
    Swing,
    ClampScale,
    Velocity,
    ReleaseVelocity,
    Probability,
    Pan,
    Random,
}

#[derive(Clone, Copy)]
pub struct MidiOptions {
    pub grid: NoteGrid,
    pub amount: u8,
    pub value: u8,
    pub pan: i8,
    pub strength: u8,
    pub scale_root: u8,
    pub scale_intervals: u16,
}

impl Default for MidiOptions {
    fn default() -> Self {
        Self {
            grid: NoteGrid::default(),
            amount: 50,
            pan: 0,
            value: 100,
            strength: 100,
            scale_root: 0,
            scale_intervals: 0x05ad,
        }
    }
}

impl MidiTool {
    pub fn from_command(command: &str) -> Option<Self> {
        match command {
            "midi.script.accent" => Some(Self::Accent),
            "midi.quantize" => Some(Self::Quantize),
            "midi.humanize" => Some(Self::Humanize),
            "midi.subdivide" => Some(Self::Subdivide),
            "midi.gate" => Some(Self::Gate),
            "midi.swing" => Some(Self::Swing),
            "midi.scale.clamp" => Some(Self::ClampScale),
            "midi.velocity" => Some(Self::Velocity),
            "midi.release_velocity" => Some(Self::ReleaseVelocity),
            "midi.pan" => Some(Self::Pan),
            "midi.probability" => Some(Self::Probability),
            "midi.random" => Some(Self::Random),
            _ => None,
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Self::Accent => include_str!("../../assets/scripts/midi/accent.lua"),
            Self::Quantize => include_str!("../../assets/scripts/midi/quantize.lua"),
            Self::Humanize => include_str!("../../assets/scripts/midi/humanize.lua"),
            Self::Subdivide => include_str!("../../assets/scripts/midi/subdivide.lua"),
            Self::Gate => include_str!("../../assets/scripts/midi/gate.lua"),
            Self::Swing => include_str!("../../assets/scripts/midi/swing.lua"),
            Self::ClampScale => include_str!("../../assets/scripts/midi/clamp_scale.lua"),
            Self::ReleaseVelocity => include_str!("../../assets/scripts/midi/release_velocity.lua"),
            Self::Pan => include_str!("../../assets/scripts/midi/pan.lua"),
            Self::Probability => include_str!("../../assets/scripts/midi/probability.lua"),
            Self::Velocity => include_str!("../../assets/scripts/midi/velocity.lua"),
            Self::Random => include_str!("../../assets/scripts/midi/random.lua"),
        }
    }

    pub fn options(self, args: &[String], mut current: MidiOptions) -> Result<MidiOptions, String> {
        match (self, args) {
            (_, []) => Ok(current),
            (Self::Quantize | Self::Subdivide, [value]) => {
                current.grid = NoteGrid::new(value.parse().map_err(|_| "Denominador inválido")?)?;
                Ok(current)
            }
            (Self::Quantize, [grid, strength]) => {
                current.grid = NoteGrid::new(grid.parse().map_err(|_| "Denominador inválido")?)?;
                current.strength = strength.parse().map_err(|_| "Strength: 0–100")?;
                if current.strength > 100 {
                    return Err("Strength: 0–100".into());
                }
                Ok(current)
            }
            (Self::Humanize | Self::Probability, [value]) => {
                current.amount = value.parse().map_err(|_| "Intensidad: 0–100")?;
                if current.amount > 100 {
                    return Err("Intensidad: 0–100".into());
                }
                Ok(current)
            }
            (Self::Gate | Self::Swing, [value]) => {
                current.amount = value.parse().map_err(|_| "Intensidad: 0–100")?;
                if current.amount > 100 {
                    return Err("Intensidad: 0–100".into());
                }
                Ok(current)
            }
            (Self::Random, [value]) => {
                current.amount = value.parse().map_err(|_| "Intensidad: 0–100")?;
                if current.amount > 100 {
                    return Err("Intensidad: 0–100".into());
                }
                Ok(current)
            }
            (Self::Pan, [value]) => {
                current.pan = value.parse().map_err(|_| "Pan: -64 (L) a +63 (R)")?;
                if !(-64..=63).contains(&current.pan) {
                    return Err("Pan: -64 (L) a +63 (R)".into());
                }
                Ok(current)
            }
            (Self::ReleaseVelocity, [value]) => {
                current.value = value.parse().map_err(|_| "Release velocity: 0–127")?;
                if current.value > 127 {
                    return Err("Release velocity: 0–127".into());
                }
                Ok(current)
            }
            (Self::Velocity, [value]) => {
                current.value = value.parse().map_err(|_| "Velocity: 1–127")?;
                if !(1..=127).contains(&current.value) {
                    return Err("Velocity: 1–127".into());
                }
                Ok(current)
            }
            _ => Err("Argumentos MIDI inválidos".into()),
        }
    }
}

const GRID: ActionParameter = ActionParameter {
    key: "grid",
    label: "Rejilla 1/",
    hint: "1, 2, 4, 8, 16, 32, 64 o 128",
    kind: ParameterKind::Number {
        unit: "denominador",
        min: 1.0,
        min_exclusive: false,
        max: 128.0,
        step: 1.0,
        whole: true,
    },
    default: "16",
    empty_value: None,
    argument: Argument::Positional,
};

pub static QUANTIZE: ActionSpec = ActionSpec {
    command: "midi.quantize",
    description: "Cuantizar la selección o todas las notas mediante Lua",
    parameters: &[
        GRID,
        ActionParameter {
            key: "strength",
            label: "Strength",
            hint: "100% ajusta completamente a la rejilla",
            kind: ParameterKind::Number {
                unit: "%",
                min: 0.0,
                min_exclusive: false,
                max: 100.0,
                step: 5.0,
                whole: true,
            },
            default: "100",
            empty_value: None,
            argument: Argument::Positional,
        },
    ],
};
pub static SUBDIVIDE: ActionSpec = ActionSpec {
    command: "midi.subdivide",
    description: "Dividir las notas seleccionadas en los límites de la rejilla",
    parameters: &[GRID],
};
pub static HUMANIZE: ActionSpec = ActionSpec {
    command: "midi.humanize",
    description: "Humanizar tiempo y velocity mediante Lua",
    parameters: &[ActionParameter {
        key: "amount",
        label: "Intensidad",
        hint: "Variación de tiempo y velocity, reproducible por ID",
        kind: ParameterKind::Number {
            unit: "%",
            min: 0.0,
            min_exclusive: false,
            max: 100.0,
            step: 5.0,
            whole: true,
        },
        default: "50",
        empty_value: None,
        argument: Argument::Positional,
    }],
};

const AMOUNT: ActionParameter = ActionParameter {
    key: "amount",
    label: "Cantidad",
    hint: "De 0 a 100%",
    kind: ParameterKind::Number {
        unit: "%",
        min: 0.0,
        min_exclusive: false,
        max: 100.0,
        step: 5.0,
        whole: true,
    },
    default: "50",
    empty_value: None,
    argument: Argument::Positional,
};
pub static GATE: ActionSpec = ActionSpec {
    command: "midi.gate",
    description: "Ajustar la duración de las notas seleccionadas con Lua.",
    parameters: &[AMOUNT],
};
pub static SWING: ActionSpec = ActionSpec {
    command: "midi.swing",
    description: "Aplicar swing a las notas fuera de pulso con Lua.",
    parameters: &[AMOUNT],
};
pub static CLAMP_SCALE: ActionSpec = ActionSpec {
    command: "midi.scale.clamp",
    description: "Ajustar las notas seleccionadas a la escala y tónica activas.",
    parameters: &[],
};
pub static RANDOM: ActionSpec = ActionSpec {
    command: "midi.random",
    description: "Aleatorizar tiempo y velocity de la selección mediante Lua.",
    parameters: &[AMOUNT],
};
pub static VELOCITY: ActionSpec = ActionSpec {
    command: "midi.velocity",
    description: "Fijar velocity de la selección mediante Lua.",
    parameters: &[ActionParameter {
        key: "value",
        label: "Velocity",
        hint: "De 1 a 127",
        kind: ParameterKind::Number {
            unit: "velocity",
            min: 1.0,
            min_exclusive: false,
            max: 127.0,
            step: 1.0,
            whole: true,
        },
        default: "100",
        empty_value: None,
        argument: Argument::Positional,
    }],
};

pub static RELEASE_VELOCITY: ActionSpec = ActionSpec {
    command: "midi.release_velocity",
    description: "Velocidad de NoteOff de las notas seleccionadas (todas si no hay selección). La respuesta audible depende del instrumento.",
    parameters: &[ActionParameter {
        key: "value",
        label: "Release velocity",
        hint: "0–127; sfizz recibe este valor al soltar la nota",
        kind: ParameterKind::Number {
            unit: "velocity",
            min: 0.0,
            min_exclusive: false,
            max: 127.0,
            step: 1.0,
            whole: true,
        },
        default: "64",
        empty_value: None,
        argument: Argument::Positional,
    }],
};
pub static PROBABILITY: ActionSpec = ActionSpec {
    command: "midi.probability",
    description: "Probabilidad por nota y vuelta: 0% nunca dispara, 100% siempre.",
    parameters: &[ActionParameter {
        key: "amount",
        label: "Probabilidad",
        default: "100",
        ..AMOUNT
    }],
};

pub static PAN: ActionSpec = ActionSpec {
    command: "midi.pan",
    description: "Panorama individual de las notas. Audio por voz en FluidSynth; sfizz y RustySynth conservan el dato sin aplicarlo.",
    parameters: &[ActionParameter {
        key: "pan",
        label: "Pan por nota",
        hint: "-64 izquierda, 0 centro, +63 derecha; requiere FluidSynth para escucharlo",
        kind: ParameterKind::Number {
            unit: "pan",
            min: -64.0,
            min_exclusive: false,
            max: 63.0,
            step: 1.0,
            whole: true,
        },
        default: "0",
        empty_value: None,
        argument: Argument::Positional,
    }],
};
