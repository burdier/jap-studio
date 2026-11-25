//! Decodifica fuera de RT y prepara WAV float para sfizz, incluidos los codecs de Symphonia.
use crate::audio::AudioFile;
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

pub(crate) struct SampleAsset {
    pub file: AudioFile,
    pub source: PathBuf,
    pub display_peaks: Vec<[f32; 2]>,
    pub(super) wave: PathBuf,
}
impl Drop for SampleAsset {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.wave);
    }
}
impl SampleAsset {
    #[cfg(test)]
    pub(crate) fn from_frames(frames: Vec<elevate::StereoFrame>, sample_rate: u32) -> Arc<Self> {
        let max_peak = frames
            .iter()
            .flatten()
            .fold(0.0_f32, |m, &x| m.max(x.abs()));
        Arc::new(Self {
            file: AudioFile {
                name: "test_sample".into(),
                source_path: None,
                frames,
                sample_rate,
                peaks: Vec::new(),
                max_peak,
            },
            source: PathBuf::from("test_sample.wav"),
            display_peaks: Vec::new(),
            wave: PathBuf::new(),
        })
    }

    pub fn load(path: &Path) -> Result<Arc<Self>, String> {
        let file = AudioFile::decode(path)?;
        if file.frames.is_empty() {
            return Err("El sample está vacío".into());
        }
        let bytes = file
            .frames
            .len()
            .checked_mul(8)
            .filter(|n| *n <= (u32::MAX - 36) as usize)
            .ok_or("Sample demasiado grande para WAV")? as u32;
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let wave = std::env::temp_dir().join(format!(
            "hay-sampler-{}-{}.wav",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&wave)
            .map_err(|e| e.to_string())?;
        let display_peaks = file
            .peaks
            .chunks(file.peaks.len().div_ceil(1024).max(1))
            .map(|group| {
                group
                    .iter()
                    .fold([0f32, 0f32], |p, a| [p[0].min(a[0]), p[1].max(a[1])])
            })
            .collect();
        let asset = Self {
            file,
            source: path.to_owned(),
            wave,
            display_peaks,
        };
        write_wave(output, &asset.file, bytes).map_err(|e| e.to_string())?;
        Ok(Arc::new(asset))
    }
}
fn write_wave(output: File, file: &AudioFile, bytes: u32) -> std::io::Result<()> {
    let mut writer = BufWriter::new(output);
    writer.write_all(b"RIFF")?;
    writer.write_all(&(bytes + 36).to_le_bytes())?;
    writer.write_all(b"WAVEfmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&3u16.to_le_bytes())?;
    writer.write_all(&2u16.to_le_bytes())?;
    writer.write_all(&file.sample_rate.to_le_bytes())?;
    writer.write_all(&(file.sample_rate * 8).to_le_bytes())?;
    writer.write_all(&8u16.to_le_bytes())?;
    writer.write_all(&32u16.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&bytes.to_le_bytes())?;
    for sample in file.frames.iter().flatten() {
        writer.write_all(&sample.to_le_bytes())?;
    }
    writer.flush()
}
