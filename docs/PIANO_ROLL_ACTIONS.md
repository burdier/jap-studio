# Piano roll: acciones y estado real

Los botones, el catálogo `[+]`, el modo `:` y los iconos configurables comparten
controladores. `catalog.rs` registra las acciones; `actions/piano_roll.rs` y
`scripting/actions.rs` describen sus parámetros. Sin argumentos, los comandos que
requieren datos abren el formulario existente; con argumentos, ejecutan la acción.
Las preferencias visuales se conservan al cambiar de clip durante la sesión.

## Barra del mock

| Control | Acción / comportamiento |
|---|---|
| ◀ / ▶ | `midi.clip.previous` / `midi.clip.next`; orden por pista, posición e ID |
| Selector de clip | `midi.clip.select`; menú con nombres y pistas. `midi.clip.select <ID>` selecciona directamente |
| Len | `midi.clip.length <compases>`; longitud del patrón, admite fracciones. Rechaza finales que eliminarían notas; conserva IDs y recorte válido |
| Ch | `midi.clip.channel <1–16>`; salida del clip en playback y exportación. Guardado en el documento Loro; documentos anteriores usan canal 1 |
| Grid / más fina / más gruesa | `midi.grid <denominador>`, `midi.grid.finer`, `midi.grid.coarser`. Figuras 1/1–1/128; Alt+1/2 |
| Swing | `midi.swing <0–100>`; formulario y transformación Lua |
| Q | `midi.quantize [denominador] [strength]`; formulario con rejilla e intensidad |
| H | `midi.humanize [cantidad]`; jitter de tiempo y velocity. Usa la cantidad actual si se omite |
| Subdividir | `midi.subdivide [denominador]`; usa el paquete recargable `hay.midi.subdivide` si está registrado; exige selección explícita; genera segmentos con IDs y mute heredados |
| Lua… | `midi.script <ruta>`; formulario para un script propio. `midi.script.accent` conserva el alias de Acentuar, migrado a `hay.midi.accent` |
| V: figura | `midi.draw.length <denominador>`; menú de duración de inserción, independiente del snap |
| Ghost | `midi.ghost`; muestra otros clips de la misma pista en su posición temporal |
| Follow | `midi.follow`; sigue el playhead leído del motor cuando sale de la zona visible; suspende desplazamiento durante arrastre |
| Loop | `midi.clip.loop`; repite la ventana audible del clip desde el callback por frames. Exportación MIDI conserva una pasada finita |
| Edited • | Indicador calculado comparando con el clip al abrir el editor; NO significa guardado de proyecto |
| Pointer (V), Draw, Erase, Paint, Mute | `midi.tool.pointer` (alias `.select`), `.draw`, `.erase`, `.paint`, `.mute`; Paint actúa al arrastrar, una vez por celda. Pointer: clic selecciona/mueve, doble clic crea o borra una nota |
| Región / Borrar región | `midi.select.region` (alias `midi.tool.box`) / `midi.delete.region`; arrastrar un rectángulo selecciona notas que lo intersectan. Borrar región confirma al soltar; Esc cancela sin borrar |
| Eliminar · Del | `midi.delete.selection` (alias `midi.delete`); Delete/Backspace elimina exclusivamente la selección. Vacía: no borra nada. Respeta bloqueo, conserva CC y limpia mute de notas borradas |
| Seleccionar todo / despejar | `midi.select.all` / `midi.select.clear`; Ctrl+A / Ctrl+Shift+A, también con Vim desactivado |
| Caja / lápiz / longitud | `midi.tool.box`, `.draw`, `.length` |
| Copiar / cortar / pegar | `midi.copy`, `midi.cut`, `midi.paste`; Ctrl+C/X/V, selección completa con offsets y mute. Pegar rechaza una selección que no cabe sin aplicar cambios parciales |
| Scale / Key | `midi.scale.mode major\|minor`, `midi.scale.root C\|C#\|Db…`; menús de escala y doce tónicas |
| Velocity… | `midi.velocity <1–127>`; transforma selección o todas las notas si no hay selección |
| Gate… | `midi.gate <0–100>`; porcentaje de duración, mínimo un tick |
| Humanize − / + / rueda | `midi.humanize.amount <0–100>`; ajusta la cantidad que consume H |
| Ghost / Erase / Escala asignados a iconos | Mismas acciones que sus botones. `[+]` permite vincular acciones MIDI o comandos con argumentos |
| ⚙ | `midi.settings`; abre la configuración existente |

Los menús reutilizan `components/context_menu`; Color reutiliza `color_picker`.
Los botones configurables respetan la barra elegida y ejecutan su comando real.
La ventana separada presenta también formularios, selector de iconos y configuración.

Pointer inserta con la duración **Grid** actual, ajustando el inicio a esa rejilla y
recortando solo si alcanza el final del clip. Draw/Paint conservan su duración **V: figura**.
Un clic sin arrastre no cuantiza notas existentes. El suavizado al mover/redimensionar
y acomodarse a la rejilla afecta solo la vista; los datos se confirman inmediatamente
al soltar. Estas acciones Rust están disponibles en `[+]`, iconos y comandos `:`.

Al mover notas hacia una vecina del mismo tono, o alargar el borde derecho contra ella,
el empuje Lua recorta la vecina desde el lado del contacto. Funciona a izquierda y derecha,
con mínimo de un tick y sin borrar notas: al agotarse ese espacio el avance se limita.
Los grupos seleccionados conservan sus distancias y los solapamientos previos quedan
intactos. Mientras arrastras, retroceder recupera la longitud original; Esc cancela y
soltar confirma con snap. La nota y sus vecinas comparten suavizado visual. El borde
fino y la sombra no cambian la geometría del picking.

## Inspector y vista

- `midi.clip.rename <nombre>`, `midi.clip.color auto|0–6`, `midi.clip.mute`:
  propiedades del clip activo, con bloqueo respetado. Color sin argumentos abre el picker.
- Length, Channel, escala, raíz, Q y Swing llaman las mismas acciones de la barra.
  Start/End muestran la posición real en el Arrangement. Solo opera sobre **la pista**
  del clip (`midi.track.solo`), no introduce otro estado de solo por clip.
- `midi.inspector.clip`, `.notes`, `.scale`, `.view`: páginas del inspector.
- `midi.scale.highlight`, `midi.scale.keyboard`, `midi.note.names`, `midi.velocities`:
  visibilidad y resaltado independientes. Clamp Lua usa escala y tónica actuales.
- `midi.view.reset`, `.zoom.in`, `.zoom.out`, `.taller`, `.shorter`: zoom y encuadre.
- `midi.window`, `midi.close`: separar/acoplar y cerrar. `midi.lane` abre el selector.
- Gestos de scroll/pinch, desplazamiento continuo y snap al soltar conservan el eje
  compartido con Arrangement (`ui/time_grid.rs`). Rojo fuera de escala solo durante drag.

## Lua y procesamiento

Q, H, Gate, Swing, Velocity, Release Velocity, Probability, Pan, Random y Clamp viven en
`assets/scripts/midi/`; Acentuar, Subdividir y Fundamental a acordes viven en
`assets/scripts/tools/midi/` como paquetes recargables. El último genera terceras y
quintas según la escala activa mediante la salida validada del paquete. Lua recibe una copia de las notas
en un worker; Rust valida
IDs, posiciones, duraciones y límites antes de aplicar por ClipId. No hay GPUI ni
trabajo Lua en el callback. `midi.random` sigue disponible en Notes y el catálogo.

Cambiar un dato musical detiene el plan inmutable: hay que pulsar Play para escuchar
el nuevo plan. Loop reutiliza ese mismo reproductor y no crea otro transporte.
SoundFont recibe el canal elegido; sfizz acepta todos los canales en una única instancia,
sin separación multitimbral. El importador sigue consolidando las notas SMF en un clip:
no conserva los canales originales por nota.

## Carriles de expresión y controles

Los carriles CC, pitch bend y presión de canal ya tienen puntos temporales independientes:
clic/arrastre crea o actualiza; Erase elimina. `midi.cc <CC> <valor> <beat>`,
`midi.pitch <valor> <beat>`, `midi.pressure <valor> <beat>` y
`midi.control.remove` comparten operaciones con el canvas. La posición se mide en
negras desde el inicio del patrón (origen 0). Sin argumentos se abre el formulario.
Los eventos se conservan al editar notas, recortar, materializar y persistir en Loro.
El importador SMF conserva CC, pitch bend y presión, incluso sin notas; sigue
consolidando sus canales a la salida del clip. Lua puede editar `controls`.

La automatización usa pasos MIDI; el dibujo no implica interpolación entre puntos.
El scheduler intercala controles antes de NoteOn en el mismo frame. Al iniciar desde
un punto intermedio se recuperan los últimos valores anteriores. Sustain se libera al
terminar el clip o detenerlo. En un loop los valores de control se mantienen hasta el
siguiente punto: coloca el valor inicial en el tick 0 para reiniciarlo en cada vuelta.

FluidSynth y sfizz reciben presión mediante su API nativa, contrastada con
[FluidSynth](https://www.fluidsynth.org/api/group__midi__messages.html) y
[sfizz 1.2.3](https://github.com/sfztools/sfizz/blob/1.2.3/src/sfizz.h). RustySynth 1.3.8 no procesa
aftertouch ni todos los CC: la respuesta depende del backend y del banco/instrumento.
Los carriles por nota editan velocity, release velocity (0–127), probabilidad (0–100%)
y pan (−64 izquierda / 0 centro / +63 derecha), mediante clic/arrastre y acciones Lua
`midi.release_velocity <valor>`, `midi.probability <porcentaje>` y `midi.pan <valor>`.
Las acciones transforman la selección o todas las notas si no hay selección.
`midi.lane.select <nombre>` permite asignar un carril a un icono: `velocity`,
`release_velocity`, `pan`, `probability`, `pitch`, `pressure` o cualquiera de los CC
ofrecidos por el selector (por ejemplo `cc74`). `midi.lane` conserva el menú existente.

`NoteExpression` conserva estas propiedades al copiar, recortar, subdividir, materializar
y guardar en Loro. Documentos antiguos usan release 64, probabilidad 100 y pan 0.
La decisión de probabilidad es reproducible por ID y número de vuelta, compartida por
NoteOn/NoteOff; cambia entre vueltas y se reinicia al pulsar Play. Exportar usa la primera
pasada. Los NoteOff del final exacto del clip también conservan release velocity.

Release velocity llega a sfizz, donde la respuesta depende del instrumento; FluidSynth
y RustySynth no ofrecen ese valor en su NoteOff. Note Pan desplaza el panorama de las
voces recién creadas de FluidSynth, conservando las capas del preset y sin modificar
las demás notas. Se usa su [API de voces](https://www.fluidsynth.org/api/group__voices.html)
con buffers acotados a la polifonía máxima (256), sin reservar memoria por nota.
sfizz y RustySynth conservan Note Pan en el modelo pero todavía no lo aplican al audio;
la descripción del carril y el formulario lo indican. CC10 sigue siendo panorama del canal.

Tampoco hay undo/redo del proyecto ni portapapeles MIDI del sistema. El documento Loro
puede conservar MIDI, incluido canal, pero el guardado completo de la sesión aún necesita
audio y controles de mezcla. Las preferencias del piano roll aún no se guardan al reiniciar.

Validación de este incremento: `cargo check --all-targets --locked` dentro de `gpui-dev`.
Sin ejecutar tests ni dar por comprobada la interacción visual/auditiva.
