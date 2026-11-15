# Lotes de pistas desde Lua · contrato de PR-02

Estado: **implementado en PR-02**. `track.script` ejecuta un archivo Lua;
`track.template.two` y `track.template.hilo` ejecutan plantillas incluidas. Confirman el lote
mediante el mismo ejecutor headless. [Plan](PLAN_LUA_MULTITRACK_HILOS.md#pr-02--automatizar-pistas-desde-lua).

## Uso

Desde `[+]` elige **Lua · plantilla de dos pistas** o escribe
`:track.template.two`. Se crean «Voz principal» y «Voz doble» con nivel, panorama
y un Hay Utility cada una. La fuente editable está en
[`assets/scripts/tracks/two_tracks.lua`](../assets/scripts/tracks/two_tracks.lua).
Para ejecutar una copia propia, usa `[+]` → **Ejecutar script de pistas…** o
`:track.script '/ruta/script.lua'`.

El script recibe `tracks` (arreglo ordenado de pistas con `id`, `name`, `kind`,
`parent`, `instrument`, `muted`, `solo`, `level`, `pan` y árbol `fx`) y
`track_revision`. Devuelve un arreglo denso de operaciones; editar la tabla
`tracks` no cambia la sesión. Los IDs existentes son enteros; las pistas nuevas
se referencian por el texto de su handle:

```lua
return {
    { op = "insert_audio", handle = "voz", name = "Voz" },
    { op = "level", track = "voz", value = 75 },
    { op = "pan", track = "voz", value = 60 },
    { op = "fx.insert_utility", track = "voz", gain = 0.9 },
}
```

También hay `group` (`handle`, `name`, `members`), `hilo.mode` (`track`, `mode`
igual a `layer`, `round_robin` u `off`), `rename` (`track`, `name`),
`move` (`track`, `after`), `mute` y `solo` (`track`, `value` booleano). Las
operaciones FX admitidas son `fx.insert_utility`, `fx.insert_serial`,
`fx.insert_parallel`, `fx.rename`, `fx.toggle`, `fx.remove`, `fx.duplicate`,
`fx.parameter` y `fx.move`. Un FX existente se referencia por `fx` (ID entero);
las posiciones usan **uno** de `inside`, `before` o `after` (IDs FX), y sin
posición se inserta al final de la raíz. Los contenedores nuevos se insertan
vacíos. Los handles de pistas creadas se muestran como `handle=TrackId` al
confirmar; la confirmación fallida no devuelve IDs.

## Fuente y snapshot

El controlador captura en el hilo de la sesión una copia inmutable y coherente de:

- `TrackId`, orden, nombre, `TrackKind`, padre, instrumento, `hilo_mode`, mute y solo de cada pista;
- nivel de fader y panorama de la misma fila (`track_levels` y `track_pans`, enteros
  de 0 a 100; 75 es unidad de fader y 50 es panorama central);
- árbol FX por pista, con `FxId`, tipo, posición, nombre, bypass, identidad de
  dispositivo y parámetros. Se copian datos musicales, nunca instancias DSP;
- una revisión de origen de la sesión para pistas, mezcla y FX. Avanza con las
  ediciones conectadas de teclado, ratón, MIDI Learn y Lua; una futura importación
  Loro conectada a la UI tendrá que avanzar la misma revisión.

Las referencias públicas usan `TrackId`/`FxId`, no índices de fila. El índice solo
sirve para mostrar el orden y para reconciliar los vectores actuales de mezcla.
El snapshot no incluye clips ni estado visual como campos editables del script.
La UI captura el modelo musical y sus vectores de mezcla. Antes de confirmar
construye y valida una proyección candidata en `ProjectDoc` (Loro) con pistas, FX
y clips MIDI. El esquema 4 todavía no guarda niveles, panorama, audio ni colores;
la UI aún no mantiene un documento Loro conectado en vivo.

## Operaciones tipadas de la primera versión

Un lote ordenado puede pedir:

| Operación | Restricción |
|---|---|
| Crear pista Audio | Nombre y handle temporal obligatorios; `after` y `parent` son opcionales. Sin posición se añade al final del nivel elegido |
| Renombrar pista | Nombre no vacío, de una línea, hasta 128 bytes UTF-8 |
| Mover pista | Destino existente, dentro del mismo grupo o nivel; respeta el orden de hijos contiguos |
| Fijar mute, solo, nivel o panorama | Valores absolutos; nivel y panorama enteros de 0 a 100 |
| Editar FX de una pista | Variantes soportadas por `FxEdit` sobre cadenas de Hay Utility y contenedores serie/paralelo; valores de parámetros validados por el descriptor |
| Configurar Hilo | `hilo.mode` solo en un grupo; las pistas hijas con instrumento son sus ramas. El lote no instancia instrumentos nuevos |

`group` usa un handle y al menos dos miembros raíz existentes o creados antes
en el lote. Comparte `TrackEdit::Group` y la validación de un nivel de grupos;
no expone una escritura arbitraria de `parent`.

Para un grupo creado dentro del mismo lote:

```lua
{ op = "group", handle = "hilo", name = "Hilo", members = { "a", "b" } },
{ op = "hilo.mode", track = "hilo", mode = "round_robin" },
```

`:track.template.hilo` agrupa las dos primeras pistas raíz con instrumentos
distintos, configura round robin e inserta Hay Utility en cada rama. Los
instrumentos deben estar cargados antes de ejecutar la plantilla.

No se aceptan índices Lua como IDs, nombres como referencias, mutación libre de
tablas `Track`/`FxNode`, borrado o duplicación de pistas, grabación, buses, estado
de plugins, audio ni funciones DSP nuevas. Una cadena con VST3 o Spectrum no se
edita por lote porque requiere preparar instancias o tomas en el host; sí se
conserva intacta al editar otros campos. La API MIDI de paquetes v1 sigue
separada: los lotes usan `track.script`, no `midi.tool`.

## Handles, revisión y confirmación

Cada creación declara un handle local único. Una operación posterior puede
referirse a él; las referencias hacia delante, los duplicados y los handles de
otro lote se rechazan. El ejecutor reserva IDs definitivos al confirmar y devuelve
un mapa `handle → TrackId` solo si todo el lote se publicó. Los IDs no se reutilizan;
un rechazo puede consumir números internos, pero no crea pistas visibles.

El ejecutor aplica las operaciones **en orden** sobre un candidato privado y
valida después cada paso y el estado completo: IDs únicos, destinos presentes,
tipo de padre, un nivel de grupos, hijos contiguos, asignación exclusiva de
instrumentos, FX sin ciclos ni destinos obsoletos, parámetros finitos y límites
de `FxChain` (32 niveles y 256 nodos por pista). El lote acepta como máximo 128
operaciones y 64 pistas nuevas. La fuente ocupa hasta 256 KiB y usa el sandbox
Lua de 8 MiB y 250 ms. Se rechaza entero si excede un límite. Las
acciones UI y Lua deben compartir la validación estructural, no duplicarla.

Antes de publicar se compara la revisión capturada con la vigente. Si cambió,
se rechaza el lote completo como obsoleto; no se intenta rebasarlo automáticamente.
La publicación de pistas, niveles, panorama, FX y mapas de filas ocurre como una
sola transición de sesión con un único avance de revisión. Clips y mapeos MIDI
conservan su destino mediante el mapa de índices anterior/nuevo por `TrackId`.
Selección, cursor, color y presentación se reconcilian con ese mismo mapa. Si
falla la preparación o validación, ninguna de
esas estructuras se sustituye. Los recursos de audio se preparan fuera del
callback; el callback solo recibe los planes ya preparados.

Los errores distinguen `lote_obsoleto`, `limite`, `referencia_invalida`,
`operacion_invalida`, `estructura_invalida` y `recurso_no_disponible`. Indican el
índice de operación desde 1, su tipo y la causa; un error anterior al recorrido
(por ejemplo, revisión obsoleta) indica el lote completo. Lua recibe el error y
no obtiene un mapa de handles parcial. La atomicidad es de **aplicación local del
lote**, no undo/redo global.

## Loro y colaboración futura

`ProjectDoc` ya conserva IDs, campos de pistas y FX en mapas/listas movibles,
reconcilia sin recrear el orden y valida un `merge` sobre copia antes de publicarlo.
El lote valida una proyección candidata antes de sustituir la sesión. La
conexión persistente de `ProjectDoc` a la UI queda pendiente: cuando exista,
deberá reconciliar pistas/FX por ID y publicar el documento junto con la edición
local. No se reconstruirá el orden CRDT completo para una simple renominación.
Mientras mezcla, audio y colores no estén en el esquema, se conservan por
separado; un snapshot Loro por sí solo no restaura todo el lote.

Una futura colaboración puede transportar las actualizaciones Loro existentes.
Para ello el snapshot deberá guardar también la frontera causal del documento,
además de la revisión local; una importación remota compatible avanzará la
revisión y forzará a rehacer el lote capturado. Antes de aceptar cambios remotos,
se materializa y valida un candidato y se reconcilia la sesión por `TrackId`.
La fusión CRDT no equivale a una transacción distribuida: este PR no añade red,
resolución automática de conflictos, sincronización de mezcla ni undo compartido.
