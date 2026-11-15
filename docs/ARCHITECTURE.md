# Arquitectura del prototipo

## PR-07, segundo incremento: modelo de Hilo

`musical/hilo.rs` compila las hijas de un grupo como ramas con puertos
`Events` o `Audio`, IDs estables e instrumentos distintos.
`musical/arrangement_midi/hilo.rs` prepara fuera del callback la distribución
`Layer` o `RoundRobin` y conserva la rama elegida para cada NoteOff. El modo
se guarda en el documento Loro junto al grupo; un grupo Hilo puede contener
clips MIDI. Todavía falta conectar estos planes con el motor de audio y la UI.

## PR-06: contexto temporal y propiedad de instrumentos VST3

`audio/vst3/context.rs` convierte la posición en frames del transporte de Play a
`ProcessContext`. `audio/playback/mix.rs` lo entrega a los efectos VST3 de cada pista
por el plan FX existente; serie, paralelo y subbloques preservan el offset absoluto.
Se marcan como válidos BPM, compás, posición musical, inicio del compás y ciclo
cuando existe loop. No se anuncian reloj de sistema, grabación ni muestras continuas
sin una fuente real. La audición live conserva su contexto local sin esos flags.
`ui/vst` asigna un `InstrumentId` a cada instancia VST3 cargada en una pista. El
foco cambia la instancia activa y conserva las demás por ID; NoteOff vuelve a su
dueño original. Cada instancia mantiene editor, mezcla y cadena FX propios. Tras
cambiar la salida, la UI las recarga en serie con la nueva frecuencia. En Play,
`audio/playback/mix` toma esos mismos procesadores, enruta eventos MIDI absolutos
por ID y los suma con WAV antes de FX, nivel y pan. Los streams de audición quedan
silenciosos y vuelven a sonar al detener; una cola acotada entrega el MIDI en vivo
al procesador durante Play. Un CAS sin espera impide ejecutar un VST3 desde dos
callbacks a la vez. Stop/seek/loop liberan notas; el contexto del transporte llega
también al instrumento. La cola de salida se acota entre 2 y 10 segundos tras el
último evento MIDI; aún no hay compensación de latencia ni exportación VST3.

## PR-05: instrumentos internos y mezcla por pista

`audio/piano/voice.rs` separa la síntesis del stream CPAL. `PreparedVoices` identifica
los motores por `InstrumentId`; `musical/arrangement_midi` compila clips a eventos
absolutos con TrackId, canal y offset de frame. `audio/playback/mix.rs` prepara una voz
por pista con MIDI y suma su salida con los WAV de esa pista antes de FX, nivel y pan.
La preparación de SoundFont, sfizz, recursos y plan ocurre en el worker, fuera del
callback. Session añade `audio/drums/mix.rs` al mismo reloj y salida. El plan de
percusión activo sigue inmutable hasta detener y volver a pulsar Play.
Play también prepara las voces internas asignadas a pistas sin clip MIDI. Una cola
acotada lleva sus notas en vivo al mismo `Mix`; tocar piano o sampler durante un
WAV no detiene el transporte. Fuera de Play, la audición usa su adaptador CPAL.

`:sa` crea un `InstrumentId` y un `Sampler` nuevos por pista. El editor mantiene una
instancia activa y guarda las otras por ID; cambiar de foco no comparte parámetros ni
muestras. Al eliminar una pista se retira su instancia después de liberar notas.
`MidiServer` conserva el destino de cada NoteOn para enviar NoteOff a la misma voz,
incluso si el foco cambió. Una liberación tardía tras Stop se descarta. El piano
conserva una instancia y la audición live aún usa su adaptador individual.

Seek/loop liberan notas en una discontinuidad; no reconstruyen notas sostenidas.
El plan toma una vuelta visible por clip MIDI; el loop individual del clip y el
render offline de varias instancias sampler no forman parte de esta integración.

## Foco visual y navegación opcional

`ui/workspace_focus` guarda solo el panel activo y el panel ampliado; la vista usa
las mismas entidades y componentes con todo el espacio del área de trabajo. No
modifica la proporción de división, anchos de docks ni contenido musical. El cierre
u ocultación del editor invalida la ampliación. Las ventanas GPUI separadas usan
`Window::zoom_window`; el foco de entrada determina el destino de `view.maximize`.
Los overlays y campos de formulario conservan las teclas de texto. `f` se reserva
para ampliar en Normal antes de interpretar notas del teclado; F11 queda disponible
sin Vim. Espacio y el audio comparten los controladores existentes.

`ui/keyboard_preferences` carga y guarda `VIM_ENABLED` mediante el escritor atómico
del perfil. No pertenece al tema ni al controlador MIDI. Desactivar cancela estados
modales y operadores pendientes, conserva entrada convencional y mantiene la acción
`vim.toggle` accesible desde catálogo y Configuración → Atajos. Por defecto Vim
sigue activado para preservar el comportamiento de perfiles existentes.

## Spectrum dentro del plan FX

`devices/spectrum` declara `hay.spectrum` sin importar UI ni audio. El árbol guarda un
`TrackDevice::Hay` y conserva las operaciones existentes por `FxId`. `audio/effects`
compila un nodo de captura en su posición de señal, incluida cada rama serie/paralelo.
En cadenas VST3 el tap recibe el bloque real entre procesadores. Las cadenas puramente
Utility conservan la ruta escalar y su rampa existente: el plan de observación evalúa
una copia acotada con los coeficientes de cada nodo para capturar puntos intermedios,
sin duplicar ganancias en la salida. Durante una rampa Utility, esa captura representa
el coeficiente objetivo del nodo; el medidor del piano sigue observando su salida real.

`audio/analyzer/tap` copia bloques estéreo en una cola SPSC acotada. Un CAS sin espera
protege al productor si coinciden fuentes; saturación omite mediciones, nunca audio.
No hay FFT, GPUI, bloqueo, E/S ni reserva de memoria en la captura. Los planes se
publican usando el RCU existente y se destruyen fuera del callback. Sin VST3 la ruta
acepta la frecuencia real del stream, transportada junto a cada bloque de muestras.

`ui/fx/analyzers` mantiene consumidores, modos e historia por ID; limpia datos al
cambiar topología, frecuencia o disponibilidad. Desactiva tomas ocultas y heredadas
por bypass. `audio/analyzer` calcula energía estéreo (evita cancelación mono), FFT
Hann de 2048 muestras, 96 bandas, historia de 128 ventanas, peak y RMS. `ui/spectrum`
adapta resultados a datos visuales; `ui/components/spectrum` no importa el motor.
Piano e inspectores Track FX comparten ese canvas y los controles `< >`. Cambiar la
vista no modifica el plan de audio. Las acciones del catálogo comparten controladores
con esos controles; el historial/modo no se serializan como parámetros DSP.

## Dispositivos y configuración de audio

`audio/device::AudioConfig` es una configuración de máquina, separada del proyecto
musical: backend, IDs CPAL de entrada/salida, frecuencia, buffer y canales físicos.
`audio/device/inventory` consulta capacidades en background. Conserva los dispositivos
que CPAL enumera aunque consultar formatos falle (por ejemplo, ALSA ocupado), junto
a dirección y error por entrada/salida; no confunde un error con ausencia de hardware.
En Linux, `audio/device/aliases` agrupa rutas ALSA conocidas por tarjeta/endpoint
físico (resolviendo el ID de tarjeta mediante `/proc/asound`), conservando los alias
para reconocer preferencias anteriores. No agrupa solo por nombre; conserva endpoints
distintos y rutas personalizadas. Las capacidades visibles proceden de `hw`, evitando
confundir los canales virtuales de `plughw` con salidas físicas. Los pares estéreo
se exponen como Salida 1 (1/2), Salida 2 (3/4), etc.
La negociación valida los formatos compatibles al aplicar. `audio/output` negocia
un formato compatible y distribuye el master estéreo a la pareja elegida, dejando
las otras salidas en silencio; hasta 256 canales físicos, sin reservas en el callback.
CPAL habilita `pipewire` (dependencias pipewire/libspa 0.10, licencia MIT).
En Linux, la configuración Auto sin IDs previos prefiere PipeWire. Los perfiles con
IDs existentes conservan su backend; la UI ofrece cambiar explícitamente a PipeWire.
Los endpoints se seleccionan por ID de nodo y conservan las descripciones del sistema.
`DeviceInfo.address` permite ofrecer salidas hermanas del mismo endpoint físico en el
selector estéreo sin fusionar sus IDs ni sumar sus canales. La operación `OutputRoute`
cambia nodo y canal local juntos: la salida física 3/4 es un nodo estéreo de dos canales.
El stream nativo comparte el servidor y no reserva el dispositivo ALSA ni cambia el
sink predeterminado. La enumeración/negociación continúa fuera del callback.

`ui/audio_preferences` coordina borrador, validación asíncrona, comandos y aplicación.
`components/select` adapta el Select existente de GPUI Kit con valores tipados, búsqueda,
popup y foco. No depende de audio ni de `DawApp`; los selectores de Audio mantienen
entidades y suscripciones por ventana, sincronizadas únicamente cuando cambian las
opciones o el valor. Tab recorre los controles y el teclado del popup queda en Kit.
`list device` / `use device [input|output] <número|ID|nombre|default>` y las acciones
`audio.*` comparten ese controlador con Configuración → Audio. Aplicar detiene la
reproducción, invalida las preparaciones antiguas del piano y reconfigura el sampler.
Los cuatro adaptadores reciben la misma configuración; no se introduce otro transporte
ni se implementa mezcla multipista. Los IDs se guardan en las claves `AUDIO_*` del perfil
existente mediante reemplazo atómico. Un dispositivo ausente produce un error explícito;
no se cambia silenciosamente a otra salida. La validación consulta capacidades; abrir el
stream todavía puede fallar por permisos, exclusividad o desconexión.

`ui/file_playback` prepara la salida y el remuestreo de clips con el SoXR existente en un
worker. Solo abre el stream cuando el resultado sigue vigente; Stop cancela el arranque.
Se comprueba la fuente activa y se leen los valores actuales de mezcla antes de empezar.
Exportación conserva sus propios ajustes, independientes de los del dispositivo.

`audio/device_probe` ofrece un tono temporal a −24 dBFS y un medidor de entrada mono.
Los callbacks solo generan muestras o publican pico/tamaño de bloque mediante atómicos.
Las pruebas usan el borrador y terminan al cerrar Audio. No hay grabación a pistas ni
monitorización de entrada a salida. La duración de bloque mostrada no equivale a latencia
round-trip medida; con Auto se consulta el tamaño observado durante la prueba.

## Tempo y compás

`musical/time.rs` define `Tempo` y `Meter` sin dependencias de UI/audio. `Timeline`
mantiene los valores actuales (120 BPM, 4/4 al iniciar). El transporte, los formularios
de acciones `tempo.set`/`meter.set`, comandos `:tempo`/`:meter`, la regla en segundos,
rejillas, captura e importación MIDI, reproducción y exportación leen ese estado.
El audio se coloca en negras; modificar BPM altera su longitud visual en negras sin
alterar los segundos de la fuente. El compás modifica divisiones y etiquetas sin
mover eventos. El documento Loro aún no guarda tempo ni compás porque la UI todavía
no expone guardado completo del proyecto.

`ui/time_grid/musical` calcula una secuencia de marcas en ticks enteros para las
reglas y rejillas del Arrangement y piano roll. El zoom determina la separación visual
de líneas y etiquetas, no el snap ni PPQ. Compases, pulsos y subdivisiones tienen
distinto contraste y longitud en la regla; las etiquetas progresan de compás a
`compás.pulso` y `compás.pulso.ticks` (offset dentro del pulso, PPQ=960). Se alinean
con su posición real, sin fijar a la izquierda el número de un compás fuera de vista.
`components/bar_grid` comparte franjas, estilos de líneas y composición de etiquetas
(incluida la prevención de solapamientos). Ambas vistas dibujan una sola línea por marca.
El piano roll limita las subdivisiones a su `NoteGrid` seleccionado; compases y pulsos
siguen siendo referencias estructurales. El zoom no modifica esa resolución ni las notas.

## Sugerencias de comandos

`ui/command_suggestions` combina alias ejecutables y acciones invocables del catálogo,
elimina duplicados y prioriza coincidencias exactas o por prefijo. `ui/input` conserva
el texto libre de `:` y gestiona la selección con flechas, Tab y Enter; si el comando
exacto requiere parámetros, abre el formulario existente. El panel visual compartido
`views/command_bar` se superpone al pie de la ventana principal y del piano roll separado.
Las sugerencias no ejecutan operaciones hasta Enter; un clic solo completa el texto.

## Lotes Lua de pistas (PR-02)

`musical/track/snapshot` copia pistas, mezcla de sesión y árboles FX por ID con
una revisión de origen. `musical/track/validation` es la regla de jerarquía que
comparten `TrackEdit`, el ejecutor de lotes y `project/track` al importar Loro:
IDs únicos, un nivel de grupos e hijos contiguos. `musical/track/batch` aplica
operaciones tipadas en una copia, resuelve handles locales, limita tamaño y
devuelve el mapa de origen. Una revisión distinta o un error en cualquier paso
descarta el candidato completo.

`scripting/tracks` evalúa Lua en el sandbox de edición existente y convierte
el arreglo devuelto en operaciones Rust; las tablas de snapshot no son mutables
para la sesión. `ui/track_scripts` lee archivos en un worker. `ui/track_batch`
compara el snapshot vigente, valida la compilación de las cadenas Hay Utility modificadas y
valida una proyección candidata `ProjectDoc::from_session` con pistas, FX y MIDI.
Después `ui/track_commit` publica filas, mezcla, clips, selección y MIDI Learn
mediante el mismo remapeo que las acciones normales. La revisión de sesión
avanza en ambas rutas. La plantilla incluida se invoca desde el catálogo.

El documento Loro de esquema 4 conserva identidad y orden de pistas, FX y MIDI,
pero aún no está conectado en vivo a la UI ni representa mezcla, audio o colores.
La proyección candidata verifica compatibilidad antes del commit local; no crea
una transacción distribuida ni sincronización de red. Las futuras importaciones
remotas deberán avanzar la revisión de sesión y añadir una frontera causal al
snapshot antes de habilitar colaboración. [API y límites](LUA_TRACK_BATCHES.md).

## Componentes de audio Lua (PR-03)

`scripting/audio_components` evalúa descriptores en el sandbox existente y valida
puertos estéreo, nodos Volume (v1/v2) y SVF FunDSP (v2), aristas acíclicas, controles y límites. No lleva una
VM Lua al callback. `devices/audio_component` prepara primitivas y buffers de 512
frames fuera de RT; procesa el grafo en orden topológico y suma ramas paralelas.
`audio/effects/component` mantiene una instancia por `FxId`, con valores atómicos
para que el callback aplique la macro sin GPUI, locks ni reservas por bloque. El
plan FX ya publicado incluye el componente junto a Utility y VST3.
El piano compartido toma la ruta FX de la pista del clip mientras lo reproduce
(o prepara), y la ruta de la pista asignada al tocar en vivo. `ui/fx/audio`
comparte esa selección entre ganancia y rack; al arrancar un clip se fija su
ruta antes de abrir el reproductor.

`primitives/filter` implementa el SVF reutilizable sin GPUI. Lua v2 expone parámetros
por `(nodo, ParameterId)`, controles choice/log y una respuesta opcional del filtro;
`ui/views/lua_component/response` calcula y dibuja esa respuesta fuera de RT.

`ui/lua_components` carga las plantillas distribuidas, recarga candidatos en un worker
y conserva registro e instancias ante error. `[+]`, `fx.parameter` y el slider de
Track FX usan `FxEdit::Parameter`; `FxId` separa dos instancias del mismo tipo.
Loro conserva ID de tipo y parámetros numéricos, pero su documento sigue sin
conexión persistente a la UI ni transporte de la fuente Lua. El render offline
actual tampoco procesa este componente. [Contrato y prueba](LUA_AUDIO_COMPONENTS.md).

## Scripts Lua para edición MIDI

`scripting/packages` implementa el registro headless del contrato v1. El host entrega
las rutas distribuida y de usuario; `PackageRegistry::reload` construye un candidato
completo de archivos regulares `.lua` del nivel inmediato, ordenados y acotados. Cada
descriptor se evalúa en una VM aislada del sandbox compartido, sin ejecutar `run`.
Solo se conservan fuente y metadatos propios de Rust en snapshots `Arc<Package>`.
Una carga válida reemplaza el registro e incrementa su generación; cualquier error
conserva el registro anterior. Los snapshots ya capturados sobreviven a su retirada.
`ActionParameter<String>` y `OwnedActionSpec` reutilizan la validación de las acciones
nativas, cuyo tipo por defecto conserva cadenas estáticas. `catalog/{entry,tools}`
indexa metadatos propios y comandos; no hay fugas de memoria ni VM en el catálogo.

`ui/lua_tools` coordina una recarga explícita a la vez en un worker mediante
`midi.tools.reload`; el arranque, render, buscador y ejecución no escanean paquetes.
`PackageRegistry::request` resuelve `midi.tool <id> [valores...]`. La paleta y las
sugerencias usan ese mismo comando. `ActionForm` conserva un `OwnedActionSpec`
tanto para nativas como para Lua, con los controles Kit y el validador compartidos.
Su `PackageForm` rechaza confirmaciones de una generación anterior antes de leer
valores. `ToolInvocation` conserva fuente y valores tipados; puede terminar tras
una recarga o retirada. Cada invocación crea una VM y verifica metadatos estables
antes de llamar a `run(context, params)`, con un solo plazo para evaluación y run.
Los arreglos originales de notas/controles se validan completos; sustituirlos o
devolver un resultado distinto de nil rechaza la edición.

`scripting/{notes,controls,arrays,output}` comparte codificación y validación con
los scripts existentes. `MidiScriptTarget` conserva ID/patrón y valida identidad,
bloqueo y arrastre al regresar del worker. `ui/scripting` aplica por el mismo
controlador, conservando selección, silencios y descendientes. Los alias de Acentuar
y Subdividir usan los paquetes recargados `hay.midi.accent` y `hay.midi.subdivide`;
antes de la primera recarga correcta siguen disponibles sus fuentes integradas.
[Contrato](LUA_PACKAGES.md).

`src/scripting/builtin.rs` conserva la ruta de respaldo de las herramientas integradas;
sus fuentes Lua siguen siendo las mismas. Los paquetes `hay.midi.accent` y
`hay.midi.subdivide` se publican en `assets/scripts/tools/midi/` y los alias actuales
los invocan tras una recarga válida. Si falta el paquete, la subdivisión integrada
conserva su vía inmediata acotada: máximo 16 notas seleccionadas, 128 segmentos,
4096 notas de entrada y 8192 controles. Los paquetes se ejecutan en el worker.
`ui/scripting::apply_midi_script_result` es el commit compartido para ambas vías.
Los scripts de usuario conservan la API completa y siempre corren en background.

`scripting::edit_midi` es headless: una VM Lua 5.4 recibe una copia de las notas
del patrón y devuelve un `MidiEdit` validado. No importa GPUI ni audio. `ui/scripting`
lee los archivos y ejecuta las operaciones grandes en un worker; aplica su resultado al clip activo solo si
su ID y patrón siguen iguales y el clip no se bloqueó. El comando `midi.script` usa el
formulario de parámetros existente; `midi.quantize`, `midi.humanize`, `midi.subdivide`
y `midi.script.accent` comparten controlador con botones, atajos y gestos. Sus algoritmos
viven en `assets/scripts/midi/`; `scripting/actions` declara parámetros y opciones.
`musical/grid::NoteGrid` define la resolución 1/1–1/128, independiente del zoom y compás.
`ui/piano_roll/grid_actions` coordina resolución y acumulación de gestos, sin editar notas
al cambiar la rejilla. Alt+1 afina, Alt+2 engrosa; Alt+rueda o Shift+pellizco invocan
subdivisión sobre la selección explícita. La rejilla gobierna dibujo, snap y paso del cursor.
Lua solicita cortes mediante `note.cuts`; los paquetes también pueden pedir notas nuevas
en `context.new_notes`. `scripting/output` valida límites, asigna IDs monótonos y devuelve
la relación de descendientes para heredar silencio y, cuando corresponde, selección.
Cada primer segmento conserva su ID; las notas nuevas heredan instrumento y expresión
de una fuente seleccionada. La expansión está limitada a 16 384 notas; no hay borrado
arbitrario ni undo.
La futura capa de
displays usará datos declarativos y un renderizador GPUI común; todavía no está conectada.
Detalles de API y límites en [LUA.md](LUA.md).

## Acciones del piano roll

`ui/piano_roll/commands` adapta comandos del catálogo al controlador compartido;
`properties` valida longitud, canal y preferencias; `choices` alimenta el menú
contextual existente; `keyboard` conserva navegación y atajos. Las barras e inspector
no implementan transformaciones. Los iconos aceptan las mismas acciones y comandos
con parámetros, tanto en el panel como en la ventana separada.

`pointer` reutiliza inserción y borrado del controlador; `selection` concentra selección
y eliminación por lote (conserva CC, expresión e IDs), y `clipboard` conserva copiar/cortar/pegar.
`drag` confirma el patrón con el mismo eje temporal que Arrangement al soltar el ratón.
`motion` interpola únicamente el dibujo a demanda de frames, con retraso limitado a 6 px
y acomodo breve al snap; nunca programa notas ni cambia el tiempo del motor.
`Delete`/Backspace afectan solo notas seleccionadas; borrar región confirma al soltar,
y Esc cancela el gesto antes de restaurar un panel ampliado.

El empuje se define en `assets/scripts/midi/push.lua`; `scripting/push` prepara una VM
por gesto y transmite solo geometría/selección, sin CC ni expresión. Una búsqueda de
fronteras por tono calcula contactos a ambos lados; la vecina se recorta hasta un tick
y limita el avance, sin eliminar notas ni alterar solapamientos preexistentes. La entrada
original es inmutable: retroceder restaura el tamaño dentro del gesto. `ui/piano_roll/push`
aplica los límites del clip y el eje/snap compartido; Lua devuelve geometría validada,
y el canvas solo consume offsets suavizados de `motion`. Sin Lua en paint ni en audio.
La confirmación conserva IDs, expresión, CC y mute y rechaza cambios de snapshot.
El marco se dibuja en `views/piano_roll/note_frame` con borde fino y sombra GPU.

`Pattern::resize` rechaza perder notas. `MidiClip::channel` declara salida 1–16;
Loro lo persiste de forma compatible (ausencia = canal 1). Materialize exige el mismo
canal. El render live/offline entrega eventos al canal elegido de SoundFont;
sfizz recibe todos en su instancia. El importador todavía normaliza canales SMF.
El comando de canal viaja por la cola existente y prepara preset/parámetros fuera del
callback. La repetición de audición se ejecuta por segmentos en límites de frames,
sin reservas ni temporizadores de UI; exportación MIDI produce la duración finita del clip.
`Follow` solo ajusta el viewport según el reloj leído del motor.

Inventario y límites: [PIANO_ROLL_ACTIONS.md](PIANO_ROLL_ACTIONS.md).

## Controles MIDI temporales

`musical/automation` posee puntos con ID estable, carril tipado, tick y valor. `Pattern`
los incluye en su igualdad y copia; cambiar notas conserva sus controles. El scheduler
compila un flujo único de Off → Control → On en cada frame. El motor live/offline consume
mensajes nativos de CC, bend y presión; ninguna edición ni asignación sucede en el callback.
Al recortar se persigue el último control anterior por carril y al iniciar playback
intermedio el adaptador prepara los valores previos fuera del callback.

`project/midi_controls` usa listas/mapas Loro por ID; ausencia de claves significa
patrón antiguo sin controles. `scripting/controls` codifica y valida el arreglo Lua;
`ui/piano_roll/control_actions` es el controlador común al canvas y a los comandos.
`views/piano_roll/control_lane` dibuja eventos reales sobre la geometría compartida.
El importador SMF conserva controles y release velocity, todavía consolidando canales.
`musical/note_expression` contiene release velocity, probabilidad y pan por nota.
La propiedad viaja con ambos eventos del scheduler, en Lua y en Loro (claves opcionales
para compatibilidad). Probabilidad usa ID/vuelta sin memoria de voces; NoteOn y NoteOff
comparten decisión. sfizz recibe release velocity. `audio/piano/fluidsynth/voices` aplica
pan relativo solo a voces recién creadas, con buffers fijos de 256 entradas y sin
conservar punteros entre renderizados. sfizz/RustySynth aún no aplican pan por nota;
no se traduce a CC10 porque alteraría las demás notas.

## Documento de proyecto (Loro)

`src/project.rs` y sus adaptadores aportan un documento CRDT headless con
Loro 1.17.1 (MIT).
El esquema 4 guarda nombre, identidad, orden, tipo, grupo padre, asignación de instrumento,
mute y solo de las pistas. El orden usa `LoroMovableList` y cada pista tiene un mapa propio;
renombrar o silenciar una pista no reemplaza toda la lista. `reconcile_tracks` adapta el
modelo musical existente conservando las entradas del documento. `snapshot`/`save` y
`import`/`load` permiten intercambiar el documento binario; `all_updates`/`merge` exponen
las operaciones Loro y validan esquema, IDs y jerarquía antes de aceptar datos externos.
`src/project/fx.rs` guarda cada nodo FX por ID, sus parámetros y el orden de hijos en listas
movibles. Al cargar, reconstruye serie/paralelo y reserva los IDs restaurados para nuevas
inserciones. El guardado ocurre fuera del callback y reemplaza el archivo de forma atómica.
`src/project/midi.rs` guarda clips MIDI en una lista movible y cada nota en un mapa por ID.
Cada clip refiere su pista por TrackId; al materializar el documento, se resuelve el índice
de carril actual y se reservan los IDs de pistas, FX y clips para futuras inserciones.
El siguiente ID de nota se guarda por patrón, el silencio de cada nota se guarda en su
mapa, y el siguiente ID de clip se guarda en el
documento, incluso cuando se eliminó la última entidad creada.
`from_session` captura pistas, FX y clips MIDI; `materialize_midi_session` los reconstruye.

Este esquema aún no representa referencias a audio, parámetros de mezcla
ni colores. Por eso no se conecta a «Abrir/Guardar proyecto» en la UI: cargarlo
como proyecto completo descartaría material. Tampoco hay sincronización de red ni undo
del proyecto. El siguiente incremento debe adaptar los recursos de audio por ruta y
capturar los controles de mezcla antes de activar esos comandos.

## Análisis espectral para efectos

La evolución a dispositivo Spectro enrutable y el contrato interno de puertos/eventos
están definidos en [VISUAL_MODULATION.md](VISUAL_MODULATION.md).

`audio/analyzer` usa RealFFT 3.5.1 (MIT) para convertir bloques reales de 2048 muestras
en 48 bandas logarítmicas. El callback del piano solo empuja muestras mono a una cola
`rtrb` acotada, cuando el editor está visible; nunca ejecuta FFT, reserva memoria ni
espera un bloqueo. La UI drena la cola y calcula el espectro con buffers preparados.
`ui/components/spectrum` es un panel reutilizable con cabecera `<` `>`, modo Espectro
o Visualizador, bandas, forma de onda y guía de frecuencia; recibe datos y callbacks
sin conocer el piano ni el motor. El modo elegido vive en la UI del instrumento.
El filtro SVF TPT sigue procesando audio por separado;
el espectro muestra señal medida, no la respuesta teórica del filtro.

## Sampler sfizz

`devices/sampler` describe parámetros sin GPUI ni audio. `audio/sampler` posee el ABI C,
SFZ generado y recursos preparados. `instruments/sampler` posee teclas, motor preparado
y reproductor; `ui/sampler` solo carga, coordina y presenta controles existentes.
La primera instancia de sampler usa InstrumentId(2) y las siguientes reciben ID
propio; el piano conserva InstrumentId(1). El [+] asigna cada instancia a una pista.
Track FX proyecta el instrumento asignado como fila virtual antes de los efectos;
no lo duplica en `FxChain` ni cambia la topología DSP. Su editor se muestra bajo el
Arrangement, sin una vista central Instrument independiente.
La acción `track.insert_sampler` (`:sa`) reutiliza `ui/track_actions` para insertar
una pista de instrumento (tipo Synth, presentada como Sampler por su asignación).
Cada inserción crea una pista y una instancia nueva.

`audio/piano/backend` acepta ahora SoundFont o sfizz. El constructor del stream recibe
un backend preparado y reutiliza el scheduler, cola acotada, fader, paneo y Utility.
La reproducción del Arrangement usa la mezcla multipista común. Los parámetros continuos del sampler
viajan como CC de alta resolución; cambios estructurales reemplazan el motor preparado
fuera del callback y cortan las notas anteriores. `SamplerSnapshot` retiene el WAV
compartido durante exportación; las instancias offline conservan colas de release.
Detalles de uso, límites y dependencia fijada: [SAMPLER.md](SAMPLER.md).

## Piano: presentación, filtros y carga diferida

El editor SoundFont sigue la jerarquía visual de Hay Sampler: cabecera compacta, fuente,
bloques de perillas (afinación/voces, filtro, efectos y salida) y teclado de altura fija.
`views/instrument` compone `instrument_banks`, `instrument_params` e `instrument_actions`.
Bancos y créditos se construyen al desplegarlos; MIDI Learn conserva los índices previos.

`Piano::new` no descubre ni decodifica bancos. Abrir el editor o solicitar notas/Play inicia
un worker en `instruments/piano/loading`; `audio/piano/prepare` descubre la salida y prepara
el sintetizador sin abrir el stream. La UI recoge el resultado, nunca programa el scheduler.
Solo hay una preparación en curso: cambios de banco/configuración invalidan su generación;
el resultado antiguo se descarta antes de preparar el destino más reciente. El stream se
abre al tocar/reproducir. NoteOff y Stop cancelan la intención pendiente durante la carga;
un error permanece visible hasta recargar o elegir otro banco. La caché SoundFont tiene una
única referencia débil, sin retener todos los bancos visitados. Exportar puede cargar el banco
en su worker aunque no se haya abierto el editor.

`devices/filter` declara el estado headless del filtro. `audio/piano/filter` aplica un SVF
TPT estéreo LP/HP/BP después de sumar las voces y antes de FX/fader/pan/Utility, tanto con
FluidSynth como con RustySynth. Corte, resonancia y bypass/modo se suavizan durante 5 ms;
no hay reservas ni E/S en su procesamiento. El estado viaja por la cola del reproductor
y se captura en `PianoSnapshot` para el mismo procesamiento offline. El filtro propio de
sfizz permanece en el sampler; el filtro estéreo adicional está desactivado por defecto.

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
`ui/track_drag.rs` conserva el orden musical durante el arrastre y proyecta un orden visual
provisional. La cabecera y los clips de la pista salen del flujo para seguir el puntero;
las demás filas ocupan el hueco y se dibuja un destino vacío. `TrackEdit::Move` aplica un
único cambio al soltar.
Solo se reordena dentro del mismo nivel: una hija entre sus hermanas, una pista raíz entre
raíces. Un grupo raíz conserva y mueve todos sus hijos. La identidad del clip MIDI activo
y la vista del piano roll se conservan al reordenar.
`Track.chain` es ahora `musical::fx::FxChain`, con nodos Effect/SerialChain/ParallelChain e IDs
estables. Cada hijo directo de ParallelChain es una rama de la misma entrada; una rama con
varios pasos se representa mediante SerialChain. Las ramas se suman sin normalizar; un
contenedor vacío o en bypass pasa la entrada. La edición es transaccional: mover no cambia
IDs, duplicar renueva todos los IDs, y se rechazan ciclos/destinos inexistentes. Se limitan
las cadenas a 256 nodos y 32 niveles. Duplicar pistas también renueva sus IDs FX.
Hay Utility es el efecto disponible. Los nodos de efectos VST3 conservan identidad
declarativa; el host de instrumento Pianoteq es una etapa previa, descrita en `VST3.md`.
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
                                                 └→ audio/playback → CPAL

Biblioteca headless:
musical/model.rs → scheduler::compile(TimeBase) → PreparedPattern
                                                   │
Transport::advance(frames) → FrameWindow → events_in()
                                                   ↓
                                     ScheduledEvent { sample_offset, ... }
                                                   │
                                   audio/drums/mix.rs → DrumKit
                                                   ↓
                                         Volume → CPAL
```

Session y Arrangement comparten el Mix de audio, MIDI y percusión provisional.
La audición live del piano conserva un adaptador separado. `examples/sequencer.rs`
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
| Pertenencia de notas a escala | `musical/scale.rs::PitchScale` | Regla musical headless; el mock usa Do menor fijo para sombreado y advertencia solo durante el arrastre, sin corregir ni bloquear notas |
| Clips del Arrangement | `Timeline::{clip,other_*_clips}`, provisional | Se conservan al insertar, mover y solapar; aún no hay modelo de proyecto |
| Archivo decodificado | `Arc<AudioFile>` | Preparado fuera del callback |
| Voces del piano | RustySynth dentro del callback | No pasan por GPUI |
| Voces de percusión | `DrumKit` en `audio/drums/voice.rs` | Banco fijo; sin asignaciones por nota |
| Plan de notas | `PreparedPattern` | Inmutable; percusión lo compila en Play |
| Posición de transporte | `Transport` en cada callback / demo headless | Avanzar con frames procesados |
| Posición del patrón | `AtomicU64` en `DrumState` | Snapshot de solo lectura para la UI |
| Partículas | `ParticleTrail` | Tiempo visual; no afecta al scheduler |

## Catálogo y buscador

`catalog/plugins` enumera bundles VST3
con módulo Linux para la arquitectura actual, incluidos los wrappers de yabridge.
No ejecuta código de plugins. `audio/vst3/scan` consulta `IPluginFactory2` en un proceso
separado y devuelve ID de clase, nombre y tipo (instrumento/efecto). Prefiere
VST3 a VST2, deduplica rutas canónicas y limita profundidad y trabajo del recorrido.
`ui/palette/plugins` adapta metadatos propios al mismo listado y despacho por clic/Enter
del catálogo estático. La exploración corre en background al abrir Add; la actualización
conserva filtro y selección. La selección de un instalado delega a `ui/vst`, que prepara
el instrumento fuera de UI/RT y asigna la pista tras abrir la salida correctamente.
`audio/vst3` usa el binding vst3 0.3.0 (MIT/Apache); `instruments/vst` posee la ejecución
y comparte PianoPlayer, cola MIDI y scheduler con piano/sampler. Cada pista MIDI puede
poseer una instancia VST3, sin persistencia de estado. `audio/vst3/editor` conserva componente/controlador
y ventana X11 en un hilo propietario, separado del procesador transferido al stream.
`IPlugView` dibuja el editor original y `IComponentHandler` comunica sus cambios por
una cola SPSC acotada. No hay panel de parámetros específico por plugin; el mapeo Lua
queda pendiente. Ver `VST3.md` para compatibilidad y verificación.

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
la asignación entre ratón y Enter. `ui/icons.rs` carga SVG; `ui/toolbar/persistence.rs`
guarda y restaura asignaciones de ambos ámbitos en el perfil atómico, incluidos comandos
personalizados. `views/icon_picker{,_browser}.rs` presenta búsquedas independientes y
listas desplazables.

Al elegir el piano, `ui/palette.rs` llama a `musical/track.rs::assign_instrument`,
que mueve esa identidad a la pista destino. `:sa` crea una instancia sampler nueva
por pista. La cabecera y Track FX muestran la asignación. `Volume` y `Componente serial` aparecen como
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
de edición/presentación se conecta ahora a la mezcla de clips de audio de PR-04: cada pista suma sus clips antes de los FX. Los instrumentos MIDI aún usan adaptadores separados.

`ui/clip_menu` posee el contexto y las acciones del menú de Arrangement, separado de
`PianoRoll`. Su lista de opciones sirve tanto al teclado como al ratón; solo MIDI ofrece
abrir el piano roll. `ui/clip_actions.rs` concentra las operaciones comunes y
`timeline/clip_edit.rs` divide/restaura ventanas sin modificar las fuentes. El nombre de
un `AudioClip` pertenece a la instancia, separado del nombre de su `Arc<AudioFile>`.
Dividir conserva la alineación de las dos ventanas; restaurar recupera el inicio original
hasta el límite del beat cero. Silenciar, cambiar ganancia y editar ventanas detienen
la reproducción actual; no representan edición de planes en caliente.

`drums/mix.rs` compila el plan fuera del callback, segmenta el loop por frames y
entrega offsets exactos a `DrumKit`; `audio/playback` suma el bloque con MIDI/WAV
antes de Utility master y CPAL. La síntesis es provisional (sin samples) y el
plan se recompila solo al iniciar Play. La audición live y el reemplazo de plan en
caliente siguen pendientes. Los planes y samples retirados deben destruirse fuera
del callback.

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

## Foco musical y posición visual

`ui/transport::handle_performance_key` prioriza Espacio y teclado MIDI activado en los
focos musicales del workspace, FX y ventana de instrumento. La fase capture evita que
la navegación del árbol consuma notas; se respetan focos de texto y overlays. El destino
sigue en MidiServer; enfocar el mismo instrumento no cancela notas. La ventana separada
comparte el drenaje de MIDI físico sin crear otro transporte.

`timeline/visual_transport` interpola exclusivamente coordenadas dibujadas. Conserva
la posición exacta de audio y edición; extrapola como máximo 80 ms entre muestras del
reloj y reinicia el trazo ante discontinuidades. El cursor converge en unos pocos frames.
El ciclo visual pide frames mientras converge; nunca programa notas ni avanza el audio.

`timeline/slip` comparte el gesto de desplazamiento de contenido entre audio y MIDI,
con coordenadas en beats durante el gesto y conversión a segundos solo para audio.
La ventana visible permanece fija y los offsets de origen están acotados al contenido
existente. No se altera el archivo/patrón, ni se introduce otro modelo de reproducción.
`toolbar_icon_button` recibe estado activo; el controlador sigue siendo la única fuente
de verdad para icono, texto de estado y botón del instrumento.


### Efectos VST3 en el plan de audio

`audio/effects` adapta los FxNode existentes a planes de Utility, serie, paralelo y
procesadores VST3 estéreo. Las instancias viven por FxId; la edición del árbol no cambia
su identidad. `ui/vst/effects` coordina la carga en background y publica planes por RCU.
El callback solo lee el plan publicado; la publicación y la retirada de planes son
externas al callback. La topología musical conserva exclusivamente IDs y parámetros,
sin importar audio ni GPUI. `FxGain` mantiene su rampa escalar para cadenas Utility y
recibe además una ruta de efectos para los streams de audio/MIDI. El guardado binario,
la duplicación de instancias, los buses de grupo y el export VST3 quedan pendientes.

## Fuente de clip extraída (PR-04, commit 1)

`audio/clip_source` posee referencias a las muestras originales o remuestreadas y
los límites del clip en frames de salida. Su render recibe una posición absoluta
y un bloque prestado; sobrescribe las muestras, con silencio fuera del intervalo.
No abre CPAL, no avanza un reloj propio, no reserva memoria ni prepara recursos
al renderizar. Permite reutilizar la misma fuente con distintos tamaños de bloque.

`audio/playback` prepara esa fuente antes de construir el stream y conserva posición,
FX, nivel, pan, Utility y publicación visual. La preparación/remuestreo en worker y
su cancelación siguen en la ruta existente. El segundo incremento usa esta fuente
para mezclar clips; el cuarto incremento añade seek/loop y preparación FX a la frecuencia real.

## Mezcla de clips de audio (PR-04, commit 2)

`PreparedPlayback` selecciona una salida y prepara `playback/mix::Mix` en el worker.
El plan tiene hasta 256 pistas y 4096 clips. Cada pista conserva su `TrackId`, fuentes
recortadas/remuestreadas, nivel, balance y ruta FX. Renderizar suma los clips con su
ganancia individual en un bus de pista, procesa FX una sola vez, aplica nivel/pan
y suma al master flotante. Utility procesa el master y `output::write_stereo` limita
una sola vez en la salida física. Buffers de 256 frames en pila; sin E/S ni reservas
por bloque. El callback posee el plan preparado; CPAL se destruye antes del estado.

La UI toma un snapshot de todos los clips de audio, incluidos los no seleccionados.
Stop descarta el trabajo pendiente; cambios de fuentes, revisión de pistas, tempo o
salida antes de completarlo cancelan el arranque. Los controles y rutas existentes se
publican por TrackId para evitar aplicar los de la pista seleccionada al master.
El tercer incremento verifica y completa las operaciones de mezcla por pista.

Un único contador de frames alimenta todas las fuentes y finaliza en el último clip.
Los desplazamientos, loop y seek en caliente están conectados al reloj compartido.
La actualización de fuentes durante Play sigue pendiente: el plan de fuentes es inmutable.
El modo MIDI conserva su prioridad actual; no se mezcla todavía con estos clips.
Loro no recibe buffers ni procesadores; guardado global/collaboración siguen pendientes.

La resolución de mute/solo/nivel/balance vive en `musical/track/mix` y se comparte
entre preparación y controles de UI. `UNITY_LEVEL` define el fader unitario; mute
prevalece sobre solo, varios solos permanecen audibles y se conserva la herencia
actual del grupo padre. Los atómicos de cada pista se direccionan por TrackId,
independientemente de selección u orden visual. La prueba de mezcla verifica
cambios durante render con audio y reordenación de pistas.

## Transporte de audio y preparación por frecuencia (PR-04, commit 4)

`audio/playback/transport::Cursor` adapta el `musical::time::Transport` existente;
no crea un segundo reloj. Divide cada bloque por los límites del loop y llama al
mismo mezclador con posiciones absolutas. El callback publica frames y fin de
reproducción; los comandos de seek/loop se publican por `audio/publication` (RCU
extraído del rack). Una revisión confirmada evita que un fin de reproducción
anterior cierre el stream antes de atender un seek o un nuevo loop. No hay
asignaciones ni locks en render; la publicación y retiro ocurren en el escritor.

`ui/transport/audio` concentra comandos y botones. Home y ⏮ hacen seek a cero,
`:transport.seek 8` salta a la negra 8, `:transport.loop 0 8` repite dos compases
4/4 y `:transport.loop off` desactiva el loop. Las posiciones parten de cero.
Stop cierra el stream y conserva su último frame observado en el cursor de edición.
La carga prepara todos los clips, incluidos los anteriores al cursor, para poder
volver a ellos mediante seek. Stop cancela pendientes; las otras audiciones se
siguen deteniendo al pasar a la mezcla de audio y viceversa.

Antes de activar una ruta, `Rack` registra la frecuencia real y prepara sus
componentes Lua. Cada instancia mantiene variantes de DSP por frecuencia mediante
RCU y controles atómicos comunes. El callback elige la variante ya preparada; la
UI dibuja la curva con la frecuencia activa. Un worker cancelado no invalida otra
variante. Al publicar un nuevo plan, se preparan todas las frecuencias registradas,
incluidos los FX nuevos, antes de sustituir el plan activo. VST3 no se reprepara de
este modo: una frecuencia incompatible se rechaza antes del arranque y requiere
recargar el plugin. Datos Loro, UI y procesadores siguen separados; loop de sesión
no implica persistencia global ni colaboración de proyectos.
