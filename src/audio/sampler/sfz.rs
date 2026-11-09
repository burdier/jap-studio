//! Traducción del contrato musical a SFZ; el usuario no edita archivos intermedios.
use super::SamplerSnapshot;
use elevate::devices::sampler::*;

pub(super) fn instrument(snapshot: &SamplerSnapshot) -> String {
    let p = &snapshot.params;
    let frames = snapshot.asset.file.frames.len();
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
    // sfizz resuelve sample.wav respecto a la ruta virtual de la misma carpeta.
    let name = snapshot.asset.wave.file_name().unwrap().to_string_lossy();
    format!(
        "<region> sample={name}\n offset={offset} end={end} loop_start={loop_start} loop_end={end} loop_mode={mode}\n pitch_keycenter={} tune=-2400 pitch_oncc83=4800 volume=-60 volume_oncc92=66 pan=-100 pan_oncc93=200\n ampeg_attack=0 ampeg_attack_oncc88=10 ampeg_decay=0 ampeg_decay_oncc89=10 ampeg_sustain=0 ampeg_sustain_oncc90=100 ampeg_release=0.001 ampeg_release_oncc91=9.999\n{}",
        p.get(ROOT) as u8,
        if p.filter {
            "fil_type=lpf_2p cutoff=20 cutoff_oncc86=11958.941 resonance=0 resonance_oncc87=24"
        } else {
            ""
        }
    )
}
