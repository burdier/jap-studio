# Guía de edición localizada

## Dónde cambiar cada cosa

| Tarea | Archivo principal | Revisar también |
|---|---|---|
| Menús Archivo y Vista | `src/ui/views/menu_bar.rs` | `views/{chrome,palette}.rs`, `render.rs`; usan las tarjetas de Add y llaman a las acciones de proyecto, vista y barra existentes |
| Ampliar panel / preferencia Vim | `src/ui/{workspace_focus,keyboard_preferences}.rs` | `views/{workspace_focus,workspace,piano_roll/mod,settings_shortcuts}.rs`, `input.rs`, `render.rs`, `detached.rs`; acciones `view.maximize` y `vim.toggle`, perfil `VIM_ENABLED`, conserva geometría y foco |
| Spectrum insertable y vistas de análisis | `src/audio/analyzer{,/tap}.rs` | `audio/effects/plan.rs`, `devices/spectrum.rs`, `ui/fx/analyzers.rs`, `ui/spectrum.rs`, `ui/components/spectrum{,/plot}.rs`, `ui/views/fx_analyzers.rs`; mismo componente en piano y Track FX, captura acotada, sin FFT en RT |
| Interfaz y preferencias de audio | `src/audio/{device,output,device_probe}.rs` | `audio/device/{inventory,aliases}.rs`, `ui/audio_preferences{,/commands,/profile,/probe}.rs`, `views/settings_audio.rs`, `actions/audio.rs`; `:list device`, `:use device`, IDs CPAL, PipeWire compartido, rutas por nombre/canales locales, Hz, buffer, pruebas y perfil |
| Mezcla de WAV, MIDI y percusión en una salida (PR-05) | `src/audio/playback/mix.rs` | `playback.rs`, `ui/file_playback.rs`, `audio/drums/mix.rs`: preparación en worker, suma MIDI/WAV por pista antes de FX y percusión en master; un transporte por frames; `playback/mix/tests.rs` verifica fuentes simultáneas |
| Contexto temporal VST3 de la mezcla (PR-06, commit 1) | `src/audio/vst3/context.rs` | `vst3/process.rs`, `audio/{playback,effects,fx_gain}.rs`: frame absoluto, BPM, compás y loop por bloque de FX; flags válidos según Steinberg, sin reloj nuevo |
| Instancia VST3 por pista (PR-06, commit 2) | `src/ui/vst.rs` | `instruments/vst.rs`, `ui/{audio_actions,track_commit,audio_preferences,render}.rs`, `ui/vst/effects.rs`: ID, foco MIDI, editor, FX, retiro y recarga por pista; la audición aún usa streams individuales |
| Instrumentos VST3 en Play común (PR-06, commit 3) | `src/audio/vst3/shared.rs` | `audio/piano/{backend,voice,render,prepare}.rs`, `audio/playback{,/mix}.rs`, `ui/file_playback.rs`: misma instancia/editora cambia de audición a Arrangement; CAS y cola MIDI acotada, eventos por frame y contexto temporal, cola de sonido limitada; `examples/vst3_probe.rs --shared` |
| Acción Lua VST3 (PR-06, commit 4) | `src/scripting/vst3.rs` | `ui/vst/script.rs`, `actions.rs`, `catalog.rs`, `docs/VST3_LUA.md`: una acción validada por TrackId/FxId y revisión; carga y FX delegan a controladores existentes, parámetros internos pendientes |
| MIDI interno en vivo durante Play | `src/audio/playback.rs` | `audio/playback/mix.rs`, `audio/piano/voice.rs`, `ui/{audio_actions,file_playback}.rs`, `ui/sampler/audio.rs`, `instruments/{piano,sampler}.rs`: cola acotada hacia las voces preparadas, incluso sin clip MIDI; tocar sobre WAV conserva el transporte |
| Fuente de clip sin stream (PR-04, commit 1) | `src/audio/clip_source.rs` | Fuente inmutable con render por posición absoluta; pruebas de recortes, particiones, seek y buffer remuestreado. `audio/playback.rs` conserva CPAL, transporte y FX del reproductor actual |
| Preparación y remuestreo de clips para salida | `src/ui/file_playback.rs` | `audio/playback.rs`, `audio/export/soxr.rs`; worker cancelable, Stop cancela el arranque, snapshots de audio/MIDI/patrón y voz por instancia |
| Documento CRDT de proyecto, pistas, FX, MIDI, audio, tiempo y mezcla | `src/project.rs` | `src/project/{track,fx,midi,audio,session_state}.rs`, `audio/assets.rs`, `ui/track_batch.rs`, `docs/LORO_SESSION.md`: esquema 4, IDs estables, metadata WAV por TrackId y ruta, tempo/compás, snapshots/updates; abrir/guardar local mediante `ui/project/` |
| Abrir/guardar proyecto local | `src/ui/project/{mod,snapshot,dialogs,install}.rs` | `project/instruments.rs`, `instruments/piano/persistence.rs`, `input.rs`, `render.rs`, `views/chrome.rs`; candidato completo en worker, selector nativo, dirty, guardado atómico, pruebas `ui::project::tests` |
| Colectar fuentes de audio | `src/project/collect.rs` | `ui/collect.rs`, `actions.rs`, `catalog.rs`, `tests/project_audio.rs`: copia deduplicada a carpeta nueva `Audio/`, snapshot Loro con rutas relativas y publicación tras completar; no abre la sesión completa |
| Expresión por nota y probabilidad | `src/musical/note_expression.rs` | `scheduler.rs`, `project/midi.rs`, `scripting/{actions,output}.rs`, `ui/piano_roll/lane.rs`, `audio/piano/{render,fluidsynth/voices}.rs`; release sfizz, pan FluidSynth, probabilidad por vuelta |
| Controles MIDI, CC, pitch bend y presión | `src/musical/automation.rs` | `scheduler.rs`, `project/midi_controls.rs`, `scripting/controls.rs`, `ui/piano_roll/control_actions.rs`, `views/piano_roll/control_lane.rs`; puntos por ID, acciones y mensajes por frame |
| Acciones e inventario del piano roll | `src/ui/piano_roll/{actions,commands,properties,choices,keyboard,paint,edit}.rs` | `docs/PIANO_ROLL_ACTIONS.md`, `views/piano_roll/{toolbar,grid,inspector}.rs`, `musical/{model,clip}.rs`; botones, teclado y canvas comparten controlador |
| Secuencia y audición de acordes | `src/musical/chord.rs` | `ui/piano_roll/chord_sequence.rs`, `ui/views/piano_roll/chord_sequence.rs`: posiciones por compás/tiempo, plan de notas y NoteOn/NoteOff reutilizables; ratón libera al soltar, inserción completa en copia del patrón |
| Escala y feedback de notas | `src/musical/scale.rs` | `views/piano_roll/{grid,grid_background}.rs`: Escala mayor/menor y tónica editables, iluminación del borde de duración y color `danger` fuera de escala |
| Franjas por compás | `src/ui/components/bar_grid.rs`, `src/ui/time_grid.rs` | `timeline/paint.rs`, `views/piano_roll/grid_background.rs`; alternancia por índice absoluto, divisiones independientes del snap y firma real |
| Eje e intervalos temporales compartidos Arrangement/piano roll | `src/ui/time_grid.rs` | `timeline/{viewport,trim}.rs`, `piano_roll/{geometry,edit}.rs`, `views/piano_roll/grid_background.rs`: beats/píxeles, snap, densidad, borde, mover/recortar; ejes verticales distintos |
| Overlay 2D de curvas y formas (`CurveOverlay`) | `src/ui/components/curve_overlay.rs` | `curve_overlay/{paint,shapes}.rs`, `views/lua_component/response.rs`, `sampler/controls.rs`, `views/piano_roll/control_lane.rs`: curva decidida por el implementador, arrastre continuo con Shift/rueda/reset y contornos sobre palitos del piano roll |
| Sampler: contrato de parámetros/unidades | `src/devices/sampler.rs` | Headless; región, afinación, voces, LP/HP/BP, AHDSR, Pitch Env, Vel→Vol/Cut, Keytrack/Choke/Reverse y preset Drum |
| Espectro y visualizador | `src/audio/analyzer.rs`, `src/ui/components/spectrum.rs` | RealFFT fuera del callback; cola acotada desde piano; panel UI reutilizable con `<` `>` y modos Espectro/Visualizador |
| Plan Spectro, LFO y host CLAP/VST3 | `docs/VISUAL_MODULATION.md` | Puertos tipados, rutas estables, eventos por frame y adaptación de plugins; requiere ampliar el motor |
| Sampler: sfizz y recursos | `src/audio/sampler/` | ABI C, WAV temporal compartido, SFZ; preparación fuera de RT; `scripts/build-sfizz.sh` |
| Sampler: ejecución y MIDI | `src/instruments/sampler.rs` | `ui/sampler/audio.rs`, `audio/piano/{backend,render}.rs`: reproductor y scheduler compartidos |
| Voz de instrumento separada del stream (PR-05, commit 1) | `src/audio/piano/voice.rs` | `audio/piano/plan.rs` procesa eventos por frame también en export offline; `render.rs` conserva CPAL. La voz posee sintetizador, canal y patrón, sin GPUI ni dispositivo. Todavía se instancia dentro de un stream exclusivo |
| Varias instancias de Hay Piano por pista | `src/ui/piano_instances.rs` | `instruments/piano.rs`, `ui/{palette,audio_actions,file_playback,track_commit,fx/audio,vst/effects,export}.rs`; ID por instancia, editor activo, MIDI al destino original y snapshot de Play por rama; exportación multipiano rechazada explícitamente |
| Conjunto de voces por ID (PR-05, commit 2) | `src/audio/piano/voice.rs` | `PreparedVoices` registra motores ya preparados, enruta comandos y suma bloques; `audio/piano/render.rs` usa el conjunto con una voz local. Prueba con dos RustySynth y SoundFont compartido; aún sin asignación multipista en UI |
| Plan MIDI multipista por frames (PR-05) | `src/musical/arrangement_midi.rs` | `musical/scheduler.rs` publica eventos preparados; `audio/piano/voice.rs` renderiza segmentos entre eventos y `audio/playback/mix.rs` los suma con WAV en el bus de su pista |
| Hilo de grupo: capas y round robin (PR-07) | `src/musical/hilo.rs` | `musical/arrangement_midi/hilo.rs`, `musical/{track,midi}.rs`, `audio/playback/mix.rs`, `ui/{file_playback,audio_actions,track_menu,track_actions,fx/}.rs`, `project{,/midi}.rs`, `scripting/tracks/`; las hijas son ramas con FX/nivel propios, NoteOff vuelve a su rama y el grupo comparte transporte |
| Tejer desde selección y regla preparada (PR-09) | `src/ui/track_actions.rs` | `ui/{track_menu,input,weave,audio_actions,file_playback}.rs`, `audio/midi_in.rs`, `catalog.rs`, `actions.rs`, `musical/{weave,midi,arrangement_midi/hilo,track}.rs`, `project{,/track}.rs`, `scripting/weave.rs`, `audio/playback/mix.rs`, `assets/scripts/hilos/tejer.lua`, `docs/TEJER.md`; agrupa/activa desde UI, guarda la secuencia de cada Hilo, Lua global opcional se evalúa al abrir, y MIDI físico conserva canal para emparejar NoteOff. `musical/clip.rs` guarda el destino del Hilo por TrackId; `ui/clip_menu/{items,input}.rs`, `ui/clip_actions.rs` eligen salida sin mover el clip; `ui/timeline/waveform.rs` muestra el destino, `project/midi.rs` lo persiste y `audio/export/` lo aplica offline |
| Retirar una rama del Hilo durante Play | `src/ui/track_commit.rs` | `audio/playback{,/mix,/mix/retirement,/transport}.rs`, `musical/track/management.rs`; retirar por TrackId, procesar cola sin volver a leer la fuente y remapear la secuencia del Hilo |
| Contrato de preset de Hilo (PR-10) | `docs/HILO_PRESETS.md` | Formato versionado previsto; todavía no hay captura VST3, guardado de preset ni carga desde la UI |
| Despacho MIDI live por destino (PR-05, commit 4) | `src/musical/midi.rs` | `ui/audio_actions.rs`, `ui/views/tracks.rs`, `ui/sampler/audio.rs`, `ui/vst.rs`: pistas activadas por ID, varios destinos a la vez, NoteOff al ID original en cambios de foco, descarte de liberaciones tardías y limpieza al detener; ratón libera su propio instrumento |
| Sampler: panel y carga | `src/ui/sampler/` | Catálogo `[+]`, perillas/teclado existentes, carga en background, waveform de 1024 picos |
| Insertar pista sampler (`:sa`) | `src/ui/sampler/input.rs` | Cada inserción crea una pista e instancia nueva; `ui/sampler/audio.rs` cambia la instancia activa sin mezclar muestras, `ui/track_commit.rs` retira las eliminadas |
| Sampler: exportación | `src/audio/export/sources.rs` | Snapshot por tipo de instrumento; `docs/SAMPLER.md` |
| Arranque/ventana | `src/ui/app.rs` | `src/main.rs` solo llama `ui::run()` |
| GPUI Kit, tema y atajos de integración | `src/ui/kit.rs` | `Cargo.toml`: alias `gpui` → `gpui-kit =0.9.0`; una sola familia GPUI `0.3.11`; `Root` en las tres ventanas |
| Estado de UI | `src/ui/mod.rs` | Mantenerlo privado al árbol UI |
| Interpretación MIDI/Espacio con instrumento abierto | `src/ui/transport.rs` | `render.rs`, `input.rs`, `detached.rs`, `fx/host.rs`; prioridad en focos musicales, preserva formularios y menús |
| Shift + arrastrar contenido de clip | `src/ui/timeline/slip.rs` | Slip continuo audio/MIDI mediante offsets de fuente; bordes y pista fijos; `views/arrangement.rs` inicia, `timeline/interaction.rs` mueve/finaliza |
| Indicador MIDI del icono asignado | `src/ui/views/toolbar.rs` | `icons.rs` recibe estado activo; refleja `controller.enabled()`, incluida acción personalizada `midi`/`kbd`; M funciona también desde OFF |
| Movimiento visual del playhead/cursor | `src/ui/timeline/visual_transport.rs` | Interpolación acotada por frames de audio; ruler, Arrangement y piano roll comparten dibujo, sin modificar el transporte |
| Teclas y comandos | `src/ui/input.rs` | Controlador que ejecuta la acción |
| Sugerencias del modo `:` | `src/ui/command_suggestions.rs` | `ui/views/command_bar.rs`, `ui/input.rs`, `ui/kit.rs`, `ui/render.rs`, `ui/piano_roll/window.rs`; catálogo + alias, flechas, Tab, clic y Enter |
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
| Colores/dimensiones y contraste estructural | `src/ui/design.rs` | `~/.jaybu.profile`; 16 paletas Studio con acentos moderados, colores explícitos del perfil tienen prioridad; `gutter_color` y `frame_border` separan paneles sin un nuevo acento; revisar `views/{mod,arrangement,chrome,transport,workspace,tracks}.rs` y `components/splitter.rs` |
| Preset Daynight Studio | `src/ui/design.rs` | `assets/themes/daynight-studio.profile`: variante oscura de Daylight, contraste entre superficies y acentos mates; Configuración → Apariencia |
| Preset Ableton Live Dark | `src/ui/design.rs` | `assets/themes/ableton-live-dark.profile`: paleta y tamaños reconocidos por el perfil; se elige en Configuración → Apariencia |
| Contraste de selección | `src/ui/design.rs::selection_color` | `SELECTION_CONTRAST`; `views/settings_appearance.rs`, `profile.rs`, cabeceras/carriles, clips y selectores compartidos |
| Guardado automático de tema/cursor | `src/ui/profile.rs` | `settings.rs`, `profile_tests.rs`; conserva el resto del perfil y reemplaza el archivo de forma atómica |
| Ciclo de render | `src/ui/render.rs` | Nunca avanzar aquí el transporte musical |
| Seek y loop de mezcla de audio | `src/audio/playback/transport.rs` | `ui/transport/audio.rs`, `input.rs`, `views/transport.rs`, `toolbar/dispatch.rs`; reloj musical existente, comandos/GUI comunes, publicación RCU de pedidos y confirmación de revisión |
| Preparación FX Lua a frecuencia real | `src/audio/effects/component.rs` | `effects/{mod,plan}.rs`, `audio/publication.rs`; variantes por frecuencia, valores comunes, preparación antes de conectar/publicar, retiro fuera del callback |
| Play/Stop | `src/ui/audio_actions.rs` | `src/audio/` |
| Espacio para Play/Stop desde cualquier panel | `src/ui/transport.rs` | `input.rs`, `views/piano_roll/mod.rs`, `detached.rs`; el explorador deja propagar Espacio; excluye texto/modales y repetición de tecla |
| Importación común a drop, explorador y comandos | `src/ui/file_import.rs` | Audio/MIDI preparados en background; destino por TrackId y beat capturados |
| Árbol de bibliotecas y lectura de carpetas | `src/explorer/{mod,tree,scan}.rs` | Headless; cache por carpeta y filas expandidas, sin recorrido recursivo del disco |
| Persistencia de bibliotecas | `src/explorer/persistence.rs` | Mapeos en `dirs::config_dir()/hay-daw/libraries`; escritura atómica, no modifica archivos de las bibliotecas |
| TreeView visual compartido por archivos y FX | `src/ui/components/tree.rs` | Superficie, cabecera, pie, lista virtualizada y filas originales del explorador; tokens `TreeStyle` |
| Topología FX por pista | `src/musical/fx/{mod,edit}.rs` | `Track.chain`; IDs estables, serie/paralelo recursivos, ediciones transaccionales y rechazo de ciclos |
| Track FX Tree y memoria visual | `src/ui/fx/{mod,presentation,view,input,drag}.rs` | Entidad cacheada, selección/expansión/scroll por TrackId; comparte TreeView con archivos |
| Instrumento en Track FX | `src/ui/fx/{presentation,host,input}.rs`, `src/ui/views/workspace.rs` | Fila virtual por asignación de pista; editor bajo Arrangement, sin duplicar el dispositivo en la cadena DSP |
| Add contextual, menú y comandos FX | `src/ui/fx/{host,commands,menu}.rs` | `palette.rs`, `actions/fx.rs`, `action_form.rs`; reutiliza catálogo, menú plano y formulario; destino TrackId/FxId |
| Audio de cadenas Utility | `src/devices/fx_gain.rs`, `src/audio/fx_gain.rs` | Compilación lineal fuera del callback, publicación atómica y rampa de 256 frames; `ui/fx/audio.rs`, streams audio/piano y snapshot export |
| Explorador visual independiente | `src/ui/explorer/` | Entidad GPUI propia, `uniform_list`, E/S en workers y guardados serializados |
| Atajo e, ancho y acción insertar archivo | `src/ui/explorer_host.rs` | `input.rs`, `views/workspace.rs`, `render.rs`, `catalog.rs`, `actions.rs`; acción compartida con doble clic e iconos |
| Catálogo de instrumentos/acciones | `src/catalog.rs`, `src/catalog/{entry,tools}.rs` | `tests/catalog.rs`, `tests/lua_tools.rs`; entradas nativas estáticas y metadatos Lua propios, sin VM |
| Plugins VST3 instalados en el buscador | `src/catalog/plugins.rs` | `ui/palette/plugins.rs`, `ui/palette.rs`; enumera bundles; `audio/vst3/scan.rs` consulta fábricas en proceso separado y clasifica sus clases |
| Host VST3 de instrumento | `src/audio/vst3/{mod,lifecycle,module,buffers,events,parameters,process,host}.rs` | `instruments/vst.rs`, `ui/vst{,/commands,/view}.rs`, `audio/piano/{backend,prepare}.rs`; lifecycle COM, MIDI y audio estéreo; límites en `docs/VST3.md` |
| Efectos VST3 en pistas | `src/audio/effects/{mod,plan}.rs`, `src/audio/publication.rs`, `src/ui/vst/effects.rs` | Plan de audio publicado por RCU; FX Tree, catálogo y comandos `fx.vst3.load/editor`; alcance en `docs/VST3.md` |
| Editor original VST3 | `src/audio/vst3/editor/{session,window,x11,frame,handler,stream}.rs` | `ui/fx/host.rs`, `ui/vst/commands.rs`; IPlugView/IRunLoop, cola de parámetros, ventana externa; diagnóstico `vst3_probe --editor` |
| Espacio en ventana original VST3 | `src/audio/vst3/editor/{x11,window,shortcut}.rs` | `editor/{mod,session}.rs`, `ui/{vst,render,transport}.rs`: X11 captura Espacio del editor hijo, filtra repetición y entrega solicitud al transporte común en UI |
| Wine del host desde distrobox | `scripts/wine-host.sh` | `scripts/run.sh`, `audio/vst3/environment.rs`, `ui/app.rs`; WINELOADER, prefijo compartido, diagnóstico `examples/vst3_probe.rs` |
| Contrato/registro de parámetros de acciones | `src/actions.rs` | `CatalogAction::parameters()`, `export/action.rs`; texto, cantidades con unidad y booleanos |
| Herramientas Lua para MIDI | `src/scripting.rs`, `src/scripting/{actions,builtin,output}.rs` | `ui/scripting.rs`, `assets/scripts/midi/`, `assets/scripts/tools/midi/`, `tests/scripting.rs`, `docs/LUA.md`; paquetes generan notas desde una fuente seleccionada con escala activa; Acentuar/Subdividir mantienen aliases; cortes y notas nuevas validados por ID |
| Transpositor Lua previo a Play (PR-08, alcance revisado) | `src/scripting/{live,live_notes,live_preplay}.rs` | `ui/file_playback{,/lua}.rs`, `assets/scripts/components/midi/transposer.lua`, `examples/lua_live_latency.rs`, `docs/{LUA_LIVE_PROFILE,LUA_LIVE_REPORT}.md`; snapshots en worker, error atómico, ninguna llamada Lua en RT |
| Registro headless de paquetes Lua v1 | `src/scripting/packages/{mod,scan,descriptor,fields,parameters,error}.rs` | `scripting/sandbox.rs`, `actions.rs`, `tests/lua_packages.rs`: carga explícita acotada, metadatos propios, generaciones y conservación ante fallo; contrato en `docs/LUA_PACKAGES.md` |
| Ejecutar/recargar herramientas Lua (PR-01, commit 4) | `src/scripting/packages/{command,invocation,execute}.rs`, `src/ui/lua_tools.rs` | `ui/scripting{,/commands}.rs`, `scripting/{notes,controls,arrays,target}.rs`, `ui/palette/{entry,activate,tools}.rs`, `assets/scripts/tools/midi/`, `tests/lua_tools.rs`; formulario por generación, worker con snapshot, aliases de Acentuar/Subdividir y commit MIDI compartido |
| Presets Hay Acent para selección MIDI | `assets/scripts/midi/hay_acent/` | Pulso, Alternado, Creciente y Clic 4/4 por `midi.script`; `tests/hay_acent.rs`, `docs/LUA.md#hay-acent`; modifican notas existentes, sin registro dinámico ni metrónomo global |
| Rejilla y subdivisión de notas | `src/musical/grid.rs`, `src/ui/piano_roll/grid_actions.rs` | `piano_roll/{actions,edit,navigation}.rs`, `views/piano_roll/{toolbar,grid_background}.rs`; Alt+1/2, Alt+rueda, Shift+pellizco; `midi.subdivide` ejecuta Lua |
| Formulario flotante de acciones | `src/ui/action_form.rs` | `action_form/{input,defaults,submit}.rs`, `views/action_form.rs`, `actions/owned.rs`; contratos propios, controles compartidos, suscripciones, generación Lua y foco en la ventana de origen |
| Select reutilizable y selectores Audio | `src/ui/components/select.rs` | Adaptador tipado de GPUI Kit: búsqueda, popup anclado, teclado; `audio_preferences/{selectors,selector_models}.rs` conserva entidades por ventana; diagnóstico `cargo run --example audio_devices -- SSL` |
| Controles de parámetros reutilizables | `src/ui/components/parameter_field.rs` | Input, NumberInput y Checkbox de Kit; conserva unidades/límites del contrato y delega edición, IME, selección y portapapeles |
| Acciones del catálogo y despacho de edición de clips | `src/catalog.rs` | `ui/palette.rs`, `ui/clip_actions.rs`, `ui/input.rs` |
| Apunte general persistente (`:note`) | `src/ui/general_note.rs` | `views/general_note.rs`, `catalog.rs`; textarea de GPUI Kit, guardado en datos del usuario |
| Pistas y asignación de instrumento | `src/musical/track.rs` | `ui/palette.rs`, `views/tracks.rs` |
| Insertar/eliminar/duplicar/agrupar pistas | `src/musical/track/management.rs` | `ui/track_actions.rs`, `timeline/track_edit.rs`; devuelve mapa de origen para conservar clips, controles y MIDI Learn |
| Lotes Lua de pistas (PR-02) | `src/musical/track/{snapshot,validation,batch}.rs` | `scripting/tracks/`, `ui/{track_scripts,track_batch,track_commit}.rs`, `ui/timeline/track_edit.rs`, `project{,/track,/fx}.rs`, `assets/scripts/tracks/`, `tests/track_batch*.rs`; contrato y límites en `docs/LUA_TRACK_BATCHES.md` |
| Filtro nativo y kit Lua v2 (PR-03B) | `src/primitives/filter.rs` | `scripting/audio_components/{native,fields,graph}.rs`, `assets/scripts/components/audio/filter.lua`, `ui/views/lua_component/response.rs`, `tests/lua_filter.rs`; SVF FunDSP, choice/log, curva de respuesta y comandos comunes |
| Componentes de audio Lua (PR-03) | `src/scripting/audio_components.rs` | `docs/LUA_AUDIO_COMPONENTS.md`, `assets/scripts/components/audio/`, `devices/audio_component.rs`, `audio/effects/{component,plan}.rs`, `ui/{lua_components,palette}.rs`, `ui/fx/{host,commands}.rs`, `ui/views/lua_component.rs`; grafo validado y preparado fuera de RT, controles por FxId y recarga explícita |
| Arrastrar cabeceras para ordenar pistas | `src/ui/track_drag.rs` | `views/{tracks,arrangement}.rs`, `timeline/paint.rs`: proyecta filas y clips sin cambiar el modelo, deja un hueco y flota la pista por píxel; `TrackEdit::Move` al soltar |
| Selección y menú de pistas | `src/ui/{track_actions,track_menu}.rs` | `views/{tracks,track_menu,empty_arrangement}.rs`; reutiliza `context_menu` y formulario declarativo para nombre |
| Plegar grupos y altura individual de pistas | `src/ui/{track_layout,track_presentation}.rs` | Presentación por TrackId; `timeline/rows.rs` centraliza coordenadas para dibujo, picking, arrastre, recorte y preview |
| Buscador: estado y teclas | `src/ui/palette.rs` | `ui/input.rs` |
| Buscador: overlay | `src/ui/views/palette.rs` | `[+]` en `views/tracks.rs` |
| Configuración por categorías | `src/ui/settings.rs` | `views/settings.rs`: navegación lateral; `settings_{appearance,tracks,shortcuts}.rs`: contenido |
| Preferencias de exportación guardadas y formulario | `src/ui/export_preferences.rs` | `export_preferences/input.rs`, `views/settings_export.rs`, `profile.rs`; borrador validado antes de guardar |
| Ventana separada de instrumento (`:detach`/`:dock`) | `src/ui/detached.rs` | `views/instrument.rs` |
| Parámetros del piano (lista, rangos, traducción a MIDI/RPN) | `src/instruments/piano_params.rs` | `audio/piano/render.rs` |
| Piano: panel compacto, bancos y acciones | `src/ui/views/instrument.rs` | `instrument_banks.rs`, `instrument_actions.rs`; listado de bancos/créditos desplegables |
| Piano: bloques de parámetros, perillas y MIDI Learn | `src/ui/views/instrument_params.rs` | `ui/components/{knob,button}.rs`; índices de `piano_params::SPECS` conservados |
| Piano: carga diferida y cancelación de notas/Play pendientes | `src/instruments/piano/loading.rs` | `audio/piano/prepare.rs`, `ui/render.rs`; un worker, resultados versionados, sin precarga al arrancar |
| Piano: filtros LP/HP/BP y exportación | `src/devices/filter.rs`, `src/audio/piano/filter.rs` | `instruments/piano_params.rs`, `audio/piano/{render,offline}.rs`; filtro estéreo después de las voces |
| Perilla rotatoria (knob) nativa en GPU | `src/ui/components/knob.rs` | `views/tracks.rs`, `KNOB_SIZE` en `design.rs` |
| Botones, píldoras, chips y etiquetas reutilizables | `src/ui/components/button.rs` | `views/{transport,tracks,mixer,instrument,settings,palette}.rs` |
| Medidores verticales y barras horizontales | `src/ui/components/meter.rs` | `views/{tracks,mixer}.rs` |
| Fondo modal oscurecido reutilizable | `src/ui/components/modal.rs` | `views/{palette,settings}.rs` |
| Separadores y redimensionado de módulos (`SplitDrag`) | `src/ui/components/splitter.rs` | `ui/render.rs`, `views/{arrangement,workspace}.rs` |
| Carga y dibujo de SVG, estado de iconos asignados | `src/ui/icons.rs` | `assets/icons/toolbar/`, `views/toolbar.rs` |
| Acciones asignables y persistencia del toolbar | `src/ui/toolbar/{catalog,dispatch,picker,persistence}.rs` | `src/ui/profile.rs`, `src/catalog.rs`, `views/icon_picker{,_browser}.rs`, `toolbar/tests.rs`; perfil guarda ambos ámbitos y comandos personalizados |
| Estado del piano | `src/instruments/piano.rs` | `audio/piano/`, `views/instrument.rs` |
| Editar pasos/velocity | `src/ui/pattern_editor.rs` | `src/musical/model.rs` |
| Dibujar pasos | `src/ui/views/pattern.rs` | Controlador anterior |
| Layout y contraste del Arrangement | `src/ui/views/arrangement.rs` | `views/tracks.rs`, `timeline/{appearance,paint}.rs`; la superficie y la selección se derivan del tema activo |
| Track Headers configurables (TCP estilo REAPER) | `src/ui/views/tracks.rs` | `components/button.rs`, `views/settings_tracks.rs`, `design.rs`; identidad, mezcla, estado e instrumentos/FX comparten agrupación visual y opciones |
| Estado M/S/R y audibilidad de pistas | `src/musical/track/mix.rs`, `src/ui/audio_actions.rs` | `src/musical/track.rs`, `views/tracks.rs` |
| Workspace Arrangement + piano roll acoplado/separado | `src/ui/views/workspace.rs` | `src/ui/piano_roll/window.rs`, `views/piano_roll/` |
| Pointer, selección y borrado de notas/regiones | `src/ui/piano_roll/{pointer,selection,edit,drag}.rs` | `keyboard`, `commands`, `actions` y `views/piano_roll/{toolbar,grid}`; catálogo compartido, Delete/Backspace, Ctrl+A y doble clic |
| Suavizado visual de notas y snap | `src/ui/piano_roll/motion.rs` | `drag.rs`, `views/piano_roll/grid.rs`; interpolación por frame solo visual, independiente del motor |
| Empuje entre notas MIDI | `assets/scripts/midi/push.lua`, `src/scripting/push.rs` | `ui/piano_roll/push.rs`: VM por gesto, recorte de vecinas del mismo tono a izquierda/derecha, límite de un tick; commit validado al soltar |
| Borde y sombra de notas | `src/ui/views/piano_roll/note_frame.rs` | Canvas existente; borde de 0,7 px y sombra GPU tenue, sin alterar picking ni contenido |
| Copiar/cortar/pegar notas | `src/ui/piano_roll/clipboard.rs` | Reutiliza selección; conserva expresión, offsets y mute |
| Puntero GPU / efectos de cursor (`Fueguito`, etc.) | `src/ui/cursor_fx.rs` | `render.rs`, `views/settings.rs`, `views/chrome.rs` |
| Piano clicable | `src/ui/views/instrument.rs` | `instruments/piano.rs` |
| Mixer de ejemplo | `src/ui/views/mixer.rs` | Es mock, no medidores reales |
| Título/estado | `src/ui/views/chrome.rs` | `views/transport.rs` para botones |
| Zoom y conversión visual | `src/ui/timeline/viewport.rs` | `timeline/mod.rs` |
| Drag/scroll/picking | `src/ui/timeline/interaction.rs` | `timeline/tests.rs` |
| Sombra al importar MIDI/audio por drag & drop | `src/ui/timeline/import_preview.rs` | `views/arrangement.rs`, `timeline/paint.rs`; guía de entrada de un compás, sin leer archivos durante el arrastre |
| Gestos de touchpad (pinch, dos ejes, Ctrl/Shift+rueda) | `src/ui/navigation.rs` | `timeline/interaction.rs`, `views/arrangement.rs`; `PinchEvent` nativo de GPUI, sin dependencia adicional |
| Zoom, desplazamiento y picking del piano roll | `src/ui/piano_roll/navigation.rs` | Super+scroll: altura; Ctrl+scroll/pellizco: ancho; Alt+scroll/Shift+pellizco: subdividir selección. `geometry.rs`, `navigation_tests.rs`, `views/piano_roll/{grid,grid_background}.rs` |
| Recorte no destructivo y solapamiento de clips | `src/ui/timeline/trim.rs` | `midi_clips.rs`, `waveform.rs`, `timeline/tests.rs` |
| Brillo de borde de clips en Arrangement | `src/ui/timeline/clip_frame.rs` | `interaction.rs` detecta el extremo con el mismo umbral que `trim.rs`; `paint.rs` y `waveform.rs` comparten el dibujo para audio/MIDI |
| Menú contextual de clips audio/MIDI e inserción | `src/ui/clip_menu/` | `views/clip_menu.rs`, `components/context_menu.rs`; filas y acciones compartidas por ratón/teclado |
| Selector visual de color de clips y pistas | `src/ui/components/color_picker.rs` | `views/{clip_menu,track_color_menu}.rs`; `design.rs::clip_palette` mantiene los índices existentes |
| Renombrar, duplicar, silenciar y mover clips | `src/ui/clip_actions.rs` | `audio/assets.rs` conserva el nombre de cada instancia de audio |
| Dividir y restaurar recorte de clips | `src/ui/timeline/clip_edit.rs` | `clip_edit_tests.rs`; fuentes intactas, respeta bloqueo y origen temporal |
| Regla/cursor/carriles | `src/ui/timeline/paint.rs` | `waveform.rs` |
| Marcas y números adaptativos del Arrangement y piano roll | `src/ui/time_grid/musical.rs` | `components/bar_grid.rs`, `timeline/paint.rs`, `views/piano_roll/grid_background.rs`: ticks enteros, estilos y etiquetas comunes según zoom; respeta resolución del piano roll sin cambiar snap. Verificar con `cargo test --bin daw-gpui ui:: --locked` en gpui-dev |
| Imán y guía de snap al arrastrar (clips y notas) | `src/ui/time_grid/magnet.rs` | Distancia en píxeles a la línea destino (puro). `components/snap_guide.rs` dibuja intención, captura, destello y etiqueta de posición; lo usan `timeline/interaction.rs` (inicio del clip, imán libre) y `piano_roll/push.rs` + `drag.rs` (preview pegado; el commit sigue ajustando a la resolución) |
| Tempo y compás editables | `src/musical/time.rs`, `src/ui/timing.rs` | `src/ui/timeline/mod.rs`, `views/transport.rs`, `actions.rs`, `catalog.rs`, captura/importación MIDI, reproducción/exportación |
| Regla inferior de minutos/segundos | `src/ui/timeline/time_ruler.rs` | `views/arrangement.rs`; mismo viewport, picking y gestos que la regla musical |
| Partículas | `src/ui/timeline/particles.rs` | `paint.rs`, tokens en `design.rs` |
| Decodificar archivo | `src/audio/assets.rs` | Symphonia, trabajo offline |
| Opciones/parser y serialización de preferencias de exportación | `src/export/{mod,command,preset}.rs` | `tests/export.rs`, `docs/EXPORT.md`; argumentos explícitos prevalecen sobre los valores guardados |
| Acción de exportación y snapshot de Arrangement | `src/ui/export.rs` | `ui/export/hilo.rs`, `input.rs`, `catalog.rs`, `render.rs`; captura ramas e instrumentos del Hilo sin abrir audio físico |
| Mezcla offline y fuentes | `src/audio/export/{render,sources,hilo}.rs` | `job.rs`, `tests.rs`, `audio/piano/voice.rs`; posiciones por frames, rango común y plan de Tejer compartido con Play |
| Codecs C y publicación de exportaciones | `src/audio/export/{sndfile,lame,soxr,native,files}.rs` | `docs/EXPORT.md`; carga dinámica y archivos temporales |
| Síntesis compartida live/offline y snapshot del piano | `src/audio/piano/{backend,offline}.rs` | `render.rs`, `instruments/piano.rs`; sin abrir CPAL en export |
| Elegir formato CPAL | `src/audio/output.rs` | Frecuencia, buffer y pareja física de salida; todos los reproductores usan `AudioConfig` |
| Paneo estéreo sin asignaciones en callback | `src/audio/stereo_pan.rs` | `audio/playback.rs`, `audio/piano/render.rs` |
| Reproducir clips | `src/audio/playback.rs` | Varias pistas; remuestreo SoXR preparado en worker, ruteo estéreo a interfaz multicanal |
| Reproducir patrón | `src/audio/drums/mix.rs` | Fuente provisional del Mix; loop por frames en la misma salida |
| Sintetizar percusión | `src/audio/drums/voice.rs` | Mapeo instrumento → voz |
| Resaltar paso sonando | `src/ui/pattern_editor.rs` | `views/pattern.rs` |
| Ejecutar la app | `scripts/run.sh` | `scripts/check.sh` |
| Descargar bancos FreePats | `scripts/fetch_soundfonts.sh` | `scripts/soundfonts.txt`, `assets/soundfonts/freepats/` |
| Descubrir/cargar SoundFonts y créditos | `src/audio/piano/soundfont.rs` | `HAY_SOUNDFONT`, `instruments/piano.rs` |
| Render del piano | `src/audio/piano/render.rs` | Fachada `piano/mod.rs` |
| Notas/validaciones | `src/musical/model.rs` | `tests/musical.rs` |
| Conversión tick/frame | `src/musical/time.rs` | Scheduler y tests temporales |
| Programar eventos y loop | `src/musical/scheduler.rs` | `drums/mix.rs`, `examples/sequencer.rs` |
| DSP y composición | `src/primitives/`, `src/component.rs` | `tests/elevate.rs` |
| Descriptor/fábrica de dispositivos internos y Hay Utility | `src/devices/{mod,utility}.rs` | `catalog.rs`, `tests/utility.rs`; Component → Volume → FunDSP |
| Slot Utility de salida y snapshot para audio | `src/devices/utility_slot.rs` | `audio/{playback,piano/render}.rs`; prepara fuera del callback |
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
el loop de percusión vive en `audio/drums/mix.rs`; `PatternAction` no es un bus;
`playback/mix.rs` realiza la suma. `catalog` indexa metadatos: no
instancia primitivos, componentes ni instrumentos. El buscador asigna el piano actual a la
pista destino mediante `musical/track.rs`; `:sa` crea samplers por pista.
La asignación es estado musical: no la guardes en la vista ni en `instruments::Piano`.

La vista del piano no debe importar `audio::piano` ni `rustysynth`. El estado de ejecución
(note-on/off, ratón, voces abiertas) pertenece a `src/instruments/piano.rs`; la UI solo
presenta el descriptor del catálogo y llama al controlador. Al añadir un instrumento,
registra su entrada en `src/catalog.rs` y evita duplicar comandos entre teclado y ratón.

La percusión usa un plan inmutable y síntesis provisional. Si cambias el modelo de notas,
detén y vuelve a pulsar Play para escuchar el plan nuevo. No añadas asignaciones,
reservas ni liberaciones costosas al callback de `playback.rs`.

## Comandos (desde la raíz)

```bash
# Formato y verificación completa del incremento
distrobox enter gpui-dev -- bash scripts/check.sh

# Solo contratos musicales, sin UI ni dispositivo
distrobox enter gpui-dev -- cargo test --no-default-features --test musical --locked

# Registro de paquetes, sandbox MIDI y compatibilidad de acciones Lua
distrobox enter gpui-dev -- cargo test --no-default-features --lib --test lua_packages --test lua_tools --test scripting --test hay_acent --test catalog --locked

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

Backend PipeWire: instalar `pipewire-devel clang-devel` en gpui-dev; `cargo run --example audio_devices -- SSL PipeWire` enumera nombres/canales sin emitir audio. `cargo run --example audio_devices -- "Line Outputs 3 + 4" PipeWire --connect-silent` abre ocho segundos de silencio y comprueba actividad del callback; no cambia la salida del sistema.
