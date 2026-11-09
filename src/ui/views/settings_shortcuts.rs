use crate::ui::DawApp;
use gpui::{IntoElement, div, prelude::*, px, rgb};
impl DawApp {
    pub(super) fn render_shortcut_help(&self) -> gpui::AnyElement {
        let ds = &self.ds;
        let groups: &[(&str, &[(&str, &str)])] = &[
            (
                "VIM-LIKE / GENERAL",
                &[
                    ("h  j  k  l", "mover cursor / selección según la vista"),
                    ("i", "modo Insert"),
                    ("e (Normal)", "mostrar / ocultar explorador de bibliotecas"),
                    (":", "modo Command; escribe un comando y pulsa Enter"),
                    (":sa", "insertar pista Hay Sampler o abrir la existente"),
                    ("Esc", "salir del overlay o liberar notas"),
                    ("Tab", "cambiar a la vista siguiente"),
                    ("1  2  3  4", "Arrangement · Pattern · Mixer · Instrument"),
                ],
            ),
            (
                "ARRANGEMENT",
                &[
                    ("h / l", "mover el cursor en el tiempo"),
                    ("j / k", "seleccionar pista"),
                    (
                        "Delete / dd (Normal)",
                        "eliminar pistas seleccionadas y sus clips; un grupo incluye sus hijos",
                    ),
                    ("+ / -", "acercar / alejar"),
                    ("Home", "volver al inicio"),
                    ("Space", "reproducir / detener"),
                    (
                        "Touchpad: 2 dedos",
                        "desplazar tiempo y pistas en ambos ejes",
                    ),
                    ("Pellizcar / Ctrl + rueda", "zoom alrededor del puntero"),
                    ("Shift + rueda", "desplazar horizontalmente"),
                ],
            ),
            (
                "EXPLORADOR",
                &[
                    (
                        "Doble clic / Enter",
                        "insertar audio/MIDI en la pista activa, en el cursor",
                    ),
                    ("j / k · ↑ / ↓", "seleccionar archivo o carpeta"),
                    ("h / l · ← / →", "plegar / expandir carpetas"),
                    (
                        "e / Esc",
                        "ocultar explorador; e permite recuperarlo al reducir su ancho a cero",
                    ),
                    (
                        "Añadir biblioteca / Quitar",
                        "guardar / quitar acceso a una carpeta sin borrar sus archivos",
                    ),
                    (
                        "Recargar",
                        "actualizar las bibliotecas después de cambios en el disco",
                    ),
                ],
            ),
            (
                "PIANO ROLL",
                &[
                    (
                        "Shift+E",
                        "herramienta Borrar; e en Normal abre el explorador",
                    ),
                    ("Touchpad: 2 dedos", "desplazar tiempo y registro de notas"),
                    (
                        "Super + 2 dedos ↓ / ↑",
                        "aumentar / reducir altura de las filas",
                    ),
                    (
                        "Alt + 2 dedos ↑ / ↓",
                        "aumentar / reducir ancho de las notas",
                    ),
                    (
                        "Pellizcar / Ctrl + rueda",
                        "zoom horizontal alrededor del puntero",
                    ),
                    ("0 / Ajustar vista", "restaurar el encuadre inicial"),
                ],
            ),
            (
                "COMANDOS",
                &[
                    (
                        ":export -n 93.[mp3,wave] -master",
                        "mezcla offline del Arrangement → exports/; -individuals para pistas",
                    ),
                    (
                        ":export.help / :export.cancel",
                        "opciones de frecuencia, bits, MP3, rango, pistas, cola, Utility y destino",
                    ),
                    (
                        "Ctrl+V en modo Command",
                        "pegar texto del portapapeles en el comando",
                    ),
                    (
                        ":utility",
                        "insertar/abrir Hay Utility en la salida de audición",
                    ),
                    (
                        ":utility.gain 0.5",
                        "ganancia de Utility (0–2x), también con [ / ] y MIDI Learn",
                    ),
                    (
                        ":utility.bypass / reset / remove",
                        "utility.bypass, utility.reset y utility.remove",
                    ),
                    (
                        ":materialize / :clip.materialize",
                        "consolidar MIDI seleccionado; Ctrl+clic añade/quita clips",
                    ),
                    (":add", "abrir buscador de instrumentos y acciones"),
                    (
                        ":explorer.toggle / :explorer.insert",
                        "abrir explorador / insertar su archivo seleccionado",
                    ),
                    (
                        ":file.import '/ruta/archivo.wav'",
                        "insertar por ruta; también disponible en catálogo e iconos",
                    ),
                    (
                        ":play  :stop  :record  :capture",
                        "transporte y grabación MIDI",
                    ),
                    (
                        ":clip.duplicate  :clip.split  :clip.delete",
                        "duplicar, dividir en el cursor o eliminar el clip bajo el cursor",
                    ),
                    (
                        ":clip.mute  :clip.lock  :clip.restore_trim",
                        "mute, bloqueo y restaurar recorte del clip bajo el cursor",
                    ),
                    (
                        ":clip.gain.down  :clip.gain.reset  :clip.gain.up",
                        "ajustar la ganancia del clip en pasos de 3 dB",
                    ),
                    (
                        ":clip.track.previous  :clip.track.next",
                        "mover el clip una pista arriba o abajo",
                    ),
                    (
                        ":clip.rename  :clip.gain  :clip.color",
                        "abrir el editor correspondiente para el clip bajo el cursor",
                    ),
                    (":cursor fire|sparks|neon|stars|off", "cambiar puntero GPU"),
                    (":tcp standard|compact|mixer|minimal", "cambiar layout TCP"),
                    ("[  ]  b  R", "volumen -/+ · bypass · recargar diseño"),
                ],
            ),
            (
                "INSTRUMENT / PATTERN",
                &[
                    ("a w s e d f t g y h u j k", "tocar C4–C5 en Instrument"),
                    ("Click en las teclas", "tocar nota; soltar para Note Off"),
                    (
                        "Space",
                        "reproducir / detener (también en ventanas separadas)",
                    ),
                    ("Esc", "liberar notas en Instrument"),
                    (
                        "Enter / rueda",
                        "alternar paso / cambiar velocity en Pattern",
                    ),
                ],
            ),
        ];
        div()
            .id("shortcut-help-scroll")
            .min_h_0()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .overflow_y_scroll()
            .children(groups.iter().map(|(heading, shortcuts)| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .mb_1()
                            .text_xs()
                            .text_color(rgb(ds.cyan))
                            .child(*heading),
                    )
                    .children(shortcuts.iter().map(|(key, description)| {
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .py_1()
                            .border_b_1()
                            .border_color(rgb(ds.border))
                            .child(
                                div()
                                    .w(px(220.0))
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(rgb(ds.accent))
                                    .child(*key),
                            )
                            .child(div().text_xs().text_color(rgb(ds.text)).child(*description))
                    }))
            }))
            .into_any_element()
    }
}
