//! Medición EBU R128, normalización de sonoridad LUFS y limitador True Peak 4× offline.
//! No importa GPUI ni CPAL; trabaja por bloques con memoria acotada.
use ebur128::{EbuR128, Mode};
use std::{
    collections::VecDeque,
    f32::consts::PI,
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

/// Resumen de medición y procesamiento de sonoridad del archivo exportado.
#[derive(Clone, Debug, Default)]
pub(crate) struct LoudnessSummary {
    /// Sonoridad integrada final (LUFS-I) según ITU-R BS.1770-4 / EBU R128.
    pub integrated_lufs: Option<f64>,
    /// Máximo pico verdadero final en dBTP (4× oversampling).
    pub true_peak_dbtp: Option<f64>,
    /// Rango de sonoridad (LRA) en LU, cuando la duración es suficiente.
    pub lra_lu: Option<f64>,
    /// Ganancia lineal aplicada por normalización LUFS (en dB).
    pub applied_gain_db: f64,
    /// Máxima reducción de ganancia del limitador True Peak (en dB positivos, ej. `2.4`).
    pub max_gain_reduction_db: f64,
}

impl LoudnessSummary {
    /// Combina el resumen de varios stems conservando el peor True Peak y la máxima reducción.
    pub(crate) fn merge_stem(&mut self, other: &Self) {
        if let Some(tp) = other.true_peak_dbtp {
            self.true_peak_dbtp = Some(self.true_peak_dbtp.map_or(tp, |cur| cur.max(tp)));
        }
        if let Some(lufs) = other.integrated_lufs {
            self.integrated_lufs = Some(self.integrated_lufs.map_or(lufs, |cur| cur.max(lufs)));
        }
        if self.lra_lu.is_none() {
            self.lra_lu = other.lra_lu;
        }
        if other.applied_gain_db.abs() > self.applied_gain_db.abs() {
            self.applied_gain_db = other.applied_gain_db;
        }
        self.max_gain_reduction_db = self.max_gain_reduction_db.max(other.max_gain_reduction_db);
    }
}

/// Medidor EBU R128 offline para 1 o 2 canales intercalados (`f32`).
pub(super) struct ExportMeter {
    ebu: EbuR128,
    channels: usize,
}

impl ExportMeter {
    pub fn new(channels: usize, sample_rate: u32) -> Result<Self, String> {
        let ch = channels.clamp(1, 2);
        let mode = Mode::I | Mode::LRA | Mode::TRUE_PEAK | Mode::SAMPLE_PEAK;
        let ebu = EbuR128::new(ch as u32, sample_rate.max(8_000), mode)
            .map_err(|e| format!("Medidor EBU R128: {e:?}"))?;
        Ok(Self { ebu, channels: ch })
    }

    pub fn push_interleaved(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        let _ = self.ebu.add_frames_f32(samples);
    }

    pub fn integrated_lufs(&self) -> Option<f64> {
        self.ebu
            .loudness_global()
            .ok()
            .filter(|v| v.is_finite() && *v > -120.0)
    }

    pub fn true_peak_dbtp(&self) -> Option<f64> {
        let mut peak = 0.0_f64;
        for ch in 0..self.channels {
            if let Ok(v) = self.ebu.true_peak(ch as u32)
                && v.is_finite()
            {
                peak = peak.max(v.abs());
            }
        }
        if peak <= 1e-12 {
            None
        } else {
            Some(20.0 * peak.log10())
        }
    }

    pub fn summary(&self, applied_gain_db: f64, max_gain_reduction_db: f64) -> LoudnessSummary {
        LoudnessSummary {
            integrated_lufs: self.integrated_lufs(),
            true_peak_dbtp: self.true_peak_dbtp(),
            lra_lu: self
                .ebu
                .loudness_range()
                .ok()
                .filter(|v| v.is_finite() && *v >= 0.0),
            applied_gain_db,
            max_gain_reduction_db,
        }
    }
}

/// Calcula la ganancia en dB necesaria para llevar `measured_lufs` al objetivo `target_lufs`,
/// evitando amplificar silencio o ruido térmico por debajo de -70 LUFS.
pub(super) fn normalization_gain_db(measured_lufs: Option<f64>, target_lufs: f64) -> f64 {
    let Some(measured) = measured_lufs else {
        return 0.0;
    };
    if !measured.is_finite() || measured <= -70.0 {
        return 0.0;
    }
    (target_lufs - measured).clamp(-36.0, 36.0)
}

const FIR_TAPS: usize = 12;
const FIR_CENTER: usize = 5;

/// Limitador True Peak 4× con lookahead y compensación exacta de latencia (0 frames perdidos).
pub(super) struct TruePeakLimiter {
    channels: usize,
    ceiling_lin: f32,
    lookahead: usize,
    total_delay: usize,
    /// Coeficientes polifásicos FIR windowed-sinc para las fases 0.25, 0.50 y 0.75.
    phases: [[f32; FIR_TAPS]; 3],
    /// Historial de entrada para el filtro FIR 4× por canal (índice 0 = más antiguo).
    fir_hist: [[f32; FIR_TAPS]; 2],
    /// Línea de retardo de audio alineada al lookahead (`total_delay` frames).
    delay_line: VecDeque<[f32; 2]>,
    /// Pico 4× asociado a cada frame dentro de la ventana de lookahead.
    peak_queue: VecDeque<f32>,
    /// Cola monotónica `(frame_index, peak)` para O(1) amortizado del máximo en ventana.
    mono_max: VecDeque<(u64, f32)>,
    fed_frames: u64,
    gain: f32,
    attack_coeff: f32,
    release_coeff: f32,
    min_gain_applied: f32,
}

impl TruePeakLimiter {
    pub fn new(channels: usize, sample_rate: u32, ceiling_dbtp: f64) -> Self {
        let ch = channels.clamp(1, 2);
        let sr = sample_rate.max(8_000) as f32;
        let lookahead = ((sr * 0.005).round() as usize).clamp(16, 1024);
        let total_delay = FIR_CENTER + lookahead;
        // Margen de 0.08 dB para cubrir la tolerancia entre el FIR 12-tap y el filtro BS.1770-4 de ebur128.
        let ceiling_lin = 10.0_f32.powf(((ceiling_dbtp - 0.08) as f32) / 20.0);
        let phases = [
            sinc_phase_coeffs(0.25),
            sinc_phase_coeffs(0.50),
            sinc_phase_coeffs(0.75),
        ];
        // Ataque sobre ~1/4 del lookahead: al llegar el pico la reducción ya convergió >98%.
        let attack_coeff = (-1.0 / (lookahead as f32 * 0.22).max(1.0)).exp();
        let release_coeff = (-1.0 / (sr * 0.060).max(1.0)).exp();
        Self {
            channels: ch,
            ceiling_lin,
            lookahead,
            total_delay,
            phases,
            fir_hist: [[0.0; FIR_TAPS]; 2],
            delay_line: VecDeque::with_capacity(total_delay + 1),
            peak_queue: VecDeque::with_capacity(lookahead + 1),
            mono_max: VecDeque::with_capacity(lookahead + 1),
            fed_frames: 0,
            gain: 1.0,
            attack_coeff,
            release_coeff,
            min_gain_applied: 1.0,
        }
    }

    /// Procesa muestras intercaladas (`1` o `2` canales), aplicando `pre_gain` y limitación True Peak.
    /// Emite los frames listos (descontando la latencia inicial) en `out`.
    pub fn process_interleaved(&mut self, input: &[f32], pre_gain: f32, out: &mut Vec<f32>) {
        out.clear();
        if self.channels == 1 {
            for &s in input {
                if let Some(frame) = self.push_frame([s * pre_gain, 0.0]) {
                    out.push(frame[0]);
                }
            }
        } else {
            for pair in input.chunks_exact(2) {
                if let Some(frame) = self.push_frame([pair[0] * pre_gain, pair[1] * pre_gain]) {
                    out.push(frame[0]);
                    out.push(frame[1]);
                }
            }
        }
    }

    /// Vacía exactamente los `total_delay` frames restantes conservando la duración exacta al frame.
    pub fn flush(&mut self, out: &mut Vec<f32>) {
        out.clear();
        let remaining = self.total_delay.min(self.fed_frames as usize);
        for _ in 0..remaining {
            if let Some(frame) = self.push_frame([0.0, 0.0]) {
                out.push(frame[0]);
                if self.channels == 2 {
                    out.push(frame[1]);
                }
            }
        }
    }

    pub fn max_gain_reduction_db(&self) -> f64 {
        if self.min_gain_applied >= 0.999_99 {
            0.0
        } else {
            (-20.0 * f64::from(self.min_gain_applied.max(1e-6)).log10()).max(0.0)
        }
    }

    fn push_frame(&mut self, frame: [f32; 2]) -> Option<[f32; 2]> {
        // 1. Actualizar historial FIR y calcular True Peak 4× alineado en FIR_CENTER.
        let mut tp = 0.0_f32;
        for ch in 0..self.channels {
            let h = &mut self.fir_hist[ch];
            h.copy_within(1..FIR_TAPS, 0);
            h[FIR_TAPS - 1] = frame[ch];
            tp = tp.max(h[FIR_CENTER].abs());
            for phase in &self.phases {
                let mut acc = 0.0_f32;
                for i in 0..FIR_TAPS {
                    acc += h[i] * phase[i];
                }
                tp = tp.max(acc.abs());
            }
        }

        // 2. Registrar en ventana deslizante de lookahead.
        let idx = self.fed_frames;
        self.fed_frames += 1;
        self.delay_line.push_back(frame);
        self.peak_queue.push_back(tp);
        while self.mono_max.back().is_some_and(|&(_, p)| p <= tp) {
            self.mono_max.pop_back();
        }
        self.mono_max.push_back((idx, tp));

        if self.peak_queue.len() > self.lookahead {
            let expired_idx = idx.saturating_sub(self.lookahead as u64);
            self.peak_queue.pop_front();
            if self
                .mono_max
                .front()
                .is_some_and(|&(i, _)| i <= expired_idx)
            {
                self.mono_max.pop_front();
            }
        }

        if self.delay_line.len() <= self.total_delay {
            return None;
        }

        // 3. Extraer frame alineado y aplicar envolvente suave + tope estricto de seguridad.
        let delayed = self.delay_line.pop_front()?;
        let window_peak = self.mono_max.front().map_or(0.0, |&(_, p)| p);
        let out_peak = self.peak_queue.front().copied().unwrap_or(0.0);
        let target_gain = if window_peak > self.ceiling_lin {
            self.ceiling_lin / window_peak
        } else {
            1.0
        };
        let coeff = if target_gain < self.gain {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.gain = target_gain + (self.gain - target_gain) * coeff;
        let instant_limit = if out_peak > self.ceiling_lin {
            self.ceiling_lin / out_peak
        } else {
            1.0
        };
        let applied = self.gain.min(instant_limit).min(1.0);
        self.gain = applied;
        self.min_gain_applied = self.min_gain_applied.min(applied);

        // Recorte duro final de seguridad en `ceiling_lin` ante transitorios extremos de un frame.
        let l = (delayed[0] * applied).clamp(-self.ceiling_lin, self.ceiling_lin);
        let r = if self.channels == 2 {
            (delayed[1] * applied).clamp(-self.ceiling_lin, self.ceiling_lin)
        } else {
            0.0
        };
        Some([l, r])
    }
}

fn sinc_phase_coeffs(frac: f32) -> [f32; FIR_TAPS] {
    let mut coeffs = [0.0_f32; FIR_TAPS];
    let mut sum = 0.0_f32;
    let center = FIR_CENTER as f32 + frac;
    for (i, c) in coeffs.iter_mut().enumerate() {
        let x = i as f32 - center;
        let sinc = if x.abs() < 1e-6 {
            1.0
        } else {
            let px = PI * x;
            px.sin() / px
        };
        let hann = 0.5 * (1.0 - (2.0 * PI * (i as f32 + 0.5) / FIR_TAPS as f32).cos());
        *c = sinc * hann;
        sum += *c;
    }
    if sum.abs() > 1e-6 {
        for c in &mut coeffs {
            *c /= sum;
        }
    }
    coeffs
}

/// Archivo temporal raw `f32` para exportación en 2 pasadas (normalización LUFS) con RAM acotada.
pub(super) struct StageFile {
    path: PathBuf,
    writer: Option<BufWriter<File>>,
    total_samples: u64,
}

impl StageFile {
    pub fn create() -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            ".hay-export-stage-{}-{}.f32",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let file = OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("Temporal de normalización: {e}"))?;
        Ok(Self {
            path,
            writer: Some(BufWriter::with_capacity(64 * 1024, file)),
            total_samples: 0,
        })
    }

    pub fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        let writer = self.writer.as_mut().ok_or("StageFile cerrado")?;
        let mut bytes = [0u8; 4096];
        let mut offset = 0;
        while offset < samples.len() {
            let count = (samples.len() - offset).min(bytes.len() / 4);
            for (i, &sample) in samples[offset..offset + count].iter().enumerate() {
                bytes[i * 4..i * 4 + 4].copy_from_slice(&sample.to_ne_bytes());
            }
            writer
                .write_all(&bytes[..count * 4])
                .map_err(|e| format!("Error escribiendo temporal: {e}"))?;
            offset += count;
        }
        self.total_samples += samples.len() as u64;
        Ok(())
    }

    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }

    pub fn rewind_reader(&mut self) -> Result<BufReader<File>, String> {
        let mut writer = self.writer.take().ok_or("StageFile ya consumido")?;
        writer
            .flush()
            .map_err(|e| format!("Error vaciando temporal: {e}"))?;
        let mut file = writer
            .into_inner()
            .map_err(|e| format!("Error liberando temporal: {e}"))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| format!("Error rebobinando temporal: {e}"))?;
        Ok(BufReader::with_capacity(64 * 1024, file))
    }
}

impl Drop for StageFile {
    fn drop(&mut self) {
        self.writer = None;
        let _ = fs::remove_file(&self.path);
    }
}

/// Lee bloques de `f32` desde `reader` hacia `out` (devuelve la cantidad de muestras leídas).
pub(super) fn read_f32_chunk(
    reader: &mut BufReader<File>,
    byte_buf: &mut [u8],
    out: &mut Vec<f32>,
) -> Result<usize, String> {
    out.clear();
    let max_bytes = byte_buf.len() - (byte_buf.len() % 8);
    let mut filled = 0;
    while filled < max_bytes {
        match reader.read(&mut byte_buf[filled..max_bytes]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("Error leyendo temporal: {e}")),
        }
    }
    let valid_bytes = filled - (filled % 4);
    for chunk in byte_buf[..valid_bytes].chunks_exact(4) {
        out.push(f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(out.len())
}
