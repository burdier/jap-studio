//! Modelo headless de Hay Synth: Espacio + Cuerpo + Intención/Toque.
//! Sin GPUI ni CPAL; serializable en proyectos `.loro`, P2P y presets JSON.

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
}

impl SynthEngineKind {
    pub const ALL: [Self; 7] = [
        Self::Sub808,
        Self::Analog,
        Self::Pluck,
        Self::FmBell,
        Self::Vocal,
        Self::FoldAcid,
        Self::Texture,
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

#[derive(Clone, Debug, PartialEq)]
pub struct SynthParams {
    pub engine: SynthEngineKind,
    pub room: RoomKind,
    /// 0.0 = en el pecho (cerca/seco), 1.0 = lejos en el espacio.
    pub distance: f32,
    /// Carácter propio de cada motor.
    pub shape: f32,
    /// 0.0 = oscuro/suave, 1.0 = brillante/abierto.
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
