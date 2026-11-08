# Estado del prototipo y siguiente incremento

## Disponible

- Arrangement: un clip, waveform, drag, cursor, zoom y scroll.
- Reproducción de archivo y piano SoundFont, cada uno con Volume/bypass.
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
  forma de perilla (`knobs`, activo por defecto) o barras (`bars`), rueda del ratón para girar y
  clic derecho para restaurar al centro/defecto, además de presets (`standard`, `compact`, `mixer`,
  `minimal`) y conmutadores de elementos en `◈ Hay` o `~/.jaybu.profile` (`TRACK_CONTROL_STYLE`,
  `TRACK_HEADER_LAYOUT`, `TRACK_HEADER_HIDE`, `KNOB_SIZE`).
- Puntero inmersivo en GPU (`src/ui/cursor_fx.rs` con `particula-rs` + `fastrand`): cinco modos
  seleccionables desde la barra superior, el panel `◈ Hay`, comandos (`:cursor fire|sparks|neon|stars|off`)
  o `~/.jaybu.profile` (`CURSOR_FX=fire`), incluyendo el efecto **Fueguito** por defecto.

## Limitaciones de esta audición

- Polifonía, Reverb+chorus y el cambio de archivo `.sf2` solo los lee RustySynth al crearse:
  cambiarlos con el piano abierto lo cierra (corta notas) y se reabre en el siguiente note-on.
  Cambiar de preset dentro del mismo banco sí se aplica en vivo por MIDI Bank/Program.
- Los parámetros no se guardan al cerrar la app (no hay persistencia todavía).

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
- Piano roll, launcher, automatización, MIDI físico y hosting CLAP/VST3.
- Mixer y M/S/R de las pistas siguen siendo mock. Volume de los reproductores sí es funcional.

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
KNOB_SIZE=22
TRACK_HEADER_LAYOUT=standard
# Ejemplo TCP a medida por filas: TRACK_HEADER_LAYOUT=name,mute,solo,add|vol,pct,pan
# Ejemplo ocultar elementos: TRACK_HEADER_HIDE=pan,pct
```

Rate en partículas/segundo; vida en segundos; tamaño/velocidad/gravedad en píxeles,
píxeles/segundo y píxeles/segundo². Rate 0 desactiva emisión. Pool máximo: 512.
Solo emite al avanzar el playhead visible; al detenerse la estela se desvanece.
Su reloj visual está separado del transporte musical.
