# Lotes de pistas desde Lua · contrato de PR-02

Estado: **contrato de diseño**. Este commit no añade comandos, snapshots ni edición
de pistas desde Lua. La implementación seguirá el orden del [plan](PLAN_LUA_MULTITRACK_HILOS.md#pr-02--automatizar-pistas-desde-lua).

## Fuente y snapshot

El controlador captura en el hilo de la sesión una copia inmutable y coherente de:

- `TrackId`, orden, nombre, `TrackKind`, padre, instrumento, mute y solo de cada pista;
- nivel de fader y panorama de la misma fila (`track_levels` y `track_pans`, enteros
  de 0 a 100; 75 es unidad de fader y 50 es panorama central);
- árbol FX por pista, con `FxId`, tipo, posición, nombre, bypass, identidad de
  dispositivo y parámetros. Se copian datos musicales, nunca instancias DSP;
- una revisión de origen de la sesión para pistas, mezcla y FX. Esta revisión
  monotónica **aún no existe**: debe avanzar con cualquier edición por teclado,
  ratón, MIDI Learn, Lua o importación que cambie esos datos.

Las referencias públicas usan `TrackId`/`FxId`, no índices de fila. El índice solo
sirve para mostrar el orden y para reconciliar los vectores actuales de mezcla.
El snapshot no incluye clips ni estado visual como campos editables del script.
La lectura de Loro (`ProjectDoc`) aporta la identidad y topología persistibles;
el documento de esquema 4 todavía no guarda niveles, panorama, audio ni colores.
Por eso el snapshot de lote combina la proyección musical con la mezcla de sesión
sin presentar a Loro como fuente completa de la sesión actual.

## Operaciones tipadas de la primera versión

Un lote ordenado puede pedir:

| Operación | Restricción |
|---|---|
| Crear pista Audio | Nombre y handle temporal obligatorios; posición explícita, opcionalmente dentro de un grupo existente o creado antes en el lote |
| Renombrar pista | Nombre no vacío, de una línea, hasta 128 bytes UTF-8 |
| Mover pista | Destino existente, dentro del mismo grupo o nivel; respeta el orden de hijos contiguos |
| Fijar mute, solo, nivel o panorama | Valores absolutos; nivel y panorama enteros de 0 a 100 |
| Editar FX de una pista | Solo variantes ya soportadas por `FxEdit`: insertar, quitar, duplicar, renombrar, activar/desactivar, parámetro y mover; la inserción usa dispositivos disponibles en el catálogo y `FxPlacement` |

Para crear un grupo desde un lote se usará una operación estructural tipada con
handle, miembros existentes y al menos dos pistas que acepten clips, con las
restricciones actuales de un nivel de grupos. Esta operación deberá entrar por la
misma validación que `TrackEdit::Group`; no se expone como una escritura arbitraria
de `parent`. El lote inicial de dos pistas no necesita un grupo.

No se aceptan índices Lua como IDs, nombres como referencias, mutación libre de
tablas `Track`/`FxNode`, borrado o duplicación de pistas, grabación, buses, estado
de plugins, audio ni funciones DSP nuevas. Los efectos VST3 solo se podrán usar
cuando el host pueda validar y preparar esa inserción por la ruta existente; la
plantilla incluida usará capacidades Hay ya disponibles. La API MIDI de paquetes
v1 sigue separada hasta que un commit posterior añada la invocación de lotes.

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
operaciones y 64 pistas nuevas; se rechaza entero si excede el límite. Las
acciones UI y Lua deben compartir la validación estructural, no duplicarla.

Antes de publicar se compara la revisión capturada con la vigente. Si cambió,
se rechaza el lote completo como obsoleto; no se intenta rebasarlo automáticamente.
La publicación de pistas, niveles, panorama, FX y mapas de filas ocurre como una
sola transición de sesión con un único avance de revisión. Clips y mapeos MIDI
conservan su `TrackId`; selección, cursor, color y presentación se reconcilian
contra el mapa anterior/nuevo. Si falla la preparación o validación, ninguna de
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
La implementación del lote debe validar la proyección Loro en un candidato antes
de sustituir la sesión; si existe un documento de proyecto conectado, sus cambios
de pistas/FX se reconcilian por ID en ese candidato y se publican junto con la
edición local. Nunca se reconstruye el orden CRDT completo para una simple
renominación. Mientras mezcla, audio y colores no estén en el esquema, se
conservan por separado; no se promete que un snapshot Loro por sí solo restaure
todo el lote.

Una futura colaboración puede transportar las actualizaciones Loro existentes.
Para ello el snapshot deberá guardar también la frontera causal del documento,
además de la revisión local; una importación remota compatible avanzará la
revisión y forzará a rehacer el lote capturado. Antes de aceptar cambios remotos,
se materializa y valida un candidato y se reconcilia la sesión por `TrackId`.
La fusión CRDT no equivale a una transacción distribuida: este PR no añade red,
resolución automática de conflictos, sincronización de mezcla ni undo compartido.
