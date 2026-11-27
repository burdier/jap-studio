//! Importador nativo de presets `.vital` (JSON) y bancos `.vitalbank` (ZIP) hacia `SynthParams`.
//! Headless: sin GPUI ni CPAL; traduce OSC 1/2/3, Noise, Filter 1, ENV 1/2/3, LFO 1/2,
//! Chorus, Distortion, Reverb (Room + Distance), Portamento y Mod Matrix (12 slots).
use super::synth::{
    CUSTOM_WT_FRAMES, CUSTOM_WT_SIZE, CustomWavetable, DistMode, EnvelopeMode, FilterKind,
    LfoShape, MATRIX_SLOTS, ModSlot, ModSource, ModTarget, RoomKind, SynthEngineKind, SynthParams,
    VoiceParams, WarpMode, WavetableKind,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct VitalImportSummary {
    pub name: String,
    pub style: String,
    pub mapped_mods: usize,
    pub osc_a_wt: String,
    pub osc_b_wt: String,
    pub decoded_wts: u8,
}

#[derive(Clone, Debug)]
pub struct VitalPresetEntry {
    pub name: String,
    pub pack: String,
    pub path: PathBuf,
}

pub fn load_vital_file(path: &Path) -> Result<(SynthParams, VitalImportSummary), String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("No se pudo leer {}: {e}", path.display()))?;
    let fallback = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    parse_vital_json(&raw, &fallback)
}

/// Parsea el JSON de un archivo `.vital` real y devuelve un `SynthParams` listo para tocar.
pub fn parse_vital_json(
    raw: &str,
    fallback_name: &str,
) -> Result<(SynthParams, VitalImportSummary), String> {
    let root: Value =
        serde_json::from_str(raw).map_err(|e| format!("Archivo .vital inválido: {e}"))?;
    let s = root
        .get("settings")
        .and_then(Value::as_object)
        .ok_or("El archivo .vital no contiene bloque `settings`")?;

    let get_f = |k: &str, d: f32| -> f32 {
        s.get(k)
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            .filter(|v| v.is_finite())
            .unwrap_or(d)
    };
    let get_on = |k: &str| get_f(k, 0.0) >= 0.5;

    // Pre-evaluar offsets iniciales de `macro_control_1..4` sobre sus destinos reales
    // (muchos bancos ponen `filter_1_cutoff = 8` y abren el filtro con `macro_control_1 = 0.6`).
    let mut macro_cutoff_midi_delta = 0.0_f32;
    let mut macro_osc1_frame_delta = 0.0_f32;
    let mut macro_osc2_frame_delta = 0.0_f32;
    let mut macro_osc1_lvl_delta = 0.0_f32;
    let mut macro_osc2_lvl_delta = 0.0_f32;
    let mut macro_osc3_lvl_delta = 0.0_f32;
    let mut macro_reverb_wet_delta = 0.0_f32;
    let mut macro_chorus_wet_delta = 0.0_f32;
    let mut macro_dist_drive_delta = 0.0_f32;
    if let Some(mods) = s.get("modulations").and_then(Value::as_array) {
        for (idx, m) in mods.iter().enumerate() {
            let src_str = m.get("source").and_then(Value::as_str).unwrap_or("");
            let dst_str = m.get("destination").and_then(Value::as_str).unwrap_or("");
            if !src_str.starts_with("macro_control_") || dst_str.is_empty() {
                continue;
            }
            let macro_val = get_f(src_str, 0.0).clamp(0.0, 1.0);
            if macro_val <= 0.001 {
                continue;
            }
            let raw_amt = get_f(&format!("modulation_{}_amount", idx + 1), 0.0).clamp(-1.0, 1.0);
            let contrib = macro_val * raw_amt;
            match dst_str {
                "filter_1_cutoff" | "filter_2_cutoff" => macro_cutoff_midi_delta += contrib * 128.0,
                "osc_1_wave_frame" => macro_osc1_frame_delta += contrib,
                "osc_2_wave_frame" => macro_osc2_frame_delta += contrib,
                "osc_1_level" => macro_osc1_lvl_delta += contrib,
                "osc_2_level" => macro_osc2_lvl_delta += contrib,
                "osc_3_level" => macro_osc3_lvl_delta += contrib,
                "reverb_dry_wet" => macro_reverb_wet_delta += contrib,
                "chorus_dry_wet" => macro_chorus_wet_delta += contrib,
                "distortion_drive" => macro_dist_drive_delta += contrib * 24.0,
                _ => {}
            }
        }
    }

    let name = root
        .get("preset_name")
        .and_then(Value::as_str)
        .filter(|n| !n.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| fallback_name.to_owned());
    let style = root
        .get("preset_style")
        .and_then(Value::as_str)
        .unwrap_or("Vital")
        .trim()
        .to_owned();

    // Wavetables por nombre + decodificación directa de `wave_data` base64 (f32[2048]) cuando viene embebido.
    let wt_objs = s.get("wavetables").and_then(Value::as_array);
    let wt_names: Vec<String> = wt_objs
        .map(|arr| {
            arr.iter()
                .map(|w| {
                    w.get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("Basic Shapes")
                        .to_owned()
                })
                .collect()
        })
        .unwrap_or_default();
    let wt_a_name = wt_names
        .first()
        .cloned()
        .unwrap_or_else(|| "Basic Shapes".into());
    let wt_b_name = wt_names
        .get(1)
        .cloned()
        .unwrap_or_else(|| "Analog Warm".into());
    let custom_wt_a = wt_objs
        .and_then(|arr| arr.first())
        .and_then(|w| extract_vital_wavetable(w, &wt_a_name));
    let custom_wt_b = wt_objs
        .and_then(|arr| arr.get(1))
        .and_then(|w| extract_vital_wavetable(w, &wt_b_name));
    let decoded_wts = u8::from(custom_wt_a.is_some()) + u8::from(custom_wt_b.is_some());

    let mut voice = VoiceParams::default();

    // ── OSC 1 -> OSC A (con headroom Hi-Fi) ──
    let osc1_on = get_on("osc_1_on");
    voice.osc_a_table = map_wavetable_name(&wt_a_name, &style, false);
    voice.osc_a_level = if osc1_on {
        (get_f("osc_1_level", 0.707) + macro_osc1_lvl_delta).clamp(0.0, 0.85)
    } else {
        0.0
    };
    voice.osc_a_unison = (get_f("osc_1_unison_voices", 1.0).round() as i32).clamp(1, 7) as u8;
    voice.osc_a_detune = (get_f("osc_1_unison_detune", 2.0) / 10.0).clamp(0.0, 0.85);
    voice.osc_a_spread = get_f("osc_1_stereo_spread", 0.8).clamp(0.0, 1.0);
    voice.osc_a_semi = (get_f("osc_1_transpose", 0.0).round() as i32).clamp(-24, 24) as i8;
    let shape = (get_f("osc_1_wave_frame", 64.0) / 256.0 + macro_osc1_frame_delta).clamp(0.0, 1.0);

    // Solo activar Warp cuando `osc_1_distortion_type` está explícitamente activo (evita artefactos en pads suaves).
    let dist_type = get_f("osc_1_distortion_type", 0.0).round() as i32;
    let dist_amt = get_f("osc_1_distortion_amount", 0.5).clamp(0.0, 1.0);
    if dist_type != 0 {
        voice.osc_a_warp_mode = match dist_type {
            1 => WarpMode::Sync,
            2 | 4 | 5 | 6 => WarpMode::Bend,
            3 => WarpMode::Fold,
            7 | 8 => WarpMode::FmFromB,
            9 | 10 => WarpMode::RingMod,
            _ => WarpMode::Bend,
        };
        voice.osc_a_warp = ((dist_amt - 0.5).abs() * 1.4).clamp(0.0, 0.75);
    }

    // ── OSC 2 -> OSC B ──
    let osc2_on = get_on("osc_2_on");
    voice.osc_b_table = map_wavetable_name(&wt_b_name, &style, true);
    voice.osc_b_level = if osc2_on {
        (get_f("osc_2_level", 0.5) + macro_osc2_lvl_delta).clamp(0.0, 0.75)
    } else {
        0.0
    };
    voice.osc_b_pos =
        (get_f("osc_2_wave_frame", 64.0) / 256.0 + macro_osc2_frame_delta).clamp(0.0, 1.0);
    voice.osc_b_semi = (get_f("osc_2_transpose", 0.0).round() as i32).clamp(-24, 24) as i8;
    voice.osc_b_fine = get_f("osc_2_tune", 0.0).clamp(-1.0, 1.0);
    voice.osc_b_unison = (get_f("osc_2_unison_voices", 1.0).round() as i32).clamp(1, 7) as u8;
    voice.osc_b_detune = (get_f("osc_2_unison_detune", 2.0) / 10.0).clamp(0.0, 0.85);

    // ── OSC 3 -> SUB ──
    if get_on("osc_3_on") {
        let osc3_lvl = (get_f("osc_3_level", 0.5) + macro_osc3_lvl_delta).clamp(0.0, 1.0);
        let osc3_semi = get_f("osc_3_transpose", -12.0).round() as i32;
        voice.sub_level = (osc3_lvl * 0.65).clamp(0.0, 0.75);
        voice.sub_octave = if osc3_semi <= -18 { 1 } else { 0 };
        if !osc1_on && !osc2_on {
            voice.osc_a_level = 0.60;
        }
    } else if !osc1_on && !osc2_on {
        voice.osc_a_level = 0.75;
    }

    // ── SAMPLE -> NOISE / AIR ──
    if get_on("sample_on") {
        voice.noise_level = (get_f("sample_level", 0.35) * 0.35).clamp(0.0, 0.55);
        voice.noise_color = 0.58;
    }

    // ── FILTER 1 -> TPT SVF ──
    let f1_on = get_on("filter_1_on");
    let tone = if f1_on {
        let cutoff_midi = (get_f("filter_1_cutoff", 84.0) + macro_cutoff_midi_delta).clamp(24.0, 136.0);
        let hz = 440.0 * 2.0_f32.powf((cutoff_midi - 69.0) / 12.0);
        ((hz / 70.0).max(1.0).ln() / 250.0_f32.ln()).clamp(0.14, 1.0)
    } else {
        0.94
    };
    if f1_on {
        let f_style = get_f("filter_1_style", 0.0).round() as i32;
        let f_blend = get_f("filter_1_blend", 0.0);
        voice.filter_kind = if f_blend > 1.4 {
            FilterKind::Hp12
        } else if f_blend > 0.65 {
            FilterKind::Bp12
        } else if f_style == 1 {
            FilterKind::Lp24
        } else if f_style == 2 {
            FilterKind::Notch
        } else {
            FilterKind::Lp12
        };
        voice.filter_res = get_f("filter_1_resonance", 0.12).clamp(0.0, 0.72);
        voice.filter_drive = (get_f("filter_1_drive", 0.0) / 28.0).clamp(0.0, 0.65);
        voice.filter_keytrack = get_f("filter_1_keytrack", 0.25).clamp(0.0, 1.0);
    }

    // ── ENVELOPES (Vital usa raíz cuarta de segundos: s = v^4) ──
    let vital_time_to_norm = |v: f32, max_s: f32| -> f32 {
        let sec = v.clamp(0.0, 2.5).powi(4);
        (sec / max_s).clamp(0.0, 1.0).sqrt()
    };
    voice.env1_override = true;
    voice.env1_attack = vital_time_to_norm(get_f("env_1_attack", 0.15), 2.4);
    voice.env1_decay = vital_time_to_norm(get_f("env_1_decay", 1.0), 3.5);
    voice.env1_sustain = get_f("env_1_sustain", 0.8).clamp(0.0, 1.0);
    voice.env1_release = vital_time_to_norm(get_f("env_1_release", 0.55), 4.0);

    voice.env2_attack = vital_time_to_norm(get_f("env_2_attack", 0.10), 2.2);
    voice.env2_decay = vital_time_to_norm(get_f("env_2_decay", 0.9), 3.0);
    voice.env2_sustain = get_f("env_2_sustain", 0.0).clamp(0.0, 1.0);
    voice.env2_release = vital_time_to_norm(get_f("env_2_release", 0.5), 3.0);

    voice.env3_decay = vital_time_to_norm(get_f("env_3_decay", 0.5), 0.9);

    // ── LFO 1 & LFO 2 ──
    let map_lfo = |prefix: &str| -> (LfoShape, f32, bool, bool) {
        let sync = get_f(&format!("{prefix}_sync"), 1.0) >= 0.5;
        let retrig = get_f(&format!("{prefix}_sync_type"), 0.0).round() as i32 == 0;
        let rate = if sync {
            // Vital tempo index 0..12 -> normalizado 0..1
            (get_f(&format!("{prefix}_tempo"), 7.0) / 12.0).clamp(0.0, 1.0)
        } else {
            let hz = 2.0_f32.powf(get_f(&format!("{prefix}_frequency"), 0.0));
            ((hz / 0.05).max(1.0).ln() / 320.0_f32.ln()).clamp(0.0, 1.0)
        };
        (LfoShape::Sine, rate, sync, retrig)
    };
    let (s1, r1, sy1, rt1) = map_lfo("lfo_1");
    voice.lfo1_shape = s1;
    voice.lfo1_rate = r1;
    voice.lfo1_sync = sy1;
    voice.lfo1_retrigger = rt1;

    let (s2, r2, sy2, rt2) = map_lfo("lfo_2");
    voice.lfo2_shape = s2;
    voice.lfo2_rate = r2;
    voice.lfo2_sync = sy2;
    voice.lfo2_retrigger = rt2;

    // ── CHORUS ──
    if get_on("chorus_on") {
        voice.chorus_mix = (get_f("chorus_dry_wet", 0.35) + macro_chorus_wet_delta).clamp(0.0, 0.85);
        let ch_hz = 2.0_f32.powf(get_f("chorus_frequency", -2.0));
        voice.chorus_rate = ((ch_hz - 0.15) / 4.2).clamp(0.0, 1.0).sqrt();
    }

    // ── DISTORTION (0 dB = sin drive fantasma) ──
    if get_on("distortion_on") {
        let dt = get_f("distortion_type", 0.0).round() as i32;
        voice.dist_mode = match dt {
            0 | 1 => DistMode::Soft,
            2 => DistMode::Hard,
            3 | 4 => DistMode::Fold,
            5 => DistMode::Bitcrush,
            _ => DistMode::Soft,
        };
        let drv_db = get_f("distortion_drive", 0.0) + macro_dist_drive_delta;
        if drv_db > 0.5 {
            voice.dist_drive = (drv_db / 32.0).clamp(0.0, 0.80);
            voice.dist_mix = get_f("distortion_mix", 0.5).clamp(0.0, 0.85);
        }
    }

    // ── REVERB -> ROOM + DISTANCE ──
    let (room, distance) = if get_on("reverb_on") {
        let wet = (get_f("reverb_dry_wet", 0.25) + macro_reverb_wet_delta).clamp(0.0, 0.85);
        let decay = get_f("reverb_decay_time", 1.5);
        let room = if decay < 0.6 {
            RoomKind::Close
        } else if decay < 1.8 {
            RoomKind::Wood
        } else if decay < 3.5 {
            RoomKind::Hall
        } else {
            RoomKind::Open
        };
        (room, (wet * 0.90).clamp(0.05, 0.85))
    } else {
        (RoomKind::Close, 0.08)
    };

    // ── PORTAMENTO -> GLIDE ──
    let porta = get_f("portamento_time", -10.0);
    let glide = if porta <= -8.5 {
        0.0
    } else {
        ((porta + 8.5) / 10.0).clamp(0.0, 1.0)
    };

    // ── MODULATIONS -> MOD MATRIX (hasta 12 slots) ──
    let mut mapped_mods = 0usize;
    if let Some(mods) = s.get("modulations").and_then(Value::as_array) {
        for (idx, m) in mods.iter().enumerate() {
            if mapped_mods >= MATRIX_SLOTS {
                break;
            }
            let src_str = m.get("source").and_then(Value::as_str).unwrap_or("");
            let dst_str = m.get("destination").and_then(Value::as_str).unwrap_or("");
            if src_str.is_empty() || dst_str.is_empty() {
                continue;
            }
            // Los `macro_control_*` ya fueron horneados en sus destinos base arriba;
            // evitar sobreescribir el Stage XY (`Distance`/`Tone`) con un macro externo.
            if src_str.starts_with("macro_control_") {
                continue;
            }
            let Some(source) = map_vital_source(src_str) else {
                continue;
            };
            let Some(target) = map_vital_target(dst_str) else {
                continue;
            };
            let raw_amt = get_f(&format!("modulation_{}_amount", idx + 1), 0.35).clamp(-1.0, 1.0);
            if raw_amt.abs() < 0.005 {
                continue;
            }
            // Escala calibrada: en `tune` (±1 st) o pitch evitar barridos extremos de ±24 st.
            let scale = match dst_str {
                "osc_1_tune" | "osc_2_tune" => 0.06,
                "osc_1_transpose" | "osc_2_transpose" => 0.25,
                "filter_1_resonance" | "filter_2_resonance" | "distortion_drive" => 0.45,
                _ => 0.72,
            };
            let amt = (raw_amt * scale).clamp(-0.90, 0.90);
            voice.matrix[mapped_mods] = ModSlot {
                source,
                target,
                amount: amt,
            };
            mapped_mods += 1;
        }
    }

    let body = ((f32::from(voice.osc_a_unison) / 7.0) * 0.55
        + voice.osc_a_spread * 0.25
        + voice.chorus_mix * 0.20)
        .clamp(0.20, 0.95);
    let motion = (if mapped_mods > 0 { 0.38 } else { 0.18 } + voice.chorus_mix * 0.25).clamp(0.10, 0.80);

    let mode = if voice.env1_attack > 0.35 {
        EnvelopeMode::Swell
    } else if voice.env1_sustain < 0.15 {
        EnvelopeMode::Hit
    } else {
        EnvelopeMode::Hold
    };

    let active_layers = usize::from(voice.osc_a_level > 0.05)
        + usize::from(voice.osc_b_level > 0.05)
        + usize::from(voice.sub_level > 0.05);
    let gain = if active_layers <= 1 { 0.88 } else { 0.80 };

    let mut params = SynthParams {
        engine: SynthEngineKind::Wavetable,
        room,
        distance,
        shape,
        tone,
        body,
        motion,
        mode,
        time: ((voice.env1_decay + voice.env1_release) * 0.5).clamp(0.1, 0.95),
        touch: 0.62,
        glide,
        gain,
        preset_name: name.clone(),
        voice,
        custom_wt_a,
        custom_wt_b,
    };
    params.clamp_all();

    Ok((
        params,
        VitalImportSummary {
            name,
            style: if style.is_empty() { "Vital".into() } else { style },
            mapped_mods,
            osc_a_wt: wt_a_name,
            osc_b_wt: wt_b_name,
            decoded_wts,
        },
    ))
}

/// Extrae y remuestrea keyframes (`Wave Source`, `Audio File Source` y `Line Source`) de una wavetable.
fn extract_vital_wavetable(wt: &Value, name: &str) -> Option<Arc<CustomWavetable>> {
    let groups = wt.get("groups")?.as_array()?;
    let mut keyframes: Vec<(f32, [f32; CUSTOM_WT_SIZE])> = Vec::new();
    for g in groups {
        let Some(comps) = g.get("components").and_then(Value::as_array) else {
            continue;
        };
        for c in comps {
            // 1. Audio File Source (PCM i16 LE en `audio_file` + `window_size`)
            if let Some(audio_b64) = c.get("audio_file").and_then(Value::as_str) {
                let win_size = c
                    .get("window_size")
                    .and_then(Value::as_f64)
                    .map(|w| (w.round() as usize).clamp(128, 8192))
                    .unwrap_or(2048);
                if let Some(extracted) = decode_audio_file_source(audio_b64, win_size) {
                    keyframes.extend(extracted);
                    continue;
                }
            }

            let Some(kfs) = c.get("keyframes").and_then(Value::as_array) else {
                continue;
            };
            for kf in kfs {
                let pos = kf
                    .get("position")
                    .and_then(Value::as_f64)
                    .map(|p| (p as f32 / 256.0).clamp(0.0, 1.0))
                    .unwrap_or(0.0);

                // 2. Wave Source (`wave_data` base64 f32[2048] o i16[2048])
                if let Some(b64) = kf.get("wave_data").and_then(Value::as_str) {
                    if b64.len() >= 32 {
                        if let Some(frame) = decode_vital_wave_frame(b64) {
                            keyframes.push((pos, frame));
                            continue;
                        }
                    }
                }

                // 3. Line Source (`line.points` [x0, y0, x1, y1, ...])
                if let Some(pts) = kf
                    .get("line")
                    .and_then(|l| l.get("points"))
                    .and_then(Value::as_array)
                {
                    if let Some(frame) = rasterize_line_source(pts) {
                        keyframes.push((pos, frame));
                    }
                }
            }
        }
    }
    if keyframes.is_empty() {
        return None;
    }
    keyframes.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut frames = [[0.0_f32; CUSTOM_WT_SIZE]; CUSTOM_WT_FRAMES];
    for (fi, dst) in frames.iter_mut().enumerate() {
        let target_pos = fi as f32 / (CUSTOM_WT_FRAMES - 1) as f32;
        if keyframes.len() == 1 || target_pos <= keyframes[0].0 {
            *dst = keyframes[0].1;
        } else if target_pos >= keyframes[keyframes.len() - 1].0 {
            *dst = keyframes[keyframes.len() - 1].1;
        } else {
            let mut right_idx = 1;
            while right_idx < keyframes.len() - 1 && keyframes[right_idx].0 < target_pos {
                right_idx += 1;
            }
            let (p0, ref f0) = keyframes[right_idx - 1];
            let (p1, ref f1) = keyframes[right_idx];
            let t = ((target_pos - p0) / (p1 - p0).max(1e-5)).clamp(0.0, 1.0);
            for i in 0..CUSTOM_WT_SIZE {
                dst[i] = f0[i] + (f1[i] - f0[i]) * t;
            }
        }
        // Quitar DC y normalizar suavemente cada frame.
        let mean = dst.iter().sum::<f32>() / CUSTOM_WT_SIZE as f32;
        let mut peak = 1e-5_f32;
        for s in dst.iter_mut() {
            *s -= mean;
            peak = peak.max(s.abs());
        }
        if peak > 0.02 {
            let inv = 0.95 / peak;
            for s in dst.iter_mut() {
                *s = (*s * inv).clamp(-1.0, 1.0);
            }
        }
    }

    Some(Arc::new(CustomWavetable::new(name.to_owned(), frames)))
}

fn decode_audio_file_source(
    b64: &str,
    window_size: usize,
) -> Option<Vec<(f32, [f32; CUSTOM_WT_SIZE])>> {
    let bytes = decode_base64(b64)?;
    let total_samples = bytes.len() / 2;
    if total_samples < 64 {
        return None;
    }
    let win = window_size.min(total_samples).max(64);
    let max_start = total_samples.saturating_sub(win);
    let mut out = Vec::with_capacity(CUSTOM_WT_FRAMES);

    for fi in 0..CUSTOM_WT_FRAMES {
        let pos = fi as f32 / (CUSTOM_WT_FRAMES - 1) as f32;
        let start = ((pos * max_start as f32).round() as usize).min(max_start);
        let mut frame = [0.0_f32; CUSTOM_WT_SIZE];
        let step = win as f32 / CUSTOM_WT_SIZE as f32;
        let avg_n = (step.floor() as usize).max(1);
        for (i, dst) in frame.iter_mut().enumerate() {
            let s0 = start + (i as f32 * step).floor() as usize;
            let mut acc = 0.0_f32;
            let mut cnt = 0usize;
            for k in 0..avg_n {
                let idx = (s0 + k).min(total_samples - 1);
                let o = idx * 2;
                let sample = i16::from_le_bytes([bytes[o], bytes[o + 1]]);
                acc += f32::from(sample) / 32768.0;
                cnt += 1;
            }
            *dst = acc / cnt as f32;
        }
        out.push((pos, frame));
    }
    Some(out)
}

fn rasterize_line_source(pts: &[Value]) -> Option<[f32; CUSTOM_WT_SIZE]> {
    if pts.len() < 4 || pts.len() % 2 != 0 {
        return None;
    }
    let mut coords: Vec<(f32, f32)> = Vec::with_capacity(pts.len() / 2);
    for pair in pts.chunks_exact(2) {
        let x = pair[0].as_f64()? as f32;
        let y = pair[1].as_f64()? as f32;
        coords.push((x.clamp(0.0, 1.0), (1.0 - 2.0 * y).clamp(-1.0, 1.0)));
    }
    if coords.len() < 2 {
        return None;
    }
    let mut frame = [0.0_f32; CUSTOM_WT_SIZE];
    for (i, dst) in frame.iter_mut().enumerate() {
        let x = i as f32 / CUSTOM_WT_SIZE as f32;
        let mut y = coords[0].1;
        for seg in coords.windows(2) {
            let (x0, y0) = seg[0];
            let (x1, y1) = seg[1];
            if x >= x0 && x <= x1 {
                let span = (x1 - x0).max(1e-6);
                let t = ((x - x0) / span).clamp(0.0, 1.0);
                y = y0 + (y1 - y0) * t;
                break;
            } else if x > x1 {
                y = y1;
            }
        }
        *dst = y;
    }
    Some(frame)
}

fn decode_vital_wave_frame(b64: &str) -> Option<[f32; CUSTOM_WT_SIZE]> {
    let bytes = decode_base64(b64)?;
    let mut out = [0.0_f32; CUSTOM_WT_SIZE];
    if bytes.len() == 2048 * 4 {
        // 2048 f32 LE -> promedio 8:1 a 256 muestras (anti-aliasing limpio).
        for (i, dst) in out.iter_mut().enumerate() {
            let mut acc = 0.0_f32;
            let base = i * 8 * 4;
            for k in 0..8 {
                let o = base + k * 4;
                let s = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
                if s.is_finite() {
                    acc += s.clamp(-4.0, 4.0);
                }
            }
            *dst = acc * 0.125;
        }
        Some(out)
    } else if bytes.len() == 2048 * 2 {
        for (i, dst) in out.iter_mut().enumerate() {
            let mut acc = 0.0_f32;
            let base = i * 8 * 2;
            for k in 0..8 {
                let o = base + k * 2;
                let s = i16::from_le_bytes([bytes[o], bytes[o + 1]]);
                acc += f32::from(s) / 32768.0;
            }
            *dst = acc * 0.125;
        }
        Some(out)
    } else {
        None
    }
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity((input.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;
    for b in input.bytes() {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        buf = (buf << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

fn map_wavetable_name(name: &str, style: &str, secondary: bool) -> WavetableKind {
    let low = format!("{} {}", name.to_lowercase(), style.to_lowercase());
    if low.contains("vocal") || low.contains("formant") || low.contains("choir") || low.contains("vox") {
        WavetableKind::VocalFormant
    } else if low.contains("growl") || low.contains("monster") || low.contains("dubstep") || low.contains("reese") || low.contains("dirty") || low.contains("acid") {
        WavetableKind::GrowlBass
    } else if low.contains("glass") || low.contains("crystal") || low.contains("digital") || low.contains("fm") || low.contains("metallic") {
        WavetableKind::DigitalGlass
    } else if low.contains("organ") || low.contains("bell") || low.contains("chime") || low.contains("keys") {
        WavetableKind::OrganBell
    } else if low.contains("pluck") || low.contains("string") || low.contains("harp") || low.contains("guitar") {
        WavetableKind::PluckString
    } else if low.contains("fold") || low.contains("harsh") || low.contains("scream") || low.contains("lead") {
        WavetableKind::HarshFold
    } else if low.contains("analog") || low.contains("warm") || low.contains("classic") || low.contains("pad") {
        WavetableKind::AnalogWarm
    } else if secondary {
        WavetableKind::AnalogWarm
    } else {
        WavetableKind::BasicShapes
    }
}

fn map_vital_source(src: &str) -> Option<ModSource> {
    match src {
        "env_1" => Some(ModSource::Env1),
        "env_2" => Some(ModSource::Env2),
        "env_3" | "env_4" | "env_5" | "env_6" => Some(ModSource::Env3),
        "lfo_1" | "lfo_3" | "lfo_5" => Some(ModSource::Lfo1),
        "lfo_2" | "lfo_4" | "lfo_6" => Some(ModSource::Lfo2),
        "velocity" | "lift" => Some(ModSource::Velocity),
        "note" | "note_in_octave" => Some(ModSource::Keytrack),
        "mod_wheel" | "aftertouch" | "slide" => Some(ModSource::ModWheel),
        "random" | "random_1" | "random_2" | "random_3" | "random_4" => Some(ModSource::Random),
        "macro_control_1" => Some(ModSource::Distance),
        "macro_control_2" => Some(ModSource::Tone),
        "macro_control_3" => Some(ModSource::Shape),
        "macro_control_4" => Some(ModSource::Body),
        _ => None,
    }
}

fn map_vital_target(dst: &str) -> Option<ModTarget> {
    match dst {
        "osc_1_wave_frame" | "osc_1_spectral_morph_amount" => Some(ModTarget::OscAPos),
        "osc_1_distortion_amount" => Some(ModTarget::OscAWarp),
        "osc_1_transpose" | "osc_1_tune" => Some(ModTarget::OscAPitch),
        "osc_1_unison_detune" => Some(ModTarget::OscADetune),
        "osc_1_level" => Some(ModTarget::OscALevel),
        "osc_2_wave_frame" | "osc_2_spectral_morph_amount" | "osc_2_distortion_amount" => {
            Some(ModTarget::OscBPos)
        }
        "osc_2_transpose" | "osc_2_tune" => Some(ModTarget::OscBPitch),
        "osc_2_level" => Some(ModTarget::OscBLevel),
        "osc_3_level" => Some(ModTarget::SubLevel),
        "sample_level" => Some(ModTarget::NoiseLevel),
        "filter_1_cutoff" | "filter_2_cutoff" | "filter_1_mix" => Some(ModTarget::FilterCutoff),
        "filter_1_resonance" | "filter_2_resonance" => Some(ModTarget::FilterRes),
        "filter_1_drive" | "filter_2_drive" => Some(ModTarget::FilterDrive),
        "chorus_dry_wet" => Some(ModTarget::ChorusMix),
        "distortion_drive" | "distortion_mix" => Some(ModTarget::DistDrive),
        "reverb_dry_wet" | "reverb_decay_time" => Some(ModTarget::Distance),
        "lfo_1_frequency" | "lfo_1_tempo" => Some(ModTarget::Lfo1Rate),
        "lfo_2_frequency" | "lfo_2_tempo" => Some(ModTarget::Lfo2Rate),
        "osc_1_pan" | "osc_2_pan" => Some(ModTarget::Pan),
        "volume" => Some(ModTarget::OutGain),
        _ => None,
    }
}

/// Extrae un `.vitalbank` (archivo ZIP estándar) a `~/.local/share/jap-studio/presets/vital/<banco>/`.
pub fn extract_vitalbank(bank_path: &Path) -> Result<Vec<VitalPresetEntry>, String> {
    let base = dirs::data_local_dir()
        .ok_or("No se encontró directorio local de datos")?
        .join("jap-studio")
        .join("presets")
        .join("vital");
    let bank_name = bank_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let dest = base.join(&bank_name);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let status = std::process::Command::new("unzip")
        .arg("-oq")
        .arg(bank_path)
        .arg("-d")
        .arg(&dest)
        .status()
        .map_err(|e| format!("No se pudo ejecutar unzip para .vitalbank: {e}"))?;
    if !status.success() {
        return Err("No se pudo descomprimir el archivo .vitalbank".into());
    }
    let mut found = Vec::new();
    collect_vital_files(&dest, 5, &mut found);
    found.sort_by(|a, b| a.pack.cmp(&b.pack).then_with(|| a.name.cmp(&b.name)));
    Ok(found)
}

/// Descubre automáticamente presets `.vital` en las librerías locales del sistema.
pub fn discover_vital_presets() -> Vec<VitalPresetEntry> {
    let mut roots = Vec::new();
    if let Some(data) = dirs::data_local_dir() {
        roots.push(data.join("jap-studio").join("presets").join("vital"));
        roots.push(data.join("vital"));
    }
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("library"));
        roots.push(home.join("Music"));
    }
    if let Ok(media) = std::fs::read_dir("/run/media") {
        for user_dir in media.flatten() {
            if let Ok(mounts) = std::fs::read_dir(user_dir.path()) {
                for mount in mounts.flatten() {
                    let lib = mount.path().join("library");
                    if lib.is_dir() {
                        roots.push(lib);
                    }
                }
            }
        }
    }
    let mut found = Vec::new();
    let mut seen_names = std::collections::HashSet::new();
    for root in roots {
        if root.is_dir() {
            let mut batch = Vec::new();
            collect_vital_files(&root, 6, &mut batch);
            for item in batch {
                let key = format!("{}::{}", item.pack, item.name);
                if seen_names.insert(key) {
                    found.push(item);
                }
                if found.len() >= 1200 {
                    break;
                }
            }
        }
    }
    found.sort_by(|a, b| a.pack.cmp(&b.pack).then_with(|| a.name.cmp(&b.name)));
    found
}

fn collect_vital_files(dir: &Path, depth: usize, out: &mut Vec<VitalPresetEntry>) {
    if depth == 0 || out.len() >= 1200 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
            if fname.starts_with('.') || fname == "__MACOSX" {
                continue;
            }
        }
        if path.is_dir() {
            collect_vital_files(&path, depth - 1, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let is_vital = ext.eq_ignore_ascii_case("vital");
            let is_fxp = ext.eq_ignore_ascii_case("fxp") && {
                use std::io::Read;
                let mut hdr = [0u8; 64];
                std::fs::File::open(&path)
                    .and_then(|mut f| f.read_exact(&mut hdr))
                    .is_ok_and(|()| super::serum_import::is_serum_fxp_header(&hdr))
            };
            if is_vital || is_fxp {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let pack = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| if is_fxp { "Serum".into() } else { "Librería".into() });
                out.push(VitalPresetEntry { name, pack, path });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_vital_json_structure_and_maps_modulations() {
        let sample = r#"{
            "preset_name": "Test Vital Bass",
            "preset_style": "Bass",
            "settings": {
                "osc_1_on": 1.0,
                "osc_1_level": 0.7,
                "osc_1_unison_voices": 5.0,
                "osc_1_unison_detune": 3.0,
                "osc_1_wave_frame": 128.0,
                "osc_2_on": 1.0,
                "osc_2_level": 0.5,
                "osc_2_transpose": -12.0,
                "filter_1_on": 1.0,
                "filter_1_style": 1.0,
                "filter_1_cutoff": 52.0,
                "filter_1_resonance": 0.45,
                "filter_1_drive": 8.0,
                "distortion_on": 1.0,
                "distortion_type": 2.0,
                "distortion_drive": 12.0,
                "distortion_mix": 0.5,
                "reverb_on": 1.0,
                "reverb_dry_wet": 0.3,
                "reverb_decay_time": 2.2,
                "modulations": [
                    { "source": "lfo_1", "destination": "filter_1_cutoff" },
                    { "source": "env_2", "destination": "osc_1_wave_frame" }
                ],
                "modulation_1_amount": 0.6,
                "modulation_2_amount": -0.4,
                "wavetables": [
                    { "name": "Growl Monster" },
                    { "name": "Basic Shapes" }
                ]
            }
        }"#;
        let (params, summary) = parse_vital_json(sample, "fallback").unwrap();
        assert_eq!(params.preset_name, "Test Vital Bass");
        assert_eq!(params.engine, SynthEngineKind::Wavetable);
        assert_eq!(params.voice.osc_a_table, WavetableKind::GrowlBass);
        assert_eq!(params.voice.osc_a_unison, 5);
        assert_eq!(params.voice.osc_b_semi, -12);
        assert_eq!(params.voice.filter_kind, FilterKind::Lp24);
        assert_eq!(params.voice.dist_mode, DistMode::Hard);
        assert_eq!(params.room, RoomKind::Hall);
        assert_eq!(summary.mapped_mods, 2);
        assert_eq!(params.voice.matrix[0].source, ModSource::Lfo1);
        assert_eq!(params.voice.matrix[0].target, ModTarget::FilterCutoff);
        assert!(params.voice.matrix[0].amount > 0.3 && params.voice.matrix[0].amount < 0.6);
    }

    #[test]
    fn decodes_vital_wave_data_base64_keyframes() {
        // Genera una onda seno de 2048 f32 LE, codifica en base64 y valida la decodificación.
        let mut raw = Vec::with_capacity(2048 * 4);
        for i in 0..2048 {
            let s = ((i as f32 / 2048.0) * std::f32::consts::TAU).sin();
            raw.extend_from_slice(&s.to_le_bytes());
        }
        const ALPHA: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut b64 = String::new();
        for chunk in raw.chunks(3) {
            let b0 = u32::from(chunk[0]);
            let b1 = if chunk.len() > 1 { u32::from(chunk[1]) } else { 0 };
            let b2 = if chunk.len() > 2 { u32::from(chunk[2]) } else { 0 };
            let n = (b0 << 16) | (b1 << 8) | b2;
            b64.push(ALPHA[((n >> 18) & 63) as usize] as char);
            b64.push(ALPHA[((n >> 12) & 63) as usize] as char);
            b64.push(if chunk.len() > 1 { ALPHA[((n >> 6) & 63) as usize] as char } else { '=' });
            b64.push(if chunk.len() > 2 { ALPHA[(n & 63) as usize] as char } else { '=' });
        }
        let sample = format!(
            r#"{{"preset_name":"SineWT","settings":{{"osc_1_on":1.0,"wavetables":[{{"name":"Custom Sine","groups":[{{"components":[{{"type":"Wave Source","keyframes":[{{"position":0,"wave_data":"{b64}"}}]}}]}}]}}]}}}}"#
        );
        let (params, summary) = parse_vital_json(&sample, "fallback").unwrap();
        assert_eq!(summary.decoded_wts, 1);
        let wt = params.custom_wt_a.expect("custom wavetable decoded");
        assert_eq!(wt.name, "Custom Sine");
        // En el cuarto de ciclo (índice 64 de 256) el seno debe estar cerca de +0.95.
        assert!(wt.frames[0][64] > 0.85, "val={}", wt.frames[0][64]);
        assert!(wt.frames[0][192] < -0.85, "val={}", wt.frames[0][192]);
    }
}
