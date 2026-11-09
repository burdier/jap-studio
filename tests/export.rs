//! Contrato de comandos: evitar cambios silenciosos de formato, ruta o calidad.
use elevate::export::{BitDepth, ExportOptions, Format, Mp3Mode, Target};

#[test]
fn saved_defaults_apply_to_export_and_explicit_arguments_override_only_their_fields() {
    let defaults = ExportOptions::parse("export -n Mix.flac -individuals -sr 44100 -bits 16 -mono -tracks 2 -length 8 -master-fx off -tail 2 -out '/Music/Stems'").unwrap();
    let plain = ExportOptions::parse_with_defaults("export", &defaults).unwrap();
    assert_eq!(plain.to_command().unwrap(), defaults.to_command().unwrap());
    let overridden = ExportOptions::parse_with_defaults(
        "export -n Take.mp3 -master -tracks all -length all -master-fx auto",
        &defaults,
    )
    .unwrap();
    assert_eq!(overridden.formats, [Format::Mp3]);
    assert_eq!(overridden.target, Target::Master);
    assert_eq!(overridden.sample_rate, 44_100);
    assert_eq!(overridden.tail_seconds, 2.0);
    assert!(overridden.mono && overridden.tracks.is_empty() && overridden.length_beats.is_none());
    assert!(overridden.includes_master_fx());
    assert_eq!(defaults.target, Target::Individuals);
}

#[test]
fn preferences_round_trip_quotes_spaces_paths_and_all_export_parameters() {
    let mut options = ExportOptions::parse("export -n Mix.[wav,mp3] -sr 44100 -bits 24 -vbr 2 -tracks 1,3 -start 4 -length 8 -tail 1.5 -master-fx off").unwrap();
    options.name = "Canción 'uno' \"dos\"".into();
    options.directory = "/Music/Artista's \"Mix\"".into();
    let command = options.to_command().unwrap();
    let loaded = ExportOptions::parse(&command).unwrap();
    assert_eq!(loaded.name, options.name);
    assert_eq!(loaded.directory, options.directory);
    assert_eq!(loaded.to_command().unwrap(), command);
    options.directory = "carpeta\nEXPORT_DEFAULTS=otra cosa".into();
    assert!(options.to_command().is_err());
}

#[test]
fn export_preserves_names_paths_and_supports_multiple_formats() {
    let options = ExportOptions::parse(":export -n 'Mi Mezcla.[mp3, wave,flac]' -individuals -out '/Music/Mis Stems' -sr 44100 -bits 24 -vbr 2 -tracks 1,3 -start 8 -length 16 -tail 2.5 -mono -master-fx on").unwrap();
    assert_eq!(options.name, "Mi Mezcla");
    assert_eq!(options.directory.to_str().unwrap(), "/Music/Mis Stems");
    assert_eq!(options.formats, [Format::Mp3, Format::Wav, Format::Flac]);
    assert_eq!(options.target, Target::Individuals);
    assert_eq!(options.mp3, Mp3Mode::Vbr(2));
    assert_eq!(options.tracks, [0, 2]);
    assert_eq!(
        (
            options.start_beat,
            options.length_beats,
            options.tail_seconds
        ),
        (8.0, Some(16.0), 2.5)
    );
    assert!(options.mono && options.includes_master_fx());
    let float = ExportOptions::parse("export -n 93.[wav,aiff] -bits 32f -sr 96000").unwrap();
    assert_eq!(float.bits, BitDepth::Float32);
}

#[test]
fn invalid_or_incompatible_settings_fail_instead_of_falling_back() {
    for command in [
        "export -n 93.[mp3,wave] -sr 96000",
        "export -n a.flac -bits 32f",
        "export -n a.mp3 -sr 22050 -bitrate 320",
        "export -n a.mp3 -vbr 10",
        "export -n a.mp3 -bitrate 192 -vbr 2",
        "export -master -individuals",
        "export -start NaN",
        "export -tail inf",
        "export -length 0",
        "export -tracks 0",
        "export -sr 0",
        "export -n ../bad.wav",
        "export -n 93.[mp3,wave",
        "export -n 'sin cerrar.wav",
        "export -n foo.ogg",
        "export -desconocida 5",
        "export -out",
    ] {
        assert!(ExportOptions::parse(command).is_err(), "aceptó {command}");
    }
    assert!(
        !ExportOptions::parse("export -individuals")
            .unwrap()
            .includes_master_fx()
    );
    assert!(
        ExportOptions::parse("export -master")
            .unwrap()
            .includes_master_fx()
    );
}
