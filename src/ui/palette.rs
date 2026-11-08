//! Estado del buscador de instrumentos, primitivos, componentes y acciones.
//! La UI solo presenta resultados; el catálogo y las acciones viven fuera de GPUI.
use super::DawApp;
use elevate::catalog::{self, CatalogAction, CatalogEntry, EntryKind};

pub(super) struct Palette {
    open: bool,
    query: String,
    selected: usize,
    results: Vec<&'static CatalogEntry>,
}

impl Palette {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            selected: 0,
            results: catalog::search(""),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
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

    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.refresh();
    }

    pub fn close(&mut self) {
        self.open = false;
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
        self.palette.open();
        self.status_message = "Buscador · instrumentos, primitivos, componentes y acciones".into();
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
        self.palette.close();
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
}

#[cfg(test)]
mod tests {
    use super::Palette;
    use elevate::catalog;

    #[test]
    fn palette_filters_by_accent_insensitive_query_and_resets_selection() {
        let mut palette = Palette::new();
        palette.open();
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
        palette.open();
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
}
