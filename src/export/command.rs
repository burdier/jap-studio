use crate::actions::tokenize;
// Parser común: conserva mayúsculas/rutas, admite comillas y rechaza opciones ignoradas.
use super::{BitDepth, ExportOptions, Format, Mp3Mode, Target};

impl ExportOptions {
    pub fn parse(command: &str) -> Result<Self, String> {
        Self::parse_with_defaults(command, &Self::default())
    }

    /// Los argumentos explícitos sustituyen solo las preferencias indicadas.
    pub fn parse_with_defaults(command: &str, defaults: &Self) -> Result<Self, String> {
        let tokens = tokenize(command.trim().trim_start_matches(':'))?;
        if !tokens
            .first()
            .is_some_and(|s| s.eq_ignore_ascii_case("export"))
        {
            return Err("Se esperaba export".into());
        }
        let mut options = defaults.clone();
        let mut args = tokens.iter().skip(1);
        let mut target = None;
        let mut mp3_mode = false;
        while let Some(arg) = args.next() {
            let flag = arg.to_ascii_lowercase();
            if matches!(flag.as_str(), "-master" | "-individuals") {
                let selected = if flag == "-master" {
                    Target::Master
                } else {
                    Target::Individuals
                };
                if target.is_some_and(|old| old != selected) {
                    return Err("Elige -master o -individuals en cada exportación".into());
                }
                target = Some(selected);
                options.target = selected;
                continue;
            }
            if matches!(flag.as_str(), "-mono" | "-stereo") {
                options.mono = flag == "-mono";
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("Falta el valor de {arg}"))?;
            match flag.as_str() {
                "-n" => {
                    let (name, suffix) = value
                        .rsplit_once('.')
                        .ok_or("Usa -n nombre.wav o nombre.[mp3,wave]")?;
                    let suffix = suffix
                        .strip_prefix('[')
                        .and_then(|s| s.strip_suffix(']'))
                        .unwrap_or(suffix);
                    let mut formats = Vec::new();
                    for format in suffix.split(',') {
                        let format = match format.trim().to_ascii_lowercase().as_str() {
                            "wav" | "wave" => Format::Wav,
                            "aif" | "aiff" => Format::Aiff,
                            "flac" => Format::Flac,
                            "mp3" => Format::Mp3,
                            other => return Err(format!("Formato desconocido: {other}")),
                        };
                        if !formats.contains(&format) {
                            formats.push(format);
                        }
                    }
                    options.name = name.into();
                    options.formats = formats;
                }
                "-out" => options.directory = value.into(),
                "-sr" => {
                    options.sample_rate = value.parse().map_err(|_| "Usa -sr 48000 (Hz enteros)")?
                }
                "-bits" => {
                    options.bits = match value.to_ascii_lowercase().as_str() {
                        "16" => BitDepth::Int16,
                        "24" => BitDepth::Int24,
                        "32" => BitDepth::Int32,
                        "32f" => BitDepth::Float32,
                        _ => return Err("Profundidad: 16, 24, 32 o 32f".into()),
                    }
                }
                "-bitrate" | "-vbr" => {
                    if mp3_mode {
                        return Err("Elige -bitrate (CBR) o -vbr, una sola vez".into());
                    }
                    mp3_mode = true;
                    options.mp3 = if flag == "-vbr" {
                        Mp3Mode::Vbr(value.parse().map_err(|_| "VBR: 0–9")?)
                    } else {
                        Mp3Mode::Cbr(
                            value
                                .parse()
                                .map_err(|_| "Bitrate en kbps: por ejemplo 320")?,
                        )
                    };
                }
                "-start" => options.start_beat = number(value)?,
                "-length" => {
                    options.length_beats = if value.eq_ignore_ascii_case("all") {
                        None
                    } else {
                        Some(number(value)?)
                    }
                }
                "-tail" => options.tail_seconds = number(value)?,
                "-master-fx" => {
                    options.master_fx = match value.to_ascii_lowercase().as_str() {
                        "on" => Some(true),
                        "off" => Some(false),
                        "auto" => None,
                        _ => return Err("Usa -master-fx on|off|auto".into()),
                    }
                }
                "-tracks" => {
                    options.tracks.clear();
                    if value.eq_ignore_ascii_case("all") {
                        continue;
                    }
                    for track in value.split(',') {
                        let index = track
                            .parse::<usize>()
                            .ok()
                            .and_then(|n| n.checked_sub(1))
                            .ok_or("Pistas numeradas desde 1: -tracks 1,3")?;
                        if !options.tracks.contains(&index) {
                            options.tracks.push(index);
                        }
                    }
                }
                _ => return Err(format!("Opción desconocida: {arg}; consulta :export.help")),
            }
        }
        options.validate()?;
        Ok(options)
    }
}

fn number(value: &str) -> Result<f64, String> {
    value
        .parse()
        .map_err(|_| format!("Número inválido: {value}"))
}
