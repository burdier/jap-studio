//! Carriles de expresión, propiedades de nota y controladores MIDI del piano roll.
use gpui::{Pixels, Point};

/// Opciones coherentes de parámetros para el carril inferior del piano roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RollLane {
    #[default]
    Velocity,
    ReleaseVelocity,
    Pan,
    MpeSlide,
    MpePressure,
    PitchBend,
    Probability,
    Modulation,
    Breath,
    Foot,
    Volume,
    PanCC,
    Expression,
    Sustain,
    Resonance,
    Cutoff,
    Pressure,
}

impl RollLane {
    pub fn key(self) -> &'static str {
        match self {
            Self::Velocity => "velocity",
            Self::ReleaseVelocity => "release_velocity",
            Self::Pan => "pan",
            Self::MpeSlide => "mpe_slide",
            Self::MpePressure => "mpe_pressure",
            Self::Probability => "probability",
            Self::PitchBend => "pitch",
            Self::Modulation => "cc1",
            Self::Breath => "cc2",
            Self::Foot => "cc4",
            Self::Volume => "cc7",
            Self::PanCC => "cc10",
            Self::Expression => "cc11",
            Self::Sustain => "cc64",
            Self::Resonance => "cc71",
            Self::Cutoff => "cc74",
            Self::Pressure => "pressure",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        LANE_MENU_ITEMS
            .iter()
            .find(|item| item.lane.key().eq_ignore_ascii_case(key))
            .map(|item| item.lane)
    }

    pub fn note_property(self) -> Option<elevate::musical::note_expression::NoteProperty> {
        use elevate::musical::note_expression::NoteProperty;
        match self {
            Self::Pan => Some(NoteProperty::Pan),
            Self::MpeSlide => Some(NoteProperty::Slide),
            Self::MpePressure => Some(NoteProperty::Pressure),
            Self::Velocity => Some(NoteProperty::Velocity),
            Self::ReleaseVelocity => Some(NoteProperty::ReleaseVelocity),
            Self::Probability => Some(NoteProperty::Probability),
            _ => None,
        }
    }
    pub fn note_fraction(self, note: &elevate::musical::model::NoteEvent) -> f32 {
        if let Some(property) = self.note_property() {
            property.fraction(note)
        } else {
            0.0
        }
    }

    pub fn controller(self) -> Option<elevate::musical::automation::ControlLane> {
        use elevate::musical::automation::ControlLane::{Cc, PitchBend, Pressure};
        Some(match self {
            Self::Modulation => Cc(1),
            Self::Breath => Cc(2),
            Self::Foot => Cc(4),
            Self::Volume => Cc(7),
            Self::PanCC => Cc(10),
            Self::Expression => Cc(11),
            Self::Sustain => Cc(64),
            Self::Resonance => Cc(71),
            Self::Cutoff => Cc(74),
            Self::PitchBend => PitchBend,
            Self::Pressure => Pressure,
            _ => return None,
        })
    }
    pub fn control_value(self, fraction: f32) -> i16 {
        let f = fraction.clamp(0.0, 1.0);
        match self {
            Self::PitchBend => (f * 16383.0).round() as i16 - 8192,
            Self::Sustain => {
                if f >= 0.5 {
                    127
                } else {
                    0
                }
            }
            _ => (f * 127.0).round() as i16,
        }
    }
    pub fn control_fraction(self, value: i16) -> f32 {
        match self {
            Self::PitchBend => (f32::from(value) + 8192.0) / 16383.0,
            _ => f32::from(value) / 127.0,
        }
    }

    /// Nombre completo del parámetro.
    pub fn label(self) -> &'static str {
        match self {
            Self::Velocity => "Velocity",
            Self::ReleaseVelocity => "Release Velocity",
            Self::Pan => "Note Pan",
            Self::MpeSlide => "MPE Slide (Timbre Y)",
            Self::MpePressure => "MPE Pressure (Aftertouch Z)",
            Self::PitchBend => "Pitch Bend",
            Self::Probability => "Probability (Chance)",
            Self::Modulation => "Modulation Wheel (CC 1)",
            Self::Breath => "Breath Controller (CC 2)",
            Self::Foot => "Foot Controller (CC 4)",
            Self::Volume => "Channel Volume (CC 7)",
            Self::PanCC => "Pan (CC 10)",
            Self::Expression => "Expression (CC 11)",
            Self::Sustain => "Sustain Pedal (CC 64)",
            Self::Resonance => "Filter Resonance (CC 71)",
            Self::Cutoff => "Filter Cutoff (CC 74)",
            Self::Pressure => "Channel Pressure (Aftertouch)",
        }
    }

    /// Etiqueta corta para la cabecera del carril ("{} ▾").
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Velocity => "Velocity",
            Self::ReleaseVelocity => "Rel Vel",
            Self::Pan => "Pan",
            Self::MpeSlide => "MPE Slide",
            Self::MpePressure => "MPE Press",
            Self::PitchBend => "Pitch Bend",
            Self::Probability => "Chance",
            Self::Modulation => "Mod CC1",
            Self::Breath => "Breath CC2",
            Self::Foot => "Foot CC4",
            Self::Volume => "Vol CC7",
            Self::PanCC => "Pan CC10",
            Self::Expression => "Expr CC11",
            Self::Sustain => "Sustain CC64",
            Self::Resonance => "Res CC71",
            Self::Cutoff => "Cutoff CC74",
            Self::Pressure => "Aftertouch",
        }
    }

    /// Descripción explicativa en español para la barra de estado / pista de ayuda.
    pub fn description(self) -> &'static str {
        match self {
            Self::Velocity => "Velocidad de pulsación de nota (1–127)",
            Self::ReleaseVelocity => "Velocidad de liberación de tecla (0–127)",
            Self::Pan => "Pan por nota (-64 L .. 0 C .. +63 R); Hay Synth y FluidSynth",
            Self::MpeSlide => "Eje Y MPE (Slide / Timbre CC 74 por nota, 0–127, neutro 64)",
            Self::MpePressure => "Eje Z MPE (Presión / Aftertouch por nota, 0–127)",
            Self::PitchBend => "Inflexión continua de tono (-8192 .. +8191)",
            Self::Probability => "Probabilidad de disparo de nota (0%–100%)",
            Self::Modulation => "Rueda de modulación continua MIDI CC 1 (0–127)",
            Self::Breath => "Controlador de respiración / soplo CC 2 (0–127)",
            Self::Foot => "Pedal de control continuo CC 4 (0–127)",
            Self::Volume => "Volumen principal de canal MIDI CC 7 (0–127)",
            Self::PanCC => "Panorama continuo del canal MIDI CC 10 (0–127)",
            Self::Expression => "Expresión y dinámica continua MIDI CC 11 (0–127)",
            Self::Sustain => "Pedal de sostenido / resonancia CC 64 (0 o 127)",
            Self::Resonance => "Resonancia armónica de filtro CC 71 (0–127)",
            Self::Cutoff => "Brillo y frecuencia de corte de filtro CC 74 (0–127)",
            Self::Pressure => "Presión continua de canal / Aftertouch (0–127)",
        }
    }

    /// Indica si el parámetro es bipolar (centrado en el medio) o unipolar (de abajo arriba).
    pub fn is_bipolar(self) -> bool {
        matches!(self, Self::Pan | Self::PitchBend)
    }

    /// Marcas de escala en el eje Y (fracción de altura de 0.0 a 1.0 y etiqueta).
    pub fn scale_marks(self) -> &'static [(f32, &'static str)] {
        match self {
            Self::Velocity
            | Self::ReleaseVelocity
            | Self::MpeSlide
            | Self::MpePressure
            | Self::Modulation
            | Self::Breath
            | Self::Foot
            | Self::Volume
            | Self::Expression
            | Self::Resonance
            | Self::Cutoff
            | Self::Pressure => &[
                (1.0, "127"),
                (96.0 / 127.0, "96"),
                (64.0 / 127.0, "64"),
                (32.0 / 127.0, "32"),
            ],
            Self::Pan => &[(1.0, "+63"), (0.5, "0"), (0.0, "-64")],
            Self::PitchBend => &[
                (1.0, "+8k"),
                (0.75, "+4k"),
                (0.5, "0"),
                (0.25, "-4k"),
                (0.0, "-8k"),
            ],
            Self::PanCC => &[(1.0, "127 R"), (0.5, "64 C"), (0.0, "0 L")],
            Self::Sustain => &[(1.0, "127 On"), (0.0, "0 Off")],
            Self::Probability => &[(1.0, "100%"), (0.75, "75%"), (0.5, "50%"), (0.25, "25%")],
        }
    }

    /// Formatea un valor según la fracción (0.0..=1.0).
    pub fn format_value(self, fraction: f32) -> String {
        let f = fraction.clamp(0.0, 1.0);
        match self {
            Self::Velocity => format!("{}", (f * 127.0).round().max(1.0) as u8),
            Self::ReleaseVelocity
            | Self::MpeSlide
            | Self::MpePressure
            | Self::Modulation
            | Self::Breath
            | Self::Foot
            | Self::Volume
            | Self::Expression
            | Self::Resonance
            | Self::Cutoff
            | Self::Pressure => {
                let v = (f * 127.0).round() as u8;
                format!("{v}")
            }
            Self::Pan => {
                let p = elevate::musical::note_expression::NoteProperty::Pan.value_at_fraction(f);
                match p.cmp(&0) {
                    std::cmp::Ordering::Greater => format!("+{p} (R)"),
                    std::cmp::Ordering::Less => format!("{p} (L)"),
                    std::cmp::Ordering::Equal => "0 (C)".into(),
                }
            }
            Self::PitchBend => {
                let b = self.control_value(f);
                format!("{b:+}")
            }
            Self::PanCC => {
                let v = (f * 127.0).round() as u8;
                format!("{v}")
            }
            Self::Sustain => {
                if f >= 0.5 {
                    "127 (On)".into()
                } else {
                    "0 (Off)".into()
                }
            }
            Self::Probability => {
                let pct = (f * 100.0).round() as u8;
                format!("{pct}%")
            }
        }
    }
}

/// Elemento del menú contextual del selector de carriles.
pub struct LaneMenuItem {
    pub lane: RollLane,
    pub section: Option<&'static str>,
    pub separator_after: bool,
}

/// Lista completa y estructurada de todas las opciones para el selector.
pub const LANE_MENU_ITEMS: &[LaneMenuItem] = &[
    LaneMenuItem {
        lane: RollLane::Velocity,
        section: Some("Propiedades de nota"),
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::ReleaseVelocity,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::Pan,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::MpeSlide,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::MpePressure,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::PitchBend,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::Probability,
        section: None,
        separator_after: true,
    },
    LaneMenuItem {
        lane: RollLane::Modulation,
        section: Some("Controladores MIDI (CC)"),
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::Expression,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::Volume,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::PanCC,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::Sustain,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::Cutoff,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::Resonance,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::Breath,
        section: None,
        separator_after: false,
    },
    LaneMenuItem {
        lane: RollLane::Foot,
        section: None,
        separator_after: true,
    },
    LaneMenuItem {
        lane: RollLane::Pressure,
        section: Some("Presión"),
        separator_after: false,
    },
];

/// Estado del menú emergente cuando se despliega el selector "Velocity ▾".
pub struct RollLaneMenu {
    pub position: Point<Pixels>,
    pub selected: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_coherent_lane_has_labels_and_scale_marks() {
        let all_lanes = [
            RollLane::Velocity,
            RollLane::ReleaseVelocity,
            RollLane::Pan,
            RollLane::MpeSlide,
            RollLane::MpePressure,
            RollLane::PitchBend,
            RollLane::Probability,
            RollLane::Modulation,
            RollLane::Breath,
            RollLane::Foot,
            RollLane::Volume,
            RollLane::PanCC,
            RollLane::Expression,
            RollLane::Sustain,
            RollLane::Resonance,
            RollLane::Cutoff,
            RollLane::Pressure,
        ];
        for lane in all_lanes {
            assert!(!lane.label().is_empty());
            assert!(!lane.short_label().is_empty());
            assert!(!lane.description().is_empty());
            let marks = lane.scale_marks();
            assert!(marks.len() >= 2);
            for &(frac, label) in marks {
                assert!((0.0..=1.0).contains(&frac));
                assert!(!label.is_empty());
            }
        }
    }

    #[test]
    fn bipolar_lanes_center_at_zero() {
        assert!(RollLane::Pan.is_bipolar());
        assert!(RollLane::PitchBend.is_bipolar());
        assert!(!RollLane::Velocity.is_bipolar());
        assert!(!RollLane::Volume.is_bipolar());
        assert!(!RollLane::Modulation.is_bipolar());
    }

    #[test]
    fn lane_menu_items_cover_all_lanes_without_duplicates() {
        let mut seen = HashSet::new();
        for item in LANE_MENU_ITEMS {
            assert!(
                seen.insert(item.lane),
                "Carril duplicado en el menú: {:?}",
                item.lane
            );
        }
        assert_eq!(seen.len(), 17);
    }

    #[test]
    fn format_value_produces_coherent_strings() {
        assert_eq!(RollLane::Velocity.format_value(1.0), "127");
        assert_eq!(RollLane::Velocity.format_value(0.5), "64");
        assert_eq!(RollLane::Pan.format_value(0.5), "0 (C)");
        assert!(RollLane::Pan.format_value(1.0).contains("(R)"));
        assert!(RollLane::Pan.format_value(0.0).contains("(L)"));
        assert_eq!(RollLane::Probability.format_value(1.0), "100%");
        assert_eq!(RollLane::Probability.format_value(0.5), "50%");
        assert_eq!(RollLane::Sustain.format_value(1.0), "127 (On)");
        assert_eq!(RollLane::Sustain.format_value(0.0), "0 (Off)");
    }
}
