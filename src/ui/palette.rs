//! Estado del buscador de instrumentos, primitivos, componentes y acciones.
//! La UI solo presenta resultados; el catálogo y las acciones viven fuera de GPUI.
use super::DawApp;
use elevate::{
    catalog::{self, CatalogAction, CatalogEntry, EntryKind},
    musical::track::assign_instrument,
};
use gpui::{Context, Window};

mod activate;
mod entry;
mod plugins;
mod tools;
use entry::PaletteEntry;

pub(super) struct Palette {
    open: bool,
    target_track: Option<usize>,
    pub(super) fx_target: Option<super::fx::FxTarget>,
    category: Option<EntryKind>,
    query: String,
    selected: usize,
    results: Vec<PaletteEntry>,
    plugins: Vec<catalog::plugins::InstalledInstrument>,
    tools: Vec<catalog::tools::ToolCatalogEntry>,
    scan_needed: bool,
    scan_task: Option<gpui::Task<()>>,
}

impl Palette {
    pub const CATEGORIES: [Option<EntryKind>; 5] = [
        None,
        Some(EntryKind::Instrument),
        Some(EntryKind::Primitive),
        Some(EntryKind::Component),
        Some(EntryKind::Action),
    ];

    pub fn new() -> Self {
        Self {
            open: false,
            target_track: None,
            fx_target: None,
            category: None,
            query: String::new(),
            selected: 0,
            results: catalog::search("").into_iter().map(Into::into).collect(),
            plugins: Vec::new(),
            tools: Vec::new(),
            scan_needed: false,
            scan_task: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn target_track(&self) -> Option<usize> {
        self.target_track
    }

    pub fn category(&self) -> Option<EntryKind> {
        self.category
    }

    pub fn set_category(&mut self, category: Option<EntryKind>) {
        self.category = category;
        self.refresh();
    }

    pub fn cycle_category(&mut self) {
        let idx = Self::CATEGORIES
            .iter()
            .position(|c| *c == self.category)
            .unwrap_or(0);
        self.category = Self::CATEGORIES[(idx + 1) % Self::CATEGORIES.len()];
        self.refresh();
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn results(&self) -> &[PaletteEntry] {
        &self.results
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn open_for_track(&mut self, track: usize) {
        self.open = true;
        self.scan_needed = true;
        self.target_track = Some(track);
        self.fx_target = None;
        self.category = None;
        self.query.clear();
        self.refresh();
    }

    pub fn close(&mut self) {
        self.open = false;
        self.target_track = None;
        self.fx_target = None;
        self.category = None;
        self.query.clear();
        self.selected = 0;
    }

    /// Devuelve la entrada elegida al pulsar Enter. El llamador decide cómo actuar.
    pub fn handle_key(&mut self, key: &str, text: bool) -> Option<PaletteEntry> {
        match key {
            "tab" => self.cycle_category(),
            "up" => {
                if !self.results.is_empty() {
                    self.selected = (self.selected + self.results.len() - 1) % self.results.len();
                }
            }
            "down" => {
                if !self.results.is_empty() {
                    self.selected = (self.selected + 1) % self.results.len();
                }
            }
            "enter" => return self.results.get(self.selected).cloned(),
            "backspace" => {
                if self.query.pop().is_some() {
                    self.refresh();
                }
            }
            _ if text && key.chars().count() == 1 => {
                self.query.push_str(key);
                self.refresh();
            }
            _ => {}
        }
        None
    }

    fn refresh(&mut self) {
        self.results = catalog::search(&self.query)
            .into_iter()
            .map(PaletteEntry::from)
            .chain(
                self.plugins
                    .iter()
                    .filter(|plugin| plugin.matches(&self.query))
                    .map(PaletteEntry::from),
            )
            .chain(
                self.tools
                    .iter()
                    .filter(|tool| tool.entry.matches(&self.query))
                    .map(PaletteEntry::from),
            )
            .filter(|entry| self.category.is_none_or(|kind| entry.kind == kind))
            .collect();
        self.selected = 0;
    }
}

impl DawApp {
    pub(super) fn open_palette(&mut self) {
        self.open_palette_for_track(self.cursor_row);
    }

    pub(super) fn open_palette_for_track(&mut self, track: usize) {
        if self.tracks.is_empty() {
            self.palette.open_for_track(0);
            self.palette.target_track = None;
            self.status_message = "Buscador · inserta una pista o elige un instrumento".into();
            return;
        }
        let track = track.min(self.tracks.len() - 1);
        self.cursor_row = track;
        self.palette.open_for_track(track);
        let id = self.tracks[track].id;
        self.palette.fx_target = Some(
            self.fx
                .current
                .filter(|target| target.track == id && self.fx.width > 0.0)
                .unwrap_or(super::fx::FxTarget {
                    track: id,
                    node: None,
                }),
        );
        self.status_message = format!("Buscador · añadir a la pista {}", self.tracks[track].name);
    }

    pub(super) fn handle_palette_key(
        &mut self,
        key: &str,
        text: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if key == "escape" {
            self.palette.close();
            self.status_message = "Buscador cerrado".into();
            return;
        }
        if let Some(entry) = self.palette.handle_key(key, text) {
            self.activate_palette_entry(entry, window, cx);
        } else if key == "enter" {
            self.status_message = "El buscador no tiene resultados".into();
        }
    }

    pub(super) fn activate_catalog_entry(
        &mut self,
        entry: &'static CatalogEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut target_track = self.palette.target_track();
        let fx_target = self.palette.fx_target;
        if elevate::devices::find(entry.id).is_some() {
            if let Some(target) = fx_target.or_else(|| self.selected_fx_target()) {
                self.palette.close();
                if let Err(error) = self.insert_fx(target, entry.id) {
                    self.status_message = error;
                }
                cx.notify();
                return;
            }
            self.palette.close();
            self.status_message = "Añade una pista antes de insertar efectos".into();
            return;
        }
        if entry.kind == EntryKind::Instrument && self.tracks.is_empty() {
            self.edit_track_structure(elevate::musical::track::TrackEdit::Insert);
            target_track = Some(self.cursor_row);
        }
        self.palette.close();
        if let CatalogAction::Invoke(command) = entry.action {
            if command.starts_with("fx.") && command != "fx.show" {
                if let Some(target) = fx_target.or_else(|| self.selected_fx_target()) {
                    if let Err(error) = self.execute_fx_at(target, command) {
                        self.status_message = error;
                    }
                    return;
                }
            }
        }
        if entry.kind == EntryKind::Instrument {
            if let Some(track) = target_track {
                match entry.action {
                    CatalogAction::Invoke(_) => self.assign_instrument_to_track(track, entry),
                    CatalogAction::Unavailable(reason) => {
                        self.status_message = format!("{} · {reason}", entry.name);
                    }
                }
                return;
            }
        }
        match entry.action {
            CatalogAction::Invoke(command) => {
                if let Some(spec) = entry.action.parameters() {
                    self.open_action_form(entry, spec, window, cx);
                } else if matches!(command, "clip.rename" | "clip.gain" | "clip.color") {
                    self.open_clip_catalog_page(
                        command,
                        window.mouse_position(),
                        window.viewport_size(),
                    );
                    cx.notify();
                } else {
                    self.execute_command_with_context(command, window, cx);
                }
                if entry.kind == EntryKind::Instrument {
                    self.status_message = format!("Instrumento abierto · {}", entry.name);
                }
            }
            CatalogAction::Unavailable(reason) => {
                self.status_message = format!("{} · {reason}", entry.name);
            }
        }
    }

    fn assign_instrument_to_track(&mut self, track: usize, entry: &'static CatalogEntry) {
        if track >= self.tracks.len() {
            self.status_message = "Pista inválida".into();
            return;
        }
        if !self.track_accepts_clips(track) {
            self.status_message =
                "El grupo recibe sus pistas hijas; no admite un instrumento MIDI directo".into();
            return;
        }
        if entry.id == "instrument.sampler" {
            self.add_sampler(track);
            return;
        }
        if entry.id != "instrument.piano" {
            self.status_message = format!("{} · instrumento sin implementación", entry.name);
            return;
        }
        let previous = assign_instrument(&mut self.tracks, track, self.instrument.id());
        self.mix_vst();
        self.cursor_row = track;
        self.open_track_instrument(self.tracks[track].id);
        self.sync_midi_focus();
        let name = self.tracks[track].name.clone();
        self.status_message = match previous {
            Some(previous) => format!(
                "{} movido de {} a {}",
                entry.name, self.tracks[previous].name, name
            ),
            None => format!("{} asignado a {}", entry.name, name),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::Palette;
    use elevate::catalog;

    #[test]
    fn palette_filters_by_accent_insensitive_query_and_resets_selection() {
        let mut palette = Palette::new();
        palette.open_for_track(0);
        assert!(palette.results().len() > 1);
        for key in "reproduccion".chars() {
            palette.handle_key(&key.to_string(), true);
        }
        assert_eq!(palette.results().len(), 1);
        assert_eq!(palette.results()[0].name, "Reproducción");
        assert_eq!(palette.selected(), 0);
    }

    #[test]
    fn backspace_restores_the_catalog_and_enter_returns_the_first_match() {
        let mut palette = Palette::new();
        palette.open_for_track(0);
        for key in "volume".chars() {
            palette.handle_key(&key.to_string(), true);
        }
        assert!(!palette.results().is_empty());
        assert_eq!(palette.results()[0].id, "primitive.volume");
        for _ in 0.."volume".len() {
            palette.handle_key("backspace", true);
        }
        assert_eq!(palette.results().len(), catalog::entries().len());
        let entry = palette
            .handle_key("enter", true)
            .expect("catálogo no vacío");
        assert_eq!(entry.id, catalog::entries()[0].id);
    }

    #[test]
    fn palette_remembers_the_target_track_and_forgets_it_on_close() {
        let mut palette = Palette::new();
        palette.open_for_track(3);
        assert_eq!(palette.target_track(), Some(3));
        palette.close();
        assert_eq!(palette.target_track(), None);
    }
}
