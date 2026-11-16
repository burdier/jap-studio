//! Índice de comandos para el minibuffer; el render y la ejecución usan el mismo estado.
use super::DawApp;
use elevate::catalog::{self, CatalogAction};

type Suggestion = (String, String);

/// Alias y comandos con valores predefinidos que no figuran como entradas independientes.
const KNOWN: &[(&str, &str)] = &[
    ("list device", "Listar interfaces de audio disponibles"),
    ("use device", "Elegir interfaz de audio"),
    ("add", "Abrir buscador de catálogo"),
    ("arr", "Vista Arrangement"),
    ("arrangement", "Vista Arrangement"),
    ("browse", "Abrir buscador"),
    ("capture", "Capturar MIDI tocado"),
    ("cursor fire", "Efecto cursor: fuego"),
    ("cursor neon", "Efecto cursor: neon"),
    ("cursor off", "Sin efecto de cursor"),
    ("cursor sparks", "Efecto cursor: chispas"),
    ("cursor stars", "Efecto cursor: estrellas"),
    ("detach", "Separar instrumento en ventana"),
    ("dock", "Acoplar ventana de instrumento"),
    ("export", "Exportar audio del Arrangement"),
    ("export.cancel", "Cancelar exportación en curso"),
    ("export.help", "Mostrar ayuda de exportación"),
    ("export.settings", "Abrir preferencias de exportación"),
    ("explorer", "Mostrar/ocultar explorador"),
    ("explorer.insert", "Insertar selección del explorador"),
    ("explorer.toggle", "Mostrar/ocultar explorador"),
    ("file.import", "Importar archivo de audio o MIDI"),
    ("fx.components.reload", "Recargar componentes de audio Lua"),
    (
        "fx.parameter",
        "Cambiar un control del FX seleccionado por ID",
    ),
    ("icons", "Selector de iconos de la barra"),
    ("inst", "Abrir editor de instrumento"),
    ("instrument", "Abrir editor de instrumento"),
    ("kbd", "Activar/desactivar teclado MIDI"),
    ("learn", "Activar MIDI Learn"),
    ("materialize", "Materializar selección MIDI"),
    ("meter", "Cambiar compás"),
    ("midi", "Activar/desactivar teclado MIDI"),
    ("midi.gate", "Aplicar duración (Gate) a la selección"),
    ("midi.scale.clamp", "Ajustar selección a Do menor"),
    ("midi.swing", "Aplicar swing a la selección"),
    ("midi-scan", "Buscar controladores MIDI hardware"),
    ("mix", "Vista Mixer"),
    ("mixer", "Vista Mixer"),
    ("pat", "Vista Pattern"),
    ("pattern", "Vista Pattern"),
    ("piano", "Abrir editor de piano"),
    ("transport.seek", "Ir a una posición en negras"),
    ("transport.loop", "Loop de audio: inicio fin / off"),
    ("play", "Iniciar reproducción"),
    ("rec", "Activar grabación MIDI"),
    ("record", "Activar grabación MIDI"),
    ("reload", "Recargar tema y perfil"),
    ("sa", "Insertar pista Hay Sampler"),
    ("sampler", "Abrir Hay Sampler"),
    ("session", "Vista Session"),
    ("stop", "Detener reproducción"),
    ("hilo.layer", "Hilo seleccionado en capas"),
    ("hilo.rr", "Hilo seleccionado en round robin"),
    ("tejer.selection", "Agrupar instrumentos y activar Tejer"),
    ("hilo.off", "Desactivar Hilo seleccionado"),
    (
        "track.template.hilo",
        "Agrupar dos instrumentos existentes en Hilo RR",
    ),
    ("tempo", "Cambiar tempo"),
    ("theme", "Cambiar tema"),
    ("theme ableton-live-dark", "Tema Ableton Live Dark"),
    ("theme dark", "Tema oscuro"),
    ("theme light", "Tema claro"),
    ("toolbar", "Selector de iconos de la barra"),
    ("track.delete", "Eliminar pista seleccionada"),
    ("track.duplicate", "Duplicar pista seleccionada"),
    ("track.fold", "Plegar/desplegar grupo"),
    ("track.group", "Agrupar pistas seleccionadas"),
    ("track.height.compact", "Altura compacta (40 px)"),
    ("track.height.default", "Altura predeterminada"),
    ("track.height.tall", "Altura amplia (120 px)"),
    ("track.insert", "Insertar pista nueva"),
    ("track.insert_sampler", "Insertar pista Hay Sampler"),
    ("track.midi", "Activar/desactivar MIDI de la pista"),
    ("track.mute", "Silenciar/activar pista"),
    ("track.rename", "Renombrar pista"),
    ("track.solo", "Solo de pista"),
    ("track.ungroup", "Desagrupar pistas"),
    ("utility", "Abrir Hay Utility"),
    ("utility.bypass", "Activar/desactivar bypass del Utility"),
    ("utility.remove", "Quitar Hay Utility"),
    ("utility.reset", "Restablecer ganancia del Utility"),
];

pub(super) fn completions(
    query: &str,
    tools: &[catalog::tools::ToolCatalogEntry],
) -> Vec<Suggestion> {
    let query = query.trim_start().to_ascii_lowercase();
    let mut results: Vec<Suggestion> = KNOWN
        .iter()
        .map(|(command, description)| (command.to_string(), description.to_string()))
        .collect();
    for entry in catalog::entries() {
        if let CatalogAction::Invoke(command) = entry.action {
            if !results.iter().any(|(known, _)| *known == command) {
                results.push((command.into(), entry.name.into()));
            }
        }
    }
    for tool in tools {
        if let CatalogAction::Invoke(command) = &tool.entry.action {
            results.push((command.clone(), tool.entry.name.clone()));
        }
    }
    results.retain(|(command, description)| {
        query.is_empty()
            || command.to_ascii_lowercase().contains(&query)
            || description.to_ascii_lowercase().contains(&query)
    });
    results.sort_by(|a, b| {
        let rank = |command: &str| {
            if command == query {
                0
            } else if command.starts_with(&query) {
                1
            } else {
                2
            }
        };
        rank(&a.0).cmp(&rank(&b.0)).then_with(|| a.0.cmp(&b.0))
    });
    results
}

impl DawApp {
    pub(super) fn cmd_completions(&self) -> Vec<Suggestion> {
        completions(&self.command_buffer, self.palette.tool_entries())
    }

    pub(super) fn move_command_selection(&mut self, backwards: bool) {
        let count = self.cmd_completions().len();
        if count == 0 {
            self.cmd_selected = None;
            return;
        }
        self.cmd_selected = Some(match self.cmd_selected {
            None if backwards => count - 1,
            None => 0,
            Some(0) if backwards => count - 1,
            Some(index) if backwards => index - 1,
            Some(index) => (index + 1) % count,
        });
    }

    pub(super) fn complete_command(&mut self) {
        if let Some((command, _)) = self.cmd_completions().get(self.cmd_selected.unwrap_or(0)) {
            self.command_buffer = (*command).to_string();
            self.cmd_selected = None;
        }
    }
}
