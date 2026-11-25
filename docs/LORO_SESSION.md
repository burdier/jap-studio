# Estado de sesión y colaboración

La referencia de Ardour es su separación de responsabilidades: el archivo de
sesión XML describe objetos editables y la carpeta guarda audio, MIDI y otros
recursos. Ardour no usa Loro para ese archivo. Sus objetos exponen operaciones
de lectura y restauración de estado; Hay puede aplicar la misma separación con
su documento Loro, sin copiar el formato XML.

Referencias: [sesiones de Ardour](https://manual.ardour.org/introducing-ardour/understanding-basic-concepts-and-terminology/),
[archivos y snapshots de Ardour](https://manual.ardour.org/ardourmanual.html),
[sincronización Loro](https://loro.dev/docs/tutorial/sync).

## Mapa de Hay

| Estado editable | Documento Loro | Ejecución |
|---|---|---|
| Pistas, Hilos, FX y notas MIDI | IDs estables, listas y mapas por entidad | Planes y voces preparados fuera del callback |
| Tempo y compás | Campos de proyecto separados | Transporte común en frames |
| Nivel y panorama | Campos por `TrackId` | Controles de mezcla publicados al motor |
| Clips de audio | ID, `TrackId`, ruta UTF-8, posición, recorte y propiedades | Archivo decodificado y buffers fuera del callback |
| Plugins y samples | Pendiente: tipo, recurso y estado serializable | Instancias y editores con ciclo de vida propio |

El esquema 4 acepta opcionalmente tempo, compás, nivel, panorama y clips de audio; un snapshot
anterior recupera 120 BPM, 4/4, nivel 75 y panorama 50. `set_tempo`, `set_meter`,
`set_track_level` y `set_track_pan` escriben campos independientes. Así, dos
copias que editan controles distintos pueden fusionar sus operaciones sin que
un guardado completo pise el otro. El compás se escribe como un valor único
(`7/8`), para evitar mezclar partes de dos cambios concurrentes; todo
merge se aplica a un candidato antes de publicar el documento.

`project/audio` guarda metadata de cada clip WAV en mapas por ID y el orden en una
lista movible. `AudioFile::decode` conserva la ruta canónica del recurso y los
clips duplicados reciben ID nuevo, aunque compartan la misma fuente. El documento
admite leer metadata aun cuando el archivo ya no exista: la resolución y
decodificación deben hacerse al preparar una sesión candidata para Abrir. Las
rutas son UTF-8. `audio.collect` copia las fuentes a una carpeta nueva `Audio/`
y guarda `session.loro` con rutas relativas; si varios clips comparten archivo,
se copia una sola vez. La carpeta se publica después de completar todas las copias
y el snapshot. Los clips de la sesión abierta se redirigen a las copias si
conservan su fuente; los originales permanecen donde estaban. Una fuente generada solo en memoria no puede guardarse
como referencia de archivo.
La posición y la ganancia disponen de ediciones por campo para fusionar cambios
independientes de dos copias; todavía no se conectan a gestos de la UI.

Loro hace converger las operaciones que reciban las copias. Hay debe definir
la respuesta musical a cambios concurrentes que puedan dejar referencias
inválidas, como borrar una rama mientras otra copia dirige un clip hacia ella.
El `merge` actual rechaza candidatos inválidos. Para editar en continuo hay que
resolver o aplazar esos cambios de forma determinista y probar convergencia en
ambos órdenes. El transporte de audio, las voces y los buffers no son datos CRDT.

## Apertura y guardado local

`ui/project` captura el estado editable de la UI, conserva el documento del último
archivo y reconcilia el siguiente guardado sobre él. Guardar como cambia la ubicación
solo después del éxito. Las escrituras usan el reemplazo atómico de `ProjectDoc::save`.

La apertura valida el documento, resuelve recursos relativos al archivo, deduplica
la decodificación de audio y comprueba SoundFonts/presets antes de publicar la sesión.
Cambios durante la carga o un recurso faltante conservan la sesión anterior. Los IDs
restaurados reservan los contadores para futuras pistas, clips e instrumentos.
`project/instruments` añade registros opcionales de Hay Piano con ruta, preset y
parámetros; documentos antiguos siguen siendo legibles, pero la UI rechaza instrumentos
sin estado recuperable. Sampler/VST3/FX de host siguen pendientes.

El alcance y exclusiones de este incremento están en README, «Abrir y guardar proyectos».
La acción `audio.collect` conserva su alcance anterior; no se convierte en un guardado
completo ni incorpora SoundFonts automáticamente.

## Colaboración en vivo por Iroh

Abre una copia guardada del mismo proyecto en cada ventana. En la primera, elige
**Archivo → Compartir sesión** y pulsa **Copiar invitación** en el diálogo.
También puedes seleccionar el código y copiarlo con Ctrl+C.
En la segunda, copia esa invitación y elige **Archivo → Unirse**. El título muestra
**En sesión** cuando entra el invitado. **Salir de la sesión** cierra la conexión.
La prueba en Wayland leyó la invitación del portapapeles y la usó en la segunda
ventana.

Ambas ventanas intercambian el estado editable del `.loro` en vivo. Incluye
pistas, grupos, Hilos y Utility de pista, tempo y compás, mezcla, clips de
Arrangement, sus notas y controles MIDI, Drums Latex y Hay Piano. Un arrastre de
varios clips se publica al soltar el ratón. Si el Piano Roll tiene abierto el
mismo clip, conserva el foco mientras llegan sus notas.

Al entrar, la ventana invitada recibe el estado del anfitrión. Después, ambas
envían operaciones Loro con la versión desde la que editaron. Las ediciones sobre
clips distintos se pueden combinar. Si las dos cambian el mismo clip de maneras
incompatibles o la fusión deja referencias inválidas, la sesión se cierra y cada
músico puede guardar sus cambios en su copia. Guardar sigue siendo una acción local.

Las fuentes de audio, muestras y SoundFonts se comparten por referencia y deben
estar disponibles en ambas copias. Sampler, el estado de plugins externos,
MIDI Learn y Utility de salida quedan fuera del estado recuperable. Cada ventana
controla su selección, su vista y su transporte.

Con dos ventanas del mismo release, se movieron, duplicaron y borraron dos
clips de pistas distintas en ambos sentidos. Con el mismo clip abierto en
ambos Piano Rolls, el anfitrión duplicó notas y el invitado las borró y restauró.
Los cambios se vieron antes de guardar. Los dos archivos guardados conservaron
los mismos IDs, posiciones y notas, además del tempo y el mute compartidos.

## Intercambio puntual desde terminal

Con la feature `p2p`, el propio binario de Hay ofrece un intercambio puntual entre
dos copias `.loro` del mismo proyecto. El anfitrión genera una invitación de un uso
y los dos procesos se autentican con identidades Iroh distintas. Cada lado valida
el origen de la sesión y fusiona las operaciones de su par antes de guardar
atómicamente su propia copia. La invitación da acceso al historial y se comparte
con el colaborador previsto. El límite del piloto es 16 MiB de operaciones por copia.

```sh
cargo build --release --bin daw-gpui --features p2p --locked -j 2
target/release/daw-gpui --collab share anfitrion.loro
target/release/daw-gpui --collab join invitado.loro '<invitación de HAY_INVITE>'
```

`share` usa Iroh en modo automático. Opcionalmente acepta `direct` sin relay o
`relay` sin conexión IP directa. Después de `relay` se puede indicar la URL de
un relay propio. La salida `HAY_SYNC` muestra la ruta seleccionada.

Para verificar ambos sentidos, usar dos carpetas de salida nuevas. `probe` crea
un proyecto de prueba, copia la base y hace un cambio en cada copia. Lanza dos
procesos del mismo ejecutable y comprueba que los dos archivos recuperen ambos cambios.

```sh
target/release/daw-gpui --collab probe /tmp/hay-p2p-directo direct
target/release/daw-gpui --collab probe /tmp/hay-p2p-relay relay
```

La prueba de release pasó primero por QUIC directo y luego por un
`iroh-relay` oficial en local y por un relay público de Iroh. Las dos pruebas de
relay desactivaron los sockets IP directos. En cada prueba las dos copias recuperaron
el golpe de hi-hat del anfitrión y el nivel 53 del invitado.

Los cambios del intercambio puntual se publican en los archivos `.loro` y se ven
al abrirlos en Hay.

## Siguientes incrementos

1. Estado de Sampler, plugins, Pattern, MIDI Learn y Utility de salida.
2. Integrar colectar en el proyecto activo y recuperación de recursos faltantes.
3. Transferir los archivos de audio y SoundFonts asociados al proyecto compartido.

Los nodos de efectos VST3 guardan su preset en el campo binario `vst_state`. Cada participante necesita la misma clase de efecto instalada. El volumen y Solo de las ramas se sincronizan por claves independientes y conservan la instancia que está sonando.
