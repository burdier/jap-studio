//! Traducción del contrato musical a SFZ; el usuario no edita archivos intermedios.
use super::SampleAsset;
use elevate::devices::{filter::FilterMode, sampler::*};

pub(super) fn region(asset: &SampleAsset, p: &SamplerParams, velocity: (u8, u8)) -> String {
    let frames = asset.file.frames.len();
    let offset = ((p.get(START) as f64 / 100.) * frames as f64) as usize;
    let end = (((p.get(END) as f64 / 100.) * frames as f64) as usize)
        .max(offset + 1)
        .min(frames)
        - 1;
    let loop_start =
        (((p.get(LOOP_START) as f64 / 100.) * frames as f64) as usize).clamp(offset, end);
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
    // sfizz resuelve sample.wav respecto a la ruta virtual de la misma carpeta.
    let name = asset.wave.file_name().unwrap().to_string_lossy();
    let (lo, hi) = velocity;
    format!(
        "<region> sample={name}{direction}{choke} lovel={lo} hivel={hi}\n offset={offset} end={end} loop_start={loop_start} loop_end={end} loop_mode={mode}\n pitch_keycenter={} pitch_keytrack={keytrack} tune=-2400 pitch_oncc83=4800 volume=-60 volume_oncc92=66 amp_veltrack={:.1} pan=-100 pan_oncc93=200\n ampeg_attack=0 ampeg_attack_oncc88=10 ampeg_hold=0 ampeg_hold_oncc94=5 ampeg_decay=0 ampeg_decay_oncc89=10 ampeg_sustain=0 ampeg_sustain_oncc90=100 ampeg_release=0.001 ampeg_release_oncc91=9.999\n pitcheg_depth=-2400 pitcheg_depth_oncc95=4800 pitcheg_decay=0.001 pitcheg_decay_oncc96=1.999 pitcheg_sustain=0\n{filter_line}",
        p.get(ROOT) as u8,
        p.get(VEL_VOLUME),
    )
}
