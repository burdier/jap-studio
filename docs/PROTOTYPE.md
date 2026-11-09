# Estado del prototipo y siguiente incremento

## Disponible

- Acción **Insertar sampler** en `[+]`, menú de pistas y `:sa`: crea una pista con
  Hay Sampler asignado o abre la existente (una instancia por sesión). Asignable a la barra.

- Hay Sampler basado en sfizz, accesible desde el [+] actual y `:sampler`. Panel compacto
  inspirado en Simpler con waveform, región/loop, Classic/1-Shot, nota raíz/afinación,
  polifonía, filtro LP12, ADSR y salida. Reutiliza botones, perillas y teclado de Hay.
  Carga por selector, drop externo o archivo seleccionado en biblioteca; prepara en background.
  Parámetros continuos por CC de alta resolución; cambios estructurales reemplazan el motor
  al terminar y cortan notas. Piano roll, controlador MIDI y exportación usan sfizz en la
  pista asignada. Scheduler y reproductor compartidos con el piano; no se añade otro reloj.
  Una instancia por sesión, sin Warp/Slice/LFO/MPE, persistencia ni mezcla multipista live.
  sfizz 1.2.3 compilado localmente; no se ejecutaron tests ni una sesión gráfica/auditiva.
  [Uso, arquitectura y construcción](SAMPLER.md).

- Track FX Tree junto al Arrangement, con el mismo TreeView extraído del explorador de archivos.
  [+] / A / :add conservan el catálogo general; FX añade un panel contextual, sin sustituir Add.
  Hay Utility se inserta en la pista/contenedor indicado. Serie/paralelo admiten anidamiento;
  selección, plegado y scroll por pista; bypass independiente, nombre, duplicar, quitar y drag/drop
  con rechazo de ciclos. Enter/doble clic: parámetros; B: bypass; F2: nombre; Ctrl+D: duplicar;
  Delete: quitar; A: Add; Espacio: transporte. Menú y formulario son los componentes existentes.
  `fx.show/add/serial/parallel/rename/parameters/duplicate/bypass/remove` están en catálogo/comandos.
  IDs estables; al duplicar nodos o pistas se generan identidades nuevas. Editar nombre/bypass
  no reconstruye filas; el árbol está cacheado y virtualizado. Su panel se puede reducir a cero.
  Utility procesa en audio/piano y exportación, mediante compilación lineal de serie/paralelo
  fuera del callback y publicación atómica suavizada. Un grupo compone su ganancia con sus hijos.
  Paralelo suma sin normalizar; vacío/bypass pasa la entrada. No añade EQ/Reverb, host VST3,
  buses generales, mezcla multipista live, FX de Session ni persistencia/undo del proyecto.
  Validación: compilación; sin ejecutar tests ni validar interacción/audio en una sesión gráfica.

- Espacio inicia/detiene el transporte desde Arrangement, piano roll, explorador e instrumento,
  también en ventanas separadas y en modo Insert. Reutiliza Play/Stop existentes; Session conserva
  la audición del patrón. Mantener la tecla pulsada no alterna repetidamente. Campos de texto,
  comandos y diálogos conservan sus controles. Esc libera notas en el instrumento.
  Validación de este ajuste: compilación, sin ejecutar tests ni comprobación interactiva.

- GPUI Kit 0.9.0 integrado como biblioteca de UI, con runtime/plataformas 0.3.11 alineados.
  Primer uso: formulario de parámetros de acciones (exportar, importar por ruta, nombre/altura
  de pista y ganancia Utility). Campos de texto con selección, IME, copiar/cortar/pegar y
  undo/redo del campo; cantidades con botones y ↑/↓ según unidad/incremento del contrato;
  booleanos con casillas y botones OK/Cancelar de Kit. Tab/Shift+Tab recorren el diálogo,
  Enter desde un editor o Ctrl+Enter confirman, Esc cancela y devuelve el foco. Los errores
  conservan el borrador. El formulario aparece en la ventana que lo abrió, incluido el piano
  roll separado. Colores y contraste proceden del perfil Hay; el motor de audio no cambia.
  Este historial de texto no implementa undo/redo del proyecto. Validación: compilación,
  sin ejecutar tests ni comprobación interactiva de la UI.

- Explorador de bibliotecas independiente a la izquierda del Arrangement y su editor, con
  separación visual y ancho arrastrable hasta ocultarse. `e` en modo Normal lo abre/cierra
  y recupera el panel reducido a cero; con foco dentro, `e`/Esc lo ocultan. En el piano roll,
  Borrar queda disponible con Shift+E. Árbol con sangría y guías, carpetas primero, iconos
  audio/MIDI y selección con el contraste del perfil. Flechas/j/k/h/l navegan el árbol.
  «Añadir biblioteca» permite elegir carpetas; sus accesos sobreviven al reinicio en
  `~/.config/hay-daw/libraries` (o la carpeta de configuración del sistema). «Quitar» olvida
  el acceso sin borrar archivos. Bibliotecas desconectadas se conservan y muestran error.
  «Recargar» consulta cambios externos; todavía no hay vigilancia automática del disco.
  Doble clic/Enter insertan audio o MIDI en la pista activa y el cursor del Arrangement.
  `explorer.toggle`, `explorer.insert` y `file.import` están en catálogo e iconos; esta última
  recoge una ruta con el formulario declarativo. La importación conserva TrackId al reordenar.
  Modelo headless, entidad GPUI cacheada, lista virtual y E/S en background, incluido MIDI.
  Sin nuevas dependencias. Validación de este incremento: compilación, sin ejecutar tests.

- Grupos plegables con ▸/▾ en su cabecera y «Plegar/Desplegar grupo» en el menú contextual.
  Al plegar queda una única fila de grupo con su contador; sus hijos, clips y audio se conservan.
  Al entrar en un grupo, las pistas pasan a altura compacta (40 px), también al insertar
  una pista nueva dentro de él. Después se pueden ampliar manualmente; duplicar conserva
  la altura elegida en el original.
  Las cabeceras de los hijos tienen 12 px adicionales de sangría para marcar la jerarquía.
  Flechas/j/k y Shift+clic recorren únicamente las filas visibles. El menú de pistas ofrece
  altura Compacta (40 px), Predeterminada (perfil), Amplia (120 px) y Personalizada (32–240 px)
  para toda la selección. La personalizada reutiliza el formulario de acciones. Las filas
  compactas conservan nombre, mute, solo y + según la configuración de controles; las mayores
  recuperan el resto. Cabeceras, waveforms, MIDI, recorte y drag/drop comparten geometría variable.
  Comandos: `:track.fold`, `:track.height 40`, `:track.height.compact/default/tall`; también catálogo
  e iconos. Estado visual de sesión, sin alterar audio ni exportaciones; no se guarda al reiniciar.

- Selección con mayor contraste, separada del hover: cabeceras y carriles de todas las
  pistas seleccionadas, clips, filas de menús, buscador y selectores compartidos. En
  Configuración → Apariencia, «Contraste de selección» ofrece Suave/Medio/Alto y una vista
  comparativa. Se guarda automáticamente como `SELECTION_CONTRAST` en `~/.jaybu.profile`;
  por defecto `0.18`, rango `0.0–0.4`. Los fondos se adaptan al texto del tema para funcionar
  tanto en temas claros como oscuros; cambiar de tema conserva la intensidad elegida.

- Pistas dinámicas: insertar, eliminar con sus clips, duplicar selección/clips, renombrar,
  color, mute/solo y grupos básicos de un nivel. Ctrl+clic selecciona varias cabeceras;
  Shift+clic selecciona un rango. El menú derecho usa las mismas filas planas y selector de
  color que los clips. En Arrangement, Delete o dos pulsaciones de `d` en modo Normal eliminan
  las pistas seleccionadas (incluidos hijos/clips de grupos); Esc u otra tecla cancela `d`.
  Mantener pulsada la tecla no dispara eliminaciones sucesivas. Los campos y overlays conservan
  su entrada de texto. Las acciones también se encuentran en el catálogo.
  Las acciones `track.insert/delete/duplicate/group/ungroup/rename/mute/solo`
  están en catálogo, comandos y selector de iconos. Renombrar usa el formulario de parámetros.
  Agrupar mueve las pistas juntas; desagrupar conserva los hijos. Eliminar un grupo elimina
  también sus hijos/clips; duplicarlo copia la estructura con IDs nuevos. La única instancia
  de piano conserva su asignación original: duplicar clips no crea un segundo instrumento.
  Un Arrangement sin pistas muestra fondo oscuro y un + central que abre el mismo buscador
  de `:add`/A. Desde allí se puede insertar una pista vacía o elegir el piano para crearla.
- Grupos como pistas contenedoras de audio/MIDI, sin clips ni grabación propios. El título
  muestra `[activas/total]` según mute. Mute/solo del grupo y nivel se heredan por los hijos;
  el balance suma el desplazamiento del grupo al de cada hijo y limita a L/R. También se
  aplica al snapshot de exportación; seleccionar un grupo exporta sus hijos (stems por hijo).
  Son controles jerárquicos básicos, no un bus de mezcla en vivo. `Track.chain` usa ahora el árbol FX recursivo descrito arriba.
  Los grupos de pistas siguen limitados a un nivel; no hay host VST3 ni bus DSP general. El proyecto sigue
  sin guardado/undo. En este incremento se valida únicamente compilación, sin ejecutar tests.

- Acciones con parámetros declarativos: texto, booleanos y cantidades con unidad/límites.
  «Exportar audio» desde catálogo o icono abre un formulario flotante con nombre/formato,
  carpeta, pistas, inicio/duración en beats, cola en segundos, frecuencia en Hz, mono y
  archivos individuales. Parte de los valores guardados; profundidad, calidad MP3 y Utility
  mantienen las preferencias. «Utility · ajustar ganancia» reutiliza el formulario en ×.
  Tab recorre los controles, ↑/↓ ajustan cantidades, Espacio alterna bool, Enter en un editor
  o Ctrl+Enter/OK ejecutan y Esc cancela. Los errores conservan el borrador; confirmar no
  modifica preferencias. Los comandos manuales mantienen su ejecución directa.

- Configuración organizada por categorías laterales: Apariencia, Pistas, Exportación y Atajos.
  Exportación permite guardar nombre/carpeta, varios formatos, master o archivos por pista,
  frecuencia, profundidad PCM, MP3 CBR/VBR, canales, Utility, pistas y rango/cola.
  El borrador se valida antes de guardar en `EXPORT_DEFAULTS` del perfil personal; reiniciar
  o `:reload` recupera los valores. «Exportar audio» y `:export` los usan; los argumentos
  explícitos del comando prevalecen. En el selector de iconos, ahora un botón SVG «+»,
  «Exportar audio» y «Configurar exportación» aparecen al principio.

- Regla de tiempo fija al pie del Arrangement, alineada con el grid: minutos/segundos,
  subdivisiones adaptadas al zoom y fracciones de segundo al acercarse. Comparte cursor,
  playhead, desplazamiento y zoom con la regla musical; clic coloca el cursor de edición.
  La conversión usa el tempo constante actual de 120 BPM.

- Al arrastrar un archivo MIDI o audio sobre Arrangement se muestra una sombra con su nombre
  y una línea en el punto de inserción, usando el zoom y scroll actuales. Desaparece al salir,
  cancelar o soltar. Su ancho es una guía de un compás; la duración real se obtiene al importar.
  Mantiene la colocación libre existente y la importación de un archivo por gesto.

- **Exportación offline del Arrangement**: `:export -n 93.[mp3,wave] -master` o
  `-individuals`, también desde catálogo. WAV/AIFF/FLAC con libsndfile, MP3 CBR/VBR con LAME,
  conversión VHQ con libsoxr. Frecuencia, bits, mono, pistas, rango, cola, Utility y destino
  configurables por comando; progreso y `:export.cancel`. Snapshot independiente de cambios
  en UI, mezcla de clips superpuestos y stems alineados. Pruebas reales de codecs/MIDI sin
  dispositivo. [Opciones, dependencias y límites](EXPORT.md). Esta mezcla es offline;
  la audición en vivo aún usa reproductores excluyentes.

- **Hay Utility**: primer dispositivo interno de Elevate (`hay.utility`), construido por
  descriptor/fábrica como `Component → Volume → FunDSP`. Ganancia 0–2x, bypass suavizado,
  reset e inserción/retirada en un único slot de **salida de audición** mediante `:utility`.
  Buscar «Hay Utility» en Add inserta ahora una instancia en el Track FX Tree. `:utility.gain 0.5`, `:utility.bypass`,
  `:utility.reset` y `:utility.remove` comparten operaciones con el panel, transporte y MIDI.
  Los atajos `[`/`]` y `b` actúan ahora sobre este dispositivo; insertan el slot si está vacío.
  Audio, piano y percusión preparan su procesador antes de abrir el stream y reciben un snapshot
  atómico coherente. Utility se aplica después del nivel de pista y el paneo: bypass nunca
  cancela Mute/Solo. Retirar restablece sus valores y lleva suavemente a ganancia unitaria.
  Abrir :utility de nuevo reutiliza ese mismo slot de salida, independiente del árbol FX.
  La persistencia del proyecto sigue pendiente.

- **Materialize MIDI** (`:materialize`, `:clip.materialize`, menú contextual y buscador):
  clic selecciona un MIDI; Ctrl+clic añade/quita clips y sus marcos indican la selección.
  Con uno, consolida el recorte visible en un patrón nuevo; con varios de la misma pista,
  crea un único clip desde el primer inicio al último final, conservando silencios y notas
  solapadas. Sustituye solo los seleccionados después de preparar el resultado completo.
  Requiere clips desbloqueados con igual ganancia, mute, pitch y loop, que se conservan;
  nombre/color proceden del primero cronológico. El resultado queda sin recorte: es la nueva
  fuente MIDI, no un render de audio ni un guardado del proyecto. Undo/redo sigue pendiente.

- Arrangement: clips de audio/MIDI conservados en sus carriles, arrastre, recorte no destructivo
  desde ambos bordes, cursor, zoom y scroll. Los clips se pueden solapar sin borrarse; en pantalla
  el activo queda encima y MIDI se dibuja sobre audio. La reproducción simultánea aún no está disponible.
- La categoría **Acciones** del buscador incluye operaciones de clip: duplicar, dividir en cursor,
  restaurar recorte, mute, bloqueo, ganancia, movimiento entre pistas y eliminación. Renombrar y
  color abren sus opciones contextuales. El objetivo es el clip bajo el cursor de edición; en un
  solapamiento audio/MIDI, MIDI tiene prioridad visual y de selección.
  Las mismas opciones se invocan en modo Command (`:`) con `:clip.duplicate`, `:clip.split`,
  `:clip.delete`, `:clip.mute`, `:clip.lock`, `:clip.restore_trim`, `:clip.gain.down`,
  `:clip.gain.reset`, `:clip.gain.up`, `:clip.track.previous`, `:clip.track.next`,
  `:clip.rename`, `:clip.gain` y `:clip.color`; aparecen en la ayuda.
- Reproducción de archivo y piano SoundFont, cada uno con Volume/bypass.
- Touchpad en Arrangement y piano roll: pellizcar hace zoom horizontal anclado al
  puntero; dos dedos desplazan ambos ejes, incluidos movimientos diagonales y deltas
  fraccionales. En Arrangement desplaza tiempo/pistas; en el piano roll, tiempo/registro
  MIDI (0–127). Ctrl+rueda hace zoom y Shift+rueda desplaza horizontalmente.
  En el piano roll, Windows/Super + dos dedos hacia abajo aumenta la altura de las
  filas; Alt + dos dedos hacia arriba aumenta el ancho. El gesto contrario reduce
  cada dimensión. Ambas escalas son independientes, mantienen el punto musical bajo
  el puntero y no editan tono, duración ni posición de notas. La escala vertical
  permite desde mostrar las 128 notas MIDI hasta ampliar ocho veces la altura inicial.
  El piano roll permite volver al encuadre inicial con «Ajustar vista» o `0`, también
  en ventana separada. Los gestos solo cambian la vista, no clips, notas ni transporte.
  Se utiliza `PinchEvent` de la revisión actual de GPUI: el backend y el sistema deben
  entregar el gesto (Wayland con pointer-gestures, X11 con XInput 2.4). La dirección
  natural y la inercia recibida proceden del sistema; no se añade un reloj de gestos.
- Menú de clips MIDI/audio con el mismo estilo del menú de inserción: renombrar,
  duplicar a continuación, dividir en el cursor, restaurar recorte, silenciar,
  bloquear posición/tamaño, color, ganancia, mover de pista y eliminar. Solo MIDI
  ofrece «Editar en Piano Roll». Las opciones son texto plano, sin cajas ni sombras,
  con resaltado discreto al navegar. El color se elige en un selector visual al pie
  del menú (siete muestras y «A» para heredar el de la pista), compartido con las pistas.
  Izquierda/derecha recorren sus muestras y Enter aplica. Ganancia tiene página secundaria.
  Flechas y Enter navegan/ejecutan; Escape vuelve o cierra. Al renombrar, escribir
  sustituye el nombre seleccionado; Ctrl+A selecciona todo, Enter guarda y Escape cancela.
  Clic derecho sobre un clip conserva el cursor de edición para dividir en él;
  las dos partes deben medir al menos 0,25 beats. El bloqueo impide dividir,
  restaurar, mover y redimensionar. Restaurar conserva la alineación temporal
  sin extender antes del beat cero. WAV conserva su nombre de archivo original.
  Loop y transposición no se ofrecen en el menú mientras su reproducción siga pendiente.
- Efecto de partículas del playhead aislado en UI, con particula-rs y fastrand.
- Vista `2` / `:pattern`: cuatro slots, 16 pasos iniciales, click/Enter para alternar
  notas y rueda para velocity. Modelo real en ticks, conservado al cambiar de vista.
- Audición del patrón en loop con cuatro percusiones sintetizadas (sin samples externos),
  plan compilado al pulsar Play, offsets por frame y transporte por frames. Space
  inicia/detiene desde la vista Pattern; el paso que suena se resalta.
- Biblioteca musical sin GPUI/CPAL: patrón variable, notas simultáneas, tempo constante,
  transporte en frames y plan NoteOn/NoteOff con offsets por buffer.
- Buscador tipo rofi con `[+]`, `a` o `:add`: filtra instrumentos, primitivos, componentes
  y acciones sin distinguir acentos. Navegación con ↑/↓, Enter y Escape.
- Selector de iconos con todas las acciones ejecutables del catálogo (incluido Materialize,
  edición de clips y vistas), más controles de transporte, pistas, instrumento y zoom.
  El Piano Roll ofrece sus controles visuales, ajuste de vista y zoom por eje; las herramientas
  de edición pendientes conservan la etiqueta «mock». Iconos y acciones tienen búsquedas
  independientes (Tab alterna, Enter asigna) y listas desplazables. Las acciones nuevas del
  catálogo aparecen automáticamente en Transporte; las asignaciones duran la sesión.
- El `[+]` asigna el piano a su pista y la cabecera muestra `Upright Piano KW`; la vista
  Instrument indica la pista activa. `a`/`:add` usan la pista seleccionada.
- Piano desacoplado de la vista: `src/instruments/piano.rs` posee reproductor, notas
  sostenidas y ratón; la vista pinta el descriptor de `elevate::catalog`.
- Demo offline `sequencer` y contratos de fronteras, IDs, errores e invariancia por tamaño de buffer.

- Piano separable en ventana propia del sistema (botón o `:detach` / `:dock`): sigue
  sonando al cambiar de vista; teclado y ratón funcionan en esa ventana.

- Teclado del ordenador como controlador MIDI reutilizable (`M` / botón / `:midi`):
  octava Z/X, velocity C/V, desactivado por defecto; indicador en la barra de estado.

- Parámetros del piano (RustySynth): Master, Volume/Expression/Pan/Modulación, sends de
  Reverb y Chorus, Sustain, Pitch bend, rango de bend, afinación fina/gruesa (RPN) y, al
  abrir el piano, Polifonía y Reverb+chorus. Click en la etiqueta restaura el valor por defecto.
- Selector de bancos SoundFont (`.sf2`) y presets (Bank/Program MIDI) dentro de la vista
  Instrument: descubre automáticamente `assets/soundfonts/` y `assets/soundfonts/freepats/`,
  muestra el nombre oficial y los créditos/licencia del autor leídos de cada `.readme.txt`.
- Componente `MidiClip` y grabación/recuperación MIDI (`MidiCapture`): mapea todo lo tocado
  (por controlador o teclado gráfico) a `NoteEvent` en ticks (`Pattern`), permite **Recuperar MIDI**
  retrospectivamente o **Grabar MIDI** (`●` / `:record` / `:capture`), inserta el clip en el carril
  del Arrangement y lo reproduce de forma sample-accurate a través del instrumento SoundFont.
- Track Headers configurables estilo REAPER TCP (`TrackHeaderConfig` en `src/ui/views/tracks.rs`)
  con perillas rotatorias en GPU (`src/ui/components/knob.rs`): Pan bipolar y Volumen unipolar en
  forma de perilla (`knobs`, activo por defecto) o barras (`bars`), arrastre vertical o rueda para ajustar y
  clic derecho para restaurar al centro/defecto; tamaño por defecto 28 px y arrastre vertical
  continuo, además de presets (`standard`, `compact`, `mixer`,
  `minimal`) y conmutadores de elementos en `◈ Hay` o `~/.jaybu.profile` (`TRACK_CONTROL_STYLE`,
  `TRACK_HEADER_LAYOUT`, `TRACK_HEADER_HIDE`, `KNOB_SIZE`).
- Los controles M/S/R de los headers son operativos: M silencia, S deja audible solo la pista
  seleccionada (combinable con M) y R inicia/detiene la captura MIDI dirigida a esa pista.
  El contrato de audibilidad vive en `musical::track`; el stream existente recibe la ganancia
  mediante su adaptador, sin procesamiento ni GPUI en el callback.
- Pan y volumen de pista responden a rueda y click; el paneo se aplica en el adaptador estéreo
  (`audio::stereo_pan`) después de Volume, leyendo un valor atómico por bloque y sin asignar.
- Al abrir un clip MIDI, Arrangement queda arriba y el piano roll acoplado debajo. Separarlo
  en una ventana muestra el Mixer mock en el panel inferior; cerrar la ventana lo vuelve a acoplar.
- Puntero inmersivo en GPU (`src/ui/cursor_fx.rs` con `particula-rs` + `fastrand`): cinco modos
  seleccionables desde Configuración (`◈ Hay`), comandos (`:cursor fire|sparks|neon|stars|off`)
  o `~/.jaybu.profile` (`CURSOR_FX=fire`), incluyendo el efecto **Fueguito** por defecto.
- Tema y efecto del cursor se guardan automáticamente al seleccionarlos, también por comando.
  Configuración concentra estos controles; ya no están en la barra superior ni en las acciones
  asignables al Transporte. El perfil conserva dimensiones, claves ajenas y comentarios;
  se actualizan también los colores para que una paleta explícita antigua no anule el tema
  al reiniciar o recargar. Los errores de guardado se muestran en Configuración y en el estado.

## Limitaciones de esta audición

- Polifonía, Reverb+chorus y el cambio de archivo `.sf2` solo los lee RustySynth al crearse:
  cambiarlos con el piano abierto lo cierra (corta notas) y se reabre en el siguiente note-on.
  Cambiar de preset dentro del mismo banco sí se aplica en vivo por MIDI Bank/Program.
- Los parámetros del instrumento no se guardan al cerrar la app; el tema y el cursor sí.

- El plan activo es inmutable. Editar notas mientras suena no cambia el audio hasta
  detener y volver a pulsar Play. El Loop y los cambios de plan en caliente siguen pendientes.
- Los cuatro instrumentos son síntesis provisional; todavía no cargan samples del usuario.
- Clip, piano y percusión abren streams excluyentes. Al iniciar uno se detiene el anterior;
  la audición del patrón se detiene al salir de su vista.
- Solo existe una instancia de piano. El buscador la asigna a una pista y la mueve al
  reasignarla; todavía no crea instancias nuevas. `Volume` y `Componente serial` son metadatos.
- Las acciones del catálogo usan comandos string provisionales; no son todavía el
  sistema de acciones definitivo.
- No hay chase de notas sostenidas, tempo editable desde la UI ni map de tempo.

## Todavía pendiente

- Docking estilo Visual Studio: paneles acoplables con imán/gravedad por tamaño,
  generalizar `DetachedPanel` a cualquier componente con salida de audio y recordar posición.

- Sustituir la percusión sintetizada por un sampler de cuatro samples con pool de voces.
- Un stream persistente común: el piano, el clip y la percusión aún son excluyentes.
- Cola RT acotada y snapshots coherentes; el piano conserva su canal mpsc provisional;
  la percusión aún no recibe actualizaciones de plan durante la reproducción.
- Loop configurable, tempo map, tempo editable en UI, chase de notas y cambios de plan en caliente.
- Guardado `.dawproj`, IDs de proyecto, undo/redo, remuestreo y mezcla multipista.
- Edición MIDI avanzada del piano roll, launcher, automatización, MIDI físico y hosting CLAP/VST3.
- Mixer y paneo por pista siguen siendo mock; Volume y M/S/R aplicados a las fuentes conectadas sí funcionan.

## Orden recomendado

1. **Varias instancias por pista:** la asignación única y el `[+]` ya están conectados;
   falta crear instancias independientes, decidir si cada una abre su stream o comparte
   motor y generalizar el catálogo. Separar identidad, datos de ejecución y vista.
2. **Motor único:** extraer procesadores del clip, piano y percusión, abrir una salida y
   definir el contrato `procesar bloque + eventos`. Resolver frecuencia de samples y motor.
3. **Puente RT:** aprobar `rtrb`, limitar comandos por bloque y decidir saturación,
   prioridad de Stop/AllOff y devolución de recursos fuera del callback.
4. **Samples reales:** sustituir la síntesis provisional por cuatro samples cargados
   fuera del callback y un pool de voces acotado, conservando los offsets verificados.
5. **Transporte conectado:** GPUI observa snapshots. Loop configurable y cambios de plan
   en caliente; edición de BPM y preparación de planes coherentes.
6. **Persistencia:** aprobar Serde/JSON y guardado temporal; versionar datos musicales
   separadamente del layout, nunca serializar voces/streams.
7. Swing/probabilidad/microtiming, más vistas y extensiones sobre esos contratos.

La futura arquitectura completa se mantiene como dirección; estos puntos son tareas
concretas, no módulos vacíos ya creados.

## Partículas: tokens opcionales

Añadir a `~/.jaybu.profile` y recargar con `R` o `:reload`:

```ini
PLAYHEAD_PARTICLE_RATE=120
PLAYHEAD_PARTICLE_LIFE=0.6
PLAYHEAD_PARTICLE_SIZE=2
PLAYHEAD_PARTICLE_SPEED=70
PLAYHEAD_PARTICLE_GRAVITY=60
PLAYHEAD_PARTICLE_COLOR=52c77a
CURSOR_FX=fire
TRACK_CONTROL_STYLE=knobs
KNOB_SIZE=28
TRACK_HEADER_LAYOUT=standard
# Ejemplo TCP a medida por filas: TRACK_HEADER_LAYOUT=name,mute,solo,add|vol,pct,pan
# Ejemplo ocultar elementos: TRACK_HEADER_HIDE=pan,pct
```

Rate en partículas/segundo; vida en segundos; tamaño/velocidad/gravedad en píxeles,
píxeles/segundo y píxeles/segundo². Rate 0 desactiva emisión. Pool máximo: 512.
Solo emite al avanzar el playhead visible; al detenerse la estela se desvanece.
Su reloj visual está separado del transporte musical.
