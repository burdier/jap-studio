//! Estado del buscador de instrumentos, primitivos, componentes y acciones.
//! La UI solo presenta resultados; el catálogo y las acciones viven fuera de GPUI.
use super::{CenterView, DawApp};
use elevate::{
    catalog::{self, CatalogAction, CatalogEntry, EntryKind},
    musical::track::assign_instrument,
};

pub(super) struct Palette {
    open: bool,
    target_track: Option<usize>,
    query: String,
    selected: usize,
    results: Vec<&'static CatalogEntry>,
}

impl Palette {
    pub fn new() -> Self {
        Self {
            open: false,
            target_track: None,
            query: String::new(),
            selected: 0,
            results: catalog::search(""),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn target_track(&self) -> Option<usize> {
        self.target_track
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn results(&self) -> &[&'static CatalogEntry] {
        &self.results
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn open_for_track(&mut self, track: usize) {
        self.open = true;
        self.target_track = Some(track);
        self.query.clear();
        self.refresh();
    }

    pub fn close(&mut self) {
        self.open = false;
        self.target_track = None;
        self.query.clear();
        self.selected = 0;
    }

    /// Devuelve la entrada elegida al pulsar Enter. El llamador decide cómo actuar.
    pub fn handle_key(&mut self, key: &str, text: bool) -> Option<&'static CatalogEntry> {
        match key {
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
            "enter" => return self.results.get(self.selected).copied(),
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
        self.results = catalog::search(&self.query);
        self.selected = 0;
    }
}

impl DawApp {
    pub(super) fn open_palette(&mut self) {
        self.open_palette_for_track(self.cursor_row);
    }

    pub(super) fn open_palette_for_track(&mut self, track: usize) {
        let track = track.min(self.tracks.len() - 1);
        self.cursor_row = track;
        self.palette.open_for_track(track);
        self.status_message = format!("Buscador · añadir a la pista {}", self.tracks[track].name);
    }

    pub(super) fn handle_palette_key(&mut self, key: &str, text: bool) {
        if key == "escape" {
            self.palette.close();
            self.status_message = "Buscador cerrado".into();
            return;
        }
        if let Some(entry) = self.palette.handle_key(key, text) {
            self.activate_catalog_entry(entry);
        } else if key == "enter" {
            self.status_message = "El buscador no tiene resultados".into();
        }
    }

    pub(super) fn activate_catalog_entry(&mut self, entry: &'static CatalogEntry) {
        let target_track = self.palette.target_track();
        self.palette.close();
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
                self.execute_command(command);
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
        if entry.id != "instrument.piano" {
            self.status_message = format!("{} · instrumento sin implementación", entry.name);
            return;
        }
        let previous = assign_instrument(&mut self.tracks, track, self.instrument.id());
        self.cursor_row = track;
        self.set_center_view(CenterView::Instrument);
        let name = self.tracks[track].name;
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
        assert_eq!(palette.results().len(), 1);
        assert_eq!(palette.results()[0].id, "primitive.volume");
        for _ in 0.."volume".len() {
            palette.handle_key("backspace", true);
        }
        assert_eq!(palette.results().len(), catalog::entries().len());
        let entry = palette
            .handle_key("enter", true)
            .expect("catálogo no vacío");
        assert_eq!(entry.id, "instrument.piano");
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
