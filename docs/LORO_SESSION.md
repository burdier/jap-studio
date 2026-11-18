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
| Clips de audio | Pendiente: ID de clip y referencia de recurso | Archivo decodificado y buffers fuera del callback |
| Plugins y samples | Pendiente: tipo, recurso y estado serializable | Instancias y editores con ciclo de vida propio |

El esquema 4 acepta opcionalmente tempo, compás, nivel y panorama; un snapshot
anterior recupera 120 BPM, 4/4, nivel 75 y panorama 50. `set_tempo`, `set_meter`,
`set_track_level` y `set_track_pan` escriben campos independientes. Así, dos
copias que editan controles distintos pueden fusionar sus operaciones sin que
un guardado completo pise el otro. El compás se escribe como un valor único
(`7/8`), para evitar mezclar partes de dos cambios concurrentes; todo
merge se aplica a un candidato antes de publicar el documento.

Loro hace converger las operaciones que reciban las copias. Hay debe definir
la respuesta musical a cambios concurrentes que puedan dejar referencias
inválidas, como borrar una rama mientras otra copia dirige un clip hacia ella.
El `merge` actual rechaza candidatos inválidos; antes de conectar red hay que
resolver o aplazar esos cambios de forma determinista y probar convergencia en
ambos órdenes. El transporte de audio, las voces y los buffers no son datos CRDT.

## Orden de integración

1. Añadir IDs y referencias verificables a clips de audio y recursos; completar
   parámetros de instrumentos y estado permitido de plugins.
2. Preparar una sesión candidata completa antes de **Abrir**. Un recurso ausente
   debe producir un error recuperable, sin sustituir el sonido ni borrar material.
3. Conectar las operaciones de la UI al documento y guardar un snapshot local
   atómicamente. La UI actual aún no ofrece Abrir/Guardar proyecto.
4. Probar dos copias sin red con ediciones concurrentes, conflictos y reordenación.
   Después transportar snapshots y actualizaciones entre usuarios.
