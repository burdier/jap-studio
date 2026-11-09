# Arquitectura del prototipo

## Sampler sfizz

`devices/sampler` describe parámetros sin GPUI ni audio. `audio/sampler` posee el ABI C,
SFZ generado y recursos preparados. `instruments/sampler` posee teclas, motor preparado
y reproductor; `ui/sampler` solo carga, coordina y presenta controles existentes.
La instancia de sampler usa InstrumentId(2), el piano conserva InstrumentId(1).
El [+] asigna cada instancia mediante `musical::track::assign_instrument`.
La acción `track.insert_sampler` (`:sa`) reutiliza `ui/track_actions` para insertar
una pista de instrumento (tipo Synth, presentada como Sampler por su asignación).
Si la instancia ya está asignada, selecciona y abre su pista sin cambiar la topología.

`audio/piano/backend` acepta ahora SoundFont o sfizz. El constructor del stream recibe
un backend preparado y reutiliza el scheduler, cola acotada, fader, paneo y Utility.
No hay otro transporte ni mezcla multipista nueva. Los parámetros continuos del sampler
viajan como CC de alta resolución; cambios estructurales reemplazan el motor preparado
fuera del callback y cortan las notas anteriores. `SamplerSnapshot` retiene el WAV
compartido durante exportación; las instancias offline conservan colas de release.
Detalles de uso, límites y dependencia fijada: [SAMPLER.md](SAMPLER.md).

## Dos targets, un paquete

El explorador de bibliotecas tiene una frontera propia. `elevate::explorer` conserva
raíces, expansión, cache de directorios y filas; `scan` lee un solo nivel y ordena carpetas
primero. No importa GPUI, audio ni tipos del DAW. `persistence` guarda las rutas absolutas
en `dirs::config_dir()/hay-daw/libraries` (normalmente `~/.config/hay-daw/libraries` en Linux),
con reemplazo atómico. Quitar un acceso nunca borra archivos; una carpeta no disponible
permanece guardada. Las rutas de bibliotecas deben ser UTF-8.

`ui::explorer::Explorer` es una entidad GPUI independiente, sin observación del transporte.
El workspace la incrusta como vista cacheada con tamaño definido; `uniform_list` construye
solo las filas visibles. Restauración, lectura al expandir y persistencia se ejecutan en
background. Los resultados de escaneos cancelados se descartan y las escrituras se serializan
para que un guardado antiguo no sobrescriba otro reciente. Los cambios de disco se consultan
mediante «Recargar»; todavía no hay watcher. No se recorren enlaces simbólicos a carpetas.

`ui/explorer_host` recibe eventos de foco/cierre/inserción y conserva ancho/foco del dock.
La acción `explorer.insert` usa el archivo seleccionado; `file.import` acepta una ruta mediante
el contrato de parámetros. Doble clic y Enter llegan a la misma operación de importación.
`ui/file_import` prepara tanto audio como MIDI fuera de la UI, conserva TrackId y beat al
iniciar y resuelve la fila al terminar. Si la pista desapareció descarta el resultado; durante
una importación informa si se intenta iniciar otra. Grupos y un proyecto vacío requieren
seleccionar/crear una pista de clips. No añade dependencias ni cambia el motor de audio.

Las pistas del proyecto son una colección dinámica. `TrackId` es estable y no depende de la
fila; `TrackKind::Group` representa un contenedor de un nivel con hijos enlazados por `parent`.
`track/management` calcula el nuevo orden y un mapa de filas de origen. `ui/track_actions`
aplica la edición deteniendo reproducción/grabación, y remapea clips, niveles, pan, colores y
MIDI Learn. Los clips aún guardan índices de fila: la traducción se centraliza en
`timeline/track_edit`, con IDs MIDI nuevos solo al duplicar. Las importaciones asíncronas
capturan TrackId y resuelven la fila al terminar, o descartan el resultado si se eliminó.
`Track.chain` es ahora `musical::fx::FxChain`, con nodos Effect/SerialChain/ParallelChain e IDs
estables. Cada hijo directo de ParallelChain es una rama de la misma entrada; una rama con
varios pasos se representa mediante SerialChain. Las ramas se suman sin normalizar; un
contenedor vacío o en bypass pasa la entrada. La edición es transaccional: mover no cambia
IDs, duplicar renueva todos los IDs, y se rechazan ciclos/destinos inexistentes. Se limitan
las cadenas a 256 nodos y 32 niveles. Duplicar pistas también renueva sus IDs FX.
Hay Utility es el efecto disponible. VST3 conserva su identidad declarativa, sin host.
Un bus general de grupo y la carga de plugins requieren un incremento específico del motor.

El plegado y las alturas de pista pertenecen a `ui::TrackPresentation`, indexado por TrackId,
no al estado musical. `TrackLayout` calcula filas visibles con índice de pista, origen Y y
altura. Cabeceras, canvas de clips/carriles, picking, recorte, hover, arrastre y sombra de
importación comparten esa geometría. Ocultar hijos no elimina clips ni cambia el snapshot de
audio/exportación. Las alturas y el plegado se conservan al reordenar y se copian al duplicar;
se eliminan sus entradas cuando desaparece la pista. Son estado de sesión hasta disponer
de guardado de proyecto. El valor predeterminado sigue siendo `TRACK_HEADER_HEIGHT` del perfil.

El TreeView se extrae del explorador original a `ui/components/tree.rs`: ambos árboles llaman
al mismo renderer de fila, superficie, cabecera, pie y `uniform_list`. Conserva alturas,
sangría, guías, colores, tipografía e iconos. Los adaptadores aportan datos e interacciones;
no hay un segundo árbol visual. `ui/fx` es otra entidad cacheada, con presentación por TrackId.
Solo reconstruye filas por cambios de topología/expansión; nombre y bypass parchean filas.
Cambiar el track conserva selección, expansión y scroll de los demás. Reordenar conserva IDs.

Add sigue siendo el mismo buscador completo, con sus categorías/acciones/instrumentos. Su
contexto FX captura TrackId/FxId: inserta dentro de un contenedor o después de un efecto.
El botón FX junto al [+], el menú de pista y `fx.show` abren el panel; su ancho es redimensionable.
A abre Add desde el árbol, Enter/doble clic abre parámetros, B cambia bypass, F2 renombra,
Ctrl+D duplica y Delete quita. Espacio sigue llegando al transporte. Los menús reutilizan
`components/context_menu` y los parámetros `ActionForm`; un formulario conserva el destino
original aunque la selección cambie. Un destino eliminado produce un error sin editar otro nodo.

`devices/fx_gain` compila únicamente las transformaciones lineales del Utility disponible:
serie multiplica y paralelo suma sus ganancias; el límite compuesto es 1024× por cadena.
No es un compilador de DSP general ni simula EQ, compresor, reverb o VST3. El adaptador de UI
valida antes de publicar; `audio/fx_gain` recibe un único escalar atómico, con rampa de 256 frames
para cambios en vivo. No transporta ni libera árboles en RT. Esta reducción preserva el resultado
estacionario de las cadenas de ganancia; la transición se suaviza para la cadena completa.
Se aplica antes del fader/pan y del Utility de salida, tanto a clips de audio como al piano.
Para grupos, su ganancia lineal se compone con cada hijo (equivalente a procesar la suma para
este único tipo de efecto); no crea un bus de plugins. Exportar captura la ganancia FX en
TrackMix y conserva el snapshot durante el trabajo. El patrón de percusión Session mantiene
su plan único sin buses de FX por pista. El motor live sigue sin mezclar varios clips/pistas.

Las acciones visuales consultan `CatalogAction::parameters()` y el registro headless
`actions::find`. Un `ActionSpec` declara claves estables, etiquetas, ayuda, valores iniciales,
campos opcionales y su enlace a argumentos. Las cantidades llevan unidad, límites (incluido
un mínimo exclusivo), incremento de edición y si admiten fracciones. `ParameterValues`
transporta texto, número, bool o ausencia sin depender de GPUI; `command_with` valida tipos
y codifica argumentos para el dispatcher existente, sin ejecutar un shell.

`ui/action_form` recoge esos valores desde el mismo formulario para catálogo e iconos.
Sus controles son `Input`, `NumberInput`, `Checkbox` y `Button` de GPUI Kit. Cada editor de
texto tiene una entidad `InputState` persistente durante la vida del formulario; Kit posee
selección, cursor, IME, portapapeles e historial de edición. `components/parameter_field`
traduce el contrato a esos estados y conserva el incremento musical, coma decimal y mínimos
exclusivos mediante `NumberInputEvent::Step`; confirmar sigue validando con `ActionParameter`.
Las suscripciones se liberan al cerrar. El diálogo limita Tab/Shift+Tab a sus controles,
recuerda la ventana/foco de origen y desplaza el campo enfocado a la vista. No intercepta
caracteres con el controlador musical. Exportar empieza con las preferencias guardadas y
valida además las combinaciones de exportación; Utility toma su ganancia actual.
OK, Enter desde un editor y Ctrl+Enter comparten controlador; Esc/Cancelar descartan.
Los comandos escritos siguen ejecutándose directamente. Este incremento no añade un motor
de scripts: deja disponible el contrato headless para esos futuros consumidores.

`Cargo.toml` usa `gpui-kit =0.9.0` (Apache-2.0) bajo el alias `gpui`, con componentes y assets.
La fachada alinea runtime/plataformas `gpui-pre =0.3.11`; no se mezclan con el antiguo GPUI
de Git. `ui/kit.rs` inicializa la biblioteca una vez y proyecta el perfil Hay a `Theme` solo
cuando cambia. Las tres ventanas montan `gpui::base::Root` para foco, overlays y controles.
Los atajos propios de Hay conservan un contexto separado de los campos. El formulario se
dibuja solo en su ventana de origen, también desde el piano roll separado.
Audio, contratos musicales y acciones headless no dependen de Kit; el explorador conserva
su entidad/cache y los canvas de Arrangement y piano roll siguen siendo específicos del DAW.
Referencias: [GPUI Kit](https://gpui-kit.com/docs/installation/) y
[controles](https://gpui-kit.com/component/input/).

- Biblioteca `elevate` (`src/elevate.rs`): DSP, núcleo musical, catálogo y árbol de bibliotecas headless.
- Binario `daw-gpui` (`src/main.rs`): instrumentos, adaptadores de audio e interfaz GPUI.
- `--no-default-features` permite trabajar en la biblioteca sin compilar GPUI/CPAL.
- No se necesita un workspace de ocho crates para mantener estas fronteras.

```text
main.rs → ui::run
             │
        DawApp (ui/mod.rs)
          ├── explorer_host ─→ Explorer (entidad independiente) ─→ elevate::explorer
          │       └── file_import ─→ audio::AudioFile / musical::smf (background)
          ├── input.rs ─────────────┐
          ├── views/* ──────────────┼→ pattern_editor.rs → musical/model.rs
          │                         ├→ palette.rs → catalog::search
          │                         │              → musical/track.rs
          │                         └→ audio_actions.rs
          ├── design.rs                          │
          └── timeline/*                         ├→ instruments::Piano → audio/piano
                                                 └→ audio/{playback,drums} → CPAL

Biblioteca headless:
musical/model.rs → scheduler::compile(TimeBase) → PreparedPattern
                                                   │
Transport::advance(frames) → FrameWindow → events_in()
                                                   ↓
                                     ScheduledEvent { sample_offset, ... }
                                                   │
                                   audio/drums/stream.rs → DrumKit
                                                   ↓
                                         Volume → CPAL
```

La ruta del patrón ya está conectada al callback de percusión. Clip y piano conservan
sus adaptadores separados; unificarlos es el siguiente incremento. `examples/sequencer.rs`
sigue mostrando el plan sin abrir audio. La vista del piano no conoce RustySynth ni CPAL:
pide notas a `instruments::Piano`, que posee el reproductor y las notas sostenidas.
El descriptor que pinta la vista procede de `elevate::catalog`.

## Responsabilidades y propiedad del estado

| Estado | Propietario actual | Restricción |
|---|---|---|
| Notas del patrón | `Pattern`, contenido en `ui::PatternEditor` | Sin coordenadas visuales ni voces |
| Selección, modo, vista | `DawApp` | Estado de workspace, no proyecto musical |
| Bibliotecas y árbol de archivos | `elevate::explorer::LibraryTree`, dentro de `ui::explorer::Explorer` | Persistencia independiente del proyecto; no observa transporte ni toca audio |
| Ancho y visibilidad del explorador | `ui::explorer_host::ExplorerDock` | Estado de sesión; e recupera el panel oculto por arrastre |
| Catálogo y búsqueda | `elevate::catalog` | Metadatos y comandos; sin GPUI/CPAL |
| Búsqueda abierta | `ui::Palette` | Query, selección, pista destino y resultados; no ejecuta audio |
| Configuración/ayuda | `ui::SettingsUi` | Visibilidad de panel y overlay; no modifica bindings |
| Preferencias de exportación | `ui::ExportPreferences`, dentro de `SettingsUi` | Valores guardados separados del borrador; validación headless y escritura atómica en el perfil antes de aplicar |
| Persistencia de apariencia | `ui/profile.rs`, llamado desde `settings.rs` | Guarda tema, colores efectivos y cursor en `~/.jaybu.profile`; conserva otras claves y comentarios, sin intervenir en audio |
| Pistas y asignación | `elevate::musical::track`, en `DawApp` | `Track.instrument` guarda identidad; no abre audio |
| Ventana separada | `ui::detached::DetachedPanel` | Solo presenta `render_instrument` y reenvía entrada a `DawApp`; no posee audio |
| Notas sostenidas y ratón | `instruments::Piano` | Fuera de `DawApp`; la vista solo lee estado |
| Zoom/cursor/arrastre | `Timeline` | Convierte píxeles ↔ beats, no gobierna audio |
| Gestos de navegación | `ui/navigation.rs` | Normaliza scroll/pinch nativos de GPUI; conserva ambos ejes y deltas precisos |
| Encuadre del piano roll | `PianoRoll::viewport`, `piano_roll/navigation.rs` | Tiempo, escalas horizontal/vertical independientes y registro compartidos por canvas/picking; sin modificar notas ni transporte |
| Clips del Arrangement | `Timeline::{clip,other_*_clips}`, provisional | Se conservan al insertar, mover y solapar; aún no hay modelo de proyecto |
| Archivo decodificado | `Arc<AudioFile>` | Preparado fuera del callback |
| Voces del piano | RustySynth dentro del callback | No pasan por GPUI |
| Voces de percusión | `DrumKit` en `audio/drums/voice.rs` | Banco fijo; sin asignaciones por nota |
| Plan de notas | `PreparedPattern` | Inmutable; percusión lo compila en Play |
| Posición de transporte | `Transport` en cada callback / demo headless | Avanzar con frames procesados |
| Posición del patrón | `AtomicU64` en `DrumState` | Snapshot de solo lectura para la UI |
| Partículas | `ParticleTrail` | Tiempo visual; no afecta al scheduler |

## Catálogo y buscador

`elevate::catalog` es un índice estático y headless de cuatro categorías: instrumentos,
primitivos, componentes y acciones. Cada entrada declara ID, nombre, resumen y palabras
clave. `search` pliega acentos y prioriza coincidencias al inicio del nombre. El catálogo
no conoce GPUI, CPAL ni instancias.

`ui/palette.rs` conserva query, selección y pista destino; `views/palette.rs` dibuja el
overlay. El botón `[+]` de cada cabecera apunta a esa pista; la tecla `a` y el comando
`:add` apuntan a la pista seleccionada. Las entradas `Invoke("...")` se traducen al comando
provisional existente en `ui/input.rs`; las entradas `Unavailable` explican por qué todavía
no se pueden instanciar.

`ui/toolbar/catalog.rs` descubre las entradas ejecutables de tipo acción y añade controles
locales de UI por ámbito (Transporte o Piano Roll). No duplica el catálogo headless.
`dispatch.rs` llama a los mismos controladores que buscador/teclado; `picker.rs` comparte
la asignación entre ratón y Enter. `ui/icons.rs` conserva carga SVG y asignaciones en memoria;
`views/icon_picker{,_browser}.rs` presenta búsquedas independientes y listas desplazables.

Al elegir un instrumento, `ui/palette.rs` llama a `musical/track.rs::assign_instrument`,
que asigna su identidad a la pista destino y la retira de cualquier otra. La cabecera y la
vista Instrument muestran esa asignación. Piano y sampler tienen una instancia cada uno: moverlo de pista lo reasigna. `Volume` y `Componente serial` aparecen como
metadatos: el buscador aún no crea primitivos, componentes ni instancias nuevas. Ese paso
pertenece al futuro host de plugins y al sistema de acciones, no a esta paleta.

## Contrato musical actual

- `MidiClip::id` identifica una instancia durante la sesión. `clone()` conserva identidad
  para snapshots; `duplicate()` crea otra para duplicación y división. La selección múltiple
  del Arrangement usa esos IDs y sobrevive a la activación/reordenación de clips.
- `musical::materialize` prepara un patrón nuevo en ticks con las ventanas visibles de los
  MIDI seleccionados. `timeline::materialize` sustituye los originales solo tras éxito;
  ninguna preparación pasa por el callback. Consolidar elimina el contenido oculto del
  resultado y conserva los silencios, no modifica otras instancias de clip.

- PPQ = 960 ticks por negra. `Tick` admite enteros con signo; este prototipo valida
  posiciones de patrón no negativas. La resolución de la vista no modifica PPQ.
- Patrón de longitud arbitraria positiva. Varias notas pueden compartir tick e instrumento.
- IDs de nota estables, locales al patrón y no reutilizados. No son UUID de proyecto.
- Notas con duración positiva, pitch 0–127 y velocity 1–127; terminan dentro del patrón.
- `compile` genera NoteOn/NoteOff, redondeando posiciones absolutas a frames; mínimo
  un frame de duración. No suma duraciones redondeadas paso a paso.
- `PreparedPattern::length_frames()` mide una vuelta, con mínimo de un frame; el stream
  de percusión divide cada bloque en tramos que no cruzan el final del loop.
- `events_in([start, end))` no reserva memoria: búsqueda binaria + eventos del bloque.
- Los NoteOff preceden a NoteOn coincidentes. Un NoteOff justo al final del patrón
  pertenece a la siguiente ventana: el consumidor debe procesarlo antes de cerrar voces.
  La percusión los ignora porque cada voz es un one-shot con envolvente propia.
- El transporte usa frames, no reloj de pared. Stop conserva posición; seek es explícito.
- La audición del patrón mapea `frame_global % length_frames` a tiempo local. Los eventos
  se consultan por segmento, así que el offset dentro del buffer sigue siendo exacto.
- No hay chase de notas sostenidas. Ante stop/seek/cambio de plan, el futuro host
  deberá liberar voces y definir la política de reanudación.
- El plan activo de percusión es inmutable: editar notas no lo actualiza. Detener y
  volver a pulsar Play recompila. Loop configurable y cambios de tempo necesitan
  segmentación y una política de reemplazo; no implementar un segundo reloj.
- Hay límites de representación explícitos y errores; comprobarlos antes de publicar un plan.

## Audio existente y siguiente frontera

### Exportación offline

`elevate::export` define opciones y parser sin UI ni codecs. `ui/export.rs` captura el
Arrangement y configura un worker con `audio/export::Snapshot`, que posee clones de clips,
referencias a audio inmutable, parámetros SoundFont, niveles/pan/mute/solo y Utility.
El worker procesa bloques de 512 frames; no abre CPAL ni toca el transporte o el playhead.
Los individuales tienen un rango común; el master suma todas las fuentes seleccionadas.
El preroll prepara el estado antes de escribir rangos recortados; la cola libera la entrada.

`audio/piano/backend.rs` y el consumidor del scheduler se comparten con la audición.
`PianoSnapshot` contiene configuración, nunca voces o streams. Las voces se crean por clip
en el worker para conservar solapamientos sin NoteOff cruzados entre clips. Es un host
offline provisional del piano actual, no un sistema de instancias por pista.

Los codecs son adaptadores C acotados a libsndfile, LAME y libsoxr, con handles RAII locales
al worker. `files.rs` prepara archivos temporales y publica el lote sin sobrescritura.
No se guarda el render completo en RAM: la salida se escribe por bloques; las fuentes
decodificadas y su conversión de frecuencia sí residen en memoria como en la importación.
La UI observa progreso atómico y un resultado por canal; cancelar solo publica una bandera.
Opciones y límites: [EXPORT.md](EXPORT.md).

### Primer dispositivo interno: Hay Utility

`devices::find("hay.utility")` devuelve el descriptor y su fábrica headless. Cada construcción
crea un `Utility` independiente, compuesto por un nodo `Volume` de FunDSP mediante `Component`.
`Device` es el contrato mínimo de procesamiento, parámetros y bypass; no abre dispositivos.

La aplicación posee un `UtilitySlot` de salida de audición (no uno por pista). Las acciones UI
validan y publican estado en un solo `AtomicU64`; cada adaptador prepara la instancia DSP antes
de construir el stream. El callback lee una vez por bloque, aplica cambios y procesa después
de Volume de pista/paneo, antes del clamp final. Insertar y retirar usan la instancia preasignada
y transiciones a ganancia unitaria: no construyen ni destruyen procesadores dentro del callback.
Los parámetros sobreviven al reinicio del stream, no al cierre de la aplicación.

El antiguo control global de ganancia/bypass ahora pertenece a Utility. Los faders y Mute/Solo
son anteriores e independientes. Las fábricas permiten instancias independientes, pero el host
UI conserva el slot de salida independiente del Track FX Tree. Las cadenas de pista disponibles
se reducen a ganancia lineal; no hay carga dinámica. El catálogo publica metadatos/comandos,
la construcción de dispositivos y compilación de cadenas viven en `devices`.

`audio/output.rs` comparte selección de formatos. `playback.rs`, `piano/` y `drums/`
poseen streams separados y excluyentes. El clip exige la frecuencia del archivo;
piano y percusión siguen la del dispositivo. El master actual es un clamp, no un limitador.

`instruments::Piano` envuelve `audio::piano::PianoPlayer` y conserva las notas sostenidas,
la nota del ratón y el descriptor del catálogo. La UI ya no importa el reproductor ni
decide cuándo abrir el dispositivo: solo llama a `note_on`/`note_off`, y el instrumento
abre CPAL en la primera nota. La exclusividad con clip y percusión sigue coordinada por
`ui/audio_actions.rs`.

El Arrangement admite solapamientos sin sobrescribir clips. El recorte de bordes cambia la
ventana visible (`trim_start`/`trim_len`) y conserva el archivo o patrón fuente. El clip activo
se dibuja encima de los anteriores de su tipo; los MIDI se dibujan sobre audio. Esta política
de edición/presentación no implica mezcla: el adaptador de audio reproduce un clip a la vez.

`ui/clip_menu` posee el contexto y las acciones del menú de Arrangement, separado de
`PianoRoll`. Su lista de opciones sirve tanto al teclado como al ratón; solo MIDI ofrece
abrir el piano roll. `ui/clip_actions.rs` concentra las operaciones comunes y
`timeline/clip_edit.rs` divide/restaura ventanas sin modificar las fuentes. El nombre de
un `AudioClip` pertenece a la instancia, separado del nombre de su `Arc<AudioFile>`.
Dividir conserva la alineación de las dos ventanas; restaurar recupera el inicio original
hasta el límite del beat cero. Silenciar, cambiar ganancia y editar ventanas detienen
la reproducción actual; no representan edición de planes en caliente.

`drums/mod.rs` compila el plan y abre el stream. `drums/stream.rs` avanza `Transport`
por frames, segmenta en el loop y entrega offsets exactos a `DrumKit`; `Volume` procesa
el bloque antes de CPAL. La síntesis es deliberadamente provisional (sin samples) y el
plan se recompila solo al iniciar la reproducción.

La siguiente integración extraerá procesadores del clip, piano y percusión, abrirá un solo
stream y consumirá eventos entre subbloques. El transporte vivirá ahí. GPUI recibirá
snapshots; editará comandos y planes preparados. `rtrb` sigue pendiente de aprobación.
También se debe definir dónde se liberan planes/samples retirados: nunca delegar
esa destrucción costosa al callback por accidente. Para percusión, el siguiente paso
es un sampler con cuatro samples y reemplazo de plan en caliente.

## Extensibilidad

Elevate mantiene `Primitive`, `Component`, parámetros y `NodeId`. `ControlEvent`
es todavía una solicitud síncrona de cambio, no un bus de notificaciones.
`PatternAction` es una operación compartida de UI, no el catálogo global de acciones.
`elevate::catalog` es el primer índice de instrumentos, primitivos, componentes y acciones;
sus comandos son strings provisionales hasta que exista el sistema de acciones descrito
en el README. El futuro host conectará eventos musicales y DSP sin importar tipos de GPUI.

## Entrada MIDI reutilizable

Capas, de dato a pantalla; ninguna importa a la siguiente:

1. `musical/midi.rs` (hecho): `MidiMessage` (NoteOn/NoteOff/AllNotesOff), `MidiTarget`
   (Instrumento o Sequencer) y `MidiServer` (arquitectura servidor/cliente). Los controladores
   publican mensajes en el servidor sin saber quién escucha; cuando el foco cambia de cliente,
   `MidiServer` emite automáticamente los `NoteOff` pendientes al cliente anterior para que
   ningún instrumento se quede secuestrando el MIDI ni con notas colgadas.
2. `musical/controller.rs` (hecho): `ComputerKeyboard`, teclado del ordenador como MIDI estilo
   Ableton: `M` activa/desactiva, `a w s e d f t g y h u j k o l p ;` notas, `Z`/`X` octava,
   `C`/`V` velocity. Estado propio (octava, velocity, teclas sostenidas); produce mensajes.
   Desactivado, solo reacciona a `M`. Un controlador nuevo (MIDI físico, pad XY) produce los
   mismos `MidiMessage`.
3. `ui/audio_actions.rs`: `apply_midi` publica en `MidiServer` y despacha los `RoutedMidi` al
   cliente activo (`Instrument` o `Sequencer`); `sync_midi_focus` actualiza el cliente suscrito
   al cambiar de vista, pista o ventana.
4. `musical/keyboard.rs` + `ui/components/keyboard.rs` (hecho): teclado clicable reutilizable
   con barra de octavas; emite la nota por callback.
5. Pendiente: pitch bend / CC y su mapa fuente → destino; MIDI físico (`midir`, requiere aprobar).
