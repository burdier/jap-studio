//! Catálogo headless de instrumentos, primitivos, componentes y acciones.
//! No conoce GPUI, CPAL ni instancias: la UI consulta y traduce las acciones.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Instrument,
    Primitive,
    Component,
    Action,
}

impl EntryKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Instrument => "Instrumento",
            Self::Primitive => "Primitivo",
            Self::Component => "Componente",
            Self::Action => "Acción",
        }
    }
}

impl fmt::Display for EntryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// Acción provisional identificada por un comando estable. El host de acciones
/// sustituirá estos strings cuando exista; el catálogo no importa la UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogAction {
    Invoke(&'static str),
    Unavailable(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub kind: EntryKind,
    pub name: &'static str,
    pub summary: &'static str,
    pub keywords: &'static [&'static str],
    pub action: CatalogAction,
}

pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        id: "instrument.piano",
        kind: EntryKind::Instrument,
        name: "Upright Piano KW",
        summary: "Kawai vertical grabado por Gonzalo y Roberto (FreePats) · CC0 · motor RustySynth",
        keywords: &[
            "piano",
            "upright",
            "kw",
            "kawai",
            "freepats",
            "teclado",
            "keys",
            "soundfont",
            "instrumento",
        ],
        action: CatalogAction::Invoke("piano"),
    },
    CatalogEntry {
        id: "primitive.volume",
        kind: EntryKind::Primitive,
        name: "Volume",
        summary: "Ganancia y bypass · primitivo FunDSP de Elevate",
        keywords: &[
            "volumen", "ganancia", "gain", "bypass", "amplitud", "fundsp",
        ],
        action: CatalogAction::Unavailable(
            "El primitivo existe; instanciarlo desde el buscador llegará con el host de plugins",
        ),
    },
    CatalogEntry {
        id: "component.serial",
        kind: EntryKind::Component,
        name: "Componente serial",
        summary: "Cadena de primitivos con identidad y eventos",
        keywords: &["componente", "cadena", "serial", "nodo", "composicion"],
        action: CatalogAction::Unavailable(
            "El componente existe; insertarlo desde el buscador llegará con el host de plugins",
        ),
    },
    CatalogEntry {
        id: "action.play",
        kind: EntryKind::Action,
        name: "Reproducción",
        summary: "Inicia el contexto actual: clip, patrón o vista",
        keywords: &["play", "reproducir", "iniciar", "transporte"],
        action: CatalogAction::Invoke("play"),
    },
    CatalogEntry {
        id: "action.stop",
        kind: EntryKind::Action,
        name: "Detener",
        summary: "Detiene el audio y libera las notas",
        keywords: &["stop", "parar", "detener", "silencio"],
        action: CatalogAction::Invoke("stop"),
    },
    CatalogEntry {
        id: "action.pattern",
        kind: EntryKind::Action,
        name: "Patrón",
        summary: "Abre el editor y la audición del patrón",
        keywords: &["pattern", "patron", "secuenciador", "pasos"],
        action: CatalogAction::Invoke("pattern"),
    },
    CatalogEntry {
        id: "action.arrangement",
        kind: EntryKind::Action,
        name: "Arrangement",
        summary: "Abre la línea de tiempo y sus carriles",
        keywords: &["arrangement", "arreglo", "timeline", "linea"],
        action: CatalogAction::Invoke("arrangement"),
    },
    CatalogEntry {
        id: "action.mixer",
        kind: EntryKind::Action,
        name: "Mixer",
        summary: "Abre la vista de mezcla provisional",
        keywords: &["mixer", "mezcla", "canales", "faders"],
        action: CatalogAction::Invoke("mixer"),
    },
    CatalogEntry {
        id: "action.reload",
        kind: EntryKind::Action,
        name: "Recargar diseño",
        summary: "Vuelve a leer los tokens de ~/.jaybu.profile",
        keywords: &["reload", "recargar", "diseno", "tema"],
        action: CatalogAction::Invoke("reload"),
    },
];

pub fn entries() -> &'static [CatalogEntry] {
    CATALOG
}

pub fn find(id: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|entry| entry.id == id)
}

/// Filtro simple y sin dependencias. Prioriza coincidencias al inicio del nombre
/// y después en nombre, resumen, ID o palabras clave. Vacío devuelve todo.
pub fn search(query: &str) -> Vec<&'static CatalogEntry> {
    let query = normalize(query);
    if query.is_empty() {
        return CATALOG.iter().collect();
    }
    let mut matches: Vec<_> = CATALOG
        .iter()
        .filter_map(|entry| entry.match_score(&query).map(|score| (score, entry)))
        .collect();
    matches.sort_by(|(left_score, left), (right_score, right)| {
        left_score
            .cmp(right_score)
            .then_with(|| left.kind.label().cmp(right.kind.label()))
            .then_with(|| left.name.cmp(right.name))
    });
    matches.into_iter().map(|(_, entry)| entry).collect()
}

impl CatalogEntry {
    fn match_score(&self, normalized_query: &str) -> Option<u8> {
        let name = normalize(self.name);
        if name.starts_with(normalized_query) {
            return Some(0);
        }
        if name
            .split_whitespace()
            .any(|word| word.starts_with(normalized_query))
        {
            return Some(1);
        }
        if name.contains(normalized_query) {
            return Some(2);
        }
        let haystack = normalize(&format!(
            "{} {} {} {}",
            self.id,
            self.summary,
            self.keywords.join(" "),
            self.kind.label()
        ));
        haystack.contains(normalized_query).then_some(3)
    }
}

fn normalize(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    for character in input.chars() {
        let replacement = match character {
            'á' | 'à' | 'ä' | 'â' | 'ã' => Some('a'),
            'é' | 'è' | 'ë' | 'ê' => Some('e'),
            'í' | 'ì' | 'ï' | 'î' => Some('i'),
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => Some('o'),
            'ú' | 'ù' | 'ü' | 'û' => Some('u'),
            'ñ' => Some('n'),
            'ç' => Some('c'),
            character if character.is_alphanumeric() => {
                Some(character.to_lowercase().next().unwrap_or(character))
            }
            _ => Some(' '),
        };
        let Some(character) = replacement else {
            continue;
        };
        if character == ' ' {
            if !normalized.is_empty() && !normalized.ends_with(' ') {
                normalized.push(' ');
            }
        } else {
            normalized.push(character);
        }
    }
    normalized.trim_end().to_string()
}
