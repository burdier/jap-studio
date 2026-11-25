//! Audición en vivo del kit de batería fuera de Play (pads MPC y roll con el transporte detenido).
//! Reutiliza `PreparedKit` + `LiveDrums` sin asignaciones ni bloqueos en el callback CPAL.
use super::{
    controls::DrumBlock,
    kit::PreparedKit,
    live::{LiveDrums, VoiceHandoff},
    mix::{DrumPreview, NoteRepeatClock, NoteRepeatControl},
};
use crate::audio::{
    device::AudioConfig,
    output::{StreamSetup, choose_output_config, write_stereo},
    publication::Publication,
};
use cpal::{
    FromSample, SampleFormat, SizedSample,
    traits::{DeviceTrait, StreamTrait},
};
use elevate::musical::{drums::DrumKit, model::InstrumentId};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;

const BLOCK: usize = 256;

pub(crate) struct DrumAudition {
    _stream: cpal::Stream,
    live: LiveDrums,
    previews: Producer<DrumPreview>,
    sample_rate: u32,
}

impl DrumAudition {
    pub fn open(
        kit: DrumKit,
        audio: &AudioConfig,
        bpm: f64,
        note_repeat: Arc<NoteRepeatControl>,
    ) -> Result<Self, String> {
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, audio)?;
        let rate = setup.config.sample_rate;
        let prepared = PreparedKit::prepare(&kit, &Default::default(), rate)?;
        let (live, handoff) = LiveDrums::prepare(kit, None, prepared.assets.clone(), rate, bpm)?;
        let state = live.state.clone();
        let (preview_tx, preview_rx) = RingBuffer::new(64);
        let stream = match setup.format {
            SampleFormat::F32 => build_stream::<f32>(
                &device,
                &setup,
                prepared,
                handoff,
                state,
                preview_rx,
                note_repeat,
            ),
            SampleFormat::I16 => build_stream::<i16>(
                &device,
                &setup,
                prepared,
                handoff,
                state,
                preview_rx,
                note_repeat,
            ),
            SampleFormat::U16 => build_stream::<u16>(
                &device,
                &setup,
                prepared,
                handoff,
                state,
                preview_rx,
                note_repeat,
            ),
            other => Err(format!("Formato de salida no soportado: {other:?}")),
        }?;
        Ok(Self {
            _stream: stream,
            live,
            previews: preview_tx,
            sample_rate: rate,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn trigger(&mut self, note: DrumPreview) -> bool {
        self.previews.push(note).is_ok()
    }

    pub fn update_kit(&mut self, kit: &DrumKit) {
        self.live.edit_kit(kit);
    }

    pub fn poll(&mut self) -> Result<(), String> {
        self.live.poll()
    }

    pub fn piece_peak(&self, id: InstrumentId) -> f32 {
        self.live.peak(Some(id))
    }

    pub fn bus_peak(&self) -> f32 {
        self.live.peak(None)
    }
}

fn build_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    setup: &StreamSetup,
    mut kit: PreparedKit,
    mut handoff: VoiceHandoff,
    state: Arc<Publication<DrumBlock>>,
    mut previews: Consumer<DrumPreview>,
    note_repeat: Arc<NoteRepeatControl>,
) -> Result<cpal::Stream, String> {
    let channels = setup.config.channels as usize;
    let first = setup.first_channel as usize;
    let mut repeat_clock = NoteRepeatClock::default();
    let mut mix_buf = [[0.0_f32; 2]; BLOCK];
    let stream = device
        .build_output_stream(
            setup.config.clone(),
            move |data: &mut [T], _| {
                let total_frames = data.len() / channels.max(1);
                let mut offset = 0usize;
                while offset < total_frames {
                    let count = (total_frames - offset).min(BLOCK);
                    let out = &mut mix_buf[..count];
                    out.fill([0.0; 2]);
                    state.read(|block| {
                        handoff.install(&mut kit, block.voice_revision);
                        kit.apply_controls(block);
                        for _ in 0..32 {
                            let Ok(note) = previews.pop() else {
                                break;
                            };
                            kit.trigger(block, note.piece, note.pitch, note.velocity, 0);
                        }
                        let mut repeat_hits = [(
                            0usize,
                            DrumPreview {
                                piece: InstrumentId(0),
                                pitch: 36,
                                velocity: 100,
                            },
                        ); 8];
                        let hits = repeat_clock.collect(&note_repeat, count, &mut repeat_hits);
                        let mut cursor = 0usize;
                        for &(pos, note) in &repeat_hits[..hits] {
                            kit.render_segment(cursor, pos);
                            cursor = pos;
                            kit.trigger(block, note.piece, note.pitch, note.velocity, 0);
                        }
                        kit.render_segment(cursor, count);
                        kit.sum(block, out, None);
                    });
                    let frame_slice = &mut data[offset * channels..(offset + count) * channels];
                    write_stereo(frame_slice, out, first, channels);
                    offset += count;
                }
            },
            |_| {},
            None,
        )
        .map_err(|e| format!("Stream de audición de batería: {e}"))?;
    stream
        .play()
        .map_err(|e| format!("Inicio de audición de batería: {e}"))?;
    Ok(stream)
}
