use crate::ui::DawApp;
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};
impl DawApp {
    pub(super) fn render_shortcut_help(&self, _cx: &mut Context<Self>) -> gpui::AnyElement {
        let ds = &self.ds;
        let groups: &[(&str, &[(&str, &str)])] = &[
            (
                "GENERAL Y VISTAS",
                &[
                    (
                        "Ctrl+P  /  :",
                        "abrir paleta de comandos con autocompletado",
                    ),
                    (
                        "F1  /  F2  /  F3",
                        "cambiar entre Arrangement · Drums · Mixer",
                    ),
                    ("Ctrl+E", "mostrar / ocultar explorador de bibliotecas"),
                    ("F11", "ampliar panel activo; repetir o Esc para restaurar"),
                    ("Ctrl+Z  /  Ctrl+Y", "deshacer / rehacer (Historial Loro)"),
                    ("Ctrl+H", "abrir / cerrar panel de historial"),
                    ("Ctrl+S  /  Ctrl+O", "guardar / abrir proyecto"),
                    ("Ctrl+K", "activar / desactivar metrónomo"),
                    ("Ctrl+Shift+A", "activar / desactivar modo Automatización"),
                    ("Ctrl+J", "consolidar clips seleccionados"),
                    (
                        "Esc",
                        "cerrar overlays, restaurar panel ampliado o liberar notas",
                    ),
                ],
            ),
            (
                "ARRANGEMENT",
                &[
                    ("← / →", "mover el cursor en el tiempo"),
                    ("↑ / ↓", "seleccionar pista"),
                    ("Delete", "eliminar pistas o clips seleccionados"),
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
                    ("↑ / ↓", "seleccionar archivo o carpeta"),
                    ("← / →", "plegar / expandir carpetas"),
                    ("Ctrl+F", "buscar dentro de las bibliotecas"),
                    ("Ctrl+E / Esc", "ocultar explorador"),
                ],
            ),
            (
                "PIANO ROLL",
                &[
                    (
                        "V / B / E / P / M",
                        "Seleccionar · Dibujar · Borrar · Pintar · Mute",
                    ),
                    ("Touchpad: 2 dedos", "desplazar tiempo y registro de notas"),
                    (
                        "Super + 2 dedos ↓ / ↑",
                        "aumentar / reducir altura de las filas",
                    ),
                    (
                        "Alt+1 / Alt+2",
                        "rejilla más fina / más gruesa; no modifica notas",
                    ),
                    (
                        "Alt+rueda / Shift+pellizco",
                        "subdividir notas seleccionadas según la rejilla",
                    ),
                    ("Q / H", "cuantizar / humanizar mediante acciones Lua"),
                    (
                        "Pellizcar / Ctrl + rueda",
                        "zoom horizontal alrededor del puntero",
                    ),
                    ("0 / Ajustar vista", "restaurar el encuadre inicial"),
                ],
            ),
            (
                "COMANDOS (Ctrl+P o :)",
                &[
                    (":return", "insertar un bus Return auxiliar (FX)"),
                    (
                        ":master.fx / :mfx",
                        "abrir cadena de efectos del Bus Master",
                    ),
                    (
                        ":export -n 93.[mp3,wave] -master",
                        "mezcla offline del Arrangement → exports/; -individuals para pistas",
                    ),
                    (
                        ":export.help / :export.cancel",
                        "opciones de frecuencia, bits, MP3, rango, pistas, cola y destino",
                    ),
                    (
                        "Ctrl+V en comandos",
                        "pegar texto del portapapeles en la barra de comandos",
                    ),
                    (":utility", "abrir Hay Utility en la salida"),
                    (":sa", "insertar pista Hay Sampler"),
                    (":add", "abrir buscador de instrumentos y acciones"),
                    (
                        ":play  :stop  :record  :rec.audio",
                        "transporte y grabación MIDI / audio",
                    ),
                    (":auto  :auto.vol  :auto.pan", "controles de automatización"),
                    (":tempo 128  /  :meter 3/4", "cambiar tempo o compás"),
                ],
            ),
            (
                "TECLADO MIDI / DRUMS",
                &[
                    ("m", "activar / desactivar teclado MIDI en el QWERTY"),
                    (
                        "a w s e d f t g y h u j k",
                        "tocar C4–C5 cuando MIDI está activo",
                    ),
                    (
                        "Space",
                        "reproducir / detener (también en ventanas separadas)",
                    ),
                    ("Esc", "liberar notas activas"),
                    ("Enter / rueda", "alternar paso / cambiar velocity en Drums"),
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
