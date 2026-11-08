//! Parámetros del piano que expone RustySynth y su traducción a mensajes MIDI/RPN.
//! Sin GPUI ni CPAL: la UI lista `SPECS`, el instrumento aplica `controls`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParamId {
    MasterVolume,
    Volume,
    Expression,
    Pan,
    Modulation,
    ReverbSend,
    ChorusSend,
    Sustain,
    PitchBend,
    BendRange,
    FineTune,
    CoarseTune,
    Polyphony,
    Effects,
}

/// Cuándo surte efecto un cambio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Applies {
    Live,
    /// El sintetizador lo lee al crearse: el piano se reabre en el siguiente note-on.
    Restart,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ParamSpec {
    pub id: ParamId,
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub step: f32,
    pub unit: &'static str,
    pub applies: Applies,
}

impl ParamSpec {
    pub fn is_toggle(&self) -> bool {
        self.min == 0.0 && self.max == 1.0 && self.step == 1.0
    }

    /// Fracción 0..1 → valor, ajustado al paso.
    pub fn from_fraction(&self, fraction: f32) -> f32 {
        let raw = self.min + fraction.clamp(0.0, 1.0) * (self.max - self.min);
        ((raw / self.step).round() * self.step).clamp(self.min, self.max)
    }

    pub fn fraction(&self, value: f32) -> f32 {
        ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    pub fn display(&self, value: f32) -> String {
        if self.is_toggle() {
            return if value >= 0.5 { "ON" } else { "OFF" }.into();
        }
        let decimals = if self.step >= 1.0 { 0 } else { 2 };
        format!("{value:.decimals$}{}", self.unit)
    }
}

const fn spec(
    id: ParamId,
    label: &'static str,
    min: f32,
    max: f32,
    default: f32,
    step: f32,
    unit: &'static str,
    applies: Applies,
) -> ParamSpec {
    ParamSpec {
        id,
        label,
        min,
        max,
        default,
        step,
        unit,
        applies,
    }
}

use Applies::{Live, Restart};
pub(crate) const SPECS: [ParamSpec; 14] = [
    spec(
        ParamId::MasterVolume,
        "Master",
        0.0,
        2.0,
        0.7,
        0.05,
        "",
        Live,
    ),
    spec(
        ParamId::Volume,
        "Volume (CC7)",
        0.0,
        127.0,
        100.0,
        1.0,
        "",
        Live,
    ),
    spec(
        ParamId::Expression,
        "Expression (CC11)",
        0.0,
        127.0,
        127.0,
        1.0,
        "",
        Live,
    ),
    spec(ParamId::Pan, "Pan (CC10)", 0.0, 127.0, 64.0, 1.0, "", Live),
    spec(
        ParamId::Modulation,
        "Modulación (CC1)",
        0.0,
        127.0,
        0.0,
        1.0,
        "",
        Live,
    ),
    spec(
        ParamId::ReverbSend,
        "Reverb (CC91)",
        0.0,
        127.0,
        40.0,
        1.0,
        "",
        Live,
    ),
    spec(
        ParamId::ChorusSend,
        "Chorus (CC93)",
        0.0,
        127.0,
        0.0,
        1.0,
        "",
        Live,
    ),
    spec(
        ParamId::Sustain,
        "Sustain (CC64)",
        0.0,
        1.0,
        0.0,
        1.0,
        "",
        Live,
    ),
    spec(
        ParamId::PitchBend,
        "Pitch bend",
        -1.0,
        1.0,
        0.0,
        0.01,
        "",
        Live,
    ),
    spec(
        ParamId::BendRange,
        "Rango bend",
        0.0,
        24.0,
        2.0,
        1.0,
        " st",
        Live,
    ),
    spec(
        ParamId::FineTune,
        "Afinación fina",
        -100.0,
        100.0,
        0.0,
        1.0,
        " ct",
        Live,
    ),
    spec(
        ParamId::CoarseTune,
        "Afinación gruesa",
        -12.0,
        12.0,
        0.0,
        1.0,
        " st",
        Live,
    ),
    spec(
        ParamId::Polyphony,
        "Polifonía",
        8.0,
        128.0,
        64.0,
        8.0,
        " voces",
        Restart,
    ),
    spec(
        ParamId::Effects,
        "Reverb + chorus",
        0.0,
        1.0,
        1.0,
        1.0,
        "",
        Restart,
    ),
];

pub(crate) fn spec_of(id: ParamId) -> &'static ParamSpec {
    SPECS
        .iter()
        .find(|spec| spec.id == id)
        .expect("todo id tiene spec")
}

/// Orden al sintetizador. `Midi` = (estado, dato1, dato2) sobre el canal del piano.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Control {
    Midi(u8, u8, u8),
    MasterVolume(f32),
}

const CC: u8 = 0xB0;

/// Mensajes que aplican `value` al sintetizador. Vacío para los parámetros `Restart`.
pub(crate) fn controls(id: ParamId, value: f32) -> Vec<Control> {
    let cc = |controller: u8, data: f32| {
        Control::Midi(CC, controller, data.round().clamp(0.0, 127.0) as u8)
    };
    // RPN: selecciona, escribe, y deselecciona (null RPN) para no alterar con otro data entry.
    let rpn = |lsb: u8, coarse: u8, fine: Option<u8>| {
        let mut messages = vec![
            cc(101, 0.0),
            cc(100, f32::from(lsb)),
            cc(6, f32::from(coarse)),
        ];
        messages.extend(fine.map(|fine| cc(38, f32::from(fine))));
        messages.extend([cc(101, 127.0), cc(100, 127.0)]);
        messages
    };
    match id {
        ParamId::MasterVolume => vec![Control::MasterVolume(value.max(0.0))],
        ParamId::Volume => vec![cc(7, value)],
        ParamId::Expression => vec![cc(11, value)],
        ParamId::Pan => vec![cc(10, value)],
        ParamId::Modulation => vec![cc(1, value)],
        ParamId::ReverbSend => vec![cc(91, value)],
        ParamId::ChorusSend => vec![cc(93, value)],
        ParamId::Sustain => vec![cc(64, if value >= 0.5 { 127.0 } else { 0.0 })],
        ParamId::PitchBend => {
            let raw = (8192.0 + value.clamp(-1.0, 1.0) * 8191.0).round() as u16;
            vec![Control::Midi(0xE0, (raw & 0x7F) as u8, (raw >> 7) as u8)]
        }
        ParamId::BendRange => rpn(0, value.round().clamp(0.0, 24.0) as u8, Some(0)),
        ParamId::FineTune => {
            let raw = (8192.0 + value.clamp(-100.0, 100.0) * 81.92)
                .round()
                .clamp(0.0, 16383.0) as u16;
            rpn(1, (raw >> 7) as u8, Some((raw & 0x7F) as u8))
        }
        ParamId::CoarseTune => rpn(2, (value.round().clamp(-12.0, 12.0) + 64.0) as u8, None),
        ParamId::Polyphony | ParamId::Effects => Vec::new(),
    }
}

/// Valores actuales del piano, indexados por `SPECS`.
#[derive(Debug, Clone)]
pub(crate) struct PianoParams {
    values: [f32; SPECS.len()],
}

impl PianoParams {
    pub fn new() -> Self {
        Self {
            values: SPECS.map(|spec| spec.default),
        }
    }

    fn index(id: ParamId) -> usize {
        SPECS
            .iter()
            .position(|spec| spec.id == id)
            .expect("todo id tiene spec")
    }

    pub fn get(&self, id: ParamId) -> f32 {
        self.values[Self::index(id)]
    }

    /// Guarda el valor ajustado a su rango y paso; devuelve el valor efectivo.
    pub fn set(&mut self, id: ParamId, value: f32) -> f32 {
        let spec = spec_of(id);
        let fixed = spec.from_fraction(spec.fraction(value));
        self.values[Self::index(id)] = fixed;
        fixed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_bend_is_14_bit_centered_and_fine_tune_uses_rpn_one() {
        assert_eq!(
            controls(ParamId::PitchBend, 0.0),
            vec![Control::Midi(0xE0, 0, 64)]
        );
        assert_eq!(
            controls(ParamId::PitchBend, 1.0),
            vec![Control::Midi(0xE0, 127, 127)]
        );
        let tune = controls(ParamId::FineTune, 0.0);
        assert_eq!(tune[1], Control::Midi(CC, 100, 1));
        assert_eq!(tune[2], Control::Midi(CC, 6, 64));
        assert_eq!(tune.last(), Some(&Control::Midi(CC, 100, 127)));
    }

    #[test]
    fn values_snap_to_their_step_and_restart_params_send_no_midi() {
        let mut params = PianoParams::new();
        assert_eq!(params.set(ParamId::Polyphony, 70.0), 72.0);
        assert_eq!(params.set(ParamId::Pan, 500.0), 127.0);
        assert!(controls(ParamId::Polyphony, 72.0).is_empty());
        assert_eq!(params.get(ParamId::Effects), 1.0);
    }
}
