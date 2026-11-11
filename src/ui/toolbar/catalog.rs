//! Metadatos de la barra y descubrimiento de acciones del catálogo existente.
use super::{ToolbarAction, ToolbarScope};
use elevate::catalog::{self, CatalogAction, EntryKind};

use super::ToolbarAction as A;
use super::ToolbarScope::{PianoRoll as R, Transport as T};

const LOCAL_ACTIONS: &[(ToolbarAction, ToolbarScope, &str, &str)] = &[
    (A::InsertMidiClip, T, "Insertar clip MIDI", "+MIDI"),
    (A::CaptureMidi, T, "Recuperar MIDI retrospectivo", "Captura"),
    (A::DuplicateClip, T, "Duplicar clip", "Duplicar"),
    (A::ToggleClipMute, T, "Alternar mute del clip", "Mute clip"),
    (A::MidiLearn, T, "Alternar MIDI Learn (CC)", "Learn"),
    (A::MidiScan, T, "Escanear USB MIDI", "USB"),
    (A::ToggleBypass, T, "Alternar bypass FX", "Bypass"),
    (A::OpenSettings, T, "Abrir configuración", "Config"),
    (A::Record, T, "Iniciar / detener grabación MIDI", "Grabar"),
    (A::GoToStart, T, "Volver al inicio", "Inicio"),
    (
        A::ToggleController,
        T,
        "Activar / desactivar teclado como MIDI",
        "Teclado MIDI",
    ),
    (
        A::OpenPalette,
        T,
        "Abrir buscador de instrumentos y acciones",
        "Buscador",
    ),
    (A::OpenInstrument, T, "Abrir instrumento", "Instrumento"),
    (
        A::OpenPianoRoll,
        T,
        "Editar clip MIDI en Piano Roll",
        "Piano Roll",
    ),
    (
        A::DetachInstrument,
        T,
        "Separar ventana de instrumento",
        "Separar inst.",
    ),
    (
        A::DockInstrument,
        T,
        "Acoplar ventana de instrumento",
        "Acoplar inst.",
    ),
    (A::ShowShortcuts, T, "Mostrar atajos de teclado", "Atajos"),
    (
        A::TrackMute,
        T,
        "Alternar mute de pista seleccionada",
        "Mute pista",
    ),
    (
        A::TrackSolo,
        T,
        "Alternar solo de pista seleccionada",
        "Solo",
    ),
    (
        A::TrackRecord,
        T,
        "Alternar grabación de pista seleccionada",
        "Grabar pista",
    ),
    (A::GainUp, T, "Subir volumen global", "Vol +"),
    (A::GainDown, T, "Bajar volumen global", "Vol −"),
    (
        A::TimelineZoomIn,
        T,
        "Arrangement: acercar al cursor",
        "Zoom +",
    ),
    (
        A::TimelineZoomOut,
        T,
        "Arrangement: alejar del cursor",
        "Zoom −",
    ),
    (
        A::PresetNext,
        T,
        "Instrumento: preset siguiente",
        "Preset +",
    ),
    (
        A::PresetPrevious,
        T,
        "Instrumento: preset anterior",
        "Preset −",
    ),
    (A::BankNext, T, "Instrumento: banco siguiente", "Banco +"),
    (A::BankPrevious, T, "Instrumento: banco anterior", "Banco −"),
    (A::RollSelect, R, "Piano Roll: seleccionar (V)", "Sel"),
    (A::RollDraw, R, "Piano Roll: dibujar nota", "Draw"),
    (A::RollErase, R, "Piano Roll: borrar nota", "Erase"),
    (
        A::RollPaint,
        R,
        "Piano Roll: pintar nota o velocity",
        "Paint",
    ),
    (A::RollMute, R, "Piano Roll: silenciar nota", "Mute"),
    (A::RollQuantize, R, "Piano Roll: cuantizar notas (Q)", "Q"),
    (A::RollHumanize, R, "Piano Roll: humanizar notas (H)", "H"),
    (A::RollSubdivide, R, "Piano Roll: subdividir notas según rejilla", "Subdividir"),
    (A::RollGridFiner, R, "Piano Roll: rejilla más fina (Alt+1)", "Grid +"),
    (A::RollGridCoarser, R, "Piano Roll: rejilla más gruesa (Alt+2)", "Grid −"),
    (A::RollCut, R, "Piano Roll: cortar nota", "Cortar"),
    (A::RollCopy, R, "Piano Roll: copiar nota", "Copiar"),
    (A::RollPaste, R, "Piano Roll: pegar nota", "Pegar"),
    (A::RollBox, R, "Piano Roll: selección rectangular", "Caja"),
    (A::RollLength, R, "Piano Roll: redimensionar nota", "Largo"),
    (A::RollGhostNotes, R, "Piano Roll: notas fantasma", "Ghost"),
    (A::RollFollow, R, "Piano Roll: seguir playhead", "Follow"),
    (
        A::RollVelocities,
        R,
        "Piano Roll: carril de velocidades",
        "Vel",
    ),
    (
        A::RollHighlightScale,
        R,
        "Piano Roll: resaltar escala",
        "Escala",
    ),
    (
        A::RollToggleWindow,
        R,
        "Piano Roll: separar / acoplar ventana",
        "Ventana",
    ),
    (
        A::RollResetView,
        R,
        "Piano Roll: ajustar vista (0)",
        "Ajustar",
    ),
    (
        A::RollClose,
        R,
        "Piano Roll: cerrar / volver al panel",
        "Cerrar",
    ),
    (
        A::RollZoomIn,
        R,
        "Piano Roll: aumentar ancho de notas",
        "Ancho +",
    ),
    (
        A::RollZoomOut,
        R,
        "Piano Roll: reducir ancho de notas",
        "Ancho −",
    ),
    (
        A::RollTaller,
        R,
        "Piano Roll: aumentar altura de filas",
        "Alto +",
    ),
    (
        A::RollShorter,
        R,
        "Piano Roll: reducir altura de filas",
        "Alto −",
    ),
];

impl ToolbarAction {
    pub fn from_catalog(id: &'static str) -> Self {
        match id {
            "action.clip.duplicate" => Self::DuplicateClip,
            "action.clip.mute" => Self::ToggleClipMute,
            _ => Self::Catalog(id),
        }
    }

    fn from_roll_catalog(id: &'static str) -> Self {
        match id {
            "action.midi.tool.select" => Self::RollSelect,
            "action.midi.tool.draw" => Self::RollDraw,
            "action.midi.tool.erase" => Self::RollErase,
            "action.midi.tool.paint" => Self::RollPaint,
            "action.midi.tool.mute" => Self::RollMute,
            "action.midi.tool.box" => Self::RollBox,
            "action.midi.tool.length" => Self::RollLength,
            "action.midi.copy" => Self::RollCopy,
            "action.midi.cut" => Self::RollCut,
            "action.midi.paste" => Self::RollPaste,
            "action.midi.quantize" => Self::RollQuantize,
            "action.midi.humanize" => Self::RollHumanize,
            "action.midi.subdivide" => Self::RollSubdivide,
            "action.midi.grid.finer" => Self::RollGridFiner,
            "action.midi.grid.coarser" => Self::RollGridCoarser,
            "action.midi.ghost" => Self::RollGhostNotes,
            "action.midi.follow" => Self::RollFollow,
            "action.midi.velocities" => Self::RollVelocities,
            "action.midi.scale.highlight" => Self::RollHighlightScale,
            "action.midi.window" => Self::RollToggleWindow,
            "action.midi.close" => Self::RollClose,
            "action.midi.view.reset" => Self::RollResetView,
            "action.midi.view.zoom.in" => Self::RollZoomIn,
            "action.midi.view.zoom.out" => Self::RollZoomOut,
            "action.midi.view.taller" => Self::RollTaller,
            "action.midi.view.shorter" => Self::RollShorter,
            _ => Self::RollCatalog(id),
        }
    }

    pub fn available(scope: ToolbarScope, query: &str) -> Vec<Self> {
        let mut actions: Vec<_> = LOCAL_ACTIONS
            .iter()
            .filter(|(_, target, label, _)| *target == scope && matches_query(label, query))
            .map(|(action, _, _, _)| *action)
            .collect();
        {
            for entry in catalog::search(query) {
                if entry.kind == EntryKind::Action
                    && (scope == ToolbarScope::Transport || entry.id.starts_with("action.midi."))
                    && matches!(entry.action, CatalogAction::Invoke(_))
                {
                    let action = if scope == ToolbarScope::PianoRoll { Self::from_roll_catalog(entry.id) } else { Self::from_catalog(entry.id) };
                    if !actions.contains(&action) {
                        actions.push(action);
                    }
                }
            }
        }
        // Exportación visible al abrir el selector, incluso antes de escribir un filtro.
        actions.sort_by_key(|action| match action {
            Self::Catalog("action.export") => 0,
            Self::Catalog("action.export.settings") => 1,
            _ => 2,
        });
        actions
    }

    pub fn scope(self) -> ToolbarScope {
        match self {
            Self::RollCatalog(_) => ToolbarScope::PianoRoll,
            Self::Catalog(_) => ToolbarScope::Transport,
            _ => self.metadata().1,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Catalog(id) | Self::RollCatalog(id) => {
                catalog::find(id).map_or("Acción no disponible", |entry| entry.name)
            }
            _ => self.metadata().2,
        }
    }
    pub fn short_tag(self) -> &'static str {
        match self {
            Self::Catalog(id) | Self::RollCatalog(id) => match id {
                "action.export" => "Exportar",
                "action.export.settings" => "Exportación",
                "action.play" => "Play",
                "action.stop" => "Stop",
                "action.pattern" => "Patrón",
                "action.arrangement" => "Arreglo",
                "action.mixer" => "Mixer",
                "action.reload" => "Recargar",
                "action.clip.materialize" => "Materialize",
                "action.clip.split" => "Dividir",
                "action.clip.restore_trim" => "Restaurar",
                "action.clip.lock" => "Bloquear",
                "action.clip.gain.down" => "Clip −3dB",
                "action.clip.gain.up" => "Clip +3dB",
                "action.clip.gain.reset" => "Clip 0dB",
                "action.clip.gain" => "Ganancia",
                "action.clip.rename" => "Nombre",
                "action.clip.color" => "Color",
                "action.clip.track.previous" => "Pista ↑",
                "action.clip.track.next" => "Pista ↓",
                "action.clip.delete" => "Eliminar",
                _ => self.label(),
            },
            _ => self.metadata().3,
        }
    }
    fn metadata(self) -> &'static (Self, ToolbarScope, &'static str, &'static str) {
        LOCAL_ACTIONS
            .iter()
            .find(|(action, _, _, _)| *action == self)
            .expect("acción local registrada")
    }
}

fn matches_query(label: &str, query: &str) -> bool {
    let normalize = |s: &str| {
        s.to_lowercase()
            .replace('á', "a")
            .replace('é', "e")
            .replace('í', "i")
            .replace('ó', "o")
            .replace('ú', "u")
    };
    normalize(label).contains(&normalize(query.trim()))
}
