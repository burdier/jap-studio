//! Representación textual de preferencias, compatible con el comando de exportación.
use super::{BitDepth, ExportOptions, Mp3Mode, Target};

impl ExportOptions {
    pub fn to_command(&self) -> Result<String, String> {
        self.validate()?;
        let directory = self
            .directory
            .to_str()
            .ok_or("La carpeta debe ser texto UTF-8")?;
        if self.name.contains(['\n', '\r', '\0'])
            || directory.contains(['\n', '\r', '\0'])
            || directory.trim().is_empty()
        {
            return Err("Nombre y carpeta deben ser texto válido de una línea".into());
        }
        // El tokenizer admite segmentos contiguos con distintos tipos de comillas.
        let quote = |s: &str| format!("'{}'", s.replace('\'', "'\"'\"'"));
        let formats = self
            .formats
            .iter()
            .map(|f| f.extension())
            .collect::<Vec<_>>()
            .join(",");
        let name = quote(&format!("{}.[{formats}]", self.name));
        let target = if self.target == Target::Master {
            "-master"
        } else {
            "-individuals"
        };
        let bits = match self.bits {
            BitDepth::Int16 => "16",
            BitDepth::Int24 => "24",
            BitDepth::Int32 => "32",
            BitDepth::Float32 => "32f",
        };
        let mp3 = match self.mp3 {
            Mp3Mode::Cbr(rate) => format!("-bitrate {rate}"),
            Mp3Mode::Vbr(quality) => format!("-vbr {quality}"),
        };
        let channels = if self.mono { "-mono" } else { "-stereo" };
        let fx = match self.master_fx {
            Some(true) => "on",
            Some(false) => "off",
            None => "auto",
        };
        let tracks = if self.tracks.is_empty() {
            "all".into()
        } else {
            self.tracks
                .iter()
                .map(|n| (n + 1).to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        let length = self.length_beats.map_or("all".into(), |n| n.to_string());
        let mut command = format!(
            "export -n {name} {target} -out {} -sr {} -bits {bits} {mp3} {channels} -start {} -length {length} -tail {} -master-fx {fx} -tracks {tracks}",
            quote(directory),
            self.sample_rate,
            self.start_beat,
            self.tail_seconds
        );
        if let Some(lufs) = self.normalize_lufs {
            command.push_str(&format!(" -lufs {lufs}"));
        }
        if let Some(tp) = self.true_peak_limit_db {
            command.push_str(&format!(" -tp {tp}"));
        }
        Ok(command)
    }
}
