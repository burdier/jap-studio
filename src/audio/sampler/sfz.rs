//! Traducción del contrato musical a SFZ; el usuario no edita archivos intermedios.
use super::SampleAsset;
use elevate::{
    devices::{filter::FilterMode, sampler::*},
    musical::drums::OffsetMode,
};

pub(super) fn region(
    asset: &SampleAsset,
    p: &SamplerParams,
    velocity: (u8, u8),
    start_offset: u16,
    offset_mode: OffsetMode,
) -> String {
    let frames = asset.file.frames.len();
    let rate = f64::from(asset.file.sample_rate.max(1));
    let start = ((p.get(START) as f64 / 100.) * frames as f64) as usize;
    let end = (((p.get(END) as f64 / 100.) * frames as f64) as usize)
        .max(start + 1)
        .min(frames)
        - 1;
    let skip = ((end + 1 - start) as u64 * u64::from(start_offset) / 1000) as usize;
    let skip = skip.min(end - start);
    let mode = match p.mode {
        PlayMode::OneShot => "one_shot",
        PlayMode::Classic if p.looped => "loop_sustain",
        PlayMode::Classic => "no_loop",
    };
    let keytrack = if p.keytrack { 100 } else { 0 };
    let direction = if p.reverse { " direction=reverse" } else { "" };
    let choke = if p.choke {
        " group=1 off_by=1 off_mode=fast"
    } else {
        ""
    };
    let fil_type = match p.filter_mode {
        FilterMode::LowPass => "lpf_2p",
        FilterMode::HighPass => "hpf_2p",
        FilterMode::BandPass => "bpf_2p",
    };
    let filter_line = if p.filter {
        format!(
            " fil_type={fil_type} cutoff=20 cutoff_oncc86=11958.941 resonance=0 resonance_oncc87=24 fil_veltrack={:.0}",
            p.get(VEL_FILTER)
        )
    } else {
        String::new()
    };
    // EQ paramétrico de 3 bandas controlado en tiempo real por CC 101..106.
    let eq_line = " eq1_freq=30 eq1_freq_oncc101=470 eq1_bw=1.5 eq1_gain=-18 eq1_gain_oncc102=36\n eq2_freq=200 eq2_freq_oncc103=5800 eq2_bw=1.2 eq2_gain=-18 eq2_gain_oncc104=36\n eq3_freq=1500 eq3_freq_oncc105=16500 eq3_bw=1.5 eq3_gain=-18 eq3_gain_oncc106=36";

    // sfizz resuelve sample.wav respecto a la ruta virtual de la misma carpeta.
    let name = asset.wave.file_name().unwrap().to_string_lossy();
    let (lo, hi) = velocity;
    let format_region = |offset: usize, region_end: usize, extra: &str| {
        let loop_start = (((p.get(LOOP_START) as f64 / 100.) * frames as f64) as usize)
            .clamp(offset, region_end);
        let loop_end = (((p.get(LOOP_END) as f64 / 100.) * frames as f64) as usize)
            .clamp((loop_start + 1).min(region_end), region_end);
        let loop_xfade = if p.looped && p.get(LOOP_XFADE) > 0.01 {
            let loop_sec = loop_end.saturating_sub(loop_start) as f64 / rate;
            let xfade_sec = (loop_sec * (f64::from(p.get(LOOP_XFADE)) / 100.0)).clamp(0.0, 2.0);
            format!(" loop_crossfade={xfade_sec:.4}")
        } else {
            String::new()
        };
        format!(
            "<region> sample={name}{direction}{choke}{extra} lovel={lo} hivel={hi}\n offset={offset} end={region_end} loop_start={loop_start} loop_end={loop_end}{loop_xfade} loop_mode={mode}\n pitch_keycenter={} pitch_keytrack={keytrack} tune=-2400 pitch_oncc83=4800 volume=-60 volume_oncc92=66 amp_veltrack={:.1} pan=-100 pan_oncc93=200\n ampeg_attack=0 ampeg_attack_oncc88=10 ampeg_hold=0 ampeg_hold_oncc94=5 ampeg_decay=0 ampeg_decay_oncc89=10 ampeg_sustain=0 ampeg_sustain_oncc90=100 ampeg_release=0.001 ampeg_release_oncc91=9.999\n pitcheg_depth=-2400 pitcheg_depth_oncc95=4800 pitcheg_decay=0.001 pitcheg_decay_oncc96=1.999 pitcheg_sustain=0\n{filter_line}\n{eq_line}",
            p.get(ROOT) as u8,
            p.get(VEL_VOLUME),
        )
    };
    let (offset, region_end) = if p.reverse {
        (start, end - skip)
    } else {
        (start + skip, end)
    };
    let _ = offset_mode;
    format_region(offset, region_end, "")
}
