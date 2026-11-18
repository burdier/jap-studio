//! Carga offline de archivos y picos para la waveform. Nunca llamar desde el callback.
use elevate::{
    StereoFrame,
    musical::clip::{ClipId, ClipProps},
    musical::track::TrackId,
    project::ProjectAudioClip,
};
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
};
use symphonia::core::{
    formats::{TrackType, probe::Hint},
    io::MediaSourceStream,
};

pub(crate) const PEAK_FRAMES: usize = 64;

pub(crate) struct AudioFile {
    pub name: String,
    /// Ruta del recurso original; `None` solo para audio generado en memoria.
    pub source_path: Option<PathBuf>,
    pub sample_rate: u32,
    pub frames: Vec<StereoFrame>,
    pub peaks: Vec<[f32; 2]>,
    pub max_peak: f32,
}

impl AudioFile {
    pub fn duration(&self) -> f64 {
        self.frames.len() as f64 / f64::from(self.sample_rate)
    }

    pub fn decode(path: &Path) -> Result<Self, String> {
        let source = File::open(path).map_err(|error| error.to_string())?;
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
            hint.with_extension(extension);
        }
        let mut format = symphonia::default::get_probe()
            .probe(
                &hint,
                MediaSourceStream::new(Box::new(source), Default::default()),
                Default::default(),
                Default::default(),
            )
            .map_err(|error| error.to_string())?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or("El archivo no contiene audio")?;
        let parameters = track
            .codec_params
            .as_ref()
            .and_then(|parameters| parameters.audio())
            .ok_or("Faltan los parámetros de audio")?;
        let track_id = track.id;
        let mut decoder = symphonia::default::get_codecs()
            .make_audio_decoder(parameters, &Default::default())
            .map_err(|error| error.to_string())?;
        let mut frames = Vec::new();
        let mut samples = Vec::<f32>::new();
        let mut sample_rate = 0;
        while let Some(packet) = format.next_packet().map_err(|error| error.to_string())? {
            if packet.track_id != track_id {
                continue;
            }
            let decoded = decoder.decode(&packet).map_err(|error| error.to_string())?;
            let channels = decoded.spec().channels().count();
            if !(1..=2).contains(&channels) {
                return Err("Este incremento admite archivos mono o estéreo".into());
            }
            let rate = decoded.spec().rate();
            if rate == 0 || (sample_rate != 0 && rate != sample_rate) {
                return Err("La frecuencia de muestreo del archivo cambia o no es válida".into());
            }
            sample_rate = rate;
            samples.resize(decoded.samples_interleaved(), 0.0);
            decoded.copy_to_slice_interleaved(&mut samples);
            for sample in samples.chunks_exact(channels) {
                let left = sample[0];
                let right = if channels == 1 { left } else { sample[1] };
                if !left.is_finite() || !right.is_finite() {
                    return Err("El archivo contiene muestras no finitas".into());
                }
                frames.push([left, right]);
            }
        }
        if frames.is_empty() {
            return Err("El archivo no contiene muestras".into());
        }
        let peaks: Vec<[f32; 2]> = frames
            .chunks(PEAK_FRAMES)
            .map(|block| {
                block
                    .iter()
                    .flatten()
                    .fold([0.0_f32, 0.0_f32], |[low, high], &value| {
                        [low.min(value), high.max(value)]
                    })
            })
            .collect();
        let max_peak = peaks.iter().fold(0.05_f32, |acc, &[low, high]| {
            acc.max(low.abs()).max(high.abs())
        });
        Ok(Self {
            source_path: Some(std::fs::canonicalize(path).map_err(|error| error.to_string())?),
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            sample_rate,
            frames,
            peaks,
            max_peak,
        })
    }
}

#[derive(Clone)]
pub(crate) struct AudioClip {
    pub id: ClipId,
    /// Nombre de la instancia, independiente del archivo compartido.
    pub name: String,
    pub file: Arc<AudioFile>,
    /// Ubicación elegida para este clip después de colectar recursos.
    pub source_path_override: Option<PathBuf>,
    pub track: usize,
    pub start_beat: f64,
    pub props: ClipProps,
    /// Recorte no destructivo: segundos del archivo omitidos al inicio.
    pub trim_start: f64,
    /// Recorte no destructivo: segundos visibles; `None` = hasta el final del archivo.
    pub trim_len: Option<f64>,
}

impl AudioClip {
    pub fn project_metadata(&self, track: TrackId) -> Result<ProjectAudioClip, String> {
        let path = self
            .source_path_override
            .as_ref()
            .or(self.file.source_path.as_ref())
            .ok_or("El clip de audio no tiene archivo de origen")?;
        let source = path
            .to_str()
            .ok_or("La ruta de audio no es UTF-8")?
            .to_owned();
        Ok(ProjectAudioClip {
            id: self.id,
            track,
            source,
            name: self.name.clone(),
            start_beat: self.start_beat,
            trim_start: self.trim_start,
            trim_len: self.trim_len,
            props: self.props,
        })
    }

    /// Copia editable; el snapshot de render usa `Clone` y conserva la identidad.
    pub fn duplicate(&self) -> Self {
        let mut copy = self.clone();
        copy.id = ClipId::fresh();
        copy
    }
    pub fn length_seconds(&self) -> f64 {
        let available = (self.file.duration() - self.trim_start).max(0.0);
        self.trim_len.map_or(available, |len| len.min(available))
    }
    pub fn length_beats(&self, bpm: f64) -> f64 {
        self.length_seconds() * bpm / 60.0
    }
    pub fn end_beat(&self, bpm: f64) -> f64 {
        self.start_beat + self.length_beats(bpm)
    }
}
