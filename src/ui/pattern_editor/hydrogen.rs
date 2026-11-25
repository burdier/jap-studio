//! Convierte el kit y el reloj de Hydrogen al modelo nativo de Drums Latex.
use super::hydrogen_xml::{self, Element};
use elevate::{
    devices::sampler::{GAIN, TUNE, VEL_VOLUME},
    musical::{
        drums::{
            DrumLayer, DrumPattern, DrumPatternId, DrumPlaybackSource, DrumSession, DrumSongCell,
        },
        grid::NoteGrid,
        model::{InstrumentId, NoteEvent, NoteId, Pattern},
        note_expression::NoteExpression,
        time::{Meter, PPQ, Tempo, Tick},
    },
};
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) struct Imported {
    pub session: DrumSession,
    pub tempo: Option<Tempo>,
    pub missing: Vec<String>,
}

const SCALE: i64 = PPQ / 48;

pub(super) fn load(path: &Path, override_dir: Option<&Path>) -> Result<Imported, String> {
    let mut xml = String::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(16 * 1024 * 1024 + 1)
        .read_to_string(&mut xml)
        .map_err(|e| e.to_string())?;
    if xml.len() > 16 * 1024 * 1024 {
        return Err("El XML de Hydrogen supera 16 MiB".into());
    }
    let document = hydrogen_xml::parse(&xml)?;
    let song = match document.tag.as_str() {
        "song" => true,
        "drumkit_info" => false,
        _ => return Err("Se esperaba una canción .h2song o un drumkit.xml de Hydrogen".into()),
    };
    let instruments = document
        .child("instrumentList")
        .ok_or("Hydrogen: falta la lista de instrumentos")?;
    let mut session = DrumSession::default();
    let mut missing = Vec::new();
    let mut ids = HashMap::new();
    let mut base_paths = vec![
        PathBuf::from("/usr/share/hydrogen/data/drumkits"),
        PathBuf::from("/usr/local/share/hydrogen/data/drumkits"),
    ];
    let flatpak =
        "app/org.hydrogenmusic.Hydrogen/current/active/files/share/hydrogen/data/drumkits";
    base_paths.push(Path::new("/var/lib/flatpak").join(flatpak));
    if let Some(home) = dirs::home_dir() {
        base_paths.extend([
            home.join(".hydrogen/data/drumkits"),
            home.join(".var/app/org.hydrogenmusic.Hydrogen/data/hydrogen/data/drumkits"),
            home.join(".local/share/hydrogen/data/drumkits"),
            home.join(".local/share/flatpak").join(flatpak),
        ]);
    }
    if let Some(data) = dirs::data_dir() {
        base_paths.push(data.join("hydrogen/data/drumkits"));
    }
    for node in instruments
        .children
        .iter()
        .filter(|i| i.tag == "instrument")
    {
        let old = node
            .value("id")
            .and_then(|id| id.parse::<i64>().ok())
            .ok_or("ID de instrumento Hydrogen inválido")?;
        let name = node
            .value("name")
            .filter(|s| !s.is_empty())
            .unwrap_or("Pieza");
        let kit_path = node
            .value("drumkitPath")
            .or_else(|| document.value("last_loaded_drumkit"));
        let kit_name = node
            .value("drumkit")
            .filter(|s| !s.is_empty())
            .or_else(|| kit_path.and_then(|p| Path::new(p).file_name()?.to_str()))
            .or_else(|| document.value("last_loaded_drumkit_name"));
        let folder = path.parent().unwrap_or(Path::new("."));
        let mut directories: Vec<_> = override_dir.into_iter().map(Path::to_owned).collect();
        directories.push(folder.to_owned());
        if let Some(kit_path) = kit_path.filter(|p| !p.is_empty()) {
            directories.push(folder.join(kit_path));
        }
        if let Some(kit_name) = kit_name {
            directories.push(folder.join(kit_name));
            directories.extend(base_paths.iter().map(|root| root.join(kit_name)));
        }
        let component = node
            .children
            .iter()
            .filter(|c| c.tag == "instrumentComponent")
            .find(|component| component.child("layer").is_some());
        let layers = component.unwrap_or(node);
        let mut samples = Vec::new();
        let mut append = |value: &Element, min: f64, max: f64| {
            let filename = value.text();
            if filename.is_empty() {
                return;
            }
            let source = directories
                .iter()
                .map(|dir| dir.join(filename))
                .filter(|file| file.is_file())
                .find_map(|file| std::fs::canonicalize(file).ok());
            if let Some(source) = source.and_then(|path| path.to_str().map(str::to_owned)) {
                let low = (min * 127.).ceil().clamp(1., 127.) as u8;
                let high = (max * 127.).floor().clamp(1., 127.) as u8;
                samples.push(DrumLayer {
                    source,
                    velocity_min: low,
                    velocity_max: high.max(low),
                    start_offset: 0,
                    offset_mode: elevate::musical::drums::OffsetMode::Fixed,
                });
            } else {
                missing.push(filename.to_owned());
            }
        };
        for layer in layers.children.iter().filter(|n| n.tag == "layer") {
            if let Some(file) = layer.child("filename") {
                append(file, number(layer, "min", 0.), number(layer, "max", 1.));
            }
        }
        if layers.child("layer").is_none()
            && let Some(file) = layers.child("filename")
        {
            append(file, 0., 1.);
        }
        // El constructor del kit reserva IDs de pieza que no colisionan con las voces legadas.
        let id = session.kit.add_sample(
            samples
                .first()
                .map_or(name.to_owned(), |s| s.source.clone()),
        )?;
        if ids.insert(old, id).is_some() {
            return Err("Instrumento Hydrogen duplicado".into());
        }
        let piece = session.kit.pieces.last_mut().unwrap();
        piece.name = name.into();
        piece.layers = samples;
        piece.mix.muted = flag(node, "isMuted");
        piece.mix.solo = flag(node, "isSoloed");
        piece.mix.fader.level = fader(number(node, "volume", 1.));
        let left = number(node, "pan_L", 1.);
        let right = number(node, "pan_R", 1.);
        let pan = number(node, "pan", right - left);
        piece.mix.fader.pan = (50. + 50. * pan.clamp(-1., 1.)).round() as i32;
        piece.choke_group = node
            .value("muteGroup")
            .and_then(|s| s.parse::<u16>().ok())
            .and_then(|group| group.checked_add(1));
        piece.sampler.choke = false;
        let gain = number(node, "gain", 1.) * component.map_or(1., |c| number(c, "gain", 1.));
        piece
            .sampler
            .set(GAIN, (20. * gain.max(0.001).log10()) as f32);
        piece
            .sampler
            .set(TUNE, number(node, "pitchOffset", 0.) as f32);
        if node.value("applyVelocity") == Some("false") {
            piece.sampler.set(VEL_VOLUME, 0.);
        }
    }
    if ids.is_empty() {
        return Err("El archivo de Hydrogen no contiene instrumentos".into());
    }
    let tempo = if song {
        session.kit.bus.fader.level = fader(number(&document, "volume", 1.));
        session.kit.bus.muted = flag(&document, "isMuted");
        patterns(&document, &ids, &mut session)?;
        Some(Tempo::new(number(&document, "bpm", 120.)).map_err(|error| error.to_string())?)
    } else {
        None
    };
    session.validate()?;
    missing.sort();
    missing.dedup();
    Ok(Imported {
        session,
        tempo,
        missing,
    })
}

fn patterns(
    document: &Element,
    ids: &HashMap<i64, InstrumentId>,
    session: &mut DrumSession,
) -> Result<(), String> {
    let mut patterns = Vec::new();
    if let Some(list) = document.child("patternList") {
        for pattern in list.children.iter().filter(|p| p.tag == "pattern") {
            let length = ticks(pattern.value("size").unwrap_or("192"))?;
            if !(1..=128 * 4 * PPQ).contains(&length.0) {
                return Err("Tamaño de patrón Hydrogen fuera de rango".into());
            }
            let name = pattern
                .value("name")
                .filter(|s| !s.is_empty())
                .unwrap_or("Patrón");
            let denominator = pattern
                .value("denominator")
                .and_then(|s| s.parse::<u8>().ok())
                .unwrap_or(4);
            let numerator = (length.0 * i64::from(denominator) / (4 * PPQ)).clamp(1, 32) as u8;
            let meter = Meter::new(numerator, denominator).unwrap_or_default();
            let mut notes = Vec::new();
            if let Some(list) = pattern.child("noteList") {
                for node in list
                    .children
                    .iter()
                    .filter(|n| n.tag == "note" && !flag(n, "note_off"))
                {
                    let old = node
                        .value("instrument")
                        .and_then(|s| s.parse::<i64>().ok())
                        .ok_or("Nota Hydrogen sin instrumento")?;
                    let instrument = *ids
                        .get(&old)
                        .ok_or_else(|| format!("Hydrogen: falta el instrumento {old}"))?;
                    let start = ticks(node.value("position").ok_or("Nota Hydrogen sin posición")?)?;
                    let duration = node
                        .value("length")
                        .map(ticks)
                        .transpose()?
                        .filter(|d| d.0 > 0)
                        .unwrap_or(Tick(SCALE));
                    if start.0 < 0 || start.0 >= length.0 {
                        return Err(format!("Nota fuera del tamaño del patrón {name}"));
                    }
                    let velocity = number(node, "velocity", 0.8);
                    if velocity <= 0. {
                        continue;
                    }
                    let note = key_offset(node.value("key").unwrap_or("C0"))
                        + number(node, "pitch", 0.).round().clamp(-128., 128.) as i32;
                    notes.push(NoteEvent {
                        id: NoteId(notes.len() as u64 + 1),
                        instrument,
                        start,
                        duration: Tick(duration.0.min(length.0 - start.0)),
                        pitch: (36 + note).clamp(0, 127) as u8,
                        velocity: (velocity * 127.).round().clamp(1., 127.) as u8,
                        expression: NoteExpression {
                            probability: (number(node, "probability", 1.) * 100.)
                                .round()
                                .clamp(0., 100.) as u8,
                            pan: (number(node, "pan", 0.) * 63.).round().clamp(-64., 63.) as i8,
                            ..Default::default()
                        },
                        pitch_curve: Default::default(),
                    });
                }
            }
            let pattern = Pattern::restore(length, notes).map_err(|e| format!("{name}: {e}"))?;
            patterns.push(DrumPattern {
                id: DrumPatternId(patterns.len() as u32 + 1),
                name: name.into(),
                notes: pattern,
                meter,
                grid: NoteGrid::default(),
            });
        }
    }
    if !patterns.is_empty() {
        session.next_pattern_id = patterns.len() as u32 + 1;
        session.active = patterns[0].id;
        session.patterns = patterns;
    }
    let mut position = Tick(0);
    if let Some(sequence) = document.child("patternSequence") {
        for group in sequence.children.iter().filter(|g| g.tag == "group") {
            let mut width = 0;
            for entry in group.children.iter().filter(|p| p.tag == "patternID") {
                let pattern = session
                    .patterns
                    .iter()
                    .find(|p| p.name == entry.text())
                    .ok_or_else(|| format!("Canción: patrón inexistente {}", entry.text()))?;
                let pattern_id = pattern.id;
                width = width.max(pattern.notes.length().0);
                session.set_song_cell(
                    DrumSongCell {
                        at: position,
                        pattern: pattern_id,
                    },
                    true,
                )?;
            }
            position.0 = position
                .0
                .checked_add(if width == 0 { 4 * PPQ } else { width })
                .ok_or("Canción Hydrogen demasiado larga")?;
        }
    }
    if let Some(cell) = session.song.first() {
        session.active = cell.pattern;
        session.source = DrumPlaybackSource::Song;
    }
    Ok(())
}

fn key_offset(key: &str) -> i32 {
    let mut chars = key.chars();
    let pitch = match chars.next() {
        Some('D') => 2,
        Some('E') => 4,
        Some('F') => 5,
        Some('G') => 7,
        Some('A') => 9,
        Some('B') => 11,
        _ => 0,
    };
    let octave = chars.as_str();
    if let Some(octave) = octave.strip_prefix(['s', '#']) {
        pitch + 1 + 12 * octave.parse::<i32>().unwrap_or(0).clamp(-10, 10)
    } else {
        pitch + 12 * octave.parse::<i32>().unwrap_or(0).clamp(-10, 10)
    }
}

fn ticks(value: &str) -> Result<Tick, String> {
    value
        .parse::<i64>()
        .ok()
        .and_then(|tick| tick.checked_mul(SCALE))
        .map(Tick)
        .ok_or_else(|| format!("Tiempo Hydrogen inválido: {value}"))
}

fn number(node: &Element, tag: &str, default: f64) -> f64 {
    node.value(tag)
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|n| n.is_finite())
        .unwrap_or(default)
}

fn flag(node: &Element, tag: &str) -> bool {
    matches!(node.value(tag), Some("true" | "1"))
}

fn fader(gain: f64) -> i32 {
    (gain * 75.).round().clamp(0., 100.) as i32
}

use crate::ui::{CenterView, DawApp, pattern_editor::PatternEditor};
use gpui::{AppContext, Context, PathPromptOptions};

impl DawApp {
    pub(in crate::ui) fn hydrogen_command(
        &mut self,
        command: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if command.split_whitespace().next() != Some("hydrogen.import") {
            return false;
        }
        if self.import_task.is_some() {
            self.status_message = "Hay una importación en curso".into();
            return true;
        }
        match elevate::actions::tokenize(command).as_deref() {
            Ok([_]) => {
                let prompt = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: None,
                });
                self.import_task = Some(cx.spawn(async move |this, cx| {
                    let selected = prompt.await;
                    let _ = this.update(cx, |this, cx| {
                        this.import_task = None;
                        if let Ok(Ok(Some(paths))) = selected
                            && let Some(path) = paths.first()
                        {
                            this.import_hydrogen(path.clone(), None, cx);
                        }
                    });
                }));
            }
            Ok([_, path]) => self.import_hydrogen(path.into(), None, cx),
            Ok([_, path, folder]) => self.import_hydrogen(path.into(), Some(folder.into()), cx),
            _ => {
                self.status_message = "Uso: hydrogen.import ruta.h2song [carpeta de samples]".into()
            }
        }
        true
    }

    pub(in crate::ui) fn import_hydrogen(
        &mut self,
        path: PathBuf,
        samples: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if self.import_task.is_some()
            || self.pattern_editor.sample_task.is_some()
            || self.midi_capture.is_recording()
        {
            self.status_message = "Espera a que termine la importación o grabación actual".into();
            return;
        }
        if self.has_drum_clips() {
            self.status_message = "Abre una sesión vacía para importar Hydrogen: el Arrangement ya contiene clips enlazados al kit".into();
            return;
        }
        let before = self.pattern_editor.session.clone();
        self.status_message = "Leyendo proyecto de Hydrogen…".into();
        let work = cx.background_spawn(async move { load(&path, samples.as_deref()) });
        self.import_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.import_task = None;
                this.status_message = match result {
                    Ok(imported)
                        if this.pattern_editor.sample_task.is_none()
                            && !this.has_drum_clips()
                            && this.pattern_editor.session == before =>
                    {
                        this.stop_all();
                        if let Some(tempo) = imported.tempo {
                            this.timeline.tempo = tempo;
                        }
                        let count = imported.session.patterns.len();
                        let pieces = imported.session.kit.pieces.len();
                        this.pattern_editor = PatternEditor::new();
                        this.pattern_editor.session = imported.session;
                        this.set_center_view(CenterView::Session);
                        let missing = if imported.missing.is_empty() {
                            String::new()
                        } else {
                            format!(
                                " · Faltan {} muestras, incluida {}",
                                imported.missing.len(),
                                imported.missing[0]
                            )
                        };
                        format!("Hydrogen: {pieces} piezas · {count} patrones{missing}")
                    }
                    Ok(_) => "La batería cambió durante la importación. Vuelve a importar.".into(),
                    Err(error) => format!("No se pudo importar Hydrogen: {error}"),
                };
                cx.notify();
            });
        }));
    }
}
