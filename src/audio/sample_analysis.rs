//! Análisis acústico offline de samples (transiente, brillo espectral, bandas,
//! envolvente y tono fundamental) para el Explorer y herramientas MCP.
//!
//! No importa GPUI ni corre en el hilo de audio RT. Cachea los resultados en memoria
//! por `(ruta, tamaño, mtime)` para evitar decodificar dos veces el mismo archivo.
use crate::audio::{AudioFile, fundamental};
use elevate::StereoFrame;
use realfft::RealFftPlanner;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::UNIX_EPOCH,
};

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

const MAX_ANALYZED_SECONDS: usize = 8;
const CACHE_CAPACITY: usize = 512;

#[derive(Clone, Debug)]
pub(crate) struct SpectralBands {
    pub sub_pct: f32,
    pub low_pct: f32,
    pub mid_pct: f32,
    pub high_pct: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct SampleProfile {
    pub duration_seconds: f64,
    pub sample_rate: u32,
    pub peak_dbfs: f32,
    pub rms_dbfs: f32,
    pub crest_db: f32,
    pub stereo_width: f32,
    pub attack_ms: f32,
    pub decay_ms: f32,
    pub transient_strength: f32,
    pub transient_label: &'static str,
    pub shape: &'static str,
    pub spectral_centroid_hz: f32,
    pub brightness_label: &'static str,
    pub bands: SpectralBands,
    pub tonal: bool,
    pub pitch_midi: Option<u8>,
    pub pitch_note: Option<String>,
    pub pitch_cents: Option<i32>,
    pub role_hint: &'static str,
    pub summary: String,
}

type CacheKey = (PathBuf, u64, u64);

fn profile_cache() -> &'static Mutex<HashMap<CacheKey, SampleProfile>> {
    static CACHE: OnceLock<Mutex<HashMap<CacheKey, SampleProfile>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(path: &Path) -> Option<CacheKey> {
    let meta = fs::metadata(path).ok()?;
    let len = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Some((canonical, len, mtime))
}

/// Decodifica y analiza un archivo de audio con caché en memoria.
/// Llamar siempre fuera del hilo RT y preferiblemente fuera del hilo principal de UI.
pub(crate) fn analyze_path(path: &Path) -> Result<SampleProfile, String> {
    let key = cache_key(path);
    if let Some(ref k) = key
        && let Ok(guard) = profile_cache().lock()
        && let Some(cached) = guard.get(k)
    {
        return Ok(cached.clone());
    }

    let file = AudioFile::decode(path)?;
    let profile = analyze_frames(&file.frames, file.sample_rate);

    if let Some(k) = key
        && let Ok(mut guard) = profile_cache().lock()
    {
        if guard.len() >= CACHE_CAPACITY {
            guard.clear();
        }
        guard.insert(k, profile.clone());
    }
    Ok(profile)
}

/// Analiza directamente un buffer estéreo en memoria.
pub(crate) fn analyze_frames(frames: &[StereoFrame], sample_rate: u32) -> SampleProfile {
    let sr = sample_rate.max(1) as f64;
    let total_len = frames.len();
    let duration_seconds = total_len as f64 / sr;

    let max_frames = (sample_rate as usize)
        .saturating_mul(MAX_ANALYZED_SECONDS)
        .max(1);
    let slice = &frames[..total_len.min(max_frames)];
    let n = slice.len().max(1);

    let mut mono = Vec::with_capacity(n);
    let mut peak = 0.0_f32;
    let mut sum_sq = 0.0_f64;
    let mut mid_sq = 0.0_f64;
    let mut side_sq = 0.0_f64;

    for &[l, r] in slice {
        let m = 0.5 * (l + r);
        let s = 0.5 * (l - r);
        mono.push(m);
        let p = l.abs().max(r.abs());
        if p > peak {
            peak = p;
        }
        sum_sq += f64::from(m) * f64::from(m);
        mid_sq += f64::from(m) * f64::from(m);
        side_sq += f64::from(s) * f64::from(s);
    }

    let rms = (sum_sq / n as f64).sqrt() as f32;
    let peak_dbfs = lin_to_db(peak);
    let rms_dbfs = lin_to_db(rms);
    let crest_db = (peak_dbfs - rms_dbfs).clamp(0.0, 40.0);
    let stereo_width = if mid_sq + side_sq > 1e-12 {
        (side_sq / (mid_sq + side_sq)).sqrt().clamp(0.0, 1.0) as f32
    } else {
        0.0
    };

    let (attack_ms, decay_ms, onset_ratio) = measure_envelope(&mono, sample_rate);

    // Fuerza de transiente (0.0 = muy suave/pad, 1.0 = muy afilado/punchy).
    let attack_score = (1.0 - (attack_ms / 25.0).clamp(0.0, 1.0)).powf(1.2);
    let crest_score = ((crest_db - 6.0) / 14.0).clamp(0.0, 1.0);
    let ratio_score = ((onset_ratio - 0.8) / 2.8).clamp(0.0, 1.0);
    let transient_strength =
        (0.45 * attack_score + 0.30 * crest_score + 0.25 * ratio_score).clamp(0.0, 1.0);
    let transient_label = if transient_strength < 0.30 {
        "débil/suave"
    } else if transient_strength < 0.56 {
        "moderado"
    } else if transient_strength < 0.78 {
        "fuerte/punchy"
    } else {
        "muy afilado/agresivo"
    };

    let shape = if duration_seconds >= 2.2 {
        "loop / textura"
    } else if decay_ms <= 260.0 && duration_seconds < 1.2 {
        "one-shot corto"
    } else if decay_ms <= 750.0 {
        "one-shot medio"
    } else {
        "one-shot largo / cola"
    };

    let (spectral_centroid_hz, bands) = measure_spectrum(&mono, sample_rate);
    let brightness_label = if spectral_centroid_hz < 600.0 {
        "muy oscuro/sub"
    } else if spectral_centroid_hz < 1600.0 {
        "oscuro/cálido"
    } else if spectral_centroid_hz < 3400.0 {
        "equilibrado"
    } else if spectral_centroid_hz < 6000.0 {
        "brillante"
    } else {
        "muy brillante/filoso"
    };

    let pitch_window = &slice[..slice.len().min((sample_rate as usize) * 3)];
    let root = fundamental::stable_root(pitch_window, sample_rate);
    let tonal = root.is_some();
    let (pitch_midi, pitch_note, pitch_cents) = match root {
        Some((midi, cents)) => (Some(midi), Some(midi_note_name(midi)), Some(cents)),
        None => (None, None, None),
    };

    let role_hint = classify_role(
        duration_seconds,
        decay_ms,
        transient_strength,
        spectral_centroid_hz,
        &bands,
        tonal,
        pitch_midi,
    );

    let pitch_part = match (&pitch_note, pitch_cents) {
        (Some(name), Some(c)) if c.abs() >= 5 => format!(" · tono {name} ({c:+}c)"),
        (Some(name), _) => format!(" · tono {name}"),
        _ => String::new(),
    };
    let summary = format!(
        "{shape} ({duration_seconds:.2}s) · transiente {transient_label} (ataque {attack_ms:.1} ms, crest {crest_db:.1} dB) · {brightness_label} (centroide {spectral_centroid_hz:.0} Hz; sub {:.0}% / graves {:.0}% / medios {:.0}% / agudos {:.0}%) · decay {decay_ms:.0} ms{pitch_part} · probable {role_hint}",
        bands.sub_pct, bands.low_pct, bands.mid_pct, bands.high_pct,
    );

    SampleProfile {
        duration_seconds: round2(duration_seconds),
        sample_rate,
        peak_dbfs: round1(peak_dbfs),
        rms_dbfs: round1(rms_dbfs),
        crest_db: round1(crest_db),
        stereo_width: round2_f32(stereo_width),
        attack_ms: round1(attack_ms),
        decay_ms: round1(decay_ms),
        transient_strength: round2_f32(transient_strength),
        transient_label,
        shape,
        spectral_centroid_hz: spectral_centroid_hz.round(),
        brightness_label,
        bands: SpectralBands {
            sub_pct: round1(bands.sub_pct),
            low_pct: round1(bands.low_pct),
            mid_pct: round1(bands.mid_pct),
            high_pct: round1(bands.high_pct),
        },
        tonal,
        pitch_midi,
        pitch_note,
        pitch_cents,
        role_hint,
        summary,
    }
}

/// Mide ataque (10%->90% del pico), decay (pico->10%) y ratio de energía del impacto inicial.
fn measure_envelope(mono: &[f32], sample_rate: u32) -> (f32, f32, f32) {
    if mono.is_empty() || sample_rate == 0 {
        return (0.0, 0.0, 1.0);
    }
    let hop = ((sample_rate as usize) / 1000).max(1); // ~1 ms por paso de envolvente
    let win = (hop * 2).max(1);
    let mut env = Vec::with_capacity(mono.len() / hop + 1);
    let mut idx = 0;
    while idx < mono.len() {
        let end = (idx + win).min(mono.len());
        let mut e = 0.0_f32;
        for &s in &mono[idx..end] {
            e += s * s;
        }
        env.push((e / (end - idx) as f32).sqrt());
        idx += hop;
    }
    let Some((peak_idx, &peak_val)) = env
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
    else {
        return (0.0, 0.0, 1.0);
    };
    if peak_val <= 1e-6 {
        return (0.0, 0.0, 1.0);
    }

    let lo_thr = peak_val * 0.10;
    let hi_thr = peak_val * 0.90;

    // Buscar el inicio del ataque caminando hacia atrás desde el pico (o primer cruce sobre 10%).
    let mut start_idx = 0usize;
    for i in (0..=peak_idx).rev() {
        if env[i] < lo_thr {
            start_idx = i;
            break;
        }
    }
    let reached_hi = (start_idx..=peak_idx)
        .find(|&i| env[i] >= hi_thr)
        .unwrap_or(peak_idx);
    let ms_per_step = hop as f32 * 1000.0 / sample_rate as f32;
    let attack_ms = ((reached_hi.saturating_sub(start_idx)) as f32 * ms_per_step).max(0.5);

    // Decay: último punto después del pico que permanece sobre el 10% (-20 dB) del pico.
    let mut tail_idx = peak_idx;
    for (offset, &v) in env[peak_idx..].iter().enumerate() {
        if v >= lo_thr {
            tail_idx = peak_idx + offset;
        }
    }
    let decay_ms = ((tail_idx.saturating_sub(peak_idx) + 1) as f32 * ms_per_step).max(1.0);

    // Ratio de impacto: energía media en los primeros 15 ms alrededor del pico vs siguientes 65 ms.
    let head_end = (peak_idx + 15).min(env.len());
    let body_end = (head_end + 65).min(env.len());
    let head_avg = mean_slice(&env[peak_idx..head_end]);
    let body_avg = if body_end > head_end {
        mean_slice(&env[head_end..body_end])
    } else {
        head_avg * 0.25
    };
    let onset_ratio = (head_avg / body_avg.max(peak_val * 0.05)).clamp(0.2, 10.0);

    (attack_ms, decay_ms, onset_ratio)
}

fn measure_spectrum(mono: &[f32], sample_rate: u32) -> (f32, SpectralBands) {
    let fft_size = 2048usize;
    if mono.is_empty() || sample_rate == 0 {
        return (
            0.0,
            SpectralBands {
                sub_pct: 0.0,
                low_pct: 0.0,
                mid_pct: 0.0,
                high_pct: 0.0,
            },
        );
    }

    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(fft_size);
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();

    // Ventana de Hann precalculada.
    let hann: Vec<f32> = (0..fft_size)
        .map(|i| {
            let x = std::f32::consts::TAU * i as f32 / (fft_size - 1) as f32;
            0.5 * (1.0 - x.cos())
        })
        .collect();

    let mut acc_power = vec![0.0_f64; output.len()];
    let hop = fft_size / 2;
    let max_windows = 32usize;
    let total_windows = ((mono.len().saturating_sub(fft_size)) / hop + 1).max(1);
    let stride = (total_windows / max_windows).max(1);

    let mut win_idx = 0usize;
    let mut offset = 0usize;
    loop {
        if win_idx % stride == 0 {
            for i in 0..fft_size {
                let s = mono.get(offset + i).copied().unwrap_or(0.0);
                input[i] = s * hann[i];
            }
            if fft.process(&mut input, &mut output).is_ok() {
                for (acc, bin) in acc_power.iter_mut().zip(&output) {
                    *acc += f64::from(bin.norm_sqr());
                }
            }
        }
        win_idx += 1;
        offset += hop;
        if offset + fft_size / 4 >= mono.len() || win_idx >= total_windows {
            break;
        }
    }

    let bin_hz = sample_rate as f64 / fft_size as f64;
    let mut total_energy = 0.0_f64;
    let mut weighted_freq = 0.0_f64;
    let mut sub = 0.0_f64;
    let mut low = 0.0_f64;
    let mut mid = 0.0_f64;
    let mut high = 0.0_f64;

    for (k, &power) in acc_power.iter().enumerate().skip(1) {
        let hz = k as f64 * bin_hz;
        if hz < 20.0 || hz > 20_000.0 {
            continue;
        }
        let mag = power.sqrt();
        total_energy += power;
        weighted_freq += hz * mag;
        if hz < 100.0 {
            sub += power;
        } else if hz < 400.0 {
            low += power;
        } else if hz < 3500.0 {
            mid += power;
        } else {
            high += power;
        }
    }

    let mag_sum: f64 = acc_power
        .iter()
        .enumerate()
        .skip(1)
        .filter(|&(k, _)| {
            let hz = k as f64 * bin_hz;
            (20.0..=20_000.0).contains(&hz)
        })
        .map(|(_, &p)| p.sqrt())
        .sum();

    let centroid = if mag_sum > 1e-12 {
        (weighted_freq / mag_sum) as f32
    } else {
        0.0
    };

    let norm = total_energy.max(1e-18);
    let bands = SpectralBands {
        sub_pct: (sub * 100.0 / norm) as f32,
        low_pct: (low * 100.0 / norm) as f32,
        mid_pct: (mid * 100.0 / norm) as f32,
        high_pct: (high * 100.0 / norm) as f32,
    };
    (centroid, bands)
}

fn classify_role(
    duration_s: f64,
    decay_ms: f32,
    transient_strength: f32,
    centroid_hz: f32,
    bands: &SpectralBands,
    tonal: bool,
    pitch_midi: Option<u8>,
) -> &'static str {
    if duration_s >= 2.2 {
        return if tonal {
            "loop melódico / pad"
        } else {
            "loop / textura"
        };
    }
    if bands.sub_pct + bands.low_pct >= 62.0 {
        if decay_ms >= 420.0 && tonal && pitch_midi.is_some_and(|m| m <= 55) {
            return "808 / bass";
        }
        return "kick / sub";
    }
    if bands.high_pct >= 45.0 || centroid_hz >= 4500.0 {
        return if decay_ms <= 260.0 {
            "hi-hat cerrado / shaker"
        } else {
            "hi-hat abierto / cymbal"
        };
    }
    if tonal && decay_ms >= 220.0 && transient_strength < 0.72 {
        return "nota melódica / acorde";
    }
    if transient_strength >= 0.48 && bands.mid_pct >= 30.0 {
        return if bands.high_pct >= 18.0 || decay_ms >= 140.0 {
            "snare / clap"
        } else {
            "perc / rim / tom"
        };
    }
    if tonal {
        "instrumento / tonal"
    } else {
        "percusión / fx"
    }
}

fn midi_note_name(midi: u8) -> String {
    let note = NOTE_NAMES[(midi % 12) as usize];
    let octave = (midi as i32 / 12) - 1;
    format!("{note}{octave}")
}

fn mean_slice(xs: &[f32]) -> f32 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f32>() / xs.len() as f32
    }
}

fn lin_to_db(v: f32) -> f32 {
    if v <= 1e-6 {
        -120.0
    } else {
        20.0 * v.log10()
    }
}

fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

fn round2_f32(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_sub_kick_is_detected_as_dark_with_weak_transient() {
        let sr = 48_000u32;
        let len = (sr as f32 * 0.40) as usize;
        let mut frames = Vec::with_capacity(len);
        for i in 0..len {
            let t = i as f32 / sr as f32;
            // Ataque lento de 40 ms + caída suave, tono subgrave de 65 Hz.
            let env = (t / 0.040).min(1.0) * (-t * 6.0).exp();
            let s = (std::f32::consts::TAU * 65.0 * t).sin() * env * 0.8;
            frames.push([s, s]);
        }
        let p = analyze_frames(&frames, sr);
        assert!(p.bands.sub_pct > 60.0, "expected sub energy, got {:?}", p.bands);
        assert!(p.attack_ms >= 12.0, "expected slow attack, got {}", p.attack_ms);
        assert_eq!(p.brightness_label, "muy oscuro/sub");
        assert!(
            p.transient_strength < 0.55,
            "expected weak/soft transient, got {} ({})",
            p.transient_strength,
            p.transient_label
        );
    }

    #[test]
    fn bright_sharp_hat_is_detected_as_bright_and_punchy() {
        let sr = 48_000u32;
        let len = (sr as f32 * 0.16) as usize;
        let mut frames = Vec::with_capacity(len);
        // Contenido agudo (>7 kHz) con impacto inmediato en sample 0 y decay corto.
        for i in 0..len {
            let t = i as f32 / sr as f32;
            let env = (-t * 45.0).exp();
            let s = ((std::f32::consts::TAU * 7_800.0 * t).sin()
                + 0.7 * (std::f32::consts::TAU * 10_400.0 * t).sin())
                * 0.45
                * env;
            frames.push([s, s]);
        }
        let p = analyze_frames(&frames, sr);
        assert!(p.bands.high_pct > 75.0, "expected high energy, got {:?}", p.bands);
        assert!(
            p.spectral_centroid_hz > 6000.0,
            "expected bright centroid, got {}",
            p.spectral_centroid_hz
        );
        assert!(
            p.transient_strength >= 0.60,
            "expected punchy transient, got {} ({})",
            p.transient_strength,
            p.transient_label
        );
        assert_eq!(p.role_hint, "hi-hat cerrado / shaker");
    }

    #[test]
    fn tonal_a4_sine_detects_pitch_a4() {
        let sr = 48_000u32;
        let len = (sr as f32 * 0.60) as usize;
        let mut frames = Vec::with_capacity(len);
        for i in 0..len {
            let t = i as f32 / sr as f32;
            let s = (std::f32::consts::TAU * 440.0 * t).sin() * 0.5;
            frames.push([s, s]);
        }
        let p = analyze_frames(&frames, sr);
        assert!(p.tonal, "expected tonal signal");
        assert_eq!(p.pitch_note.as_deref(), Some("A4"));
        assert_eq!(p.pitch_midi, Some(69));
    }
}
