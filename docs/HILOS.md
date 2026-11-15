# Hilos: identidad y contrato de ramas (PR-07, commit 1)

Un **Hilo** es un contenedor musical de una pista. Recibe eventos MIDI, los envía a
ramas con instrumento y suma audio de las ramas en el transporte existente. No es
un hilo del sistema operativo ni una instancia de plugin. La primera ejecución
puede recorrer las ramas de forma secuencial en el mismo callback de audio.

## Identidades

| Identidad | Qué representa | Regla |
|---|---|---|
| `TrackId` | Pista contenedora o pista hija usada como rama | Estable al reordenar; una rama pertenece a un solo contenedor |
| `InstrumentId` | Instancia concreta de instrumento | No se comparte entre ramas; dos copias de un plugin tienen IDs y estado distintos |
| `FxId` | Nodo de la cadena FX de una rama | Conserva la semántica de serie/paralelo y el estado de bypass existente |
| `NoteId` | Nota de un clip MIDI | Su NoteOn y NoteOff conservan la misma rama, incluso tras otras notas |
| Puerto | Entrada o salida tipada de una ruta | Su identidad pertenece al plan del Hilo; no sustituye los IDs anteriores |

El tipo o clase de plugin describe **qué cargar**; `InstrumentId` identifica
**cuál instancia suena**. El nombre visible de una rama puede cambiar sin alterar
ninguna conexión. Duplicar una rama exige un instrumento nuevo antes de publicar
el plan; copiar un `InstrumentId` produciría dos dueños de la misma voz.

## Puertos y recorrido

La ruta mínima admite dos dominios:

| Puerto | Dato | Conexiones válidas |
|---|---|---|
| `Events` | Eventos MIDI con canal, nota y offset de frame | Entrada del Hilo → entrada de instrumento |
| `Audio` | Frames estéreo a la frecuencia de la salida | Salida de instrumento o audio de clip → FX de rama → sumador |

Una rama **de instrumento** convierte `Events` en `Audio`. Una rama **de audio**
recibe `Audio` y no consume notas. Los FX existentes reciben y devuelven `Audio`;
sus contenedores serie/paralelo conservan el orden y la suma actuales. Un cable
`Audio → Events`, un ciclo o una referencia a un ID inexistente invalida el plan.
La validación y preparación completa ocurren fuera del callback. Si fallan, se
conserva el último plan válido y se informa el error sin publicar media ruta.

El plan preparado fija el orden, recursos y buffers de cada rama. El callback
solo procesa los eventos del bloque, ejecuta las ramas en ese orden y suma sus
salidas. No abre plugins, decodifica archivos, crea buffers ni llama a
temporizadores GPUI. Todas las ramas leen el mismo frame absoluto, BPM, compás y
loop del transporte. **Hilo no promete ejecución multinúcleo.**

## Distribución de notas

`Layer` envía cada NoteOn a todas las ramas de instrumento habilitadas.
`RoundRobin` envía cada NoteOn a la siguiente rama habilitada, en el orden
visible de las ramas, y vuelve al inicio después de la última. Cada NoteOff
regresa a la rama o ramas que recibieron ese NoteOn. El mapeo usa `NoteId` para
clips; en MIDI físico, que no trae ese ID, se mantiene una pila por pitch para
las pulsaciones repetidas. Los CC y controles globales se difunden a las ramas
de instrumento; una rama de audio no recibe MIDI.

Para este PR, round robin es una opción simple de entrada del Hilo. **Tejer**
podrá añadir después reglas, patrones, probabilidades y estado musical más rico
sin cambiar la propiedad de las notas ni el transporte.

## Nivel, bypass y ciclo de vida

Cada rama conserva nivel, pan, FX y mute propios. Cambiar el nivel de una rama
no cambia el de sus hermanas. Silenciar una rama deja procesar su voz y sus
colas con ganancia de salida cero; permite recuperarla sin reconstruirla.
Desactivar o retirar una rama libera primero solo las notas de esa rama. Si la
edición estructural requiere un nuevo plan, se prepara y valida antes de
publicarlo. Las colas de sonido de una rama retirada tienen un límite explícito
del motor; no se destruye un plugin ni se hace trabajo costoso en el callback.

La UI y Lua describen la misma composición por IDs. Ambas pasan por la misma
validación antes de confirmar. Guardar un documento musical no implica guardar
recursos externos ni estado de plugins hasta que esos contratos existan.

## Criterios de cierre de PR-07

- Dos instrumentos distintos suenan en capas en una pista contenedora y sus
  niveles se pueden cambiar por separado.
- Round robin alterna NoteOn y devuelve NoteOff a la rama original, también con
  notas del mismo pitch repetidas.
- Una ruta inválida deja intacto el plan activo. Retirar o silenciar una rama
  no libera las notas de otra.
- Lua y Track FX muestran y editan el mismo Hilo; las pruebas de audio no
  requieren abrir un dispositivo físico.

Este documento fija el contrato. Los commits siguientes conectan modelo,
procesamiento y presentación. El estado implementado se registra en
[`PROTOTYPE.md`](PROTOTYPE.md) y en [`index.html`](../index.html).
