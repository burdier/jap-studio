//! Catálogo headless de instrumentos, primitivos, componentes y acciones.
//! No conoce GPUI, CPAL ni instancias: la UI consulta y traduce las acciones.
use std::fmt;

mod entry;
pub mod plugins;
pub mod tools;
pub use entry::{CatalogAction, CatalogEntry, OwnedCatalogEntry};

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

const fn piano_action(id: &'static str, name: &'static str, command: &'static str) -> CatalogEntry {
    CatalogEntry {
        id,
        kind: EntryKind::Action,
        name,
        summary: "Acción del piano roll; comparte controlador con sus botones y atajos",
        keywords: &["midi", "piano roll"],
        action: CatalogAction::Invoke(command),
    }
}

pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        id: "action.midi.tools.reload",
        kind: EntryKind::Action,
        name: "Lua · recargar herramientas",
        summary: "Leer paquetes MIDI distribuidos y del usuario; conserva el registro anterior si hay errores",
        keywords: &["lua", "paquetes", "herramientas", "reload"],
        action: CatalogAction::Invoke("midi.tools.reload"),
    },
    CatalogEntry { id: "action.note.edit", kind: EntryKind::Action, name: "Nota general…", summary: "Crear o editar el apunte general persistente", keywords: &["nota", "apunte", "note", "escribir", "texto"], action: CatalogAction::Invoke("note.edit") },
    CatalogEntry { id: "action.vim.toggle", kind: EntryKind::Action, name: "Vim · activar/desactivar", summary: "Navegación modal opcional; preferencia guardada en Configuración → Atajos", keywords: &["vim", "teclado", "normal", "insert", "toggle"], action: CatalogAction::Invoke("vim.toggle") },
    CatalogEntry { id: "action.view.maximize", kind: EntryKind::Action, name: "Ampliar / restaurar panel activo", summary: "Oculta los otros paneles; conserva tamaños y contenido", keywords: &["maximizar", "fullscreen", "foco", "ventana", "piano roll"], action: CatalogAction::Invoke("view.maximize") },
    CatalogEntry { id: "action.fx.vst3.load", kind: EntryKind::Action, name: "Cargar efecto VST3…", summary: "Inserta un efecto estéreo en Track FX", keywords: &["plugin", "vst3", "efecto"], action: CatalogAction::Invoke("fx.vst3.load") },
    CatalogEntry { id: "action.fx.vst3.editor", kind: EntryKind::Action, name: "Abrir editor del efecto VST3", summary: "Interfaz original del efecto seleccionado", keywords: &["plugin", "vst3", "editor"], action: CatalogAction::Invoke("fx.vst3.editor") },
    CatalogEntry { id: "action.instrument.vst3.editor", kind: EntryKind::Action, name: "Abrir editor original del VST3", summary: "La interfaz que proporciona el plugin, mediante IPlugView", keywords: &["plugin", "pianoteq", "editor"], action: CatalogAction::Invoke("instrument.vst3.editor") },
    CatalogEntry { id: "action.instrument.vst3.editor.close", kind: EntryKind::Action, name: "Cerrar editor del VST3", summary: "Cierra la ventana y conserva el instrumento procesando audio", keywords: &["plugin", "editor"], action: CatalogAction::Invoke("instrument.vst3.editor.close") },
    CatalogEntry { id: "action.instrument.vst3.load", kind: EntryKind::Action, name: "Cargar instrumento VST3…", summary: "Carga un bundle Linux o yabridge en la pista seleccionada", keywords: &["plugin", "pianoteq", "wine"], action: CatalogAction::Invoke("instrument.vst3.load") },
    CatalogEntry { id: "action.instrument.vst3.remove", kind: EntryKind::Action, name: "Quitar instrumento VST3", summary: "Libera el instrumento externo de la pista seleccionada", keywords: &["plugin", "pianoteq", "wine"], action: CatalogAction::Invoke("instrument.vst3.remove") },
    CatalogEntry { id: "action.audio.settings", kind: EntryKind::Action, name: "Audio: configuración", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.settings") },
    CatalogEntry { id: "action.audio.devices", kind: EntryKind::Action, name: "Audio: listar interfaces", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.devices") },
    CatalogEntry { id: "action.audio.output", kind: EntryKind::Action, name: "Audio: elegir salida…", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.output") },
    CatalogEntry { id: "action.audio.input", kind: EntryKind::Action, name: "Audio: elegir entrada…", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.input") },
    CatalogEntry { id: "action.audio.driver", kind: EntryKind::Action, name: "Audio: elegir driver…", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.driver") },
    CatalogEntry { id: "action.audio.rate", kind: EntryKind::Action, name: "Audio: frecuencia de muestreo…", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.rate") },
    CatalogEntry { id: "action.audio.buffer", kind: EntryKind::Action, name: "Audio: tamaño del buffer…", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.buffer") },
    CatalogEntry { id: "action.audio.output.channel", kind: EntryKind::Action, name: "Audio: pareja de salida…", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.output.channel") },
    CatalogEntry { id: "action.audio.input.channel", kind: EntryKind::Action, name: "Audio: canal de entrada…", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.input.channel") },
    CatalogEntry { id: "action.audio.apply", kind: EntryKind::Action, name: "Audio: aplicar y guardar", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.apply") },
    CatalogEntry { id: "action.audio.revert", kind: EntryKind::Action, name: "Audio: revertir preferencias", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.revert") },
    CatalogEntry { id: "action.audio.test.output", kind: EntryKind::Action, name: "Audio: tono de prueba", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.test.output") },
    CatalogEntry { id: "action.audio.test.input", kind: EntryKind::Action, name: "Audio: medidor de entrada", summary: "Preferencias de interfaz de audio compartidas por todos los reproductores", keywords: &["audio", "device", "interfaz", "latencia"], action: CatalogAction::Invoke("audio.test.input") },
    piano_action(
        "action.midi.lane.select",
        "MIDI: seleccionar carril por nombre…",
        "midi.lane.select",
    ),
    piano_action("action.midi.pan", "MIDI: panorama por nota…", "midi.pan"),
    piano_action(
        "action.midi.release_velocity",
        "MIDI: release velocity…",
        "midi.release_velocity",
    ),
    piano_action(
        "action.midi.probability",
        "MIDI: probabilidad por nota…",
        "midi.probability",
    ),
    piano_action("action.midi.cc", "MIDI: editar controlador CC…", "midi.cc"),
    piano_action(
        "action.midi.pitch",
        "MIDI: editar pitch bend…",
        "midi.pitch",
    ),
    piano_action(
        "action.midi.pressure",
        "MIDI: editar aftertouch…",
        "midi.pressure",
    ),
    piano_action(
        "action.midi.control.remove",
        "MIDI: eliminar punto de control",
        "midi.control.remove",
    ),
    piano_action(
        "action.midi.lane",
        "MIDI: elegir carril de expresión",
        "midi.lane",
    ),
    piano_action(
        "action.midi.track.solo",
        "MIDI: solo de la pista del clip",
        "midi.track.solo",
    ),
    piano_action(
        "action.midi.clip.channel",
        "Canal del clip MIDI…",
        "midi.clip.channel",
    ),
    piano_action(
        "action.midi.clip.rename",
        "Renombrar clip MIDI…",
        "midi.clip.rename",
    ),
    piano_action(
        "action.midi.clip.length",
        "Longitud del clip MIDI…",
        "midi.clip.length",
    ),
    piano_action(
        "action.midi.draw.length",
        "Duración de dibujo MIDI…",
        "midi.draw.length",
    ),
    piano_action(
        "action.midi.grid",
        "Resolución de rejilla MIDI…",
        "midi.grid",
    ),
    piano_action("action.midi.scale.root", "Tónica MIDI…", "midi.scale.root"),
    piano_action("action.midi.scale.mode", "Escala MIDI…", "midi.scale.mode"),
    piano_action(
        "action.midi.humanize.amount",
        "Cantidad de humanización…",
        "midi.humanize.amount",
    ),
    piano_action(
        "action.midi.tool.select",
        "MIDI: Pointer / seleccionar",
        "midi.tool.select",
    ),
    piano_action("action.midi.tool.draw", "MIDI: dibujar", "midi.tool.draw"),
    piano_action("action.midi.tool.erase", "MIDI: borrar", "midi.tool.erase"),
    piano_action("action.midi.tool.paint", "MIDI: pintar", "midi.tool.paint"),
    piano_action(
        "action.midi.tool.mute",
        "MIDI: silenciar nota",
        "midi.tool.mute",
    ),
    piano_action(
        "action.midi.tool.box",
        "MIDI: selección rectangular",
        "midi.tool.box",
    ),
    piano_action(
        "action.midi.tool.length",
        "MIDI: redimensionar nota",
        "midi.tool.length",
    ),
    piano_action("action.midi.copy", "MIDI: copiar selección", "midi.copy"),
    piano_action("action.midi.cut", "MIDI: cortar selección", "midi.cut"),
    piano_action("action.midi.paste", "MIDI: pegar selección", "midi.paste"),
    piano_action("action.midi.tool.pointer", "MIDI: Pointer (doble clic crea / borra)", "midi.tool.pointer"),
    piano_action("action.midi.select.all", "MIDI: seleccionar todas las notas", "midi.select.all"),
    piano_action("action.midi.select.clear", "MIDI: despejar selección", "midi.select.clear"),
    piano_action("action.midi.select.region", "MIDI: seleccionar región de notas", "midi.select.region"),
    piano_action("action.midi.delete.selection", "MIDI: eliminar notas seleccionadas", "midi.delete.selection"),
    piano_action("action.midi.delete.region", "MIDI: eliminar región de notas", "midi.delete.region"),
    piano_action(
        "action.midi.ghost",
        "MIDI: alternar notas fantasma",
        "midi.ghost",
    ),
    piano_action("action.midi.follow", "MIDI: seguir playhead", "midi.follow"),
    piano_action(
        "action.midi.velocities",
        "MIDI: mostrar carril de expresión",
        "midi.velocities",
    ),
    piano_action(
        "action.midi.scale.highlight",
        "MIDI: resaltar escala",
        "midi.scale.highlight",
    ),
    piano_action(
        "action.midi.scale.keyboard",
        "MIDI: escala en teclado",
        "midi.scale.keyboard",
    ),
    piano_action(
        "action.midi.note.names",
        "MIDI: mostrar nombres de notas",
        "midi.note.names",
    ),
    piano_action(
        "action.midi.clip.previous",
        "MIDI: clip anterior",
        "midi.clip.previous",
    ),
    piano_action(
        "action.midi.clip.next",
        "MIDI: clip siguiente",
        "midi.clip.next",
    ),
    piano_action(
        "action.midi.clip.select",
        "MIDI: elegir clip",
        "midi.clip.select",
    ),
    piano_action(
        "action.midi.clip.mute",
        "MIDI: silenciar clip",
        "midi.clip.mute",
    ),
    piano_action(
        "action.midi.clip.loop",
        "MIDI: repetir clip",
        "midi.clip.loop",
    ),
    piano_action(
        "action.midi.clip.color",
        "MIDI: color del clip",
        "midi.clip.color",
    ),
    piano_action(
        "action.midi.view.reset",
        "MIDI: ajustar vista",
        "midi.view.reset",
    ),
    piano_action(
        "action.midi.view.zoom.in",
        "MIDI: ampliar horizontal",
        "midi.view.zoom.in",
    ),
    piano_action(
        "action.midi.view.zoom.out",
        "MIDI: reducir horizontal",
        "midi.view.zoom.out",
    ),
    piano_action(
        "action.midi.view.taller",
        "MIDI: ampliar vertical",
        "midi.view.taller",
    ),
    piano_action(
        "action.midi.view.shorter",
        "MIDI: reducir vertical",
        "midi.view.shorter",
    ),
    piano_action(
        "action.midi.window",
        "MIDI: separar/acoplar ventana",
        "midi.window",
    ),
    piano_action("action.midi.close", "MIDI: cerrar editor", "midi.close"),
    piano_action(
        "action.midi.settings",
        "MIDI: abrir configuración",
        "midi.settings",
    ),
    piano_action(
        "action.midi.inspector.clip",
        "MIDI: propiedades del clip",
        "midi.inspector.clip",
    ),
    piano_action(
        "action.midi.inspector.notes",
        "MIDI: herramientas de notas",
        "midi.inspector.notes",
    ),
    piano_action(
        "action.midi.inspector.scale",
        "MIDI: propiedades de escala",
        "midi.inspector.scale",
    ),
    piano_action(
        "action.midi.inspector.view",
        "MIDI: opciones de vista",
        "midi.inspector.view",
    ),
    CatalogEntry {
        id: "action.tempo.set",
        kind: EntryKind::Action,
        name: "Cambiar tempo",
        summary: "Tempo del proyecto en BPM",
        keywords: &["tempo", "bpm", "velocidad"],
        action: CatalogAction::Invoke("tempo.set"),
    },
    CatalogEntry {
        id: "action.meter.set",
        kind: EntryKind::Action,
        name: "Cambiar compás",
        summary: "Numerador y denominador del compás",
        keywords: &["compas", "meter", "time signature"],
        action: CatalogAction::Invoke("meter.set"),
    },
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
        id: "action.midi.script",
        kind: EntryKind::Action,
        name: "Ejecutar script MIDI…",
        summary: "Transformar las notas del clip activo con un archivo Lua",
        keywords: &["midi", "lua", "script", "notas", "piano roll"],
        action: CatalogAction::Invoke("midi.script"),
    },
    CatalogEntry {
        id: "action.track.script",
        kind: EntryKind::Action,
        name: "Ejecutar script de pistas…",
        summary: "Crear y configurar pistas con un lote Lua validado",
        keywords: &["lua", "pistas", "lote", "automatizar", "archivo"],
        action: CatalogAction::Invoke("track.script"),
    },
    CatalogEntry {
        id: "action.midi.quantize",
        kind: EntryKind::Action,
        name: "Cuantizar notas…",
        summary: "Lua: ajustar notas a la rejilla elegida · Q",
        keywords: &["midi", "lua", "quantize", "rejilla", "piano roll"],
        action: CatalogAction::Invoke("midi.quantize"),
    },
    CatalogEntry {
        id: "action.midi.humanize",
        kind: EntryKind::Action,
        name: "Humanizar notas…",
        summary: "Lua: variar tiempo y velocity con intensidad configurable · H",
        keywords: &["midi", "lua", "humanize", "velocity", "piano roll"],
        action: CatalogAction::Invoke("midi.humanize"),
    },
    CatalogEntry {
        id: "action.midi.subdivide",
        kind: EntryKind::Action,
        name: "Subdividir notas según rejilla…",
        summary: "Lua: convertir la selección en un roll · Alt+rueda / Shift+pellizco",
        keywords: &[
            "midi",
            "lua",
            "split",
            "roll",
            "ratchet",
            "rejilla",
            "piano roll",
        ],
        action: CatalogAction::Invoke("midi.subdivide"),
    },
    CatalogEntry {
        id: "action.midi.gate",
        kind: EntryKind::Action,
        name: "Gate MIDI…",
        summary: "Lua: cambiar duración de notas seleccionadas",
        keywords: &["midi", "lua", "gate", "duración", "piano roll"],
        action: CatalogAction::Invoke("midi.gate"),
    },
    CatalogEntry {
        id: "action.midi.swing",
        kind: EntryKind::Action,
        name: "Swing MIDI…",
        summary: "Lua: desplazar subdivisiones impares de la selección",
        keywords: &["midi", "lua", "swing", "piano roll"],
        action: CatalogAction::Invoke("midi.swing"),
    },
    CatalogEntry {
        id: "action.midi.scale.clamp",
        kind: EntryKind::Action,
        name: "Ajustar a la escala activa",
        summary: "Lua: mover notas seleccionadas al tono más cercano de la escala",
        keywords: &["midi", "lua", "scale", "clamp", "escala", "piano roll"],
        action: CatalogAction::Invoke("midi.scale.clamp"),
    },
    CatalogEntry {
        id: "action.midi.velocity",
        kind: EntryKind::Action,
        name: "Fijar velocity…",
        summary: "Lua: fijar velocity de las notas seleccionadas",
        keywords: &["midi", "lua", "velocity", "dinámica", "piano roll"],
        action: CatalogAction::Invoke("midi.velocity"),
    },
    CatalogEntry {
        id: "action.midi.random",
        kind: EntryKind::Action,
        name: "Random MIDI…",
        summary: "Lua: variar tiempo y velocity de forma reproducible",
        keywords: &["midi", "lua", "random", "aleatorio", "piano roll"],
        action: CatalogAction::Invoke("midi.random"),
    },
    CatalogEntry {
        id: "action.midi.grid.finer",
        kind: EntryKind::Action,
        name: "Rejilla MIDI más fina",
        summary: "1/8 → 1/16 · Alt+1; conserva las notas",
        keywords: &["midi", "grid", "rejilla", "snap", "piano roll"],
        action: CatalogAction::Invoke("midi.grid.finer"),
    },
    CatalogEntry {
        id: "action.midi.grid.coarser",
        kind: EntryKind::Action,
        name: "Rejilla MIDI más gruesa",
        summary: "1/16 → 1/8 · Alt+2; conserva las notas",
        keywords: &["midi", "grid", "rejilla", "snap", "piano roll"],
        action: CatalogAction::Invoke("midi.grid.coarser"),
    },
    CatalogEntry {
        id: "action.midi.script.accent",
        kind: EntryKind::Action,
        name: "Lua · acentuar notas",
        summary: "Ejemplo Lua: +12 de velocity a las notas seleccionadas",
        keywords: &["midi", "lua", "script", "velocity", "acento"],
        action: CatalogAction::Invoke("midi.script.accent"),
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
        id: "hay.spectrum",
        kind: EntryKind::Component,
        name: "Hay Spectrum",
        summary: "Analizador en Track FX · espectro, onda, espectrogramas y nivel · insertable entre VST3",
        keywords: &["spectrum", "espectro", "spectrogram", "analizador", "meter", "efecto", "fft"],
        action: CatalogAction::Invoke("fx.spectrum"),
    },
    CatalogEntry {
        id: "action.fx.spectrum.next", kind: EntryKind::Action, name: "Spectrum · vista siguiente",
        summary: "Cambia la vista del nodo Spectrum seleccionado", keywords: &["spectrum", "espectrograma", "visualizador"],
        action: CatalogAction::Invoke("fx.spectrum.next"),
    },
    CatalogEntry {
        id: "action.fx.spectrum.previous", kind: EntryKind::Action, name: "Spectrum · vista anterior",
        summary: "Cambia la vista del nodo Spectrum seleccionado", keywords: &["spectrum", "visualizador"],
        action: CatalogAction::Invoke("fx.spectrum.previous"),
    },
    CatalogEntry {
        id: "action.fx.spectrum.reset", kind: EntryKind::Action, name: "Spectrum · limpiar historial",
        summary: "Borra solo las mediciones del nodo seleccionado", keywords: &["spectrum", "reset", "historial"],
        action: CatalogAction::Invoke("fx.spectrum.reset"),
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
