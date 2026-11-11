# Herramientas Lua

Lua permite crear transformaciones MIDI sin tocar el motor de audio. Desde `[+]` busca
**Ejecutar script MIDI…** y elige un archivo `.lua`, o usa
`:midi.script '/ruta/herramienta.lua'`. Para probar la integración, usa
**Lua · acentuar notas** o `:midi.script.accent`. El botón **Lua…** abre el formulario para un archivo propio.

Los [paquetes v1](LUA_PACKAGES.md) ya se conectan al catálogo, formulario y ejecutor
(PR-01, commit 3). Los scripts de archivo anteriores siguen disponibles.

## Herramientas registradas

Guarda un archivo `.lua` con el [formato de paquete](LUA_PACKAGES.md#formato-y-ejemplo)
en el directorio de herramientas del usuario. En Linux es
`~/.config/hay-daw/tools/midi/`, o `$XDG_CONFIG_HOME/hay-daw/tools/midi/` si defines
esa variable; crea la carpeta si aún no existe. El archivo devuelve metadatos y
`run(context, params)`, y su ID usa un prefijo propio, por ejemplo `user.midi.accent`.
Los presets de Hay Acent de la sección siguiente usan otra API: no basta con copiarlos
a esta carpeta. La aplicación todavía no distribuye Acentuar como paquete; esa
migración corresponde al commit 4.

1. Ejecuta `:midi.tools.reload` o **Lua · recargar herramientas** desde `[+]`.
2. Abre un clip MIDI y selecciona notas; sin selección se incluyen todas.
3. Elige la herramienta en `[+]` o escribe `:midi.tool user.midi.accent` para abrir
   sus parámetros. Las herramientas sin parámetros se ejecutan directamente.
4. También puedes escribir todos sus valores en orden, por ejemplo
   `:midi.tool user.midi.accent 12`. Texto con espacios usa las citas de los comandos
   habituales; booleanos usan `true`/`false`. No se ejecuta un shell.

La recarga lee ambos directorios en un worker y publica el registro completo. Una
sola recarga puede estar activa. Si falla, conserva el registro anterior; si termina
correctamente, hay que reabrir los formularios que ya estaban abiertos. Los trabajos
enviados conservan su fuente hasta terminar, incluso si se retira el paquete. Su
resultado solo se aplica si el clip sigue siendo el mismo, conserva su patrón y
no está bloqueado ni en arrastre. No hay búsqueda automática de cambios en disco.
Los mensajes distinguen errores de carga, descriptor, argumentos, ejecución,
resultado y estado obsoleto; un ID ausente nunca usa una herramienta de respaldo.

## Hay Acent

Colección de cuatro scripts utilizables con el ejecutor actual. Abre un clip MIDI,
selecciona notas y, desde **Lua…** o **Ejecutar script MIDI…** en `[+]`, introduce
la ruta del preset en `assets/scripts/midi/hay_acent/`. No requiere recompilar:
la aplicación lee el archivo en cada ejecución. Si no seleccionas notas, se aplica
a todas las del patrón, como los demás scripts de archivo.

| Archivo | Resultado |
|---|---|
| `pulso.lua` | Intensidad 112–72–72–72, repetida en orden temporal |
| `alternado.lua` | Intensidad 112–72, repetida en orden temporal |
| `creciente.lua` | Intensidad progresiva de 48 a 112; un solo ataque recibe 112 |
| `clic_4_4.lua` | Cuatro notas cortas a negras, con intensidad 112–72–72–72 |

Los tres primeros solo cambian velocity. Las notas con el mismo inicio forman un
ataque y reciben el mismo acento; el patrón comienza en el primer ataque seleccionado.
Fijan valores absolutos, de modo que repetir un preset no acumula intensidad.

Para probar el clic, usa un clip de un compás **4/4**, con un instrumento cargado,
dibuja cuatro notas del mismo tono, selecciónalas y ejecuta `clic_4_4.lua`.
Reproduce el clip en loop. El script conserva los tonos e instrumentos: el sonido
lo produce la pista. Alinea las notas al comienzo del compás 4/4 que contiene la
primera seleccionada, con duración de 1/8 de negra y un pulso entre inicios.
Exige exactamente cuatro notas y espacio para ese compás completo; si falla,
conserva el patrón. Es una guía MIDI de 4/4 fijo, no un metrónomo global ni una
adaptación automática al compás del proyecto.

Desde la raíz del proyecto puedes usar, por ejemplo:

```text
:midi.script 'assets/scripts/midi/hay_acent/clic_4_4.lua'
```

Si la app se abrió desde otra carpeta, utiliza la ruta absoluta del archivo.
Estos presets conservan la API de scripts de archivo y no se registran como paquetes.
Verificación headless: `cargo test --no-default-features --test hay_acent --locked`.

## Acciones del piano roll

Cada herramienta tiene una acción propia en el catálogo y una implementación Lua en
`assets/scripts/midi/`. Los botones, atajos, gestos e iconos llaman al mismo controlador.

| Acción / comando | Parámetros |
|---|---|
| `:midi.quantize 16 100` | Denominador de rejilla e intensidad 0–100 % |
| `:midi.humanize 50` | Intensidad 0–100 % |
| `:midi.subdivide 16` | Denominador; requiere selección explícita |
| `:midi.release_velocity 80` | Velocidad de liberación 0–127 |
| `:midi.probability 50` | Probabilidad por nota 0–100 % |
| `:midi.pan -32` | Desplazamiento de panorama −64–63 |

Desde el catálogo o `:` sin argumentos se abre el formulario de parámetros.
Q abre su formulario; H y Subdividir aplican directamente los valores actuales del
piano roll. Los argumentos de una transformación no modifican la rejilla del editor.

### Crear un roll

1. Selecciona una o varias notas.
2. Elige la rejilla: **Alt+1** afina (1/8 → 1/16); **Alt+2** engrosa (1/16 → 1/8).
   También hay botones y comandos `:midi.grid 16`, `:midi.grid.finer` y
   `:midi.grid.coarser`. Valores: 1/1 a 1/128, potencias de dos.
3. Sobre el canvas, usa **Alt+rueda** o **Shift+pellizco**, o pulsa **Subdividir**.

Cambiar la rejilla solo cambia líneas, snap y paso del cursor; no edita las notas.
Subdividir corta en cada límite interior de la rejilla del patrón, conservando inicio,
final, tono, velocity, instrumento y silencio. En notas desalineadas, los extremos pueden
ser más cortos que la rejilla. Los fragmentos quedan seleccionados; el primero conserva
el ID y los demás reciben IDs nuevos. Las notas sin límites interiores quedan intactas.
Se exige selección explícita para esta acción, a diferencia de Q/H/acento.

Ambas direcciones del gesto ejecutan la misma subdivisión; no unen ni deshacen cortes.
Repetirla con la misma rejilla es idempotente. El pellizco acumula movimiento y dispara
una vez por gesto; la rueda acumula deltas pequeños. Ctrl+rueda o pellizco sin Shift
conservan el zoom horizontal. Funciona en el editor acoplado y separado.

## Contrato Lua

El script recibe `notes`, un arreglo de tablas con `id`, `start`, `duration`, `pitch`,
`velocity`, `release_velocity`, `probability`, `pan` y `selected`. `start`, `duration` y `length` son ticks; `ppq` indica ticks
por negra. Si no hay selección, todas las notas llevan `selected = true`.
También recibe `grid_ticks` (rejilla capturada al invocar) y `amount` (intensidad 0–100).

```lua
for _, note in ipairs(notes) do
    if note.selected then
        note.velocity = math.min(127, note.velocity + 12)
    end
end
```

El script puede cambiar posición, duración, pitch, velocity y expresión de las notas existentes.
`release_velocity` admite 0–127 (por defecto 64), `probability` 0–100 (100),
`pan` −64–63 (0). Los cortes heredan estas propiedades; Rust rechaza valores inválidos.
Los comandos `:midi.release_velocity 80`, `:midi.probability 50` y `:midi.pan -32`
ejecutan las herramientas integradas correspondientes. Las variables `value`, `amount`
y `pan` transportan sus argumentos. Release velocity se envía a sfizz; el pan por voz
se aplica en FluidSynth. Los otros backends conservan los datos sin aplicar esas
expresiones no soportadas (véase [inventario](PIANO_ROLL_ACTIONS.md)).
Puede solicitar subdivisiones con `note.cuts = {tick_absoluto, ...}`: deben ser enteros
crecientes, estrictamente interiores a la nota. Rust crea los segmentos y sus IDs; no se
permite insertar/borrar filas de `notes` ni cambiar IDs. Los scripts de archivo se ejecutan sobre una
copia en un worker; Rust valida el patrón completo antes de sustituir el clip. Si el
clip cambió, se bloqueó o hay un arrastre activo al terminar, se descarta el resultado. Errores
de Lua y notas inválidas dejan el clip intacto. No existe todavía undo/redo de proyecto.

La VM usa Lua 5.4 mediante `mlua` con bibliotecas matemáticas, de cadenas y tablas;
no recibe acceso a GPUI, audio, archivos ni red. El host lee el archivo solicitado
antes de ejecutarlo. Límite por script: 256 KiB de fuente, 8 MiB de memoria de Lua y
250 ms de ejecución medidos por hook de instrucciones. No se ejecuta Lua en el callback
de audio ni durante el dibujo de GPUI.
Entrada y resultado limitados a 16 384 notas; la validación de cortes se hace antes de
publicar el patrón. El plazo por hook no es un temporizador estricto para funciones C.
`mlua`, `mlua-sys` y el Lua vendorizado de `lua-src` declaran licencia MIT.

## Latencia de acciones integradas

Las herramientas integradas siguen ejecutando sus fuentes Lua. `edit_builtin`
reduce la entrada a las notas afectadas y conserva en Rust las notas omitidas y
los controles MIDI. Mantiene el siguiente ID del patrón completo, las propiedades,
la selección y el silencio de los fragmentos. No convierte CC ni notas ajenas a Lua.
Los scripts de archivo mantienen `edit_midi` y la API completa de `notes`/`controls`.

Subdividir usa una vía inmediata dentro del evento UI cuando hay hasta 16 notas
seleccionadas, 4096 notas en el clip, 8192 controles y como máximo 128 segmentos.
El coste se comprueba antes de ejecutar el script integrado; no pasa por la cola de
workers ni hace dos snapshots del clip. Operaciones mayores y archivos propios
siguen en background con validación de identidad/snapshot antes de aplicar. Ambas
vías comparten el mismo commit; Lua nunca se ejecuta en audio ni en el render.

Diagnóstico reproducible, sin abrir audio ni una ventana:
`distrobox enter gpui-dev -- cargo run --no-default-features --example midi_edit_latency --locked`.
En este equipo, compilación debug, dividir una nota de un clip de 38 notas pasó de
0,65–0,75 ms a aproximadamente 0,18 ms; con 8192 CC pasó de 76–82 ms a 0,19 ms;
en un clip de 4096 notas pasó de 53–56 ms a 5,7 ms. Son tiempos del procesamiento
headless, no de presentación de un frame. El diagnóstico compara además el patrón
resultante con la ruta completa. No se ejecutó la suite de tests.

## Empuje durante el arrastre

`assets/scripts/midi/push.lua` define la política de contacto entre notas del mismo
tono. Es un contrato geométrico de gesto, distinto de `midi.script`: recibe filas
`{índice, inicio, final, tono}` de las notas seleccionadas y devuelve una función reutilizable
`(delta_ticks, delta_pitch, resize, vecinas) → delta_permitido, recortes`.
El host carga `vecinas` solo al entrar en otro tono; durante movimientos en el mismo
tono pasa `nil` y Lua reutiliza los contactos anteriores.
Cada recorte es `{índice, inicio, final}`. No modifica las tablas originales.

`scripting::push::NotePush` compila la función una vez por gesto, conserva índices
por tono y reutiliza resultados cuando no cambia la geometría. No convierte CC,
velocity, expresión ni mute a Lua. Las vecinas se comprimen desde el contacto,
conservan un tick y limitan el avance; los solapamientos previos se respetan.
Retroceder restaura la longitud durante el mismo gesto. El host valida rangos e IDs,
conserva las otras propiedades y confirma un solo patrón al soltar; Esc no edita nada.

Límites: 16 384 notas, 8 MiB de Lua y presupuesto de 12 ms por evaluación mediante
hook. Un error cancela la edición y se muestra en el piano roll. La VM se prepara
al comenzar a mover, no al seleccionar. Lua se ejecuta en el controlador del gesto,
nunca en paint ni en el hilo de audio; el canvas interpola los offsets ya calculados.

## Controles MIDI

`controls` es un arreglo denso de `{id, start, lane, value}` independiente de `notes`.
`start` se mide en ticks; `lane` acepta `cc0`–`cc127`, `pitch` y `pressure`.
CC/presión usan 0–127 y pitch bend −8192–8191. Cambiar un punto conserva su `id`;
al añadirlo omite `id` para que Rust asigne uno. `table.remove` elimina un punto.
No reemplaces la variable global: modifica el arreglo recibido. Un carril no puede
tener dos puntos en el mismo tick. El resultado completo se valida antes de aplicarlo,
con máximo de 65 536 controles; notas y controles se publican juntos o se rechazan juntos.
Las herramientas de notas conservan la automatización y un cambio concurrente en ella
invalida el resultado pendiente del worker.

```lua
-- Reemplazar modulación por una rampa de cuatro puntos discretos.
for i = #controls, 1, -1 do
    if controls[i].lane == "cc1" then table.remove(controls, i) end
end
for i = 0, 3 do
    table.insert(controls, {
        start = math.floor(i * length / 4), lane = "cc1", value = i * 40
    })
end
```

El editor y exportación emiten esos mensajes en el canal del clip. La respuesta audible
corresponde al instrumento; RustySynth no interpreta presión de canal. No es una curva
interpolada ni automatización de parámetros VST: son mensajes MIDI por puntos.

## Displays

Sí pueden componerse con Lua, pero falta el adaptador visual. La frontera prevista es:
una fuente de datos tipada (medidor, espectro, curva LFO o parámetros de un FX) alimenta
un modelo compacto; Lua transforma ese modelo en una descripción declarativa limitada
(por ejemplo, líneas, barras, etiquetas y colores del tema); un componente GPUI compartido
la dibuja en piano, cabecera de pista o mixer. Las señales continuas y FFT permanecen en
Rust; Lua solo cambia la presentación cuando llegan nuevos datos a una frecuencia acotada.
Los scripts no crearán widgets GPUI ni ejecutarán código por muestra de audio.
