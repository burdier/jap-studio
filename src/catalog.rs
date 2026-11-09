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

impl CatalogAction {
    /// Some declara que la invocación visual debe recoger argumentos antes de ejecutarse.
    pub fn parameters(self) -> Option<&'static crate::actions::ActionSpec> {
        match self {
            Self::Invoke(command) => crate::actions::find(command),
            Self::Unavailable(_) => None,
        }
    }
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
        id: "instrument.sampler",
        kind: EntryKind::Instrument,
        name: "Hay Sampler",
        summary: "Sampler sfizz · Classic / One-Shot · filtro y ADSR",
        keywords: &["sample", "sampler", "sfizz", "wav", "instrumento"],
        action: CatalogAction::Invoke("sampler"),
    },
    CatalogEntry {
        id: "action.fx.show",
        kind: EntryKind::Action,
        name: "Mostrar Track FX",
        summary: "Administrar la cadena de la pista seleccionada",
        keywords: &["fx", "plugin", "efecto", "cadena"],
        action: CatalogAction::Invoke("fx.show"),
    },
    CatalogEntry {
        id: "action.fx.add",
        kind: EntryKind::Action,
        name: "Add Effect…",
        summary: "Abrir el Add actual en el contexto FX seleccionado",
        keywords: &["fx", "plugin", "efecto", "cadena"],
        action: CatalogAction::Invoke("fx.add"),
    },
    CatalogEntry {
        id: "action.fx.serial",
        kind: EntryKind::Action,
        name: "Add Serial Chain",
        summary: "Insertar un contenedor de procesamiento secuencial",
        keywords: &["fx", "plugin", "efecto", "cadena"],
        action: CatalogAction::Invoke("fx.serial"),
    },
    CatalogEntry {
        id: "action.fx.parallel",
        kind: EntryKind::Action,
        name: "Add Parallel Chain",
        summary: "Insertar ramas con la misma entrada y sumar sus salidas",
        keywords: &["fx", "plugin", "efecto", "cadena"],
        action: CatalogAction::Invoke("fx.parallel"),
    },
    CatalogEntry {
        id: "action.fx.rename",
        kind: EntryKind::Action,
        name: "Renombrar FX…",
        summary: "Renombrar efecto o contenedor",
        keywords: &["fx", "plugin", "efecto", "cadena"],
        action: CatalogAction::Invoke("fx.rename"),
    },
    CatalogEntry {
        id: "action.fx.parameters",
        kind: EntryKind::Action,
        name: "Parámetros FX…",
        summary: "Editar la instancia de efecto seleccionada",
        keywords: &["fx", "plugin", "efecto", "cadena"],
        action: CatalogAction::Invoke("fx.parameters"),
    },
    CatalogEntry {
        id: "action.fx.duplicate",
        kind: EntryKind::Action,
        name: "Duplicar FX",
        summary: "Copiar el nodo y sus hijos con identidades nuevas",
        keywords: &["fx", "plugin", "efecto", "cadena"],
        action: CatalogAction::Invoke("fx.duplicate"),
    },
    CatalogEntry {
        id: "action.fx.bypass",
        kind: EntryKind::Action,
        name: "Bypass / Enable FX",
        summary: "Activar o desactivar el nodo seleccionado",
        keywords: &["fx", "plugin", "efecto", "cadena"],
        action: CatalogAction::Invoke("fx.bypass"),
    },
    CatalogEntry {
        id: "action.fx.remove",
        kind: EntryKind::Action,
        name: "Quitar FX",
        summary: "Retirar el nodo y sus hijos de la cadena",
        keywords: &["fx", "plugin", "efecto", "cadena"],
        action: CatalogAction::Invoke("fx.remove"),
    },
    CatalogEntry {
        id: "action.explorer.toggle",
        kind: EntryKind::Action,
        name: "Mostrar / ocultar explorador",
        summary: "Bibliotecas de audio y MIDI a la izquierda del Arrangement · e en Normal",
        keywords: &["explorer", "biblioteca", "carpeta", "archivo", "browser"],
        action: CatalogAction::Invoke("explorer.toggle"),
    },
    CatalogEntry {
        id: "action.explorer.insert",
        kind: EntryKind::Action,
        name: "Insertar archivo del explorador",
        summary: "Archivo seleccionado → pista activa, en el cursor del Arrangement",
        keywords: &["explorer", "importar", "midi", "audio", "wav", "insertar"],
        action: CatalogAction::Invoke("explorer.insert"),
    },
    CatalogEntry {
        id: "action.file.import",
        kind: EntryKind::Action,
        name: "Insertar archivo por ruta…",
        summary: "Importar MIDI o audio en la pista activa y posición del cursor",
        keywords: &["explorer", "importar", "midi", "audio", "wav", "archivo"],
        action: CatalogAction::Invoke("file.import"),
    },
    CatalogEntry {
        id: "action.track.fold",
        kind: EntryKind::Action,
        name: "Plegar / desplegar grupo",
        summary: "Mostrar el grupo como una sola fila o mostrar sus hijos",
        keywords: &["track", "pista", "altura", "grupo"],
        action: CatalogAction::Invoke("track.fold"),
    },
    CatalogEntry {
        id: "action.track.height.compact",
        kind: EntryKind::Action,
        name: "Altura de pistas · compacta",
        summary: "Reducir las pistas seleccionadas a 40 px",
        keywords: &["track", "pista", "altura", "grupo"],
        action: CatalogAction::Invoke("track.height.compact"),
    },
    CatalogEntry {
        id: "action.track.height.default",
        kind: EntryKind::Action,
        name: "Altura de pistas · predeterminada",
        summary: "Restaurar la altura del perfil en las pistas seleccionadas",
        keywords: &["track", "pista", "altura", "grupo"],
        action: CatalogAction::Invoke("track.height.default"),
    },
    CatalogEntry {
        id: "action.track.height.tall",
        kind: EntryKind::Action,
        name: "Altura de pistas · amplia",
        summary: "Ampliar las pistas seleccionadas a 120 px",
        keywords: &["track", "pista", "altura", "grupo"],
        action: CatalogAction::Invoke("track.height.tall"),
    },
    CatalogEntry {
        id: "action.track.height",
        kind: EntryKind::Action,
        name: "Altura de pistas…",
        summary: "Elegir altura visual de las pistas seleccionadas",
        keywords: &["track", "pista", "altura", "grupo"],
        action: CatalogAction::Invoke("track.height"),
    },
    CatalogEntry {
        id: "action.track.insert",
        kind: EntryKind::Action,
        name: "Insertar pista",
        summary: "Crear una pista para audio o MIDI",
        keywords: &["track", "pista", "grupo"],
        action: CatalogAction::Invoke("track.insert"),
    },
    CatalogEntry {
        id: "action.track.insert_sampler",
        kind: EntryKind::Action,
        name: "Insertar sampler",
        summary: "Crear una pista Hay Sampler o abrir la existente · :sa",
        keywords: &["track", "pista", "sample", "sampler", "sfizz", "sa"],
        action: CatalogAction::Invoke("track.insert_sampler"),
    },
    CatalogEntry {
        id: "action.track.delete",
        kind: EntryKind::Action,
        name: "Eliminar pistas",
        summary: "Eliminar selección y sus clips; un grupo incluye sus hijos",
        keywords: &["track", "pista", "grupo"],
        action: CatalogAction::Invoke("track.delete"),
    },
    CatalogEntry {
        id: "action.track.duplicate",
        kind: EntryKind::Action,
        name: "Duplicar pistas",
        summary: "Copiar selección y clips en pistas nuevas",
        keywords: &["track", "pista", "grupo"],
        action: CatalogAction::Invoke("track.duplicate"),
    },
    CatalogEntry {
        id: "action.track.group",
        kind: EntryKind::Action,
        name: "Agrupar pistas",
        summary: "Reunir la selección bajo una pista de grupo",
        keywords: &["track", "pista", "grupo"],
        action: CatalogAction::Invoke("track.group"),
    },
    CatalogEntry {
        id: "action.track.ungroup",
        kind: EntryKind::Action,
        name: "Desagrupar pistas",
        summary: "Retirar el grupo conservando sus pistas hijas",
        keywords: &["track", "pista", "grupo"],
        action: CatalogAction::Invoke("track.ungroup"),
    },
    CatalogEntry {
        id: "action.track.rename",
        kind: EntryKind::Action,
        name: "Renombrar pista",
        summary: "Editar el nombre de la pista actual",
        keywords: &["track", "pista", "grupo"],
        action: CatalogAction::Invoke("track.rename"),
    },
    CatalogEntry {
        id: "action.track.mute",
        kind: EntryKind::Action,
        name: "Silenciar pista",
        summary: "Alternar mute de pista o grupo",
        keywords: &["track", "pista", "grupo"],
        action: CatalogAction::Invoke("track.mute"),
    },
    CatalogEntry {
        id: "action.track.solo",
        kind: EntryKind::Action,
        name: "Solo de pista",
        summary: "Alternar solo de pista o grupo",
        keywords: &["track", "pista", "grupo"],
        action: CatalogAction::Invoke("track.solo"),
    },
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
        id: "action.export",
        kind: EntryKind::Action,
        name: "Exportar audio",
        summary: "Revisar parámetros y exportar el Arrangement; inicia con las preferencias guardadas",
        keywords: &["export", "exportar", "render", "wav", "mp3", "stems"],
        action: CatalogAction::Invoke("export"),
    },
    CatalogEntry {
        id: "action.export.settings",
        kind: EntryKind::Action,
        name: "Configurar exportación de audio",
        summary: "Elegir destino, formatos, calidad, pistas y rango predeterminados",
        keywords: &[
            "export",
            "exportar",
            "audio",
            "configuración",
            "preferencias",
        ],
        action: CatalogAction::Invoke("export.settings"),
    },
    CatalogEntry {
        id: "action.export.cancel",
        kind: EntryKind::Action,
        name: "Cancelar exportación",
        summary: "Detiene el render offline y limpia los archivos temporales",
        keywords: &["export", "cancelar", "render"],
        action: CatalogAction::Invoke("export.cancel"),
    },
    CatalogEntry {
        id: "action.export.help",
        kind: EntryKind::Action,
        name: "Exportación · opciones",
        summary: "Sintaxis de formatos, pistas, rango, frecuencia, profundidad y MP3",
        keywords: &["export", "ayuda", "opciones", "render"],
        action: CatalogAction::Invoke("export.help"),
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
        id: "hay.utility",
        kind: EntryKind::Component,
        name: "Hay Utility",
        summary: "Insertar Utility en la cadena de la pista · ganancia y bypass · :utility abre la salida",
        keywords: &["utility", "utilidad", "efecto", "ganancia", "componente"],
        action: CatalogAction::Invoke("utility"),
    },
    CatalogEntry {
        id: "action.utility.gain",
        kind: EntryKind::Action,
        name: "Utility · ajustar ganancia",
        summary: "Elegir la ganancia de salida en múltiplos de amplitud (0–2×)",
        keywords: &["utility", "ganancia", "gain", "volumen"],
        action: CatalogAction::Invoke("utility.gain"),
    },
    CatalogEntry {
        id: "action.utility.bypass",
        kind: EntryKind::Action,
        name: "Utility · alternar bypass",
        summary: "Bypass del dispositivo de salida, conservando mute y niveles de pista",
        keywords: &["utility", "bypass", "efecto"],
        action: CatalogAction::Invoke("utility.bypass"),
    },
    CatalogEntry {
        id: "action.utility.reset",
        kind: EntryKind::Action,
        name: "Utility · restablecer",
        summary: "Ganancia 1x y bypass desactivado",
        keywords: &["utility", "reset", "ganancia"],
        action: CatalogAction::Invoke("utility.reset"),
    },
    CatalogEntry {
        id: "action.utility.remove",
        kind: EntryKind::Action,
        name: "Utility · retirar",
        summary: "Retira Utility de la salida de audición y restablece sus valores",
        keywords: &["utility", "retirar", "efecto"],
        action: CatalogAction::Invoke("utility.remove"),
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
    CatalogEntry {
        id: "action.clip.duplicate",
        kind: EntryKind::Action,
        name: "Duplicar clip",
        summary: "Duplica el clip bajo el cursor al final del original",
        keywords: &["clip", "duplicar", "copiar", "arrangement"],
        action: CatalogAction::Invoke("clip.duplicate"),
    },
    CatalogEntry {
        id: "action.clip.materialize",
        kind: EntryKind::Action,
        name: "Materialize · consolidar MIDI",
        summary: "Fija el recorte o une los clips MIDI seleccionados conservando silencios",
        keywords: &[
            "clip",
            "midi",
            "materialize",
            "consolidar",
            "unir",
            "commit",
        ],
        action: CatalogAction::Invoke("clip.materialize"),
    },
    CatalogEntry {
        id: "action.clip.split",
        kind: EntryKind::Action,
        name: "Dividir clip en el cursor",
        summary: "Divide el clip bajo el cursor sin modificar su fuente",
        keywords: &["clip", "dividir", "cortar", "split"],
        action: CatalogAction::Invoke("clip.split"),
    },
    CatalogEntry {
        id: "action.clip.restore_trim",
        kind: EntryKind::Action,
        name: "Restaurar recorte del clip",
        summary: "Recupera el contenido recortado del clip bajo el cursor",
        keywords: &["clip", "recorte", "restaurar", "trim"],
        action: CatalogAction::Invoke("clip.restore_trim"),
    },
    CatalogEntry {
        id: "action.clip.mute",
        kind: EntryKind::Action,
        name: "Alternar mute del clip",
        summary: "Silencia o reactiva el clip bajo el cursor",
        keywords: &["clip", "silenciar", "mute"],
        action: CatalogAction::Invoke("clip.mute"),
    },
    CatalogEntry {
        id: "action.clip.lock",
        kind: EntryKind::Action,
        name: "Alternar bloqueo del clip",
        summary: "Bloquea o desbloquea posición y tamaño del clip bajo el cursor",
        keywords: &["clip", "bloquear", "lock"],
        action: CatalogAction::Invoke("clip.lock"),
    },
    CatalogEntry {
        id: "action.clip.gain.down",
        kind: EntryKind::Action,
        name: "Bajar ganancia del clip 3 dB",
        summary: "Reduce la ganancia del clip bajo el cursor",
        keywords: &["clip", "ganancia", "gain", "volumen"],
        action: CatalogAction::Invoke("clip.gain.down"),
    },
    CatalogEntry {
        id: "action.clip.rename",
        kind: EntryKind::Action,
        name: "Renombrar clip…",
        summary: "Abre el editor de nombre del clip bajo el cursor",
        keywords: &["clip", "nombre", "renombrar", "rename"],
        action: CatalogAction::Invoke("clip.rename"),
    },
    CatalogEntry {
        id: "action.clip.gain",
        kind: EntryKind::Action,
        name: "Editar ganancia del clip…",
        summary: "Abre las opciones de ganancia del clip bajo el cursor",
        keywords: &["clip", "ganancia", "gain", "decibelios"],
        action: CatalogAction::Invoke("clip.gain"),
    },
    CatalogEntry {
        id: "action.clip.color",
        kind: EntryKind::Action,
        name: "Cambiar color del clip…",
        summary: "Abre las opciones de color del clip bajo el cursor",
        keywords: &["clip", "color", "apariencia"],
        action: CatalogAction::Invoke("clip.color"),
    },
    CatalogEntry {
        id: "action.clip.gain.reset",
        kind: EntryKind::Action,
        name: "Restablecer ganancia del clip",
        summary: "Vuelve la ganancia del clip bajo el cursor a 0 dB",
        keywords: &["clip", "ganancia", "gain", "volumen", "reset"],
        action: CatalogAction::Invoke("clip.gain.reset"),
    },
    CatalogEntry {
        id: "action.clip.gain.up",
        kind: EntryKind::Action,
        name: "Subir ganancia del clip 3 dB",
        summary: "Aumenta la ganancia del clip bajo el cursor",
        keywords: &["clip", "ganancia", "gain", "volumen"],
        action: CatalogAction::Invoke("clip.gain.up"),
    },
    CatalogEntry {
        id: "action.clip.track.previous",
        kind: EntryKind::Action,
        name: "Mover clip a pista anterior",
        summary: "Mueve el clip desbloqueado bajo el cursor una pista arriba",
        keywords: &["clip", "pista", "mover", "anterior"],
        action: CatalogAction::Invoke("clip.track.previous"),
    },
    CatalogEntry {
        id: "action.clip.track.next",
        kind: EntryKind::Action,
        name: "Mover clip a pista siguiente",
        summary: "Mueve el clip desbloqueado bajo el cursor una pista abajo",
        keywords: &["clip", "pista", "mover", "siguiente"],
        action: CatalogAction::Invoke("clip.track.next"),
    },
    CatalogEntry {
        id: "action.clip.delete",
        kind: EntryKind::Action,
        name: "Eliminar clip",
        summary: "Elimina el clip bajo el cursor y conserva los demás clips",
        keywords: &["clip", "eliminar", "borrar", "delete"],
        action: CatalogAction::Invoke("clip.delete"),
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
