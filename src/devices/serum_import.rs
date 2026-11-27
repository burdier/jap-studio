//! Importador nativo de presets `.fxp` (formato VST2 chunk `CcnK`/`FPCh`/`XfsX`) hacia `SynthParams`.
//! Headless, 0 dependencias externas (incluye inflador Zlib / RFC 1951 DEFLATE en Rust puro) y 0 unsafe.
use super::synth::{
    CUSTOM_WT_FRAMES, CUSTOM_WT_SIZE, CustomWavetable, DistMode, EnvelopeMode, FilterKind,
    LfoShape, ModSlot, ModSource, ModTarget, RoomKind, SynthEngineKind, SynthParams, VoiceParams,
    WavetableKind,
};
use super::vital_import::VitalImportSummary;
use std::path::Path;
use std::sync::Arc;

/// Comprueba rápidamente si los primeros bytes corresponden a un preset `.fxp` de sintetizador wavetable (`CcnK` + `FPCh` + `XfsX`).
pub fn is_serum_fxp_header(bytes: &[u8]) -> bool {
    bytes.len() >= 64
        && &bytes[0..4] == b"CcnK"
        && &bytes[8..12] == b"FPCh"
        && &bytes[16..20] == b"XfsX"
}

pub fn load_serum_fxp_file(path: &Path) -> Result<(SynthParams, VitalImportSummary), String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("No se pudo leer {}: {e}", path.display()))?;
    let fallback = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    parse_serum_fxp_bytes(&bytes, &fallback)
}

/// Parsea un archivo `.fxp` (`XfsX`), descomprime sus bloques Zlib (estado + wavetables `f32[2048]`)
/// y construye un `SynthParams` Hi-Fi listo para tocar.
pub fn parse_serum_fxp_bytes(
    bytes: &[u8],
    fallback_name: &str,
) -> Result<(SynthParams, VitalImportSummary), String> {
    if !is_serum_fxp_header(bytes) {
        return Err("El archivo .fxp no corresponde a un preset compatible (XfsX)".into());
    }
    let hdr_name = read_c_string(&bytes[28..56]);
    let chunk_size = u32::from_be_bytes([bytes[56], bytes[57], bytes[58], bytes[59]]) as usize;
    let chunk_end = (60 + chunk_size).min(bytes.len());
    let chunk = &bytes[60..chunk_end];

    let (part0, consumed0) = inflate_zlib(chunk)
        .map_err(|e| format!("No se pudo descomprimir bloque de estado .fxp: {e}"))?;

    // Stream Zlib #2 opcional: contiene las wavetables custom embebidas en f32[2048] LE.
    let part1 = if consumed0 + 6 < chunk.len() {
        let rest = &chunk[consumed0..];
        let z_off = find_zlib_header(rest);
        z_off
            .and_then(|o| inflate_zlib(&rest[o..]).ok())
            .map(|(buf, _)| buf)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // Metadatos del bloque descomprimido.
    let body_name = read_c_string_at(&part0, 18_802, 46);
    let style_raw = read_c_string_at(&part0, 18_896, 64);
    let wt_a_name = clean_wt_label(&read_wt_slot_name(&part0, 15_368, 512), "Serum WT A");
    let wt_b_name = clean_wt_label(&read_wt_slot_name(&part0, 15_880, 512), "Serum WT B");
    let noise_name = clean_wt_label(&read_wt_slot_name(&part0, 16_392, 512), "");

    let name = if !body_name.is_empty() {
        body_name
    } else if !hdr_name.is_empty() {
        hdr_name
    } else {
        fallback_name.to_owned()
    };
    let style = if !style_raw.is_empty() {
        style_raw
    } else {
        "FXP".to_owned()
    };

    // Decodificar wavetables reales f32[2048] de `part1` (si vienen embebidas).
    let (custom_wt_a, custom_wt_b) = extract_fxp_wavetables(&part1, &wt_a_name, &wt_b_name);
    let decoded_wts = u8::from(custom_wt_a.is_some()) + u8::from(custom_wt_b.is_some());

    // Leer parámetros float32 normalizados del bloque 14400..15360 (240 parámetros).
    let get_p = |idx: usize, def: f32| -> f32 {
        let off = 14_400 + idx * 4;
        if off + 4 <= part0.len() {
            let v = f32::from_le_bytes([
                part0[off],
                part0[off + 1],
                part0[off + 2],
                part0[off + 3],
            ]);
            if v.is_finite() && (-1.0..=2.0).contains(&v) {
                return v;
            }
        }
        def
    };

    let low = format!("{} {} {} {}", name, style, wt_a_name, wt_b_name).to_lowercase();
    let is_bass = low.contains("bass") || low.contains("reese") || low.contains("sub") || low.contains("808");
    let is_pad = low.contains("pad") || low.contains("swell") || low.contains("atmos");
    let is_pluck = low.contains("pluck") || low.contains("bell") || low.contains("arp");

    // Parámetros musicales calibrados con headroom Hi-Fi.
    let p_bright = get_p(63, 0.55).clamp(0.0, 1.0);
    let p_sec_mix = get_p(64, 0.30).clamp(0.0, 1.0);
    let p_pos_a = get_p(71, 0.25).clamp(0.0, 1.0);
    let p_pos_b = get_p(73, 0.35).clamp(0.0, 1.0);
    let p_res_hint = get_p(119, 0.12).clamp(0.0, 0.65);
    let p_dist_hint = get_p(192, 0.18).clamp(0.0, 0.75);
    let p_rev_hint = get_p(169, 0.25).clamp(0.0, 0.75);
    let p_chorus_hint = get_p(152, 0.22).clamp(0.0, 0.70);
    let p_atk_hint = get_p(200, if is_pad { 0.35 } else { 0.03 }).clamp(0.0, 0.85);

    let mut voice = VoiceParams::default();
    voice.osc_a_table = map_fxp_wt_kind(&wt_a_name, &low, false);
    voice.osc_a_level = 0.74;
    voice.osc_a_unison = if is_bass { 5 } else if is_pad { 5 } else { 3 };
    voice.osc_a_detune = if is_bass { 0.26 } else { 0.18 };
    voice.osc_a_spread = if is_bass { 0.78 } else { 0.85 };
    voice.osc_a_semi = if is_bass { -12 } else { 0 };

    voice.osc_b_table = map_fxp_wt_kind(&wt_b_name, &low, true);
    let has_osc_b = custom_wt_b.is_some() || (!wt_b_name.is_empty() && wt_b_name != "Serum WT B");
    voice.osc_b_level = if has_osc_b {
        (0.32 + p_sec_mix * 0.32).clamp(0.25, 0.64)
    } else {
        0.0
    };
    voice.osc_b_pos = p_pos_b;
    voice.osc_b_semi = if is_bass { -12 } else { 0 };
    voice.osc_b_fine = if is_bass { 0.12 } else { 0.06 };
    voice.osc_b_unison = if has_osc_b { 3 } else { 1 };
    voice.osc_b_detune = 0.20;

    // Sub limpio para bajos y noise/aire si el preset trae noise sample.
    voice.sub_level = if is_bass { 0.44 } else { 0.0 };
    voice.sub_octave = 0;
    if !noise_name.is_empty() || low.contains("noise") || low.contains("vinyl") {
        voice.noise_level = if low.contains("noise") || low.contains("vinyl") {
            0.22
        } else {
            0.10
        };
        voice.noise_color = 0.58;
    }

    // Filtro TPT SVF
    voice.filter_kind = if is_bass {
        FilterKind::Lp24
    } else {
        FilterKind::Lp12
    };
    voice.filter_res = (0.10 + p_res_hint * 0.55).clamp(0.08, 0.55);
    voice.filter_drive = (p_dist_hint * 0.42).clamp(0.0, 0.45);
    voice.filter_keytrack = 0.35;

    // Envolvente principal + ENV 2 de movimiento de filtro
    voice.env1_override = true;
    voice.env1_attack = if is_pad {
        (0.32 + p_atk_hint * 0.4).clamp(0.25, 0.75)
    } else if is_pluck {
        0.015
    } else {
        (0.02 + p_atk_hint * 0.25).clamp(0.02, 0.28)
    };
    voice.env1_decay = if is_pluck { 0.36 } else { 0.55 };
    voice.env1_sustain = if is_pluck { 0.08 } else { 0.85 };
    voice.env1_release = if is_pad { 0.62 } else if is_bass { 0.30 } else { 0.40 };

    voice.env2_attack = 0.015;
    voice.env2_decay = if is_pluck { 0.25 } else { 0.48 };
    voice.env2_sustain = if is_bass { 0.25 } else { 0.10 };
    voice.env2_release = 0.35;

    // LFO 1 orgánico para movimiento de wavetable y filtro (característico en Serum)
    voice.lfo1_shape = LfoShape::Sine;
    voice.lfo1_rate = if is_bass { 0.26 } else { 0.22 };
    voice.lfo1_sync = true;
    voice.lfo1_retrigger = true;

    voice.chorus_mix = (p_chorus_hint * 0.55).clamp(0.0, 0.45);
    voice.chorus_rate = 0.24;
    if p_dist_hint > 0.14 {
        voice.dist_mode = DistMode::Soft;
        voice.dist_drive = (p_dist_hint * 0.55).clamp(0.0, 0.50);
        voice.dist_mix = 0.38;
    }

    // Rutas de Mod Matrix para dar vida y movimiento inmediato.
    let mut mapped_mods = 0usize;
    voice.matrix[0] = ModSlot {
        source: ModSource::Lfo1,
        target: ModTarget::OscAPos,
        amount: 0.34,
    };
    mapped_mods += 1;
    voice.matrix[1] = ModSlot {
        source: ModSource::Env2,
        target: ModTarget::FilterCutoff,
        amount: if is_pluck { 0.52 } else { 0.24 },
    };
    mapped_mods += 1;
    if has_osc_b {
        voice.matrix[2] = ModSlot {
            source: ModSource::Lfo1,
            target: ModTarget::OscBPos,
            amount: 0.28,
        };
        mapped_mods += 1;
    }

    let tone = if low.contains("bright") || low.contains("brass") {
        (0.58 + p_bright * 0.28).clamp(0.52, 0.88)
    } else if low.contains("deep") || low.contains("sub") || low.contains("damper") {
        (0.32 + p_bright * 0.24).clamp(0.30, 0.58)
    } else if is_bass {
        (0.40 + p_bright * 0.30).clamp(0.36, 0.74)
    } else {
        (0.48 + p_bright * 0.34).clamp(0.42, 0.86)
    };

    let (room, distance) = if is_pad {
        (RoomKind::Hall, (0.38 + p_rev_hint * 0.32).clamp(0.28, 0.70))
    } else if is_bass {
        (RoomKind::Close, (0.08 + p_rev_hint * 0.18).clamp(0.06, 0.28))
    } else {
        (RoomKind::Wood, (0.14 + p_rev_hint * 0.28).clamp(0.10, 0.48))
    };

    let mut params = SynthParams {
        engine: SynthEngineKind::Wavetable,
        room,
        distance,
        shape: p_pos_a,
        tone,
        body: if is_bass { 0.74 } else { 0.62 },
        motion: 0.34,
        mode: if is_pad {
            EnvelopeMode::Swell
        } else if is_pluck {
            EnvelopeMode::Hit
        } else {
            EnvelopeMode::Hold
        },
        time: if is_pad { 0.68 } else if is_pluck { 0.32 } else { 0.46 },
        touch: 0.60,
        glide: if is_bass { 0.28 } else { 0.0 },
        gain: 0.82,
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
            style,
            mapped_mods,
            osc_a_wt: wt_a_name,
            osc_b_wt: wt_b_name,
            decoded_wts,
        },
    ))
}

fn extract_fxp_wavetables(
    part1: &[u8],
    wt_a_name: &str,
    wt_b_name: &str,
) -> (Option<Arc<CustomWavetable>>, Option<Arc<CustomWavetable>>) {
    const FRAME_BYTES: usize = 2048 * 4;
    let total_frames = part1.len() / FRAME_BYTES;
    if total_frames == 0 {
        return (None, None);
    }

    let build_wt = |slice: &[u8], n_frames: usize, label: &str| -> Option<Arc<CustomWavetable>> {
        if n_frames == 0 || slice.len() < n_frames * FRAME_BYTES {
            return None;
        }
        let mut frames = [[0.0_f32; CUSTOM_WT_SIZE]; CUSTOM_WT_FRAMES];
        for (fi, dst) in frames.iter_mut().enumerate() {
            let src_idx = if n_frames == 1 {
                0
            } else {
                ((fi * (n_frames - 1)) / (CUSTOM_WT_FRAMES - 1)).min(n_frames - 1)
            };
            let base = src_idx * FRAME_BYTES;
            for (i, out_s) in dst.iter_mut().enumerate() {
                let mut acc = 0.0_f32;
                let sb = base + i * 8 * 4;
                for k in 0..8 {
                    let o = sb + k * 4;
                    let s = f32::from_le_bytes([slice[o], slice[o + 1], slice[o + 2], slice[o + 3]]);
                    if s.is_finite() {
                        acc += s.clamp(-4.0, 4.0);
                    }
                }
                *out_s = acc * 0.125;
            }
            // Eliminar DC y normalizar suavemente.
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
        Some(Arc::new(CustomWavetable::new(label.to_owned(), frames)))
    };

    if total_frames > 256 {
        // Dos wavetables custom concatenadas: los primeros 256 frames son OSC A y el resto OSC B.
        let wt_a = build_wt(&part1[..256 * FRAME_BYTES], 256, wt_a_name);
        let b_frames = (total_frames - 256).min(256);
        let wt_b = build_wt(
            &part1[256 * FRAME_BYTES..(256 + b_frames) * FRAME_BYTES],
            b_frames,
            wt_b_name,
        );
        (wt_a, wt_b)
    } else {
        let wt_a = build_wt(&part1[..total_frames * FRAME_BYTES], total_frames, wt_a_name);
        (wt_a, None)
    }
}

fn map_fxp_wt_kind(name: &str, context: &str, secondary: bool) -> WavetableKind {
    let low = format!("{} {}", name.to_lowercase(), context);
    if low.contains("vocal") || low.contains("formant") || low.contains("vox") {
        WavetableKind::VocalFormant
    } else if low.contains("reese") || low.contains("growl") || low.contains("dirty") || low.contains("virus") {
        WavetableKind::GrowlBass
    } else if low.contains("glass") || low.contains("digital") || low.contains("fm") {
        WavetableKind::DigitalGlass
    } else if low.contains("bell") || low.contains("organ") || low.contains("juno") {
        WavetableKind::OrganBell
    } else if low.contains("pluck") || low.contains("string") {
        WavetableKind::PluckString
    } else if low.contains("fold") || low.contains("harsh") {
        WavetableKind::HarshFold
    } else if low.contains("analog") || low.contains("saw") || low.contains("moog") || secondary {
        WavetableKind::AnalogWarm
    } else {
        WavetableKind::BasicShapes
    }
}

fn read_c_string(slice: &[u8]) -> String {
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).trim().to_owned()
}

fn read_c_string_at(buf: &[u8], off: usize, max_len: usize) -> String {
    if off >= buf.len() {
        return String::new();
    }
    let end = (off + max_len).min(buf.len());
    read_c_string(&buf[off..end])
}

/// En los slots de 512 bytes de Serum suele haber `carpeta\0nombre.wav\0` o `nombre.wav\0`.
fn read_wt_slot_name(buf: &[u8], off: usize, len: usize) -> String {
    if off >= buf.len() {
        return String::new();
    }
    let slice = &buf[off..(off + len).min(buf.len())];
    let mut best = String::new();
    let mut cur = String::new();
    for &b in slice {
        if (0x20..=0x7e).contains(&b) {
            cur.push(b as char);
        } else {
            if cur.len() >= 3 && (cur.contains('.') || best.is_empty() || cur.len() > best.len()) {
                best = cur.clone();
            }
            cur.clear();
        }
    }
    if cur.len() >= 3 && (cur.contains('.') || best.is_empty()) {
        best = cur;
    }
    best.trim().to_owned()
}

fn clean_wt_label(raw: &str, fallback: &str) -> String {
    let trimmed = raw
        .trim()
        .strip_suffix(".wav")
        .or_else(|| raw.trim().strip_suffix(".WAV"))
        .unwrap_or(raw.trim());
    if trimmed.len() >= 2 {
        trimmed.to_owned()
    } else {
        fallback.to_owned()
    }
}

fn find_zlib_header(data: &[u8]) -> Option<usize> {
    let limit = data.len().min(64);
    for i in 0..limit.saturating_sub(1) {
        let cmf = data[i];
        let flg = data[i + 1];
        if (cmf & 0x0f) == 8 && (cmf >> 4) <= 7 && ((u16::from(cmf) * 256 + u16::from(flg)) % 31) == 0 {
            return Some(i);
        }
    }
    None
}

// ──────────────────────────────────────────────────────────────────────────────
// Inflador Zlib / RFC 1951 DEFLATE puro (0 dependencias externas, 0 unsafe)
// ──────────────────────────────────────────────────────────────────────────────

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_buf: u64,
    bit_cnt: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_buf: 0,
            bit_cnt: 0,
        }
    }

    #[inline]
    fn fill(&mut self, need: u32) {
        while self.bit_cnt < need && self.byte_pos < self.data.len() {
            self.bit_buf |= u64::from(self.data[self.byte_pos]) << self.bit_cnt;
            self.byte_pos += 1;
            self.bit_cnt += 8;
        }
    }

    #[inline]
    fn read_bits(&mut self, n: u32) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        self.fill(n);
        if self.bit_cnt < n {
            return Err("Fin inesperado en flujo DEFLATE".into());
        }
        let val = (self.bit_buf & ((1u64 << n) - 1)) as u32;
        self.bit_buf >>= n;
        self.bit_cnt -= n;
        Ok(val)
    }

    fn align_byte(&mut self) {
        let drop = self.bit_cnt & 7;
        self.bit_buf >>= drop;
        self.bit_cnt -= drop;
    }

    fn bytes_consumed(&self) -> usize {
        self.byte_pos.saturating_sub((self.bit_cnt / 8) as usize)
    }
}

struct HuffTable {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl HuffTable {
    fn build(lengths: &[u8]) -> Result<Self, String> {
        let mut counts = [0u16; 16];
        for &len in lengths {
            if len > 15 {
                return Err("Longitud Huffman inválida".into());
            }
            if len > 0 {
                counts[usize::from(len)] += 1;
            }
        }
        let mut offsets = [0u16; 16];
        let mut total = 0u16;
        for bits in 1..=15 {
            offsets[bits] = total;
            total += counts[bits];
        }
        let mut symbols = vec![0u16; usize::from(total)];
        for (sym, &len) in lengths.iter().enumerate() {
            if len > 0 {
                let idx = usize::from(offsets[usize::from(len)]);
                offsets[usize::from(len)] += 1;
                symbols[idx] = sym as u16;
            }
        }
        Ok(Self { counts, symbols })
    }

    #[inline]
    fn decode(&self, br: &mut BitReader<'_>) -> Result<u16, String> {
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;
        for len in 1..=15 {
            code |= br.read_bits(1)? as i32;
            let count = i32::from(self.counts[len]);
            if code - count < first {
                return Ok(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err("Símbolo Huffman inválido".into())
    }
}

pub(crate) fn inflate_zlib(input: &[u8]) -> Result<(Vec<u8>, usize), String> {
    if input.len() < 6 {
        return Err("Flujo Zlib demasiado corto".into());
    }
    let cmf = input[0];
    let flg = input[1];
    if (cmf & 0x0f) != 8 || ((u16::from(cmf) * 256 + u16::from(flg)) % 31) != 0 {
        return Err("Cabecera Zlib inválida".into());
    }
    let mut start = 2usize;
    if (flg & 0x20) != 0 {
        start += 4;
    }
    if start >= input.len() {
        return Err("Cabecera Zlib truncada".into());
    }

    let mut br = BitReader::new(&input[start..]);
    let mut out = Vec::with_capacity(65_536);

    const LEN_BASE: [usize; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LEN_EXTRA: [u32; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DIST_BASE: [usize; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [u32; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    const CL_ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];

    loop {
        let bfinal = br.read_bits(1)?;
        let btype = br.read_bits(2)?;
        match btype {
            0 => {
                br.align_byte();
                let len = br.read_bits(16)? as usize;
                let nlen = br.read_bits(16)? as usize;
                if (len ^ 0xffff) != nlen {
                    return Err("Bloque DEFLATE sin compresión corrupto".into());
                }
                for _ in 0..len {
                    out.push(br.read_bits(8)? as u8);
                }
            }
            1 | 2 => {
                let (lit_table, dist_table) = if btype == 1 {
                    let mut lit_lens = [0u8; 288];
                    lit_lens[0..144].fill(8);
                    lit_lens[144..256].fill(9);
                    lit_lens[256..280].fill(7);
                    lit_lens[280..288].fill(8);
                    let dist_lens = [5u8; 32];
                    (HuffTable::build(&lit_lens)?, HuffTable::build(&dist_lens)?)
                } else {
                    let hlit = br.read_bits(5)? as usize + 257;
                    let hdist = br.read_bits(5)? as usize + 1;
                    let hclen = br.read_bits(4)? as usize + 4;
                    let mut cl_lens = [0u8; 19];
                    for i in 0..hclen {
                        cl_lens[CL_ORDER[i]] = br.read_bits(3)? as u8;
                    }
                    let cl_table = HuffTable::build(&cl_lens)?;
                    let total = hlit + hdist;
                    let mut lengths = Vec::with_capacity(total);
                    while lengths.len() < total {
                        let sym = cl_table.decode(&mut br)?;
                        match sym {
                            0..=15 => lengths.push(sym as u8),
                            16 => {
                                let prev = *lengths
                                    .last()
                                    .ok_or("Código Huffman 16 sin longitud previa")?;
                                let rep = br.read_bits(2)? as usize + 3;
                                lengths.resize((lengths.len() + rep).min(total), prev);
                            }
                            17 => {
                                let rep = br.read_bits(3)? as usize + 3;
                                lengths.resize((lengths.len() + rep).min(total), 0);
                            }
                            18 => {
                                let rep = br.read_bits(7)? as usize + 11;
                                lengths.resize((lengths.len() + rep).min(total), 0);
                            }
                            _ => return Err("Código de longitud Huffman inválido".into()),
                        }
                    }
                    (
                        HuffTable::build(&lengths[..hlit])?,
                        HuffTable::build(&lengths[hlit..hlit + hdist])?,
                    )
                };

                loop {
                    let sym = lit_table.decode(&mut br)? as usize;
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else if sym <= 285 {
                        let li = sym - 257;
                        let length = LEN_BASE[li] + br.read_bits(LEN_EXTRA[li])? as usize;
                        let di = dist_table.decode(&mut br)? as usize;
                        if di >= 30 {
                            return Err("Distancia DEFLATE inválida".into());
                        }
                        let dist = DIST_BASE[di] + br.read_bits(DIST_EXTRA[di])? as usize;
                        if dist == 0 || dist > out.len() {
                            return Err("Distancia LZ77 fuera de rango".into());
                        }
                        out.reserve(length);
                        for _ in 0..length {
                            let b = out[out.len() - dist];
                            out.push(b);
                        }
                    } else {
                        return Err("Símbolo literal DEFLATE inválido".into());
                    }
                    // Límite de seguridad (32 MB) contra archivos malformados.
                    if out.len() > 32 * 1024 * 1024 {
                        return Err("Preset .fxp excede límite de memoria (32 MB)".into());
                    }
                }
            }
            _ => return Err("Tipo de bloque DEFLATE reservado".into()),
        }
        if bfinal != 0 {
            break;
        }
    }

    br.align_byte();
    let total_consumed = (start + br.bytes_consumed() + 4).min(input.len());
    Ok((out, total_consumed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_esw_serum_fxp_pack_if_present() {
        let dir = Path::new(
            "/home/burdier/Music/Echo+Sound+Works+-+Reese+Basses/Echo Sound Works - Reese Basses/Serum/Echo Sound Works - Reeses",
        );
        if !dir.is_dir() {
            return;
        }
        let mut count = 0usize;
        let mut with_wt = 0usize;
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("fxp") {
                continue;
            }
            let (params, summary) = load_serum_fxp_file(&p)
                .unwrap_or_else(|e| panic!("failed to parse {}: {e}", p.display()));
            assert!(!params.preset_name.is_empty());
            if summary.decoded_wts > 0 {
                with_wt += 1;
            }
            count += 1;
        }
        assert!(count >= 15, "expected >= 15 .fxp files, got {count}");
        assert!(with_wt >= 10, "expected >= 10 .fxp with decoded wavetables, got {with_wt}");
    }
}
