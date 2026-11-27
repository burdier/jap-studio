//! Modelo headless de Hay Synth: Espacio + Cuerpo + Intención/Toque + Voice Lab estilo Vital.
//! Sin GPUI ni CPAL; serializable en proyectos `.loro`, P2P y presets JSON.
use std::sync::Arc;

pub const CUSTOM_WT_FRAMES: usize = 8;
pub const CUSTOM_WT_SIZE: usize = 256;

/// Wavetable custom de 8 frames × 256 muestras (importada desde `.vital` u otra fuente).
#[derive(Clone, Debug, PartialEq)]
pub struct CustomWavetable {
    pub name: String,
    pub frames: [[f32; CUSTOM_WT_SIZE]; CUSTOM_WT_FRAMES],
}

impl CustomWavetable {
    /// Codifica de forma compacta en `i16` little-endian (4 KB por tabla) para `.loro` y P2P.
    pub fn to_i16_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(CUSTOM_WT_FRAMES * CUSTOM_WT_SIZE * 2);
        for frame in &self.frames {
            for &s in frame {
                let q = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                out.extend_from_slice(&q.to_le_bytes());
            }
        }
        out
    }

    pub fn from_i16_bytes(name: String, bytes: &[u8]) -> Option<Self> {
        if bytes.len() != CUSTOM_WT_FRAMES * CUSTOM_WT_SIZE * 2 {
            return None;
        }
        let mut frames = [[0.0_f32; CUSTOM_WT_SIZE]; CUSTOM_WT_FRAMES];
        let mut idx = 0;
        for frame in &mut frames {
            for sample in frame.iter_mut() {
                let v = i16::from_le_bytes([bytes[idx], bytes[idx + 1]]);
                *sample = f32::from(v) / 32767.0;
                idx += 2;
            }
        }
        Some(Self { name, frames })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SynthEngineKind {
    Sub808 = 0,
    #[default]
    Analog = 1,
    Pluck = 2,
    FmBell = 3,
    Vocal = 4,
    FoldAcid = 5,
    Texture = 6,
    Wavetable = 7,
}

impl SynthEngineKind {
    pub const ALL: [Self; 8] = [
        Self::Sub808,
        Self::Analog,
        Self::Pluck,
        Self::FmBell,
        Self::Vocal,
        Self::FoldAcid,
        Self::Texture,
        Self::Wavetable,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sub808 => "Sub / 808",
            Self::Analog => "Analog",
            Self::Pluck => "Pluck",
            Self::FmBell => "FM / Bell",
            Self::Vocal => "Vocal",
            Self::FoldAcid => "Fold / Acid",
            Self::Texture => "Texture",
            Self::Wavetable => "Wavetable",
        }
    }

    pub fn shape_hint(self) -> &'static str {
        match self {
            Self::Sub808 => "Punch / Grit",
            Self::Analog => "Wave / Saw",
            Self::Pluck => "Posición",
            Self::FmBell => "Ratio FM",
            Self::Vocal => "Vocal A–U",
            Self::FoldAcid => "Fold / Res",
            Self::Texture => "Aire / Grano",
            Self::Wavetable => "WT Frame",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Sub808,
            1 => Self::Analog,
            2 => Self::Pluck,
            3 => Self::FmBell,
            4 => Self::Vocal,
            5 => Self::FoldAcid,
            6 => Self::Texture,
            7 => Self::Wavetable,
            _ => Self::Analog,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RoomKind {
    Close = 0,
    #[default]
    Wood = 1,
    Hall = 2,
    Open = 3,
}

impl RoomKind {
    pub const ALL: [Self; 4] = [Self::Close, Self::Wood, Self::Hall, Self::Open];

    pub fn label(self) -> &'static str {
        match self {
            Self::Close => "Close",
            Self::Wood => "Wood",
            Self::Hall => "Hall",
            Self::Open => "Open",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Close,
            1 => Self::Wood,
            2 => Self::Hall,
            3 => Self::Open,
            _ => Self::Wood,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EnvelopeMode {
    Hit = 0,
    #[default]
    Hold = 1,
    Swell = 2,
}

impl EnvelopeMode {
    pub const ALL: [Self; 3] = [Self::Hit, Self::Hold, Self::Swell];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hit => "Hit",
            Self::Hold => "Hold",
            Self::Swell => "Swell",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Hit,
            1 => Self::Hold,
            2 => Self::Swell,
            _ => Self::Hold,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WavetableKind {
    #[default]
    BasicShapes = 0,
    AnalogWarm = 1,
    DigitalGlass = 2,
    VocalFormant = 3,
    GrowlBass = 4,
    OrganBell = 5,
    PluckString = 6,
    HarshFold = 7,
}

impl WavetableKind {
    pub const ALL: [Self; 8] = [
        Self::BasicShapes,
        Self::AnalogWarm,
        Self::DigitalGlass,
        Self::VocalFormant,
        Self::GrowlBass,
        Self::OrganBell,
        Self::PluckString,
        Self::HarshFold,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::BasicShapes => "Basic Shapes",
            Self::AnalogWarm => "Analog Warm",
            Self::DigitalGlass => "Digital Glass",
            Self::VocalFormant => "Vocal Formant",
            Self::GrowlBass => "Growl Bass",
            Self::OrganBell => "Organ / Bell",
            Self::PluckString => "Pluck String",
            Self::HarshFold => "Harsh Fold",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        Self::ALL.get(usize::from(v)).copied().unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WarpMode {
    #[default]
    Off = 0,
    Sync = 1,
    Bend = 2,
    Fold = 3,
    FmFromB = 4,
    RingMod = 5,
}

impl WarpMode {
    pub const ALL: [Self; 6] = [
        Self::Off,
        Self::Sync,
        Self::Bend,
        Self::Fold,
        Self::FmFromB,
        Self::RingMod,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Sync => "Sync",
            Self::Bend => "Bend",
            Self::Fold => "Fold",
            Self::FmFromB => "FM B→A",
            Self::RingMod => "RM A×B",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        Self::ALL.get(usize::from(v)).copied().unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilterKind {
    #[default]
    Lp12 = 0,
    Lp24 = 1,
    Bp12 = 2,
    Hp12 = 3,
    Notch = 4,
}

impl FilterKind {
    pub const ALL: [Self; 5] = [Self::Lp12, Self::Lp24, Self::Bp12, Self::Hp12, Self::Notch];

    pub fn label(self) -> &'static str {
        match self {
            Self::Lp12 => "LP 12",
            Self::Lp24 => "LP 24",
            Self::Bp12 => "BP 12",
            Self::Hp12 => "HP 12",
            Self::Notch => "Notch",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        Self::ALL.get(usize::from(v)).copied().unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LfoShape {
    #[default]
    Sine = 0,
    Tri = 1,
    SawUp = 2,
    SawDown = 3,
    Square = 4,
    Random = 5,
}

impl LfoShape {
    pub const ALL: [Self; 6] = [
        Self::Sine,
        Self::Tri,
        Self::SawUp,
        Self::SawDown,
        Self::Square,
        Self::Random,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sine => "Sine",
            Self::Tri => "Tri",
            Self::SawUp => "Saw ↑",
            Self::SawDown => "Saw ↓",
            Self::Square => "Square",
            Self::Random => "S&H",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        Self::ALL.get(usize::from(v)).copied().unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DistMode {
    #[default]
    Soft = 0,
    Hard = 1,
    Fold = 2,
    Bitcrush = 3,
}

impl DistMode {
    pub const ALL: [Self; 4] = [Self::Soft, Self::Hard, Self::Fold, Self::Bitcrush];

    pub fn label(self) -> &'static str {
        match self {
            Self::Soft => "Soft",
            Self::Hard => "Hard",
            Self::Fold => "Fold",
            Self::Bitcrush => "Crush",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        Self::ALL.get(usize::from(v)).copied().unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModSource {
    #[default]
    None = 0,
    Env1 = 1,
    Env2 = 2,
    Env3 = 3,
    Lfo1 = 4,
    Lfo2 = 5,
    Velocity = 6,
    Keytrack = 7,
    ModWheel = 8,
    Random = 9,
    Distance = 10,
    Tone = 11,
    Shape = 12,
    Body = 13,
    Motion = 14,
}

impl ModSource {
    pub const ALL: [Self; 15] = [
        Self::None,
        Self::Env1,
        Self::Env2,
        Self::Env3,
        Self::Lfo1,
        Self::Lfo2,
        Self::Velocity,
        Self::Keytrack,
        Self::ModWheel,
        Self::Random,
        Self::Distance,
        Self::Tone,
        Self::Shape,
        Self::Body,
        Self::Motion,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "—",
            Self::Env1 => "ENV 1 (Amp)",
            Self::Env2 => "ENV 2 (Mod)",
            Self::Env3 => "ENV 3 (Punch)",
            Self::Lfo1 => "LFO 1",
            Self::Lfo2 => "LFO 2",
            Self::Velocity => "Velocity",
            Self::Keytrack => "Keytrack",
            Self::ModWheel => "ModWheel",
            Self::Random => "Random",
            Self::Distance => "Macro: Distance",
            Self::Tone => "Macro: Tone",
            Self::Shape => "Macro: Shape",
            Self::Body => "Macro: Body",
            Self::Motion => "Macro: Motion",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        Self::ALL.get(usize::from(v)).copied().unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModTarget {
    #[default]
    None = 0,
    OscAPos = 1,
    OscAWarp = 2,
    OscAPitch = 3,
    OscADetune = 4,
    OscALevel = 5,
    OscBPos = 6,
    OscBPitch = 7,
    OscBLevel = 8,
    SubLevel = 9,
    NoiseLevel = 10,
    FilterCutoff = 11,
    FilterRes = 12,
    FilterDrive = 13,
    ChorusMix = 14,
    DistDrive = 15,
    Distance = 16,
    Lfo1Rate = 17,
    Lfo2Rate = 18,
    Pan = 19,
    OutGain = 20,
}

impl ModTarget {
    pub const ALL: [Self; 21] = [
        Self::None,
        Self::OscAPos,
        Self::OscAWarp,
        Self::OscAPitch,
        Self::OscADetune,
        Self::OscALevel,
        Self::OscBPos,
        Self::OscBPitch,
        Self::OscBLevel,
        Self::SubLevel,
        Self::NoiseLevel,
        Self::FilterCutoff,
        Self::FilterRes,
        Self::FilterDrive,
        Self::ChorusMix,
        Self::DistDrive,
        Self::Distance,
        Self::Lfo1Rate,
        Self::Lfo2Rate,
        Self::Pan,
        Self::OutGain,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "—",
            Self::OscAPos => "OSC A Frame / Shape",
            Self::OscAWarp => "OSC A Warp",
            Self::OscAPitch => "OSC A Pitch (±24 st)",
            Self::OscADetune => "OSC A Detune",
            Self::OscALevel => "OSC A Level",
            Self::OscBPos => "OSC B Frame",
            Self::OscBPitch => "OSC B Pitch (±24 st)",
            Self::OscBLevel => "OSC B Level",
            Self::SubLevel => "Sub Level",
            Self::NoiseLevel => "Noise / Air",
            Self::FilterCutoff => "Filter Cutoff",
            Self::FilterRes => "Filter Res",
            Self::FilterDrive => "Filter Drive",
            Self::ChorusMix => "Chorus Mix",
            Self::DistDrive => "Dist Drive",
            Self::Distance => "Space Distance",
            Self::Lfo1Rate => "LFO 1 Rate",
            Self::Lfo2Rate => "LFO 2 Rate",
            Self::Pan => "Voice Pan",
            Self::OutGain => "Out Level",
        }
    }

    pub fn from_u8(v: u8) -> Self {
        Self::ALL.get(usize::from(v)).copied().unwrap_or_default()
    }
}

pub const MATRIX_SLOTS: usize = 12;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModSlot {
    pub source: ModSource,
    pub target: ModTarget,
    /// Cantidad bipolar `-1.0..=1.0`.
    pub amount: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceParams {
    // ── OSC A (Wavetable / Unison / Warp) ──
    pub osc_a_table: WavetableKind,
    /// 0.0..1.0 nivel de OSC A.
    pub osc_a_level: f32,
    /// 1..=7 voces de unison en OSC A.
    pub osc_a_unison: u8,
    /// 0.0..1.0 desafinación de unison (hasta ~45 cents).
    pub osc_a_detune: f32,
    /// 0.0..1.0 dispersión estéreo del unison.
    pub osc_a_spread: f32,
    pub osc_a_warp_mode: WarpMode,
    /// 0.0..1.0 intensidad del warp (Sync / Bend / Fold / FM B→A / RM).
    pub osc_a_warp: f32,
    /// Semitonos `-24..=24`.
    pub osc_a_semi: i8,

    // ── OSC B (Wavetable independiente + FM source) ──
    pub osc_b_table: WavetableKind,
    /// 0.0..1.0 nivel de salida audible de OSC B (0 = apagado o solo modulador FM).
    pub osc_b_level: f32,
    /// 0.0..1.0 posición de frame en la tabla de OSC B.
    pub osc_b_pos: f32,
    /// Semitonos `-24..=24`.
    pub osc_b_semi: i8,
    /// Fine tuning `-1.0..=1.0` (±50 cents).
    pub osc_b_fine: f32,
    /// 1..=7 voces de unison en OSC B.
    pub osc_b_unison: u8,
    pub osc_b_detune: f32,

    // ── SUB + NOISE ──
    pub sub_level: f32,
    /// -2 o -1 octava (0 = -1 oct, 1 = -2 oct).
    pub sub_octave: u8,
    pub noise_level: f32,
    /// 0.0 = oscuro/vinilo, 0.5 = aire neutro, 1.0 = brillante/hiss.
    pub noise_color: f32,

    // ── FILTER (TPT SVF 12/24 dB) ──
    pub filter_kind: FilterKind,
    /// 0.0..1.0 resonancia adicional sobre el motor base.
    pub filter_res: f32,
    /// 0.0..1.0 drive analógico pre-filtro.
    pub filter_drive: f32,
    /// 0.0..1.0 seguimiento de nota (keytrack).
    pub filter_keytrack: f32,

    // ── ENVELOPES (Amp override + ENV 2 Mod + ENV 3 Punch) ──
    /// Si true, `env1_*` gobiernan directamente el ADSR de amplitud; si false, manda `Mode + Time`.
    pub env1_override: bool,
    pub env1_attack: f32,
    pub env1_decay: f32,
    pub env1_sustain: f32,
    pub env1_release: f32,

    pub env2_attack: f32,
    pub env2_decay: f32,
    pub env2_sustain: f32,
    pub env2_release: f32,

    pub env3_decay: f32,

    // ── LFO 1 & LFO 2 ──
    pub lfo1_shape: LfoShape,
    /// 0.0..1.0 (0.05 Hz .. 18 Hz o divisiones de BPM cuando sync=true).
    pub lfo1_rate: f32,
    pub lfo1_sync: bool,
    pub lfo1_retrigger: bool,

    pub lfo2_shape: LfoShape,
    pub lfo2_rate: f32,
    pub lfo2_sync: bool,
    pub lfo2_retrigger: bool,

    // ── FX INTERNOS (Chorus + Distortion) ──
    pub chorus_mix: f32,
    pub chorus_rate: f32,
    pub dist_mode: DistMode,
    pub dist_drive: f32,
    pub dist_mix: f32,

    // ── MOD MATRIX (12 ranuras) ──
    pub matrix: [ModSlot; MATRIX_SLOTS],
}

impl Default for VoiceParams {
    fn default() -> Self {
        Self {
            osc_a_table: WavetableKind::BasicShapes,
            osc_a_level: 1.0,
            osc_a_unison: 1,
            osc_a_detune: 0.22,
            osc_a_spread: 0.70,
            osc_a_warp_mode: WarpMode::Off,
            osc_a_warp: 0.0,
            osc_a_semi: 0,

            osc_b_table: WavetableKind::AnalogWarm,
            osc_b_level: 0.0,
            osc_b_pos: 0.35,
            osc_b_semi: 0,
            osc_b_fine: 0.0,
            osc_b_unison: 1,
            osc_b_detune: 0.18,

            sub_level: 0.0,
            sub_octave: 0,
            noise_level: 0.0,
            noise_color: 0.55,

            filter_kind: FilterKind::Lp12,
            filter_res: 0.12,
            filter_drive: 0.0,
            filter_keytrack: 0.25,

            env1_override: false,
            env1_attack: 0.02,
            env1_decay: 0.35,
            env1_sustain: 0.75,
            env1_release: 0.30,

            env2_attack: 0.01,
            env2_decay: 0.32,
            env2_sustain: 0.0,
            env2_release: 0.25,

            env3_decay: 0.12,

            lfo1_shape: LfoShape::Sine,
            lfo1_rate: 0.35,
            lfo1_sync: true,
            lfo1_retrigger: true,

            lfo2_shape: LfoShape::Tri,
            lfo2_rate: 0.22,
            lfo2_sync: false,
            lfo2_retrigger: false,

            chorus_mix: 0.0,
            chorus_rate: 0.32,
            dist_mode: DistMode::Soft,
            dist_drive: 0.0,
            dist_mix: 0.0,

            matrix: [ModSlot::default(); MATRIX_SLOTS],
        }
    }
}

impl VoiceParams {
    pub fn clamp_all(&mut self) {
        self.osc_a_level = self.osc_a_level.clamp(0.0, 1.0);
        self.osc_a_unison = self.osc_a_unison.clamp(1, 7);
        self.osc_a_detune = self.osc_a_detune.clamp(0.0, 1.0);
        self.osc_a_spread = self.osc_a_spread.clamp(0.0, 1.0);
        self.osc_a_warp = self.osc_a_warp.clamp(0.0, 1.0);
        self.osc_a_semi = self.osc_a_semi.clamp(-24, 24);

        self.osc_b_level = self.osc_b_level.clamp(0.0, 1.0);
        self.osc_b_pos = self.osc_b_pos.clamp(0.0, 1.0);
        self.osc_b_semi = self.osc_b_semi.clamp(-24, 24);
        self.osc_b_fine = self.osc_b_fine.clamp(-1.0, 1.0);
        self.osc_b_unison = self.osc_b_unison.clamp(1, 7);
        self.osc_b_detune = self.osc_b_detune.clamp(0.0, 1.0);

        self.sub_level = self.sub_level.clamp(0.0, 1.0);
        self.sub_octave = self.sub_octave.min(1);
        self.noise_level = self.noise_level.clamp(0.0, 1.0);
        self.noise_color = self.noise_color.clamp(0.0, 1.0);

        self.filter_res = self.filter_res.clamp(0.0, 1.0);
        self.filter_drive = self.filter_drive.clamp(0.0, 1.0);
        self.filter_keytrack = self.filter_keytrack.clamp(0.0, 1.0);

        self.env1_attack = self.env1_attack.clamp(0.0, 1.0);
        self.env1_decay = self.env1_decay.clamp(0.0, 1.0);
        self.env1_sustain = self.env1_sustain.clamp(0.0, 1.0);
        self.env1_release = self.env1_release.clamp(0.0, 1.0);

        self.env2_attack = self.env2_attack.clamp(0.0, 1.0);
        self.env2_decay = self.env2_decay.clamp(0.0, 1.0);
        self.env2_sustain = self.env2_sustain.clamp(0.0, 1.0);
        self.env2_release = self.env2_release.clamp(0.0, 1.0);

        self.env3_decay = self.env3_decay.clamp(0.0, 1.0);

        self.lfo1_rate = self.lfo1_rate.clamp(0.0, 1.0);
        self.lfo2_rate = self.lfo2_rate.clamp(0.0, 1.0);

        self.chorus_mix = self.chorus_mix.clamp(0.0, 1.0);
        self.chorus_rate = self.chorus_rate.clamp(0.0, 1.0);
        self.dist_drive = self.dist_drive.clamp(0.0, 1.0);
        self.dist_mix = self.dist_mix.clamp(0.0, 1.0);

        for slot in &mut self.matrix {
            slot.amount = slot.amount.clamp(-1.0, 1.0);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SynthParams {
    pub engine: SynthEngineKind,
    pub room: RoomKind,
    /// 0.0 = en el pecho (cerca/seco), 1.0 = lejos en el espacio.
    pub distance: f32,
    /// Carácter propio de cada motor (o Frame de Wavetable en modo Wavetable).
    pub shape: f32,
    /// 0.0 = oscuro/suave, 1.0 = brillante/abierto (Cutoff base).
    pub tone: f32,
    /// 0.0 = fino/mono, 1.0 = denso/estéreo.
    pub body: f32,
    /// 0.0 = estático, 1.0 = movimiento orgánico/drift.
    pub motion: f32,
    pub mode: EnvelopeMode,
    /// 0.0 = corto/rápido, 1.0 = largo/sostenido.
    pub time: f32,
    /// Sensibilidad a la velocidad (acopla ataque, brillo, drive y espacio).
    pub touch: f32,
    /// Continuidad / portamento entre notas ligadas.
    pub glide: f32,
    /// Nivel de salida (0.0..1.0).
    pub gain: f32,
    pub preset_name: String,
    /// Capa de diseño sonoro estilo Vital (OSC A/B, Unison, Warp, Filtro 12/24dB, Envs, LFOs, FX, Matrix).
    pub voice: VoiceParams,
    /// Wavetables reales decodificadas desde `.vital` (si `Some`, reemplazan la tabla fija de OSC A/B).
    pub custom_wt_a: Option<Arc<CustomWavetable>>,
    pub custom_wt_b: Option<Arc<CustomWavetable>>,
}

impl Default for SynthParams {
    fn default() -> Self {
        FACTORY_PRESETS[3].to_params() // Warm Analog Pad
    }
}

/// Índices de control en vivo para actualización sin bloqueos hacia el motor RT.
pub mod cc {
    pub const ENGINE: u8 = 200;
    pub const ROOM: u8 = 201;
    pub const DISTANCE: u8 = 202;
    pub const SHAPE: u8 = 203;
    pub const TONE: u8 = 204;
    pub const BODY: u8 = 205;
    pub const MOTION: u8 = 206;
    pub const MODE: u8 = 207;
    pub const TIME: u8 = 208;
    pub const TOUCH: u8 = 209;
    pub const GLIDE: u8 = 210;
    pub const GAIN: u8 = 211;
}

/// IDs `u16` completos para enviar parámetros de Voice Lab + Mod Matrix sin bloqueos al hilo RT.
pub mod param {
    // Stage (0..11)
    pub const ENGINE: u16 = 0;
    pub const ROOM: u16 = 1;
    pub const DISTANCE: u16 = 2;
    pub const SHAPE: u16 = 3;
    pub const TONE: u16 = 4;
    pub const BODY: u16 = 5;
    pub const MOTION: u16 = 6;
    pub const MODE: u16 = 7;
    pub const TIME: u16 = 8;
    pub const TOUCH: u16 = 9;
    pub const GLIDE: u16 = 10;
    pub const GAIN: u16 = 11;

    // OSC A (20..27)
    pub const OSC_A_TABLE: u16 = 20;
    pub const OSC_A_LEVEL: u16 = 21;
    pub const OSC_A_UNISON: u16 = 22;
    pub const OSC_A_DETUNE: u16 = 23;
    pub const OSC_A_SPREAD: u16 = 24;
    pub const OSC_A_WARP_MODE: u16 = 25;
    pub const OSC_A_WARP: u16 = 26;
    pub const OSC_A_SEMI: u16 = 27;

    // OSC B (30..36)
    pub const OSC_B_TABLE: u16 = 30;
    pub const OSC_B_LEVEL: u16 = 31;
    pub const OSC_B_POS: u16 = 32;
    pub const OSC_B_SEMI: u16 = 33;
    pub const OSC_B_FINE: u16 = 34;
    pub const OSC_B_UNISON: u16 = 35;
    pub const OSC_B_DETUNE: u16 = 36;

    // SUB + NOISE (40..43)
    pub const SUB_LEVEL: u16 = 40;
    pub const SUB_OCTAVE: u16 = 41;
    pub const NOISE_LEVEL: u16 = 42;
    pub const NOISE_COLOR: u16 = 43;

    // FILTER (50..53)
    pub const FILTER_KIND: u16 = 50;
    pub const FILTER_RES: u16 = 51;
    pub const FILTER_DRIVE: u16 = 52;
    pub const FILTER_KEYTRACK: u16 = 53;

    // ENVS (60..69)
    pub const ENV1_OVERRIDE: u16 = 60;
    pub const ENV1_ATTACK: u16 = 61;
    pub const ENV1_DECAY: u16 = 62;
    pub const ENV1_SUSTAIN: u16 = 63;
    pub const ENV1_RELEASE: u16 = 64;
    pub const ENV2_ATTACK: u16 = 65;
    pub const ENV2_DECAY: u16 = 66;
    pub const ENV2_SUSTAIN: u16 = 67;
    pub const ENV2_RELEASE: u16 = 68;
    pub const ENV3_DECAY: u16 = 69;

    // LFOS (70..77)
    pub const LFO1_SHAPE: u16 = 70;
    pub const LFO1_RATE: u16 = 71;
    pub const LFO1_SYNC: u16 = 72;
    pub const LFO1_RETRIGGER: u16 = 73;
    pub const LFO2_SHAPE: u16 = 74;
    pub const LFO2_RATE: u16 = 75;
    pub const LFO2_SYNC: u16 = 76;
    pub const LFO2_RETRIGGER: u16 = 77;

    // FX (80..84)
    pub const CHORUS_MIX: u16 = 80;
    pub const CHORUS_RATE: u16 = 81;
    pub const DIST_MODE: u16 = 82;
    pub const DIST_DRIVE: u16 = 83;
    pub const DIST_MIX: u16 = 84;

    // MATRIX: slot `i` (0..12) usa `200 + i*3 + {0: source, 1: target, 2: amount}`
    pub const MATRIX_BASE: u16 = 200;
    pub const fn matrix_source(slot: usize) -> u16 {
        MATRIX_BASE + (slot as u16) * 3
    }
    pub const fn matrix_target(slot: usize) -> u16 {
        MATRIX_BASE + (slot as u16) * 3 + 1
    }
    pub const fn matrix_amount(slot: usize) -> u16 {
        MATRIX_BASE + (slot as u16) * 3 + 2
    }
}

impl SynthParams {
    pub fn clamp_all(&mut self) {
        self.distance = self.distance.clamp(0.0, 1.0);
        self.shape = self.shape.clamp(0.0, 1.0);
        self.tone = self.tone.clamp(0.0, 1.0);
        self.body = self.body.clamp(0.0, 1.0);
        self.motion = self.motion.clamp(0.0, 1.0);
        self.time = self.time.clamp(0.0, 1.0);
        self.touch = self.touch.clamp(0.0, 1.0);
        self.glide = self.glide.clamp(0.0, 1.0);
        self.gain = self.gain.clamp(0.0, 1.0);
        self.voice.clamp_all();
    }

    pub fn live_controls(&self) -> [(u8, f32); 12] {
        [
            (cc::ENGINE, self.engine as u8 as f32),
            (cc::ROOM, self.room as u8 as f32),
            (cc::DISTANCE, self.distance),
            (cc::SHAPE, self.shape),
            (cc::TONE, self.tone),
            (cc::BODY, self.body),
            (cc::MOTION, self.motion),
            (cc::MODE, self.mode as u8 as f32),
            (cc::TIME, self.time),
            (cc::TOUCH, self.touch),
            (cc::GLIDE, self.glide),
            (cc::GAIN, self.gain),
        ]
    }

    pub fn set_param(&mut self, id: u16, value: f32) {
        if !value.is_finite() {
            return;
        }
        let v = &mut self.voice;
        match id {
            param::ENGINE => self.engine = SynthEngineKind::from_u8(value.round().clamp(0.0, 7.0) as u8),
            param::ROOM => self.room = RoomKind::from_u8(value.round().clamp(0.0, 3.0) as u8),
            param::DISTANCE => self.distance = value.clamp(0.0, 1.0),
            param::SHAPE => self.shape = value.clamp(0.0, 1.0),
            param::TONE => self.tone = value.clamp(0.0, 1.0),
            param::BODY => self.body = value.clamp(0.0, 1.0),
            param::MOTION => self.motion = value.clamp(0.0, 1.0),
            param::MODE => self.mode = EnvelopeMode::from_u8(value.round().clamp(0.0, 2.0) as u8),
            param::TIME => self.time = value.clamp(0.0, 1.0),
            param::TOUCH => self.touch = value.clamp(0.0, 1.0),
            param::GLIDE => self.glide = value.clamp(0.0, 1.0),
            param::GAIN => self.gain = value.clamp(0.0, 1.0),

            param::OSC_A_TABLE => v.osc_a_table = WavetableKind::from_u8(value.round().clamp(0.0, 7.0) as u8),
            param::OSC_A_LEVEL => v.osc_a_level = value.clamp(0.0, 1.0),
            param::OSC_A_UNISON => v.osc_a_unison = value.round().clamp(1.0, 7.0) as u8,
            param::OSC_A_DETUNE => v.osc_a_detune = value.clamp(0.0, 1.0),
            param::OSC_A_SPREAD => v.osc_a_spread = value.clamp(0.0, 1.0),
            param::OSC_A_WARP_MODE => v.osc_a_warp_mode = WarpMode::from_u8(value.round().clamp(0.0, 5.0) as u8),
            param::OSC_A_WARP => v.osc_a_warp = value.clamp(0.0, 1.0),
            param::OSC_A_SEMI => v.osc_a_semi = value.round().clamp(-24.0, 24.0) as i8,

            param::OSC_B_TABLE => v.osc_b_table = WavetableKind::from_u8(value.round().clamp(0.0, 7.0) as u8),
            param::OSC_B_LEVEL => v.osc_b_level = value.clamp(0.0, 1.0),
            param::OSC_B_POS => v.osc_b_pos = value.clamp(0.0, 1.0),
            param::OSC_B_SEMI => v.osc_b_semi = value.round().clamp(-24.0, 24.0) as i8,
            param::OSC_B_FINE => v.osc_b_fine = value.clamp(-1.0, 1.0),
            param::OSC_B_UNISON => v.osc_b_unison = value.round().clamp(1.0, 7.0) as u8,
            param::OSC_B_DETUNE => v.osc_b_detune = value.clamp(0.0, 1.0),

            param::SUB_LEVEL => v.sub_level = value.clamp(0.0, 1.0),
            param::SUB_OCTAVE => v.sub_octave = (value.round() as i32).clamp(0, 1) as u8,
            param::NOISE_LEVEL => v.noise_level = value.clamp(0.0, 1.0),
            param::NOISE_COLOR => v.noise_color = value.clamp(0.0, 1.0),

            param::FILTER_KIND => v.filter_kind = FilterKind::from_u8(value.round().clamp(0.0, 4.0) as u8),
            param::FILTER_RES => v.filter_res = value.clamp(0.0, 1.0),
            param::FILTER_DRIVE => v.filter_drive = value.clamp(0.0, 1.0),
            param::FILTER_KEYTRACK => v.filter_keytrack = value.clamp(0.0, 1.0),

            param::ENV1_OVERRIDE => v.env1_override = value >= 0.5,
            param::ENV1_ATTACK => v.env1_attack = value.clamp(0.0, 1.0),
            param::ENV1_DECAY => v.env1_decay = value.clamp(0.0, 1.0),
            param::ENV1_SUSTAIN => v.env1_sustain = value.clamp(0.0, 1.0),
            param::ENV1_RELEASE => v.env1_release = value.clamp(0.0, 1.0),
            param::ENV2_ATTACK => v.env2_attack = value.clamp(0.0, 1.0),
            param::ENV2_DECAY => v.env2_decay = value.clamp(0.0, 1.0),
            param::ENV2_SUSTAIN => v.env2_sustain = value.clamp(0.0, 1.0),
            param::ENV2_RELEASE => v.env2_release = value.clamp(0.0, 1.0),
            param::ENV3_DECAY => v.env3_decay = value.clamp(0.0, 1.0),

            param::LFO1_SHAPE => v.lfo1_shape = LfoShape::from_u8(value.round().clamp(0.0, 5.0) as u8),
            param::LFO1_RATE => v.lfo1_rate = value.clamp(0.0, 1.0),
            param::LFO1_SYNC => v.lfo1_sync = value >= 0.5,
            param::LFO1_RETRIGGER => v.lfo1_retrigger = value >= 0.5,
            param::LFO2_SHAPE => v.lfo2_shape = LfoShape::from_u8(value.round().clamp(0.0, 5.0) as u8),
            param::LFO2_RATE => v.lfo2_rate = value.clamp(0.0, 1.0),
            param::LFO2_SYNC => v.lfo2_sync = value >= 0.5,
            param::LFO2_RETRIGGER => v.lfo2_retrigger = value >= 0.5,

            param::CHORUS_MIX => v.chorus_mix = value.clamp(0.0, 1.0),
            param::CHORUS_RATE => v.chorus_rate = value.clamp(0.0, 1.0),
            param::DIST_MODE => v.dist_mode = DistMode::from_u8(value.round().clamp(0.0, 3.0) as u8),
            param::DIST_DRIVE => v.dist_drive = value.clamp(0.0, 1.0),
            param::DIST_MIX => v.dist_mix = value.clamp(0.0, 1.0),

            other if other >= param::MATRIX_BASE => {
                let rel = (other - param::MATRIX_BASE) as usize;
                let slot = rel / 3;
                let field = rel % 3;
                if let Some(m) = v.matrix.get_mut(slot) {
                    match field {
                        0 => m.source = ModSource::from_u8(value.round().clamp(0.0, 14.0) as u8),
                        1 => m.target = ModTarget::from_u8(value.round().clamp(0.0, 20.0) as u8),
                        2 => m.amount = value.clamp(-1.0, 1.0),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    pub fn get_param(&self, id: u16) -> f32 {
        let v = &self.voice;
        match id {
            param::ENGINE => self.engine as u8 as f32,
            param::ROOM => self.room as u8 as f32,
            param::DISTANCE => self.distance,
            param::SHAPE => self.shape,
            param::TONE => self.tone,
            param::BODY => self.body,
            param::MOTION => self.motion,
            param::MODE => self.mode as u8 as f32,
            param::TIME => self.time,
            param::TOUCH => self.touch,
            param::GLIDE => self.glide,
            param::GAIN => self.gain,

            param::OSC_A_TABLE => v.osc_a_table as u8 as f32,
            param::OSC_A_LEVEL => v.osc_a_level,
            param::OSC_A_UNISON => f32::from(v.osc_a_unison),
            param::OSC_A_DETUNE => v.osc_a_detune,
            param::OSC_A_SPREAD => v.osc_a_spread,
            param::OSC_A_WARP_MODE => v.osc_a_warp_mode as u8 as f32,
            param::OSC_A_WARP => v.osc_a_warp,
            param::OSC_A_SEMI => f32::from(v.osc_a_semi),

            param::OSC_B_TABLE => v.osc_b_table as u8 as f32,
            param::OSC_B_LEVEL => v.osc_b_level,
            param::OSC_B_POS => v.osc_b_pos,
            param::OSC_B_SEMI => f32::from(v.osc_b_semi),
            param::OSC_B_FINE => v.osc_b_fine,
            param::OSC_B_UNISON => f32::from(v.osc_b_unison),
            param::OSC_B_DETUNE => v.osc_b_detune,

            param::SUB_LEVEL => v.sub_level,
            param::SUB_OCTAVE => f32::from(v.sub_octave),
            param::NOISE_LEVEL => v.noise_level,
            param::NOISE_COLOR => v.noise_color,

            param::FILTER_KIND => v.filter_kind as u8 as f32,
            param::FILTER_RES => v.filter_res,
            param::FILTER_DRIVE => v.filter_drive,
            param::FILTER_KEYTRACK => v.filter_keytrack,

            param::ENV1_OVERRIDE => if v.env1_override { 1.0 } else { 0.0 },
            param::ENV1_ATTACK => v.env1_attack,
            param::ENV1_DECAY => v.env1_decay,
            param::ENV1_SUSTAIN => v.env1_sustain,
            param::ENV1_RELEASE => v.env1_release,
            param::ENV2_ATTACK => v.env2_attack,
            param::ENV2_DECAY => v.env2_decay,
            param::ENV2_SUSTAIN => v.env2_sustain,
            param::ENV2_RELEASE => v.env2_release,
            param::ENV3_DECAY => v.env3_decay,

            param::LFO1_SHAPE => v.lfo1_shape as u8 as f32,
            param::LFO1_RATE => v.lfo1_rate,
            param::LFO1_SYNC => if v.lfo1_sync { 1.0 } else { 0.0 },
            param::LFO1_RETRIGGER => if v.lfo1_retrigger { 1.0 } else { 0.0 },
            param::LFO2_SHAPE => v.lfo2_shape as u8 as f32,
            param::LFO2_RATE => v.lfo2_rate,
            param::LFO2_SYNC => if v.lfo2_sync { 1.0 } else { 0.0 },
            param::LFO2_RETRIGGER => if v.lfo2_retrigger { 1.0 } else { 0.0 },

            param::CHORUS_MIX => v.chorus_mix,
            param::CHORUS_RATE => v.chorus_rate,
            param::DIST_MODE => v.dist_mode as u8 as f32,
            param::DIST_DRIVE => v.dist_drive,
            param::DIST_MIX => v.dist_mix,

            other if other >= param::MATRIX_BASE => {
                let rel = (other - param::MATRIX_BASE) as usize;
                let slot = rel / 3;
                let field = rel % 3;
                v.matrix.get(slot).map_or(0.0, |m| match field {
                    0 => m.source as u8 as f32,
                    1 => m.target as u8 as f32,
                    2 => m.amount,
                    _ => 0.0,
                })
            }
            _ => 0.0,
        }
    }

    pub fn all_param_pairs(&self) -> Vec<(u16, f32)> {
        let mut out = Vec::with_capacity(96);
        let static_ids: &[u16] = &[
            param::ENGINE,
            param::ROOM,
            param::DISTANCE,
            param::SHAPE,
            param::TONE,
            param::BODY,
            param::MOTION,
            param::MODE,
            param::TIME,
            param::TOUCH,
            param::GLIDE,
            param::GAIN,
            param::OSC_A_TABLE,
            param::OSC_A_LEVEL,
            param::OSC_A_UNISON,
            param::OSC_A_DETUNE,
            param::OSC_A_SPREAD,
            param::OSC_A_WARP_MODE,
            param::OSC_A_WARP,
            param::OSC_A_SEMI,
            param::OSC_B_TABLE,
            param::OSC_B_LEVEL,
            param::OSC_B_POS,
            param::OSC_B_SEMI,
            param::OSC_B_FINE,
            param::OSC_B_UNISON,
            param::OSC_B_DETUNE,
            param::SUB_LEVEL,
            param::SUB_OCTAVE,
            param::NOISE_LEVEL,
            param::NOISE_COLOR,
            param::FILTER_KIND,
            param::FILTER_RES,
            param::FILTER_DRIVE,
            param::FILTER_KEYTRACK,
            param::ENV1_OVERRIDE,
            param::ENV1_ATTACK,
            param::ENV1_DECAY,
            param::ENV1_SUSTAIN,
            param::ENV1_RELEASE,
            param::ENV2_ATTACK,
            param::ENV2_DECAY,
            param::ENV2_SUSTAIN,
            param::ENV2_RELEASE,
            param::ENV3_DECAY,
            param::LFO1_SHAPE,
            param::LFO1_RATE,
            param::LFO1_SYNC,
            param::LFO1_RETRIGGER,
            param::LFO2_SHAPE,
            param::LFO2_RATE,
            param::LFO2_SYNC,
            param::LFO2_RETRIGGER,
            param::CHORUS_MIX,
            param::CHORUS_RATE,
            param::DIST_MODE,
            param::DIST_DRIVE,
            param::DIST_MIX,
        ];
        for &id in static_ids {
            out.push((id, self.get_param(id)));
        }
        for i in 0..MATRIX_SLOTS {
            out.push((param::matrix_source(i), self.get_param(param::matrix_source(i))));
            out.push((param::matrix_target(i), self.get_param(param::matrix_target(i))));
            out.push((param::matrix_amount(i), self.get_param(param::matrix_amount(i))));
        }
        out
    }
}

#[derive(Clone, Copy)]
pub struct FactoryPreset {
    pub name: &'static str,
    pub category: &'static str,
    pub engine: SynthEngineKind,
    pub room: RoomKind,
    pub distance: f32,
    pub shape: f32,
    pub tone: f32,
    pub body: f32,
    pub motion: f32,
    pub mode: EnvelopeMode,
    pub time: f32,
    pub touch: f32,
    pub glide: f32,
    pub gain: f32,
}

impl FactoryPreset {
    pub fn to_params(&self) -> SynthParams {
        SynthParams {
            engine: self.engine,
            room: self.room,
            distance: self.distance,
            shape: self.shape,
            tone: self.tone,
            body: self.body,
            motion: self.motion,
            mode: self.mode,
            time: self.time,
            touch: self.touch,
            glide: self.glide,
            gain: self.gain,
            preset_name: self.name.to_owned(),
            voice: VoiceParams::default(),
            custom_wt_a: None,
            custom_wt_b: None,
        }
    }
}

/// 21 presets de fábrica calibrados cubriendo los 7 cuerpos sonoros.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    // --- Sub / 808 ---
    FactoryPreset {
        name: "Sub 808 Clean",
        category: "Bass",
        engine: SynthEngineKind::Sub808,
        room: RoomKind::Close,
        distance: 0.04,
        shape: 0.28,
        tone: 0.25,
        body: 0.12,
        motion: 0.05,
        mode: EnvelopeMode::Hit,
        time: 0.62,
        touch: 0.55,
        glide: 0.28,
        gain: 0.80,
    },
    FactoryPreset {
        name: "Phonk 808 Grit",
        category: "Bass",
        engine: SynthEngineKind::Sub808,
        room: RoomKind::Close,
        distance: 0.08,
        shape: 0.82,
        tone: 0.52,
        body: 0.22,
        motion: 0.08,
        mode: EnvelopeMode::Hit,
        time: 0.56,
        touch: 0.72,
        glide: 0.36,
        gain: 0.72,
    },
    FactoryPreset {
        name: "Chest Sub Hold",
        category: "Bass",
        engine: SynthEngineKind::Sub808,
        room: RoomKind::Wood,
        distance: 0.12,
        shape: 0.18,
        tone: 0.20,
        body: 0.35,
        motion: 0.15,
        mode: EnvelopeMode::Hold,
        time: 0.45,
        touch: 0.50,
        glide: 0.42,
        gain: 0.78,
    },
    // --- Analog ---
    FactoryPreset {
        name: "Warm Analog Pad",
        category: "Pad",
        engine: SynthEngineKind::Analog,
        room: RoomKind::Hall,
        distance: 0.48,
        shape: 0.55,
        tone: 0.42,
        body: 0.78,
        motion: 0.42,
        mode: EnvelopeMode::Swell,
        time: 0.68,
        touch: 0.65,
        glide: 0.12,
        gain: 0.66,
    },
    FactoryPreset {
        name: "Lo-Fi Tape Keys",
        category: "Keys",
        engine: SynthEngineKind::Analog,
        room: RoomKind::Wood,
        distance: 0.30,
        shape: 0.32,
        tone: 0.38,
        body: 0.55,
        motion: 0.52,
        mode: EnvelopeMode::Hold,
        time: 0.44,
        touch: 0.78,
        glide: 0.06,
        gain: 0.70,
    },
    FactoryPreset {
        name: "Supersaw Drift",
        category: "Lead",
        engine: SynthEngineKind::Analog,
        room: RoomKind::Hall,
        distance: 0.36,
        shape: 0.88,
        tone: 0.78,
        body: 0.92,
        motion: 0.35,
        mode: EnvelopeMode::Hold,
        time: 0.48,
        touch: 0.60,
        glide: 0.24,
        gain: 0.58,
    },
    // --- Pluck ---
    FactoryPreset {
        name: "Nylon Pluck",
        category: "Pluck",
        engine: SynthEngineKind::Pluck,
        room: RoomKind::Wood,
        distance: 0.28,
        shape: 0.38,
        tone: 0.56,
        body: 0.42,
        motion: 0.14,
        mode: EnvelopeMode::Hit,
        time: 0.52,
        touch: 0.80,
        glide: 0.0,
        gain: 0.78,
    },
    FactoryPreset {
        name: "Dark Harp",
        category: "Pluck",
        engine: SynthEngineKind::Pluck,
        room: RoomKind::Hall,
        distance: 0.54,
        shape: 0.24,
        tone: 0.48,
        body: 0.64,
        motion: 0.22,
        mode: EnvelopeMode::Hit,
        time: 0.74,
        touch: 0.75,
        glide: 0.0,
        gain: 0.76,
    },
    FactoryPreset {
        name: "Kalimba Wood",
        category: "Pluck",
        engine: SynthEngineKind::Pluck,
        room: RoomKind::Close,
        distance: 0.18,
        shape: 0.72,
        tone: 0.66,
        body: 0.35,
        motion: 0.10,
        mode: EnvelopeMode::Hit,
        time: 0.38,
        touch: 0.82,
        glide: 0.0,
        gain: 0.76,
    },
    // --- FM / Bell ---
    FactoryPreset {
        name: "FM E-Piano",
        category: "Keys",
        engine: SynthEngineKind::FmBell,
        room: RoomKind::Wood,
        distance: 0.26,
        shape: 0.22,
        tone: 0.58,
        body: 0.52,
        motion: 0.26,
        mode: EnvelopeMode::Hold,
        time: 0.54,
        touch: 0.82,
        glide: 0.0,
        gain: 0.68,
    },
    FactoryPreset {
        name: "Glass Bell",
        category: "Keys",
        engine: SynthEngineKind::FmBell,
        room: RoomKind::Open,
        distance: 0.58,
        shape: 0.78,
        tone: 0.76,
        body: 0.68,
        motion: 0.30,
        mode: EnvelopeMode::Hit,
        time: 0.72,
        touch: 0.70,
        glide: 0.0,
        gain: 0.64,
    },
    FactoryPreset {
        name: "Phonk Metal Lead",
        category: "Lead",
        engine: SynthEngineKind::FmBell,
        room: RoomKind::Close,
        distance: 0.18,
        shape: 0.62,
        tone: 0.68,
        body: 0.40,
        motion: 0.18,
        mode: EnvelopeMode::Hit,
        time: 0.34,
        touch: 0.68,
        glide: 0.15,
        gain: 0.68,
    },
    // --- Vocal ---
    FactoryPreset {
        name: "Soft Choir",
        category: "Vocal",
        engine: SynthEngineKind::Vocal,
        room: RoomKind::Hall,
        distance: 0.52,
        shape: 0.32,
        tone: 0.52,
        body: 0.80,
        motion: 0.44,
        mode: EnvelopeMode::Swell,
        time: 0.70,
        touch: 0.72,
        glide: 0.20,
        gain: 0.74,
    },
    FactoryPreset {
        name: "Ghost Vocal",
        category: "Vocal",
        engine: SynthEngineKind::Vocal,
        room: RoomKind::Open,
        distance: 0.68,
        shape: 0.74,
        tone: 0.44,
        body: 0.72,
        motion: 0.56,
        mode: EnvelopeMode::Swell,
        time: 0.78,
        touch: 0.66,
        glide: 0.32,
        gain: 0.76,
    },
    FactoryPreset {
        name: "Close Breath Keys",
        category: "Vocal",
        engine: SynthEngineKind::Vocal,
        room: RoomKind::Close,
        distance: 0.16,
        shape: 0.48,
        tone: 0.60,
        body: 0.48,
        motion: 0.32,
        mode: EnvelopeMode::Hold,
        time: 0.46,
        touch: 0.80,
        glide: 0.14,
        gain: 0.74,
    },
    // --- Fold / Acid ---
    FactoryPreset {
        name: "Acid Bass",
        category: "Bass",
        engine: SynthEngineKind::FoldAcid,
        room: RoomKind::Close,
        distance: 0.12,
        shape: 0.68,
        tone: 0.58,
        body: 0.26,
        motion: 0.22,
        mode: EnvelopeMode::Hit,
        time: 0.38,
        touch: 0.84,
        glide: 0.34,
        gain: 0.62,
    },
    FactoryPreset {
        name: "Grit Mono Lead",
        category: "Lead",
        engine: SynthEngineKind::FoldAcid,
        room: RoomKind::Wood,
        distance: 0.24,
        shape: 0.80,
        tone: 0.72,
        body: 0.58,
        motion: 0.28,
        mode: EnvelopeMode::Hold,
        time: 0.44,
        touch: 0.74,
        glide: 0.30,
        gain: 0.58,
    },
    FactoryPreset {
        name: "Dark Fold Poly",
        category: "Keys",
        engine: SynthEngineKind::FoldAcid,
        room: RoomKind::Hall,
        distance: 0.42,
        shape: 0.44,
        tone: 0.40,
        body: 0.70,
        motion: 0.36,
        mode: EnvelopeMode::Hold,
        time: 0.56,
        touch: 0.70,
        glide: 0.10,
        gain: 0.64,
    },
    // --- Texture ---
    FactoryPreset {
        name: "Night Fog Drone",
        category: "Texture",
        engine: SynthEngineKind::Texture,
        room: RoomKind::Open,
        distance: 0.74,
        shape: 0.46,
        tone: 0.36,
        body: 0.88,
        motion: 0.64,
        mode: EnvelopeMode::Swell,
        time: 0.85,
        touch: 0.62,
        glide: 0.26,
        gain: 0.72,
    },
    FactoryPreset {
        name: "Cold Air Shimmer",
        category: "Texture",
        engine: SynthEngineKind::Texture,
        room: RoomKind::Open,
        distance: 0.64,
        shape: 0.78,
        tone: 0.72,
        body: 0.84,
        motion: 0.54,
        mode: EnvelopeMode::Swell,
        time: 0.78,
        touch: 0.68,
        glide: 0.18,
        gain: 0.68,
    },
    FactoryPreset {
        name: "Vinyl Warm Bed",
        category: "Texture",
        engine: SynthEngineKind::Texture,
        room: RoomKind::Wood,
        distance: 0.36,
        shape: 0.28,
        tone: 0.42,
        body: 0.66,
        motion: 0.48,
        mode: EnvelopeMode::Hold,
        time: 0.62,
        touch: 0.64,
        glide: 0.12,
        gain: 0.70,
    },
];

pub fn find_factory_preset(name: &str) -> Option<&'static FactoryPreset> {
    let needle = name.trim().to_lowercase();
    FACTORY_PRESETS
        .iter()
        .find(|p| p.name.to_lowercase() == needle)
        .or_else(|| {
            FACTORY_PRESETS
                .iter()
                .find(|p| p.name.to_lowercase().contains(&needle))
        })
}

/// 7 presets avanzados estilo Vital (Wavetable + OSC B + Unison + Warp + Filtro 24 dB + Matrix).
pub fn vital_factory_presets() -> Vec<(&'static str, &'static str, SynthParams)> {
    let mut out = Vec::with_capacity(7);

    // 1. Vital Supersaw
    {
        let mut p = SynthParams {
            engine: SynthEngineKind::Wavetable,
            room: RoomKind::Hall,
            distance: 0.34,
            shape: 0.56,
            tone: 0.78,
            body: 0.85,
            motion: 0.28,
            mode: EnvelopeMode::Hold,
            time: 0.52,
            touch: 0.60,
            glide: 0.0,
            gain: 0.70,
            preset_name: "Vital Supersaw".into(),
            custom_wt_a: None,
            custom_wt_b: None,
            voice: VoiceParams {
                osc_a_table: WavetableKind::AnalogWarm,
                osc_a_unison: 7,
                osc_a_detune: 0.42,
                osc_a_spread: 0.92,
                osc_b_table: WavetableKind::BasicShapes,
                osc_b_level: 0.45,
                osc_b_pos: 0.55,
                osc_b_semi: 12,
                osc_b_unison: 3,
                osc_b_detune: 0.30,
                sub_level: 0.35,
                noise_level: 0.08,
                noise_color: 0.78,
                filter_kind: FilterKind::Lp12,
                filter_res: 0.16,
                filter_drive: 0.18,
                chorus_mix: 0.32,
                env1_override: true,
                env1_attack: 0.03,
                env1_decay: 0.40,
                env1_sustain: 0.82,
                env1_release: 0.38,
                env2_decay: 0.36,
                ..VoiceParams::default()
            },
        };
        p.voice.matrix[0] = ModSlot {
            source: ModSource::Env2,
            target: ModTarget::FilterCutoff,
            amount: 0.35,
        };
        p.voice.matrix[1] = ModSlot {
            source: ModSource::Distance,
            target: ModTarget::ChorusMix,
            amount: 0.40,
        };
        out.push(("Vital Supersaw", "Vital", p));
    }

    // 2. Reese Growl
    {
        let mut p = SynthParams {
            engine: SynthEngineKind::Wavetable,
            room: RoomKind::Close,
            distance: 0.12,
            shape: 0.48,
            tone: 0.42,
            body: 0.72,
            motion: 0.40,
            mode: EnvelopeMode::Hold,
            time: 0.44,
            touch: 0.72,
            glide: 0.48,
            gain: 0.72,
            preset_name: "Reese Growl".into(),
            custom_wt_a: None,
            custom_wt_b: None,
            voice: VoiceParams {
                osc_a_table: WavetableKind::GrowlBass,
                osc_a_unison: 5,
                osc_a_detune: 0.52,
                osc_a_spread: 0.70,
                osc_a_warp_mode: WarpMode::Bend,
                osc_a_warp: 0.38,
                osc_b_table: WavetableKind::BasicShapes,
                osc_b_level: 0.42,
                osc_b_pos: 0.50,
                osc_b_semi: 0,
                osc_b_fine: 0.28,
                sub_level: 0.65,
                filter_kind: FilterKind::Lp24,
                filter_res: 0.38,
                filter_drive: 0.45,
                dist_mode: DistMode::Soft,
                dist_drive: 0.52,
                dist_mix: 0.48,
                lfo1_shape: LfoShape::Sine,
                lfo1_rate: 0.26,
                lfo1_sync: true,
                ..VoiceParams::default()
            },
        };
        p.voice.matrix[0] = ModSlot {
            source: ModSource::Lfo1,
            target: ModTarget::OscAPos,
            amount: 0.42,
        };
        p.voice.matrix[1] = ModSlot {
            source: ModSource::Lfo1,
            target: ModTarget::FilterCutoff,
            amount: 0.30,
        };
        p.voice.matrix[2] = ModSlot {
            source: ModSource::Velocity,
            target: ModTarget::DistDrive,
            amount: 0.38,
        };
        out.push(("Reese Growl", "Vital Bass", p));
    }

    // 3. Crystal FM Pluck
    {
        let mut p = SynthParams {
            engine: SynthEngineKind::Wavetable,
            room: RoomKind::Hall,
            distance: 0.38,
            shape: 0.32,
            tone: 0.66,
            body: 0.60,
            motion: 0.22,
            mode: EnvelopeMode::Hit,
            time: 0.45,
            touch: 0.85,
            glide: 0.0,
            gain: 0.74,
            preset_name: "Crystal FM Pluck".into(),
            custom_wt_a: None,
            custom_wt_b: None,
            voice: VoiceParams {
                osc_a_table: WavetableKind::DigitalGlass,
                osc_a_unison: 3,
                osc_a_detune: 0.16,
                osc_a_warp_mode: WarpMode::FmFromB,
                osc_a_warp: 0.52,
                osc_b_table: WavetableKind::BasicShapes,
                osc_b_level: 0.0, // Modulador FM puro hacia A
                osc_b_pos: 0.0,
                osc_b_semi: 19, // 5ª + octava (ratio 3:1 cristalino)
                noise_level: 0.12,
                noise_color: 0.82,
                filter_kind: FilterKind::Lp12,
                filter_res: 0.22,
                env1_override: true,
                env1_attack: 0.004,
                env1_decay: 0.38,
                env1_sustain: 0.0,
                env1_release: 0.36,
                env2_attack: 0.002,
                env2_decay: 0.18,
                chorus_mix: 0.28,
                ..VoiceParams::default()
            },
        };
        p.voice.matrix[0] = ModSlot {
            source: ModSource::Env2,
            target: ModTarget::OscAWarp,
            amount: 0.65,
        };
        p.voice.matrix[1] = ModSlot {
            source: ModSource::Env2,
            target: ModTarget::FilterCutoff,
            amount: 0.48,
        };
        p.voice.matrix[2] = ModSlot {
            source: ModSource::Velocity,
            target: ModTarget::OscAPos,
            amount: 0.45,
        };
        out.push(("Crystal FM Pluck", "Vital Pluck", p));
    }

    // 4. Formant Talk Bass
    {
        let mut p = SynthParams {
            engine: SynthEngineKind::Wavetable,
            room: RoomKind::Wood,
            distance: 0.18,
            shape: 0.42,
            tone: 0.56,
            body: 0.64,
            motion: 0.32,
            mode: EnvelopeMode::Hold,
            time: 0.36,
            touch: 0.80,
            glide: 0.25,
            gain: 0.72,
            preset_name: "Formant Talk Bass".into(),
            custom_wt_a: None,
            custom_wt_b: None,
            voice: VoiceParams {
                osc_a_table: WavetableKind::VocalFormant,
                osc_a_unison: 3,
                osc_a_detune: 0.24,
                osc_a_warp_mode: WarpMode::Sync,
                osc_a_warp: 0.34,
                osc_b_table: WavetableKind::GrowlBass,
                osc_b_level: 0.35,
                osc_b_semi: -12,
                sub_level: 0.55,
                filter_kind: FilterKind::Bp12,
                filter_res: 0.48,
                filter_drive: 0.42,
                dist_mode: DistMode::Fold,
                dist_drive: 0.36,
                dist_mix: 0.35,
                lfo1_shape: LfoShape::SawDown,
                lfo1_rate: 0.44,
                lfo1_sync: true,
                ..VoiceParams::default()
            },
        };
        p.voice.matrix[0] = ModSlot {
            source: ModSource::Lfo1,
            target: ModTarget::OscAPos,
            amount: 0.68,
        };
        p.voice.matrix[1] = ModSlot {
            source: ModSource::Lfo1,
            target: ModTarget::FilterCutoff,
            amount: 0.42,
        };
        p.voice.matrix[2] = ModSlot {
            source: ModSource::Env3,
            target: ModTarget::OscAPitch,
            amount: 0.20,
        };
        out.push(("Formant Talk Bass", "Vital Bass", p));
    }

    // 5. Glass Motion Pad
    {
        let mut p = SynthParams {
            engine: SynthEngineKind::Wavetable,
            room: RoomKind::Open,
            distance: 0.56,
            shape: 0.36,
            tone: 0.52,
            body: 0.88,
            motion: 0.52,
            mode: EnvelopeMode::Swell,
            time: 0.76,
            touch: 0.64,
            glide: 0.14,
            gain: 0.68,
            preset_name: "Glass Motion Pad".into(),
            custom_wt_a: None,
            custom_wt_b: None,
            voice: VoiceParams {
                osc_a_table: WavetableKind::OrganBell,
                osc_a_unison: 5,
                osc_a_detune: 0.28,
                osc_a_spread: 0.95,
                osc_b_table: WavetableKind::VocalFormant,
                osc_b_level: 0.52,
                osc_b_pos: 0.45,
                osc_b_semi: 12,
                osc_b_unison: 3,
                osc_b_detune: 0.22,
                noise_level: 0.16,
                noise_color: 0.65,
                filter_kind: FilterKind::Lp12,
                filter_res: 0.24,
                chorus_mix: 0.48,
                chorus_rate: 0.24,
                lfo1_shape: LfoShape::Sine,
                lfo1_rate: 0.16,
                lfo1_sync: false,
                lfo2_shape: LfoShape::Tri,
                lfo2_rate: 0.11,
                lfo2_sync: false,
                ..VoiceParams::default()
            },
        };
        p.voice.matrix[0] = ModSlot {
            source: ModSource::Lfo1,
            target: ModTarget::OscAPos,
            amount: 0.50,
        };
        p.voice.matrix[1] = ModSlot {
            source: ModSource::Lfo2,
            target: ModTarget::OscBPos,
            amount: 0.55,
        };
        p.voice.matrix[2] = ModSlot {
            source: ModSource::Distance,
            target: ModTarget::NoiseLevel,
            amount: 0.35,
        };
        out.push(("Glass Motion Pad", "Vital Pad", p));
    }

    // 6. Acid Warp Lead
    {
        let mut p = SynthParams {
            engine: SynthEngineKind::Wavetable,
            room: RoomKind::Wood,
            distance: 0.20,
            shape: 0.62,
            tone: 0.46,
            body: 0.52,
            motion: 0.24,
            mode: EnvelopeMode::Hold,
            time: 0.32,
            touch: 0.84,
            glide: 0.34,
            gain: 0.68,
            preset_name: "Acid Warp Lead".into(),
            custom_wt_a: None,
            custom_wt_b: None,
            voice: VoiceParams {
                osc_a_table: WavetableKind::HarshFold,
                osc_a_unison: 2,
                osc_a_detune: 0.18,
                osc_a_warp_mode: WarpMode::Fold,
                osc_a_warp: 0.45,
                filter_kind: FilterKind::Lp24,
                filter_res: 0.68,
                filter_drive: 0.55,
                filter_keytrack: 0.55,
                env2_attack: 0.003,
                env2_decay: 0.24,
                dist_mode: DistMode::Hard,
                dist_drive: 0.38,
                dist_mix: 0.30,
                ..VoiceParams::default()
            },
        };
        p.voice.matrix[0] = ModSlot {
            source: ModSource::Env2,
            target: ModTarget::FilterCutoff,
            amount: 0.72,
        };
        p.voice.matrix[1] = ModSlot {
            source: ModSource::Velocity,
            target: ModTarget::OscAWarp,
            amount: 0.48,
        };
        out.push(("Acid Warp Lead", "Vital Lead", p));
    }

    // 7. Lo-Fi Drift Keys
    {
        let mut p = SynthParams {
            engine: SynthEngineKind::Wavetable,
            room: RoomKind::Wood,
            distance: 0.32,
            shape: 0.28,
            tone: 0.44,
            body: 0.64,
            motion: 0.56,
            mode: EnvelopeMode::Hold,
            time: 0.54,
            touch: 0.76,
            glide: 0.04,
            gain: 0.74,
            preset_name: "Lo-Fi Drift Keys".into(),
            custom_wt_a: None,
            custom_wt_b: None,
            voice: VoiceParams {
                osc_a_table: WavetableKind::PluckString,
                osc_a_unison: 3,
                osc_a_detune: 0.22,
                osc_b_table: WavetableKind::OrganBell,
                osc_b_level: 0.28,
                osc_b_pos: 0.22,
                osc_b_semi: 12,
                noise_level: 0.14,
                noise_color: 0.22,
                filter_kind: FilterKind::Lp12,
                filter_res: 0.14,
                chorus_mix: 0.36,
                chorus_rate: 0.18,
                dist_mode: DistMode::Bitcrush,
                dist_drive: 0.18,
                dist_mix: 0.16,
                lfo1_shape: LfoShape::Random,
                lfo1_rate: 0.20,
                lfo1_sync: false,
                ..VoiceParams::default()
            },
        };
        p.voice.matrix[0] = ModSlot {
            source: ModSource::Lfo1,
            target: ModTarget::OscAPitch,
            amount: 0.04,
        };
        p.voice.matrix[1] = ModSlot {
            source: ModSource::Velocity,
            target: ModTarget::FilterCutoff,
            amount: 0.40,
        };
        out.push(("Lo-Fi Drift Keys", "Vital Keys", p));
    }

    out
}
