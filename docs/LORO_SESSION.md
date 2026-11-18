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
El `merge` actual rechaza candidatos inválidos; antes de conectar red hay que
resolver o aplazar esos cambios de forma determinista y probar convergencia en
ambos órdenes. El transporte de audio, las voces y los buffers no son datos CRDT.

## Apertura y guardado local

`ui/project` captura el estado editable de la UI, conserva el documento del último
archivo y reconcilia el siguiente guardado sobre él. No hay sincronización por gesto
ni colaboración en red. Guardar como cambia la ubicación solo después del éxito.
Las escrituras usan el reemplazo atómico de `ProjectDoc::save`.

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

## Siguientes incrementos

1. Estado de Sampler, plugins, Pattern, MIDI Learn y Utility de salida.
2. Integrar colectar en el proyecto activo y recuperación de recursos faltantes.
3. Operaciones de UI sobre el documento vivo, pruebas de concurrencia y transporte de red.
