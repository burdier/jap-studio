//! Crea una sesión de referencia para comparar la salida de Play con máster y stems.
//! Pasar una carpeta con Kick.wav, 808.wav y Hats.wav. El proyecto queda junto a las muestras.
use elevate::{
    musical::{
        clip::{ClipId, MidiClip},
        drums::{DrumClipTarget, DrumPlaybackSource, DrumSession, piece_pattern},
        model::{InstrumentId, Pattern},
        time::{PPQ, Tick},
        track::{Track, TrackKind},
    },
    project::{ProjectAudioClip, ProjectDoc, ProjectPiano},
};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Falta la carpeta del beat")?,
    )
    .canonicalize()?;
    let path = dir.join("Kick-808.loro");
    if path.exists() {
        return Err("El proyecto ya existe".into());
    }
    let mut kit = DrumSession::default();
    kit.source = DrumPlaybackSource::Arrangement;
    let kick = kit.kit.add_sample(
        dir.join("Kick.wav")
            .canonicalize()?
            .to_string_lossy()
            .into_owned(),
    )?;
    let sub = kit.kit.add_sample(
        dir.join("808.wav")
            .canonicalize()?
            .to_string_lossy()
            .into_owned(),
    )?;
    let inst = InstrumentId::fresh();
    let mut piano = Track::new("Piano MIDI", TrackKind::Synth);
    piano.instrument = Some(inst);
    let wav = Track::new("Hats WAV", TrackKind::Audio);
    let audio = ProjectAudioClip {
        edit: Default::default(),
        id: ClipId::fresh(),
        track: wav.id,
        source: "Hats.wav".into(),
        name: "Hats".into(),
        start_beat: 0.,
        trim_start: 0.,
        trim_len: None,
        props: Default::default(),
    };
    let mut tracks = vec![piano, wav];
    let mut phrase = Pattern::new(Tick(PPQ * 8))?;
    for (step, key) in [
        (2, 60),
        (6, 67),
        (11, 63),
        (14, 67),
        (18, 60),
        (22, 70),
        (26, 67),
        (30, 63),
    ] {
        phrase.add_note(inst, Tick(step * PPQ / 4), Tick(PPQ / 3), key, 91)?;
    }
    let mut clips = vec![MidiClip::new("Teclas", 0, 0., phrase)];
    let mut rhythm = Pattern::new(Tick(PPQ * 8))?;
    for step in [0, 6, 10, 16, 23, 26, 30] {
        rhythm.add_note(
            kick,
            Tick(step * PPQ / 4),
            Tick(PPQ / 4),
            36,
            if step % 8 == 0 { 122 } else { 105 },
        )?;
    }
    for (step, key) in [(0, 36), (7, 39), (12, 34), (16, 36), (23, 39), (28, 43)] {
        rhythm.add_note(sub, Tick(step * PPQ / 4), Tick(PPQ / 2), key, 110)?;
    }
    kit.patterns[0].notes = rhythm.clone();
    for (piece, name) in [(kick, "Kick"), (sub, "808")] {
        let mut clip = MidiClip::new(name, tracks.len(), 0., piece_pattern(&rhythm, piece)?);
        clip.drum_target = Some(DrumClipTarget {
            kit: kit.kit.id,
            piece,
            pattern: kit.active,
            linked: true,
        });
        tracks.push(Track::new(name, TrackKind::Drum));
        clips.push(clip);
    }
    let doc = ProjectDoc::from_session_state(
        "Kick & 808",
        &tracks,
        &clips,
        Default::default(),
        Default::default(),
        &[65, 70, 75, 75],
        &[58, 37, 50, 50],
    )?;
    doc.set_pianos(&[ProjectPiano {
        id: inst,
        source: Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/soundfonts/UprightPianoKW-20220221.sf2")
            .to_string_lossy()
            .into_owned(),
        preset: 0,
        parameters: vec![
            0.6, 100., 127., 64., 0., 0., 0., 0., 0., 2., 0., 0., 64., 0., 0., 0., 12000., 0.7,
        ],
    }])?;
    doc.set_drum_session(&kit)?;
    doc.reconcile_audio_clips(&[audio])?;
    doc.save(&path)?;
    Ok(())
}
