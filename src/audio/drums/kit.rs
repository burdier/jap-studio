//! Voces por pieza, mezcla y pico del bus de percusión.
use super::{controls::DrumBlock, voice::DrumKit as BuiltinDrums};
use crate::audio::{
    sampler::{SampleAsset, SfizzEngine},
    stereo_pan::StereoPan,
};
use elevate::{
    Primitive, ProcessSpec, StereoFrame,
    devices::sampler::TUNE,
    musical::{
        drums::{DrumKit, DrumPiece, MAX_DRUM_PIECES},
        model::InstrumentId,
    },
    primitives::amplitude::Volume,
};
use std::{collections::HashMap, path::Path, sync::Arc};

const BLOCK: usize = 256;
const MAX_LAYERS: usize = 128;

enum Sound {
    Samples(SfizzEngine),
    Builtin(Box<BuiltinDrums>),
    Silent,
}

struct Piece {
    id: InstrumentId,
    sound: Sound,
    volume: Volume,
    pan: StereoPan,
    buffer: [StereoFrame; BLOCK],
}

impl Piece {
    fn prepare(
        piece: &DrumPiece,
        assets: &mut HashMap<String, Arc<SampleAsset>>,
        spec: ProcessSpec,
        rate: u32,
    ) -> Result<Self, String> {
        let sound = if piece.layers.is_empty() {
            if (1..=4).contains(&piece.id.0) {
                Sound::Builtin(Box::new(BuiltinDrums::new()))
            } else {
                Sound::Silent
            }
        } else {
            let layers = piece
                .layers
                .iter()
                .map(|layer| {
                    let source = &layer.source;
                    let asset = if let Some(asset) = assets.get(source) {
                        asset.clone()
                    } else {
                        let asset = SampleAsset::load(Path::new(source))
                            .map_err(|error| format!("{} · {source}: {error}", piece.name))?;
                        assets.insert(source.clone(), asset.clone());
                        asset
                    };
                    Ok((asset, layer.velocity_min, layer.velocity_max))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Sound::Samples(SfizzEngine::prepare_layers(
                layers,
                piece.sampler.clone(),
                rate,
            )?)
        };
        Ok(Self {
            id: piece.id,
            sound,
            volume: Volume::new(spec),
            pan: StereoPan::new(0.0),
            buffer: [[0.0; 2]; BLOCK],
        })
    }

    fn cut(&mut self) {
        match &mut self.sound {
            Sound::Samples(engine) => engine.cut(),
            Sound::Builtin(engine) => **engine = BuiltinDrums::new(),
            Sound::Silent => {}
        }
    }

    fn render(&mut self, start: usize, end: usize, dt: f64) {
        let output = &mut self.buffer[start..end];
        match &mut self.sound {
            Sound::Samples(engine) => {
                let mut left = [0.0; BLOCK];
                let mut right = [0.0; BLOCK];
                engine.render(&mut left[..output.len()], &mut right[..output.len()]);
                for (frame, (l, r)) in output.iter_mut().zip(left.into_iter().zip(right)) {
                    *frame = [l, r];
                }
            }
            Sound::Builtin(engine) => engine.render(output, dt),
            Sound::Silent => output.fill([0.0; 2]),
        }
    }
}

pub(super) struct PreparedKit {
    pieces: Vec<Piece>,
    volume: Volume,
    pan: StereoPan,
    pub assets: Arc<HashMap<String, Arc<SampleAsset>>>,
    dt: f64,
    peak_decay: f32,
}

impl PreparedKit {
    pub fn prepare(
        kit: &DrumKit,
        previous_assets: &HashMap<String, Arc<SampleAsset>>,
        rate: u32,
    ) -> Result<Self, String> {
        if kit.pieces.len() > MAX_DRUM_PIECES
            || kit
                .pieces
                .iter()
                .any(|piece| piece.layers.len() > MAX_LAYERS)
        {
            return Err("El kit admite hasta 64 piezas y 128 capas por pieza".into());
        }
        let spec = ProcessSpec::new(rate as f64).map_err(|error| error.to_string())?;
        let mut assets = HashMap::new();
        for layer in kit.pieces.iter().flat_map(|piece| &piece.layers) {
            if let Some(asset) = previous_assets.get(&layer.source) {
                assets.insert(layer.source.clone(), asset.clone());
            }
        }
        let pieces = kit
            .pieces
            .iter()
            .map(|piece| Piece::prepare(piece, &mut assets, spec, rate))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            pieces,
            volume: Volume::new(spec),
            pan: StereoPan::new(0.0),
            assets: Arc::new(assets),
            dt: 1.0 / f64::from(rate),
            peak_decay: 1.0 / (0.24 * rate as f32),
        })
    }

    pub fn cut(&mut self) {
        for piece in &mut self.pieces {
            piece.cut();
        }
    }

    pub fn apply_controls(&mut self, state: &DrumBlock) {
        for piece in &mut self.pieces {
            if let Some(control) = state.piece(piece.id)
                && let Sound::Samples(engine) = &mut piece.sound
            {
                engine.update_live(&control.params);
            }
        }
    }

    pub fn trigger(
        &mut self,
        state: &DrumBlock,
        id: InstrumentId,
        pitch: u8,
        velocity: u8,
        note: u64,
    ) {
        let Some(control) = state.piece(id) else {
            return;
        };
        let Some(index) = self.pieces.iter().position(|piece| piece.id == id) else {
            return;
        };
        if let Some(group) = control.choke_group {
            for piece in &mut self.pieces {
                if state
                    .piece(piece.id)
                    .is_some_and(|piece| piece.choke_group == Some(group))
                {
                    piece.cut();
                }
            }
        }
        match &mut self.pieces[index].sound {
            Sound::Samples(engine) => engine.note_on(pitch, velocity),
            Sound::Builtin(engine) => {
                let offset =
                    f32::from(pitch) - f32::from(control.base_note) + control.params.get(TUNE);
                engine.trigger_pitched(id, velocity, note, (f64::from(offset) / 12.0).exp2());
            }
            Sound::Silent => {}
        }
    }

    pub fn note_off(&mut self, id: InstrumentId, pitch: u8, velocity: u8) {
        if let Some(piece) = self.pieces.iter_mut().find(|piece| piece.id == id)
            && let Sound::Samples(engine) = &mut piece.sound
        {
            engine.note_off_velocity(pitch, velocity);
        }
    }

    pub fn render_segment(&mut self, start: usize, end: usize) {
        if start < end {
            for piece in &mut self.pieces {
                piece.render(start, end, self.dt);
            }
        }
    }

    pub fn sum(&mut self, state: &DrumBlock, output: &mut [StereoFrame]) {
        let count = output.len();
        let mut bus = [[0.0; 2]; BLOCK];
        let decay = (1.0 - self.peak_decay * count as f32).max(0.0);
        for piece in &mut self.pieces {
            let Some(control) = state.piece(piece.id) else {
                continue;
            };
            let frames = &mut piece.buffer[..count];
            apply_mix(
                &mut piece.volume,
                &piece.pan,
                control.gain,
                control.pan,
                frames,
            );
            control.peak.measure(frames, decay);
            for (sum, frame) in bus.iter_mut().zip(frames) {
                sum[0] += frame[0];
                sum[1] += frame[1];
            }
        }
        let frames = &mut bus[..count];
        apply_mix(
            &mut self.volume,
            &self.pan,
            state.bus_gain,
            state.bus_pan,
            frames,
        );
        state.bus_peak.measure(frames, decay);
        for (sum, frame) in output.iter_mut().zip(frames) {
            sum[0] += frame[0];
            sum[1] += frame[1];
        }
    }
}

fn apply_mix(
    volume: &mut Volume,
    pan: &StereoPan,
    gain: f32,
    position: f32,
    frames: &mut [StereoFrame],
) {
    if volume.gain() != gain {
        let _ = volume.set_gain(gain);
    }
    volume.process(frames);
    pan.set(position);
    pan.apply(frames);
}
