# Guía de edición localizada

## Dónde cambiar cada cosa

| Tarea | Archivo principal | Revisar también |
|---|---|---|
| Sampler: contrato de parámetros/unidades | `src/devices/sampler.rs` | Headless; región, afinación, voces, LP12, ADSR, salida |
| Sampler: sfizz y recursos | `src/audio/sampler/` | ABI C, WAV temporal compartido, SFZ; preparación fuera de RT; `scripts/build-sfizz.sh` |
| Sampler: ejecución y MIDI | `src/instruments/sampler.rs` | `ui/sampler/audio.rs`, `audio/piano/{backend,render}.rs`: reproductor y scheduler compartidos |
| Sampler: panel y carga | `src/ui/sampler/` | Catálogo `[+]`, perillas/teclado existentes, carga en background, waveform de 1024 picos |
| Insertar pista sampler (`:sa`) | `src/ui/sampler/input.rs` | Acción `track.insert_sampler` del catálogo, menú de pistas y barra; reutiliza `edit_track_structure`, abre la pista existente si ya tiene la instancia |
| Sampler: exportación | `src/audio/export/sources.rs` | Snapshot por tipo de instrumento; `docs/SAMPLER.md` |
| Arranque/ventana | `src/ui/app.rs` | `src/main.rs` solo llama `ui::run()` |
| GPUI Kit, tema y atajos de integración | `src/ui/kit.rs` | `Cargo.toml`: alias `gpui` → `gpui-kit =0.9.0`; una sola familia GPUI `0.3.11`; `Root` en las tres ventanas |
| Estado de UI | `src/ui/mod.rs` | Mantenerlo privado al árbol UI |
| Teclas y comandos | `src/ui/input.rs` | Controlador que ejecuta la acción |
| Teclado del ordenador como MIDI (teclas, octava, velocity, sustain, bend, mod) | `src/musical/controller.rs` | `ui/audio_actions.rs`, `ui/input.rs` |
| Mensajes (`wmidi`) y servidor MIDI (servidor/cliente) | `src/musical/midi.rs` | `ui/audio_actions.rs` |
| Mapeo MIDI (`MidiMapper`) y `MIDI Learn` (CC/Nota → perilla/slider/acción) | `src/musical/midi_map.rs` | `ui/audio_actions.rs`, `views/{tracks,instrument_params}.rs` |
| Importación de archivos `.mid` (`midly` SMF → `MidiClip`) | `src/musical/smf.rs` | `ui/file_import.rs`; lectura y parseo en background |
| Entrada MIDI Hardware USB/ALSA (`midir` + `rtrb`) | `src/audio/midi_in.rs` | `ui/render.rs`, `ui/input.rs` (`:midi-scan`) |
| Motor SoundFont C aislado (`libfluidsynth` vía `dlopen` + respaldo `RustySynth`) | `src/audio/piano/fluidsynth.rs` | `audio/piano/render.rs`, `instruments/piano.rs` |
| Componente `MidiClip` y captura/grabación MIDI | `src/musical/clip.rs` | `ui/audio_actions.rs`, `ui/timeline/waveform.rs` |
| Materialize MIDI y selección múltiple por identidad | `src/musical/materialize.rs` | `ui/timeline/materialize.rs`, `tests/materialize.rs`, `ui/clip_menu/` |
| Rango del teclado (C3–B5) | `src/instruments/piano.rs` | `musical/keyboard.rs` |
| Geometría y etiquetas de octava | `src/musical/keyboard.rs` | `ui/components/keyboard.rs` |
| Teclado MIDI reutilizable (UI) | `src/ui/components/keyboard.rs` | `views/instrument.rs` |
| Colores/dimensiones | `src/ui/design.rs` | `~/.jaybu.profile` |
| Contraste de selección | `src/ui/design.rs::selection_color` | `SELECTION_CONTRAST`; `views/settings_appearance.rs`, `profile.rs`, cabeceras/carriles, clips y selectores compartidos |
| Guardado automático de tema/cursor | `src/ui/profile.rs` | `settings.rs`, `profile_tests.rs`; conserva el resto del perfil y reemplaza el archivo de forma atómica |
| Ciclo de render | `src/ui/render.rs` | Nunca avanzar aquí el transporte musical |
| Play/Stop | `src/ui/audio_actions.rs` | `src/audio/` |
| Espacio para Play/Stop desde cualquier panel | `src/ui/transport.rs` | `input.rs`, `views/piano_roll/mod.rs`, `detached.rs`; el explorador deja propagar Espacio; excluye texto/modales y repetición de tecla |
| Importación común a drop, explorador y comandos | `src/ui/file_import.rs` | Audio/MIDI preparados en background; destino por TrackId y beat capturados |
| Árbol de bibliotecas y lectura de carpetas | `src/explorer/{mod,tree,scan}.rs` | Headless; cache por carpeta y filas expandidas, sin recorrido recursivo del disco |
| Persistencia de bibliotecas | `src/explorer/persistence.rs` | Mapeos en `dirs::config_dir()/hay-daw/libraries`; escritura atómica, no modifica archivos de las bibliotecas |
| TreeView visual compartido por archivos y FX | `src/ui/components/tree.rs` | Superficie, cabecera, pie, lista virtualizada y filas originales del explorador; tokens `TreeStyle` |
| Topología FX por pista | `src/musical/fx/{mod,edit}.rs` | `Track.chain`; IDs estables, serie/paralelo recursivos, ediciones transaccionales y rechazo de ciclos |
| Track FX Tree y memoria visual | `src/ui/fx/{mod,presentation,view,input,drag}.rs` | Entidad cacheada, selección/expansión/scroll por TrackId; comparte TreeView con archivos |
| Add contextual, menú y comandos FX | `src/ui/fx/{host,commands,menu}.rs` | `palette.rs`, `actions/fx.rs`, `action_form.rs`; reutiliza catálogo, menú plano y formulario; destino TrackId/FxId |
| Audio de cadenas Utility | `src/devices/fx_gain.rs`, `src/audio/fx_gain.rs` | Compilación lineal fuera del callback, publicación atómica y rampa de 256 frames; `ui/fx/audio.rs`, streams audio/piano y snapshot export |
| Explorador visual independiente | `src/ui/explorer/` | Entidad GPUI propia, `uniform_list`, E/S en workers y guardados serializados |
| Atajo e, ancho y acción insertar archivo | `src/ui/explorer_host.rs` | `input.rs`, `views/workspace.rs`, `render.rs`, `catalog.rs`, `actions.rs`; acción compartida con doble clic e iconos |
| Catálogo de instrumentos/acciones | `src/catalog.rs` | `tests/catalog.rs` |
| Contrato/registro de parámetros de acciones | `src/actions.rs` | `CatalogAction::parameters()`, `export/action.rs`; texto, cantidades con unidad y booleanos |
| Formulario flotante de acciones | `src/ui/action_form.rs` | `action_form/input.rs`, `views/action_form.rs`; entidades InputState, suscripciones, foco limitado al diálogo y ventana de origen |
| Controles de parámetros reutilizables | `src/ui/components/parameter_field.rs` | Input, NumberInput y Checkbox de Kit; conserva unidades/límites del contrato y delega edición, IME, selección y portapapeles |
| Acciones del catálogo y despacho de edición de clips | `src/catalog.rs` | `ui/palette.rs`, `ui/clip_actions.rs`, `ui/input.rs` |
| Pistas y asignación de instrumento | `src/musical/track.rs` | `ui/palette.rs`, `views/tracks.rs` |
| Insertar/eliminar/duplicar/agrupar pistas | `src/musical/track/management.rs` | `ui/track_actions.rs`, `timeline/track_edit.rs`; devuelve mapa de origen para conservar clips, controles y MIDI Learn |
| Selección y menú de pistas | `src/ui/{track_actions,track_menu}.rs` | `views/{tracks,track_menu,empty_arrangement}.rs`; reutiliza `context_menu` y formulario declarativo para nombre |
| Plegar grupos y altura individual de pistas | `src/ui/{track_layout,track_presentation}.rs` | Presentación por TrackId; `timeline/rows.rs` centraliza coordenadas para dibujo, picking, arrastre, recorte y preview |
| Buscador: estado y teclas | `src/ui/palette.rs` | `ui/input.rs` |
| Buscador: overlay | `src/ui/views/palette.rs` | `[+]` en `views/tracks.rs` |
| Configuración por categorías | `src/ui/settings.rs` | `views/settings.rs`: navegación lateral; `settings_{appearance,tracks,shortcuts}.rs`: contenido |
| Preferencias de exportación guardadas y formulario | `src/ui/export_preferences.rs` | `export_preferences/input.rs`, `views/settings_export.rs`, `profile.rs`; borrador validado antes de guardar |
| Ventana separada de instrumento (`:detach`/`:dock`) | `src/ui/detached.rs` | `views/instrument.rs` |
| Parámetros del piano (lista, rangos, traducción a MIDI/RPN) | `src/instruments/piano_params.rs` | `audio/piano/render.rs` |
| Panel de parámetros y slider/toggle genéricos | `src/ui/views/instrument_params.rs` | `ui/components/slider.rs` |
| Perilla rotatoria (knob) nativa en GPU | `src/ui/components/knob.rs` | `views/tracks.rs`, `KNOB_SIZE` en `design.rs` |
| Botones, píldoras, chips y etiquetas reutilizables | `src/ui/components/button.rs` | `views/{transport,tracks,mixer,instrument,settings,palette}.rs` |
| Medidores verticales y barras horizontales | `src/ui/components/meter.rs` | `views/{tracks,mixer}.rs` |
| Fondo modal oscurecido reutilizable | `src/ui/components/modal.rs` | `views/{palette,settings}.rs` |
| Separadores y redimensionado de módulos (`SplitDrag`) | `src/ui/components/splitter.rs` | `ui/render.rs`, `views/{arrangement,workspace}.rs` |
| Carga y dibujo de SVG, estado de iconos asignados | `src/ui/icons.rs` | `assets/icons/toolbar/`, `views/toolbar.rs` |
| Acciones asignables a iconos y búsqueda del selector | `src/ui/toolbar/{catalog,dispatch,picker}.rs` | `src/catalog.rs`, `views/icon_picker{,_browser}.rs`, `toolbar/tests.rs` |
| Estado del piano | `src/instruments/piano.rs` | `audio/piano/`, `views/instrument.rs` |
| Editar pasos/velocity | `src/ui/pattern_editor.rs` | `src/musical/model.rs` |
| Dibujar pasos | `src/ui/views/pattern.rs` | Controlador anterior |
| Layout Arrangement | `src/ui/views/arrangement.rs` | `views/tracks.rs` |
| Track Headers configurables (TCP estilo REAPER) | `src/ui/views/tracks.rs` | `design.rs`, `views/settings.rs` |
| Estado M/S/R y audibilidad de pistas | `src/ui/audio_actions.rs` | `src/musical/track.rs`, `views/tracks.rs` |
| Workspace Arrangement + piano roll acoplado/separado | `src/ui/views/workspace.rs` | `src/ui/piano_roll/window.rs`, `views/piano_roll/` |
| Puntero GPU / efectos de cursor (`Fueguito`, etc.) | `src/ui/cursor_fx.rs` | `render.rs`, `views/settings.rs`, `views/chrome.rs` |
| Piano clicable | `src/ui/views/instrument.rs` | `instruments/piano.rs` |
| Mixer de ejemplo | `src/ui/views/mixer.rs` | Es mock, no medidores reales |
| Título/estado | `src/ui/views/chrome.rs` | `views/transport.rs` para botones |
| Zoom y conversión visual | `src/ui/timeline/viewport.rs` | `timeline/mod.rs` |
| Drag/scroll/picking | `src/ui/timeline/interaction.rs` | `timeline/tests.rs` |
| Sombra al importar MIDI/audio por drag & drop | `src/ui/timeline/import_preview.rs` | `views/arrangement.rs`, `timeline/paint.rs`; guía de entrada de un compás, sin leer archivos durante el arrastre |
| Gestos de touchpad (pinch, dos ejes, Ctrl/Shift+rueda) | `src/ui/navigation.rs` | `timeline/interaction.rs`, `views/arrangement.rs`; `PinchEvent` nativo de GPUI, sin dependencia adicional |
| Zoom, desplazamiento y picking del piano roll | `src/ui/piano_roll/navigation.rs` | Super+scroll: altura; Alt+scroll: ancho. `geometry.rs`, `navigation_tests.rs`, `views/piano_roll/{grid,grid_background}.rs` |
| Recorte no destructivo y solapamiento de clips | `src/ui/timeline/trim.rs` | `midi_clips.rs`, `waveform.rs`, `timeline/tests.rs` |
| Menú contextual de clips audio/MIDI e inserción | `src/ui/clip_menu/` | `views/clip_menu.rs`, `components/context_menu.rs`; filas y acciones compartidas por ratón/teclado |
| Selector visual de color de clips y pistas | `src/ui/components/color_picker.rs` | `views/{clip_menu,track_color_menu}.rs`; `design.rs::clip_palette` mantiene los índices existentes |
| Renombrar, duplicar, silenciar y mover clips | `src/ui/clip_actions.rs` | `audio/assets.rs` conserva el nombre de cada instancia de audio |
| Dividir y restaurar recorte de clips | `src/ui/timeline/clip_edit.rs` | `clip_edit_tests.rs`; fuentes intactas, respeta bloqueo y origen temporal |
| Regla/cursor/carriles | `src/ui/timeline/paint.rs` | `waveform.rs` |
| Regla inferior de minutos/segundos | `src/ui/timeline/time_ruler.rs` | `views/arrangement.rs`; mismo viewport, picking y gestos que la regla musical |
| Partículas | `src/ui/timeline/particles.rs` | `paint.rs`, tokens en `design.rs` |
| Decodificar archivo | `src/audio/assets.rs` | Symphonia, trabajo offline |
| Opciones/parser y serialización de preferencias de exportación | `src/export/{mod,command,preset}.rs` | `tests/export.rs`, `docs/EXPORT.md`; argumentos explícitos prevalecen sobre los valores guardados |
| Acción de exportación y snapshot de Arrangement | `src/ui/export.rs` | `input.rs`, `catalog.rs`, `render.rs`; worker independiente de reproducción |
| Mezcla offline y fuentes | `src/audio/export/{render,sources}.rs` | `job.rs`, `tests.rs`; posiciones por frames y rango común |
| Codecs C y publicación de exportaciones | `src/audio/export/{sndfile,lame,soxr,native,files}.rs` | `docs/EXPORT.md`; carga dinámica y archivos temporales |
| Síntesis compartida live/offline y snapshot del piano | `src/audio/piano/{backend,offline}.rs` | `render.rs`, `instruments/piano.rs`; sin abrir CPAL en export |
| Elegir formato CPAL | `src/audio/output.rs` | Tres reproductores |
| Paneo estéreo sin asignaciones en callback | `src/audio/stereo_pan.rs` | `audio/playback.rs`, `audio/piano/render.rs` |
| Reproducir clip | `src/audio/playback.rs` | Un clip, sin remuestreo |
| Reproducir patrón | `src/audio/drums/mod.rs` | `drums/stream.rs` |
| Sintetizar percusión | `src/audio/drums/voice.rs` | Mapeo instrumento → voz |
| Resaltar paso sonando | `src/ui/pattern_editor.rs` | `views/pattern.rs` |
| Ejecutar la app | `scripts/run.sh` | `scripts/check.sh` |
| Descargar bancos FreePats | `scripts/fetch_soundfonts.sh` | `scripts/soundfonts.txt`, `assets/soundfonts/freepats/` |
| Descubrir/cargar SoundFonts y créditos | `src/audio/piano/soundfont.rs` | `HAY_SOUNDFONT`, `instruments/piano.rs` |
| Render del piano | `src/audio/piano/render.rs` | Fachada `piano/mod.rs` |
| Notas/validaciones | `src/musical/model.rs` | `tests/musical.rs` |
| Conversión tick/frame | `src/musical/time.rs` | Scheduler y tests temporales |
| Programar eventos y loop | `src/musical/scheduler.rs` | `drums/stream.rs`, `examples/sequencer.rs` |
| DSP y composición | `src/primitives/`, `src/component.rs` | `tests/elevate.rs` |
| Descriptor/fábrica de dispositivos internos y Hay Utility | `src/devices/{mod,utility}.rs` | `catalog.rs`, `tests/utility.rs`; Component → Volume → FunDSP |
| Slot Utility de salida y snapshot para audio | `src/devices/utility_slot.rs` | `audio/{playback,drums/stream,piano/render}.rs`; prepara fuera del callback |
| Controles/acciones de Utility | `src/ui/utility.rs` | `views/utility.rs`, `input.rs`, `views/transport.rs`; salida de audición, no pista |

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
instancia primitivos, componentes ni instrumentos. El buscador asigna el piano actual a la
pista destino mediante `musical/track.rs`; crear varias instancias requiere el futuro host.
La asignación es estado musical: no la guardes en la vista ni en `instruments::Piano`.

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

# Catálogo y asignación de acciones a iconos, sin ventana ni dispositivo
distrobox enter gpui-dev -- cargo test --bin daw-gpui ui::toolbar --locked

# Persistencia visual en perfiles temporales, sin tocar el perfil personal
distrobox enter gpui-dev -- cargo test --bin daw-gpui ui::profile --locked

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
