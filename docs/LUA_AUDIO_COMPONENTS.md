# Componentes de audio Lua · contrato inicial de PR-03

Estado: **implementado en PR-03**. Este documento define la
primera capacidad comprobable de PR-03: describir en Lua un efecto estéreo hecho
con el primitivo `Volume`, insertarlo en una pista y controlar dos etapas con una
macro. El ejemplo distribuido está en `assets/scripts/components/audio/double_volume.lua`.
[Plan del PR](PLAN_LUA_MULTITRACK_HILOS.md#pr-03--crear-un-componente-audible-desde-lua).

## Fronteras

Un archivo Lua devuelve una **descripción**. Se evalúa en el sandbox de preparación,
fuera del callback de audio. El host valida y copia sus datos a un plan nativo;
después de preparar una instancia, el callback procesa `StereoFrame` mediante los
primitivos existentes. No conserva una VM Lua en la ruta de audio, ni evalúa Lua
por muestra, bloque o cambio de parámetro.

El componente es un **efecto** en la cadena FX de una pista. Su identidad de tipo
es el `id` del paquete; cada inserción obtiene su propio `FxId`, procesador y valores.
El `TrackId` determina la pista. Los IDs de nodo y de control del descriptor son
estables dentro de ese tipo y se combinan con `FxId` para dirigirse a una instancia.
El catálogo `[+]` solo indexa metadatos. La descripción editable y la memoria del
procesador son objetos distintos. Las vistas GPUI y los comandos llaman la misma
operación de control validada.

El paquete de componentes es una familia distinta del paquete MIDI v1 de
[herramientas Lua](LUA_PACKAGES.md): no tiene `run(context, params)` ni acceso a
notas. Usa el mismo sandbox de lectura y las mismas reglas de ID y texto, pero su
esquema es el de esta página. La versión 1 admite audio estéreo y `elevate.amplitude.volume`. La versión 2
añade `elevate.filter.svf`, controles discretos y frecuencia logarítmica; conserva
los descriptores v1. Los puertos siguen siendo estéreo.

## Descriptor mínimo

Archivo UTF-8 `.lua` que devuelve una tabla sin metatablas ni campos desconocidos:

```lua
return {
    api_version = 1,
    kind = "audio_component",
    id = "user.fx.double_volume",
    name = "Doble Volume",
    description = "Dos etapas de ganancia controladas juntas.",
    ports = {
        input = { kind = "audio", channels = 2 },
        output = { kind = "audio", channels = 2 },
    },
    graph = {
        nodes = {
            { id = "first", primitive = "elevate.amplitude.volume",
              parameters = { gain = 1.0 } },
            { id = "second", primitive = "elevate.amplitude.volume",
              parameters = { gain = 1.0 } },
        },
        edges = {
            { from = "input", to = "first" },
            { from = "first", to = "second" },
            { from = "second", to = "output" },
        },
    },
    controls = {
        { id = 0, key = "gain", label = "Ganancia", kind = "number",
          min = 0.0, max = 2.0, default = 1.0, step = 0.01, unit = "x",
          targets = {
              { node = "first", parameter = "gain" },
              { node = "second", parameter = "gain" },
          } },
    },
}
```

| Campo | Regla inicial |
|---|---|
| `api_version` | Entero `1`; versión del esquema de componentes, independiente de la API MIDI |
| `kind` | Cadena exacta `audio_component` |
| `id` | 3–64 bytes ASCII, segmentos `[a-z][a-z0-9_]*` separados por puntos, al menos dos; `hay.*` reservado a paquetes distribuidos |
| `name`, `description` | Texto UTF-8 de 1–128 y 1–512 bytes respectivamente, sin NUL ni salto de línea |
| `ports` | Exactamente `input` y `output`, ambos `{ kind = "audio", channels = 2 }` |
| `graph.nodes` | Arreglo denso de 1–32 nodos; `id` único de 1–64 bytes con sintaxis `[a-z][a-z0-9_]*` |
| `graph.edges` | Arreglo denso de 2–64 aristas `{ from, to }`, sin duplicados |
| `controls` | Arreglo denso de 1–16 controles numéricos; IDs enteros únicos `0..65535`, claves únicas con la sintaxis de nodo |

Los IDs `input` y `output` están reservados y no pueden usarse como ID de nodo.
El orden de los arreglos es estable para la presentación y los diagnósticos; los
enlaces, no el orden de declaración, determinan el flujo de señal. La ruta y los
índices de `nodes`, `edges` o `controls` deben figurar en errores de validación.

## Grafo, puertos y señal

`input` es la fuente estéreo del componente y `output` su destino estéreo. Cada
nodo `Volume` tiene implícitamente una entrada y una salida estéreo. Una arista
conecta la salida de `from` a la entrada de `to`. Solo se permiten
`input → nodo`, `nodo → nodo` y `nodo → output`; `input → output` permite una rama
directa. Una salida puede alimentar varias entradas: cada rama recibe una copia
de la misma señal. Las señales que llegan a una entrada se suman por canal,
sin normalización ni limitador; una salida paralela puede superar 1.0. El grafo
debe ser acíclico, tener un camino de `input` a `output` y poner todos sus nodos
en algún camino completo. Se rechazan nodos huérfanos, referencias ausentes,
aristas duplicadas, puertos incompatibles y ciclos antes de preparar audio.

La primera primitiva permitida es `elevate.amplitude.volume`. Su único parámetro
es `gain` (`ParameterId(0)`): valor `f32` finito en `0..=2`, por defecto `1` y
suavizado nativo de 5 ms. `nodes[].parameters` admite exactamente `gain` como
valor inicial; no se inventan parámetros del primitivo desde Lua. Un control
apunta a una o más parejas `{ node, parameter }` existentes. Cada destino solo
puede pertenecer a un control. El rango completo de un control debe caber en el
rango de todos sus destinos. Su `default` debe coincidir con el valor inicial de
cada destino para evitar dos fuentes de verdad al crear la instancia.

Los controles iniciales tienen `kind = "number"`, `id`, `key`, `label`, `min`,
`max`, `default`, `step`, `unit` y 1–32 `targets`; todos los números son finitos,
`min < max`, `step > 0` y `default` está dentro del rango. `label` y `unit`
siguen las reglas de texto de herramientas Lua, con un máximo de 128 y 32 bytes.
`step` orienta el widget y no redondea los valores escritos. Al cambiar la macro
del ejemplo, el mismo valor validado se envía a `gain` de `first` y `second`;
en estado estable la amplitud se multiplica por `gain × gain`. El valor base
de la macro se guarda por instancia; no se confunde con modulación futura.

## Instancias, bypass y recarga

El control se direcciona por `(TrackId, FxId, control_id)`. El estado de FX
conserva el ID de tipo y los valores de control por ID numérico. Dos instancias
del mismo paquete no comparten procesador, valores, bypass ni memoria de rampa.
El bypass del componente deja pasar una sola copia de la entrada sin procesar;
al desactivarlo se recuperan los valores base. `reset` limpia el estado de
procesamiento de esa instancia. El procesador inicial declara latencia cero.

La lectura, validación, construcción del grafo, `prepare`, reservas y retirada de
procesadores ocurren fuera del callback. El callback recibe un plan ya preparado,
con trabajo y memoria acotados, sin E/S, GPUI, bloqueo ni destrucción costosa.
Cambios de parámetro comparten una ruta segura hacia esa instancia; el suavizado
lo realiza `Volume`, no un temporizador de interfaz. Una edición inválida no cambia
el valor anterior. Los cálculos pueden exceder 1.0 sin clipping automático.

Los componentes distribuidos se descubren bajo `assets/scripts/components/audio/`
y los del usuario bajo `dirs::config_dir()/hay-daw/components/audio/`. La recarga
es explícita y construye un registro candidato fuera del audio. Solo lee archivos
regulares `.lua` del nivel inmediato, sin seguir enlaces simbólicos: máximo 1024
entradas por directorio, 128 componentes, 256 KiB por archivo y 8 MiB de fuente
en total. Cada evaluación usa una VM aislada de 8 MiB y el presupuesto de 250 ms
del [sandbox de paquetes](LUA_PACKAGES.md#límites-y-errores), sin archivos, red,
`require` ni acceso a GPUI o audio. Es un presupuesto de preparación, no de RT.
Una definición, ID duplicado, ciclo o preparación inválida rechaza la publicación
completa; se conservan el registro y las instancias válidas anteriores. Cada
instancia captura una versión validada del descriptor hasta una sustitución
explícita y preparada. Borrar o cambiar el archivo no altera el procesador activo
por sí solo. Si falta el paquete de un FX restaurado, se conserva su identidad y
se muestra un error; nunca se sustituye silenciosamente por otro procesador.
Un error identifica fase (`load`, `descriptor`, `graph`, `prepare` o `reload`), ruta,
ID conocido y campo o índice que falló. El mensaje se presenta fuera del callback.

El documento Loro actual conserva `FxId`, tipo y parámetros numéricos, pero no
guarda fuente Lua ni conecta el proyecto completo a la UI. PR-03 debe mantener
esa identidad y validar la proyección al modificar FX; compartir definiciones
entre colaboradores y recuperar un proyecto completo siguen pendientes.

## Criterio de aceptación de PR-03

Insertar el ejemplo desde `[+]` debe hacer audible el efecto en una ruta de
audio real. Una macro en `0.5` debe llevar ambas etapas a `0.5` y producir una
ganancia estable de `0.25`, con la transición propia de `Volume`. Bypass produce
la entrada sin duplicarla. Cambiar una de dos instancias deja intacta la otra.
Un grafo cíclico o un valor fuera de rango se rechaza sin reemplazar la versión
que estaba sonando. La comparación de audio se hace con dos `Volume` nativos
equivalentes, no solo contando nodos ni comprobando la UI.

## Uso y estado comprobado

Reinicia Hay con `bash scripts/run.sh`. En una pista con clip de audio, busca
**Hay Double Volume** desde `[+]` y pulsa Play. Selecciona el nodo nuevo en Track
FX: el panel inferior muestra la macro **Ganancia**. Muévela cerca de `0.5x` o
escribe `:fx.parameter 0 0.5` para fijar ese valor exacto; una vez terminado el
suavizado, el nivel queda en un cuarto del original. Usa el bypass del árbol
para comparar la señal seca. El comando cambia el mismo control de la instancia.
Duplica el nodo y cambia solo uno para comprobar que sus valores no se comparten.

Para instalar otro descriptor, copia un `.lua` en
`~/.config/hay-daw/components/audio/` (o bajo `$XDG_CONFIG_HOME`), luego ejecuta
`:fx.components.reload`. La recarga válida actualiza el buscador; las instancias
abiertas conservan su definición anterior. Un error muestra el motivo y conserva
el registro anterior. La recarga no escanea durante el render ni cambia por sí
sola el procesador que está sonando.

Las pruebas headless cubren descriptor, ciclos, serie, paralelo, dos instancias,
bypass, ganancia en el rack y recarga válida/inválida. El usuario confirmó que
funciona en la aplicación y aprobó su estilo; no se registraron pasos ni mediciones
de la audición. El efecto se prepara con la frecuencia solicitada
en Audio, o 48 kHz si está en Auto; el ajuste del tiempo de suavizado podría
diferir ligeramente si Auto negocia otra frecuencia. El proyecto completo aún
no se guarda desde la UI ni comparte fuentes Lua entre colaboradores. Exportar
esta cadena con el render offline actual sigue pendiente.

## Incremento PR-03B: Hay Filter y kit v2

`assets/scripts/components/audio/filter.lua` define **Hay Filter**. `[+]` lo ofrece
al arrancar; `fx.components.reload` permite recargar variantes instaladas por el
usuario. El descriptor v2 conserva grafo, IDs y reglas de recarga de v1. Las
instancias existentes conservan su descriptor al recargar.

La primitiva `elevate.filter.svf` usa filtros SVF de FunDSP 0.23 (dependencia existente,
MIT/Apache-2.0), con estados estéreo independientes y suavizado de 5 ms para
frecuencia, Q y transición entre modos. Todos los modos permanecen preparados;
no hay reconstrucción del procesador al seleccionar otro tipo. Parámetros:

| ID nativo | Clave | Rango | Inicial |
|---|---|---|---|
| 0 | `cutoff` | 20–20000 Hz | 1000 |
| 1 | `resonance` | 0.5–10 Q | 0.707 |
| 2 | `mode` | 0 LP, 1 HP, 2 BP; enteros | 0 |

`parameters` debe declarar las tres claves. El límite efectivo de frecuencia es
0.45 × sample rate. LP/HP son de segundo orden (12 dB/oct); BP es un paso banda
con caída de 6 dB/oct por lado. Drive, Mix, otras pendientes, envolvente y LFO
sincronizado quedan para incrementos posteriores.

En v2, `controls` puede usar `scale = "log"` para rangos numéricos positivos, o
`kind = "choice"` con `choices = { "LP", "HP", "BP" }`. Choice exige min=0,
max=cantidad−1, step=1 y valores enteros; también se valida desde el comando.
Los targets ahora identifican nodo **y parámetro**; el mismo nodo puede exponer
frecuencia, Q y modo en controles distintos. Los rangos de controles deben caber
en cada destino, y los valores iniciales deben coincidir. El modo requiere tres
opciones. Los sliders v1 conservan su conversión lineal.

El campo opcional `response_node = "filter"` selecciona un nodo SVF para mostrar
su respuesta estacionaria, calculada fuera de audio con FunDSP. La curva pertenece
a ese nodo, no al grafo completo; no representa un espectro medido. La vista usa
los colores, bordes y controles actuales. Esto es una primera capacidad de
presentación declarativa, no un editor de layouts arbitrarios.

Prueba manual: reiniciar con `bash scripts/run.sh`, fijar Audio a **48000 Hz**,
insertar Hay Filter en una pista con audio y pulsar Play. Seleccionar el efecto;
mover Frecuencia/Resonancia y alternar LP/HP/BP. Para valores exactos usar
`:fx.parameter 0 300`, `:fx.parameter 1 1` y `:fx.parameter 2 1` (HP).
El bypass permite comparar con el audio original. Duplicar y cambiar una sola
instancia comprueba su independencia. `:fx.parameter 2 0.5` debe rechazarse.

El adaptador actual prepara el FX con la frecuencia solicitada en Audio, o 48 kHz
en Auto. Si el dispositivo negocia otra frecuencia, el corte audible puede diferir
de la curva y del valor mostrado. La preparación con la frecuencia real del motor
compartido debe resolverse en PR-04. Exportación de esta cadena, guardado global
y colaboración completa siguen pendientes; Loro conserva tipo y parámetros
numéricos, no estado DSP ni fuente Lua.

Pruebas automáticas: equivalencia de audio Lua/nativo, respuesta frente a señal
senoidal medida a 44.1/96 kHz, aislamiento estéreo y de instancias, bypass,
particiones de bloque y rechazo de parámetros/descriptores inválidos. La prueba
interactiva de este filtro está pendiente.
