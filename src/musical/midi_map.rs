//! Mapeo MIDI en tiempo real (`MidiMapper`) y sistema `MIDI Learn` estilo REAPER.
//! Cumple los principios #4 (Mapping real) y #5 (Acciones) sin depender de GPUI ni CPAL.
use super::midi::MidiMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MidiSource {
    Cc(u8),
    Note(u8),
}

impl MidiSource {
    pub fn label(self) -> String {
        match self {
            Self::Cc(cc) => format!("CC{cc}"),
            Self::Note(note) => format!("Nota {note}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MidiAction {
    PlayToggle,
    Stop,
    RecordToggle,
    CaptureMidi,
    NextBank,
    PrevBank,
    NextPreset,
    PrevPreset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MidiBindTarget {
    TrackVolume(usize),
    TrackPan(usize),
    MasterGain,
    /// Índice dentro de `piano_params::SPECS`.
    InstrumentParam(u8),
    Action(MidiAction),
}

impl MidiBindTarget {
    pub fn label(self) -> String {
        match self {
            Self::TrackVolume(idx) => format!("Pista {} · Volumen", idx + 1),
            Self::TrackPan(idx) => format!("Pista {} · Pan", idx + 1),
            Self::MasterGain => "Master Volume".into(),
            Self::InstrumentParam(idx) => format!("Parámetro #{idx}"),
            Self::Action(action) => format!("Acción {action:?}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MidiBinding {
    pub source: MidiSource,
    pub target: MidiBindTarget,
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MappedEffect {
    /// Fracción normalizada `0.0..=1.0` lista para aplicar al parámetro destino.
    SetNormalized {
        target: MidiBindTarget,
        fraction: f32,
    },
    TriggerAction(MidiAction),
    Learned {
        source: MidiSource,
        target: MidiBindTarget,
    },
}

#[derive(Debug, Clone)]
pub struct MidiMapper {
    learn_mode: bool,
    armed_target: Option<MidiBindTarget>,
    bindings: Vec<MidiBinding>,
}

impl Default for MidiMapper {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiMapper {
    /// Sigue la identidad de las pistas después de insertar, eliminar o reordenar.
    pub fn remap_tracks(&mut self, mapping: &[Option<usize>]) {
        let remap = |target| match target {
            MidiBindTarget::TrackVolume(i) => mapping
                .get(i)
                .copied()
                .flatten()
                .map(MidiBindTarget::TrackVolume),
            MidiBindTarget::TrackPan(i) => mapping
                .get(i)
                .copied()
                .flatten()
                .map(MidiBindTarget::TrackPan),
            other => Some(other),
        };
        self.armed_target = self.armed_target.and_then(remap);
        self.bindings.retain_mut(|binding| {
            if let Some(target) = remap(binding.target) {
                binding.target = target;
                true
            } else {
                false
            }
        });
    }

    pub fn new() -> Self {
        // Mapeos MIDI 1.0 estándar hacia los parámetros del instrumento (según orden en SPECS):
        // 1=Volume(CC7), 2=Expression(CC11), 3=Pan(CC10), 4=Modulation(CC1),
        // 5=ReverbSend(CC91), 6=ChorusSend(CC93), 7=Sustain(CC64)
        let defaults = [(7, 1), (11, 2), (10, 3), (1, 4), (91, 5), (93, 6), (64, 7)];
        let bindings = defaults
            .into_iter()
            .map(|(cc, param_idx)| MidiBinding {
                source: MidiSource::Cc(cc),
                target: MidiBindTarget::InstrumentParam(param_idx),
                min: 0.0,
                max: 1.0,
            })
            .collect();
        Self {
            learn_mode: false,
            armed_target: None,
            bindings,
        }
    }

    pub fn learn_mode(&self) -> bool {
        self.learn_mode
    }

    pub fn set_learn_mode(&mut self, enabled: bool) {
        self.learn_mode = enabled;
        if !enabled {
            self.armed_target = None;
        }
    }

    pub fn toggle_learn_mode(&mut self) -> bool {
        self.set_learn_mode(!self.learn_mode);
        self.learn_mode
    }

    pub fn armed_target(&self) -> Option<MidiBindTarget> {
        self.armed_target
    }

    /// Selecciona un control (perilla, slider o botón) para aprender el siguiente CC/nota MIDI.
    pub fn arm_target(&mut self, target: MidiBindTarget) {
        self.learn_mode = true;
        self.armed_target = Some(target);
    }

    pub fn bindings(&self) -> &[MidiBinding] {
        &self.bindings
    }

    pub fn source_for(&self, target: MidiBindTarget) -> Option<MidiSource> {
        self.bindings
            .iter()
            .find(|b| b.target == target)
            .map(|b| b.source)
    }

    pub fn bind(&mut self, source: MidiSource, target: MidiBindTarget) {
        self.bindings.retain(|b| b.target != target);
        self.bindings.push(MidiBinding {
            source,
            target,
            min: 0.0,
            max: 1.0,
        });
    }

    /// Evalúa un mensaje MIDI entrante. Si hay un destino armado en modo Learn,
    /// vincula el origen al destino; si no, dispara todos los destinos mapeados (1 a N).
    pub fn process(&mut self, message: MidiMessage) -> Vec<MappedEffect> {
        let (source, raw_fraction, is_trigger) = match message {
            MidiMessage::ControlChange { cc, value } => (
                MidiSource::Cc(cc),
                (f32::from(value) / 127.0).clamp(0.0, 1.0),
                value >= 64,
            ),
            MidiMessage::NoteOn { note, velocity } if velocity > 0 => (
                MidiSource::Note(note),
                (f32::from(velocity) / 127.0).clamp(0.0, 1.0),
                true,
            ),
            _ => return Vec::new(),
        };

        if let Some(target) = self.armed_target.take() {
            self.bind(source, target);
            self.learn_mode = false;
            return vec![MappedEffect::Learned { source, target }];
        }

        let mut effects = Vec::new();
        for binding in &self.bindings {
            if binding.source == source {
                match binding.target {
                    MidiBindTarget::Action(action) => {
                        if is_trigger {
                            effects.push(MappedEffect::TriggerAction(action));
                        }
                    }
                    target => {
                        let fraction = binding.min + raw_fraction * (binding.max - binding.min);
                        effects.push(MappedEffect::SetNormalized {
                            target,
                            fraction: fraction.clamp(0.0, 1.0),
                        });
                    }
                }
            }
        }
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_learn_binds_cc_to_track_volume_and_scales_value() {
        let mut mapper = MidiMapper::new();
        let target = MidiBindTarget::TrackVolume(0);
        mapper.arm_target(target);
        assert!(mapper.learn_mode());
        assert_eq!(mapper.armed_target(), Some(target));

        let learned = mapper.process(MidiMessage::ControlChange { cc: 21, value: 64 });
        assert_eq!(
            learned,
            vec![MappedEffect::Learned {
                source: MidiSource::Cc(21),
                target
            }]
        );
        assert!(!mapper.learn_mode());
        assert_eq!(mapper.source_for(target), Some(MidiSource::Cc(21)));

        let effects = mapper.process(MidiMessage::ControlChange { cc: 21, value: 127 });
        assert_eq!(
            effects,
            vec![MappedEffect::SetNormalized {
                target,
                fraction: 1.0
            }]
        );
    }
}
