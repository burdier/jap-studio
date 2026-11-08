# Guía de edición localizada

## Dónde cambiar cada cosa

| Tarea | Archivo principal | Revisar también |
|---|---|---|
| Arranque/ventana | `src/ui/app.rs` | `src/main.rs` solo llama `ui::run()` |
| Estado de UI | `src/ui/mod.rs` | Mantenerlo privado al árbol UI |
| Teclas y comandos | `src/ui/input.rs` | Controlador que ejecuta la acción |
| Teclado C4–C5 | `src/instruments/piano.rs` | `views/instrument.rs`, `musical/pitch.rs` |
| Colores/dimensiones | `src/ui/design.rs` | `~/.jaybu.profile` |
| Ciclo de render | `src/ui/render.rs` | Nunca avanzar aquí el transporte musical |
| Play/Stop/importación | `src/ui/audio_actions.rs` | `src/audio/` |
| Catálogo de instrumentos/acciones | `src/catalog.rs` | `tests/catalog.rs` |
| Buscador: estado y teclas | `src/ui/palette.rs` | `ui/input.rs` |
| Buscador: overlay | `src/ui/views/palette.rs` | `[+]` en `views/tracks.rs` |
| Estado del piano | `src/instruments/piano.rs` | `audio/piano/`, `views/instrument.rs` |
| Editar pasos/velocity | `src/ui/pattern_editor.rs` | `src/musical/model.rs` |
| Dibujar pasos | `src/ui/views/pattern.rs` | Controlador anterior |
| Layout Arrangement | `src/ui/views/arrangement.rs` | `views/tracks.rs` |
| Piano clicable | `src/ui/views/instrument.rs` | `instruments/piano.rs` |
| Mixer de ejemplo | `src/ui/views/mixer.rs` | Es mock, no medidores reales |
| Título/estado | `src/ui/views/chrome.rs` | `views/transport.rs` para botones |
| Zoom y conversión visual | `src/ui/timeline/viewport.rs` | `timeline/mod.rs` |
| Drag/scroll/picking | `src/ui/timeline/interaction.rs` | `timeline/tests.rs` |
| Regla/cursor/carriles | `src/ui/timeline/paint.rs` | `waveform.rs` |
| Partículas | `src/ui/timeline/particles.rs` | `paint.rs`, tokens en `design.rs` |
| Decodificar archivo | `src/audio/assets.rs` | Symphonia, trabajo offline |
| Elegir formato CPAL | `src/audio/output.rs` | Tres reproductores |
| Reproducir clip | `src/audio/playback.rs` | Un clip, sin remuestreo |
| Reproducir patrón | `src/audio/drums/mod.rs` | `drums/stream.rs` |
| Sintetizar percusión | `src/audio/drums/voice.rs` | Mapeo instrumento → voz |
| Resaltar paso sonando | `src/ui/pattern_editor.rs` | `views/pattern.rs` |
| Cargar SoundFont | `src/audio/piano/soundfont.rs` | `HAY_SOUNDFONT` |
| Render del piano | `src/audio/piano/render.rs` | Fachada `piano/mod.rs` |
| Notas/validaciones | `src/musical/model.rs` | `tests/musical.rs` |
| Conversión tick/frame | `src/musical/time.rs` | Scheduler y tests temporales |
| Programar eventos y loop | `src/musical/scheduler.rs` | `drums/stream.rs`, `examples/sequencer.rs` |
| DSP y composición | `src/primitives/`, `src/component.rs` | `tests/elevate.rs` |

## Protocolo para un cambio pequeño

1. Leer la fila de la tarea y el contrato del módulo, no todo el repositorio.
2. Revisar consumidores del símbolo antes de modificar su firma.
3. Mantener imports explícitos; usar visibilidad restringida al módulo cuando sea posible.
4. Cambiar comportamiento en el controlador/modelo, no duplicarlo entre teclas y clicks.
5. En builders GPUI, extraer subcomponentes con nombre antes de anidar closures largas.
   `impl IntoElement + use<>` declara que un helper no devuelve préstamos de `cx`/`self`.
6. Verificar el contrato afectado y actualizar esta tabla si se mueve un archivo.

No interpretar nombres de archivos como garantías: `scheduler` planifica una pasada;
el loop y la percusión viven en `audio/drums/stream.rs`; `PatternAction` no es un bus;
`output` comparte configuración, todavía no mezcla. `catalog` indexa metadatos: no
instancia primitivos, componentes ni instrumentos. El buscador solo abre el piano actual;
insertar instrumentos en pistas requiere el futuro host.

La vista del piano no debe importar `audio::piano` ni `rustysynth`. El estado de ejecución
(note-on/off, ratón, voces abiertas) pertenece a `src/instruments/piano.rs`; la UI solo
presenta el descriptor del catálogo y llama al controlador. Al añadir un instrumento,
registra su entrada en `src/catalog.rs` y evita duplicar comandos entre teclado y ratón.

La percusión usa un plan inmutable y síntesis provisional. Si cambias el modelo de notas,
detén y vuelve a pulsar Play para escuchar el plan nuevo. No añadas asignaciones,
reservas ni liberaciones costosas al callback de `drums/stream.rs`.

## Comandos (desde la raíz)

```bash
# Formato y verificación completa del incremento
distrobox enter gpui-dev -- bash scripts/check.sh

# Solo contratos musicales, sin UI ni dispositivo
distrobox enter gpui-dev -- cargo test --no-default-features --test musical --locked

# Traza de eventos por bloque de 256 frames
distrobox enter gpui-dev -- cargo run --no-default-features --example sequencer --locked

# Compilar aplicación
distrobox enter gpui-dev -- cargo build --bin daw-gpui --locked

# Abrir por X11 en este entorno
distrobox enter gpui-dev -- env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET cargo run --bin daw-gpui --locked
```

Los tests no necesitan un dispositivo; la ejecución del binario sí necesita una sesión gráfica.
No añadir librerías para conseguir una comprobación que ya resuelve Cargo o la biblioteca estándar.

## Rutas antiguas

- `src/main.rs` de 1839 líneas → `src/ui/` + arranque de seis líneas.
- `src/audio.rs` → `src/audio/{assets,output,playback}.rs`.
- `src/instrument.rs` → `src/audio/piano/` y `src/instruments/piano.rs`.
- `src/ui/piano_keys.rs` → `src/instruments/piano.rs` y `src/musical/pitch.rs`.
- `src/timeline.rs` → `src/ui/timeline/`.

Biblioteca `elevate`, contratos DSP, SoundFont y comandos de ejecución conservan sus nombres.
