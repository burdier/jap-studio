//! Grabación de audio en tiempo real mediante CPAL + ring buffer lock-free (`rtrb`)
//! + hilo escritor WAV IEEE 32-bit float en streaming. Sin GPUI ni I/O en el callback RT.
use super::{device::AudioConfig, output::choose_input_config};
use cpal::{
    Sample, SampleFormat, SizedSample,
    traits::{DeviceTrait, StreamTrait},
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{
    fs::File,
    io::{BufWriter, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const RING_CAPACITY_FRAMES: usize = 131_072; // ~2.7 s @ 48 kHz

pub(crate) struct RecordedTake {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub frames: u64,
    pub dropped_frames: u64,
    pub stereo: bool,
}

struct RecorderShared {
    capturing: AtomicBool,
    stop_writer: AtomicBool,
    peak: AtomicU32,
    frames_pushed: AtomicU64,
    dropped_frames: AtomicU64,
    failed: AtomicBool,
}

pub(crate) struct AudioRecorder {
    stream: Option<cpal::Stream>,
    writer: Option<JoinHandle<Result<u64, String>>>,
    shared: Arc<RecorderShared>,
    path: PathBuf,
    pub sample_rate: u32,
    pub stereo: bool,
    pub device_name: String,
}

impl AudioRecorder {
    /// Abre el dispositivo de entrada configurado y arranca la captura RT + hilo escritor WAV.
    /// Si `start_capturing` es `false` (ej. durante pre-cuenta), el medidor ya funciona en vivo
    /// pero no escribe audio hasta llamar a `set_capturing(true)`.
    pub fn start(
        config: &AudioConfig,
        output_path: PathBuf,
        stereo: bool,
        start_capturing: bool,
    ) -> Result<Self, String> {
        let device = config.device(true)?;
        let device_name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Entrada predeterminada".into());
        let setup = choose_input_config(&device, config)?;
        let sample_rate = setup.config.sample_rate;
        let channels = usize::from(setup.config.channels);
        let first = usize::from(setup.first_channel).min(channels.saturating_sub(1));
        let effective_stereo = stereo && first + 1 < channels;

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("No se pudo crear {}: {e}", parent.display()))?;
        }

        let (producer, consumer) = RingBuffer::<[f32; 2]>::new(RING_CAPACITY_FRAMES);
        let shared = Arc::new(RecorderShared {
            capturing: AtomicBool::new(start_capturing),
            stop_writer: AtomicBool::new(false),
            peak: AtomicU32::new(0),
            frames_pushed: AtomicU64::new(0),
            dropped_frames: AtomicU64::new(0),
            failed: AtomicBool::new(false),
        });

        let writer_path = output_path.clone();
        let writer_shared = shared.clone();
        let writer = thread::Builder::new()
            .name("hay-wav-recorder".into())
            .spawn(move || run_wav_writer(&writer_path, sample_rate, consumer, &writer_shared))
            .map_err(|e| format!("No se pudo iniciar hilo de grabación: {e}"))?;

        let stream = match setup.format {
            SampleFormat::F32 => build_input_stream::<f32>(
                &device,
                setup.config,
                channels,
                first,
                effective_stereo,
                producer,
                shared.clone(),
            ),
            SampleFormat::I16 => build_input_stream::<i16>(
                &device,
                setup.config,
                channels,
                first,
                effective_stereo,
                producer,
                shared.clone(),
            ),
            SampleFormat::I32 => build_input_stream::<i32>(
                &device,
                setup.config,
                channels,
                first,
                effective_stereo,
                producer,
                shared.clone(),
            ),
            SampleFormat::U16 => build_input_stream::<u16>(
                &device,
                setup.config,
                channels,
                first,
                effective_stereo,
                producer,
                shared.clone(),
            ),
            _ => Err("Formato de entrada de audio no compatible".into()),
        };

        let stream = match stream {
            Ok(s) => s,
            Err(err) => {
                shared.stop_writer.store(true, Ordering::Release);
                let _ = writer.join();
                let _ = std::fs::remove_file(&output_path);
                return Err(err);
            }
        };

        if let Err(err) = stream.play() {
            drop(stream);
            shared.stop_writer.store(true, Ordering::Release);
            let _ = writer.join();
            let _ = std::fs::remove_file(&output_path);
            return Err(format!("No se pudo iniciar la entrada de audio: {err}"));
        }

        Ok(Self {
            stream: Some(stream),
            writer: Some(writer),
            shared,
            path: output_path,
            sample_rate,
            stereo: effective_stereo,
            device_name,
        })
    }

    pub fn set_capturing(&self, active: bool) {
        self.shared.capturing.store(active, Ordering::Release);
    }

    pub fn peak(&self) -> f32 {
        f32::from_bits(self.shared.peak.load(Ordering::Relaxed))
    }

    pub fn captured_seconds(&self) -> f64 {
        self.shared.frames_pushed.load(Ordering::Relaxed) as f64 / f64::from(self.sample_rate)
    }

    pub fn failed(&self) -> bool {
        self.shared.failed.load(Ordering::Relaxed)
    }

    /// Detiene el stream de entrada, vacía el ring buffer a disco y cierra la cabecera WAV.
    pub fn finish(mut self) -> Result<RecordedTake, String> {
        self.shared.capturing.store(false, Ordering::Release);
        drop(self.stream.take());
        self.shared.stop_writer.store(true, Ordering::Release);
        let frames = match self.writer.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| "El hilo escritor de grabación se interrumpió".to_string())??,
            None => 0,
        };
        let dropped_frames = self.shared.dropped_frames.load(Ordering::Relaxed);
        if frames == 0 {
            let _ = std::fs::remove_file(&self.path);
        }
        Ok(RecordedTake {
            path: self.path.clone(),
            sample_rate: self.sample_rate,
            frames,
            dropped_frames,
            stereo: self.stereo,
        })
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        self.shared.capturing.store(false, Ordering::Release);
        drop(self.stream.take());
        self.shared.stop_writer.store(true, Ordering::Release);
        if let Some(handle) = self.writer.take() {
            let _ = handle.join();
        }
    }
}

fn build_input_stream<T: SizedSample + Sample>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    first: usize,
    stereo: bool,
    mut producer: Producer<[f32; 2]>,
    shared: Arc<RecorderShared>,
) -> Result<cpal::Stream, String>
where
    f32: cpal::FromSample<T>,
{
    let err_shared = shared.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let active = shared.capturing.load(Ordering::Acquire);
                let mut block_peak = 0.0_f32;
                let mut pushed = 0u64;
                let mut dropped = 0u64;
                for frame in data.chunks_exact(channels) {
                    let raw_l: f32 = frame[first].to_sample();
                    let raw_r: f32 = if stereo {
                        frame[first + 1].to_sample()
                    } else {
                        raw_l
                    };
                    let l = if raw_l.is_finite() { raw_l } else { 0.0 };
                    let r = if raw_r.is_finite() { raw_r } else { 0.0 };
                    block_peak = block_peak.max(l.abs().max(r.abs()));
                    if active {
                        if producer.push([l, r]).is_ok() {
                            pushed += 1;
                        } else {
                            dropped += 1;
                        }
                    }
                }
                let prev = f32::from_bits(shared.peak.load(Ordering::Relaxed));
                let smoothed = block_peak.max(prev * 0.72);
                shared.peak.store(smoothed.to_bits(), Ordering::Relaxed);
                if pushed > 0 {
                    shared.frames_pushed.fetch_add(pushed, Ordering::Relaxed);
                }
                if dropped > 0 {
                    shared.dropped_frames.fetch_add(dropped, Ordering::Relaxed);
                }
            },
            move |err| {
                eprintln!("Error de entrada de audio: {err}");
                err_shared.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| format!("No se pudo abrir la entrada de audio: {e}"))
}

fn wav_header_f32_stereo(sample_rate: u32, data_bytes: u32) -> [u8; 44] {
    let channels: u16 = 2;
    let bits_per_sample: u16 = 32;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate.saturating_mul(u32::from(block_align));
    let riff_size = 36u32.saturating_add(data_bytes);
    let mut header = [0u8; 44];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&riff_size.to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes());
    header[20..22].copy_from_slice(&3u16.to_le_bytes()); // IEEE float
    header[22..24].copy_from_slice(&channels.to_le_bytes());
    header[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    header[32..34].copy_from_slice(&block_align.to_le_bytes());
    header[34..36].copy_from_slice(&bits_per_sample.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&data_bytes.to_le_bytes());
    header
}

fn run_wav_writer(
    path: &Path,
    sample_rate: u32,
    mut consumer: Consumer<[f32; 2]>,
    shared: &RecorderShared,
) -> Result<u64, String> {
    let file = File::create(path).map_err(|e| {
        format!(
            "No se pudo crear archivo de grabación {}: {e}",
            path.display()
        )
    })?;
    let mut writer = BufWriter::with_capacity(65_536, file);
    writer
        .write_all(&wav_header_f32_stereo(sample_rate, 0))
        .map_err(|e| format!("Error escribiendo cabecera WAV: {e}"))?;

    let mut total_frames: u64 = 0;
    let mut bytes_buf = Vec::with_capacity(4096 * 8);
    loop {
        let mut drained = 0usize;
        bytes_buf.clear();
        while drained < 4096 {
            let Ok([l, r]) = consumer.pop() else {
                break;
            };
            bytes_buf.extend_from_slice(&l.to_le_bytes());
            bytes_buf.extend_from_slice(&r.to_le_bytes());
            drained += 1;
        }
        if drained > 0 {
            writer
                .write_all(&bytes_buf)
                .map_err(|e| format!("Error escribiendo audio grabado: {e}"))?;
            total_frames += drained as u64;
            continue;
        }
        if shared.stop_writer.load(Ordering::Acquire) {
            break;
        }
        thread::sleep(Duration::from_millis(8));
    }

    writer
        .flush()
        .map_err(|e| format!("Error vaciando buffer de grabación: {e}"))?;
    let mut file = writer
        .into_inner()
        .map_err(|e| format!("Error finalizando archivo WAV: {e}"))?;
    let data_bytes = u32::try_from(total_frames.saturating_mul(8))
        .map_err(|_| "La grabación superó el límite de 4 GB del formato WAV".to_string())?;
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("Error actualizando cabecera WAV: {e}"))?;
    file.write_all(&wav_header_f32_stereo(sample_rate, data_bytes))
        .map_err(|e| format!("Error escribiendo cabecera WAV final: {e}"))?;
    file.flush()
        .map_err(|e| format!("Error sincronizando archivo WAV: {e}"))?;
    Ok(total_frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioFile;

    #[test]
    fn streaming_wav_writer_creates_decodable_32f_file() {
        let dir = std::env::temp_dir().join(format!(
            "hay-rec-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("take.wav");
        let (mut tx, rx) = RingBuffer::<[f32; 2]>::new(4096);
        let shared = Arc::new(RecorderShared {
            capturing: AtomicBool::new(true),
            stop_writer: AtomicBool::new(false),
            peak: AtomicU32::new(0),
            frames_pushed: AtomicU64::new(0),
            dropped_frames: AtomicU64::new(0),
            failed: AtomicBool::new(false),
        });
        let w_path = path.clone();
        let w_shared = shared.clone();
        let handle = thread::spawn(move || run_wav_writer(&w_path, 48_000, rx, &w_shared));

        for i in 0..1024 {
            let s = (i as f32 * 0.05).sin() * 0.6;
            tx.push([s, -s]).unwrap();
        }
        shared.stop_writer.store(true, Ordering::Release);
        let written = handle.join().unwrap().unwrap();
        assert_eq!(written, 1024);

        let decoded = AudioFile::decode(&path).unwrap();
        assert_eq!(decoded.sample_rate, 48_000);
        assert_eq!(decoded.frames.len(), 1024);
        assert!((decoded.frames[10][0] - (10.0_f32 * 0.05).sin() * 0.6).abs() < 1e-5);
        assert!((decoded.frames[10][1] + (10.0_f32 * 0.05).sin() * 0.6).abs() < 1e-5);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
