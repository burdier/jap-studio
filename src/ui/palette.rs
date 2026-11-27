//! Estado del buscador de instrumentos, primitivos, componentes y acciones.
//! La UI solo presenta resultados; el catálogo y las acciones viven fuera de GPUI.
use super::DawApp;
use elevate::catalog::{self, CatalogAction, CatalogEntry, EntryKind};
use gpui::{Context, Window};

mod activate;
mod entry;
mod plugins;
mod tools;
pub(in crate::ui) use entry::PaletteEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum PaletteCategory {
    All,
    Hey,
    Instrument,
    Component,
    Primitive,
    Action,
}

impl PaletteCategory {
    pub fn matches(self, entry: &PaletteEntry) -> bool {
        match self {
            Self::All => true,
            Self::Hey => {
                entry.plugin.is_none()
                    && !entry.is_tool
                    && (entry.id == "instrument.piano"
                        || entry.id == "instrument.sampler"
                        || entry.id == "instrument.synth"
                        || entry.id.starts_with("hay.")
                        || entry.id.starts_with("hey.")
                        || entry
                            .component_id
                            .as_deref()
                            .is_some_and(|cid| cid.starts_with("hay.") || cid.starts_with("hey.")))
            }
            Self::Instrument => entry.kind == EntryKind::Instrument,
            Self::Component => entry.kind == EntryKind::Component,
            Self::Primitive => entry.kind == EntryKind::Primitive,
            Self::Action => entry.kind == EntryKind::Action,
        }
    }
}

pub(super) struct Palette {
    open: bool,
    target_track: Option<usize>,
    pub(super) fx_target: Option<super::fx::FxTarget>,
    category: PaletteCategory,
    query: String,
    selected: usize,
    results: Vec<PaletteEntry>,
    plugins: Vec<catalog::plugins::InstalledInstrument>,
    tools: Vec<catalog::tools::ToolCatalogEntry>,
    components: Vec<(String, String, String)>,
    scan_needed: bool,
    scan_task: Option<gpui::Task<()>>,
}

impl Palette {
    pub const CATEGORIES: [PaletteCategory; 6] = [
        PaletteCategory::All,
        PaletteCategory::Hey,
        PaletteCategory::Instrument,
        PaletteCategory::Component,
        PaletteCategory::Primitive,
        PaletteCategory::Action,
    ];

    pub fn new() -> Self {
        Self {
            open: false,
            target_track: None,
            fx_target: None,
            category: PaletteCategory::All,
            query: String::new(),
            selected: 0,
            results: catalog::search("").into_iter().map(Into::into).collect(),
            plugins: Vec::new(),
            tools: Vec::new(),
            components: vec![
                (
                    "hay.fx.compressor".into(),
                    "Hay Comp".into(),
                    "Compresor estéreo estilo Ableton con Peak/RMS, Soft Knee, Dry/Wet y Sidechain.".into(),
                ),
                (
                    "hay.fx.limiter".into(),
                    "Hay Limiter".into(),
                    "Limitador con Lookahead (1.5 / 3.0 / 6.0 ms), Ceiling y medidor GR.".into(),
                ),
                (
                    "hay.fx.echo".into(),
                    "Hay Echo".into(),
                    "Delay estéreo / Ping-Pong / Mid-Side estilo Ableton con Tempo Sync, filtro, Drive y Wobble.".into(),
                ),
                (
                    "hay.fx.reverb".into(),
                    "Hay Reverb".into(),
                    "Reverb algorítmica estéreo FDN estilo Ableton con Tempo Sync, Pre-Delay, Damping y Freeze.".into(),
                ),
                (
                    "hay.fx.filter".into(),
                    "Hay Filter".into(),
                    "Filtro estéreo LP/HP/BP y EQ con frecuencia y resonancia.".into(),
                ),
                (
                    "hay.fx.double_volume".into(),
                    "Hay Double Volume".into(),
                    "Dos etapas Volume con una macro de ganancia.".into(),
                ),
            ],
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

    pub fn category(&self) -> PaletteCategory {
        self.category
    }

    pub fn set_category(&mut self, category: PaletteCategory) {
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

    pub fn select(&mut self, index: usize) {
        if index < self.results.len() {
            self.selected = index;
        }
    }

    pub fn open_for_track(&mut self, track: usize) {
        self.open = true;
        self.scan_needed = true;
        self.target_track = Some(track);
        self.fx_target = None;
        self.category = PaletteCategory::All;
        self.query.clear();
        self.refresh();
    }

    pub fn close(&mut self) {
        self.open = false;
        self.target_track = None;
        self.fx_target = None;
        self.category = PaletteCategory::All;
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
            .chain(
                self.components
                    .iter()
                    .filter(|(id, name, _)| {
                        let query = self.query.to_ascii_lowercase();
                        id.to_ascii_lowercase().contains(&query)
                            || name.to_ascii_lowercase().contains(&query)
                    })
                    .map(|(id, name, description)| {
                        PaletteEntry::component(id.clone(), name.clone(), description.clone())
                    }),
            )
            .filter(|entry| self.category.matches(entry))
            .collect();
        self.selected = 0;
    }

    pub(in crate::ui) fn replace_components(&mut self, components: Vec<(String, String, String)>) {
        self.components = components;
        self.refresh();
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
        if self.tracks[track].kind == elevate::musical::track::TrackKind::Group {
            self.status_message = "Añade instrumentos a las pistas hijas del Hilo".into();
            return;
        }
        if entry.id == "instrument.sampler" {
            self.add_sampler(track);
            return;
        }
        if entry.id == "instrument.synth" {
            self.add_synth(track);
            return;
        }
        if entry.id != "instrument.piano" {
            self.status_message = format!("{} · instrumento sin implementación", entry.name);
            return;
        }
        self.add_piano(track);
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
        assert_eq!(
            palette.results().len(),
            catalog::entries().len() + palette.components.len()
        );
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

    #[test]
    fn hey_category_includes_hay_synth_piano_and_sampler() {
        let mut palette = Palette::new();
        palette.open_for_track(0);
        palette.set_category(super::PaletteCategory::Hey);
        let ids: Vec<&str> = palette.results().iter().map(|e| e.id.as_str()).collect();
        assert!(
            ids.contains(&"instrument.synth"),
            "falta Hay Synth en categoría Hey: {ids:?}"
        );
        assert!(
            ids.contains(&"instrument.piano"),
            "falta Hay Piano en categoría Hey"
        );
        assert!(
            ids.contains(&"instrument.sampler"),
            "falta Hay Sampler en categoría Hey"
        );
    }
}
