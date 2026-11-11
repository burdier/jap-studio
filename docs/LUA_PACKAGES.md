# Paquetes de herramientas Lua · contrato v1

Estado: contrato acordado para PR-01, primer commit. **Todavía no implementado.**
La ejecución disponible hoy está descrita en [LUA.md](LUA.md). Este documento fija
el formato que deben consumir el registro, el formulario y el ejecutor siguientes.
No habilita componentes DSP, pistas, plugins, ejecución live ni undo/redo global.

## Formato y ejemplo

Un paquete es un archivo UTF-8 `.lua` que devuelve una tabla con metadatos y una
función `run(context, params)`. Se leerá con `mlua`, ya presente en el proyecto;
v1 no requiere un manifiesto TOML, Serde ni un gestor de dependencias Lua.
El archivo se evalúa dentro del sandbox incluso cuando solo se leen metadatos.

Ejemplo de contrato para Acentuar, aún no instalable en la aplicación actual:

```lua
return {
    api_version = 1,
    id = "hay.midi.accent",
    name = "Lua · acentuar notas",
    description = "Aumenta la velocity de las notas seleccionadas.",
    parameters = {
        {
            key = "amount", label = "Acento", kind = "number",
            default = 12, min = 0, max = 126, step = 1,
            whole = true, unit = "velocity",
        },
    },
    run = function(context, params)
        for _, note in ipairs(context.notes) do
            if note.selected then
                note.velocity = math.min(127, note.velocity + params.amount)
            end
        end
    end,
}
```

| Campo | Regla v1 |
|---|---|
| `api_version` | Entero obligatorio, exactamente `1`; cualquier otro valor se rechaza |
| `id` | Obligatorio, 3–64 bytes ASCII; segmentos separados por puntos, cada segmento cumple `[a-z][a-z0-9_]*`; al menos dos segmentos |
| `name` | Texto obligatorio, 1–128 bytes UTF-8 |
| `description` | Texto obligatorio, 1–512 bytes UTF-8 |
| `parameters` | Arreglo denso obligatorio, ordenado, de 0–32 descriptores |
| `run` | Función Lua obligatoria; no se ejecuta durante el registro |

Los campos desconocidos, tipos incorrectos y metatablas en el descriptor o sus
parámetros se rechazan. La lectura usa campos reales, sin resolver `__index`.
Los textos descriptivos no admiten NUL ni saltos de línea; espacios solos no
constituyen un nombre o descripción. Los límites de texto se miden en bytes UTF-8.

El ID identifica la herramienta, con independencia de su nombre visible o archivo.
`hay.*` queda reservado a paquetes distribuidos con la aplicación; un paquete del
usuario utiliza otro prefijo, por ejemplo `user.midi.accent`. La versión de API
describe el protocolo; cada recarga válida obtiene además una generación interna
del registro. Esa generación no la elige Lua ni sustituye al ID de la herramienta.

## Parámetros y comandos

Cada parámetro exige `key`, `label`, `kind` y `default`. `key` cumple
`[a-z][a-z0-9_]*`, ocupa 1–64 bytes y es único en el paquete. `label` ocupa 1–128
bytes; `hint` es opcional y ocupa hasta 256 bytes. Ambos usan las reglas de texto
del descriptor; omitir `hint` equivale a una cadena vacía.

| `kind` | Valor y campos adicionales |
|---|---|
| `number` | `default`, `min`, `max`, `step` numéricos y finitos, obligatorios; `min < max`, `step > 0`. `whole` y `min_exclusive` son booleanos opcionales, por defecto `false`. `unit` es texto opcional de hasta 32 bytes, por defecto vacío |
| `text` | `default` es texto de 1–1024 bytes, sin NUL ni saltos de línea y no compuesto solo por espacios; los valores introducidos tienen los mismos límites |
| `bool` | `default` es un booleano Lua, sin conversión desde números o cadenas |

Los valores numéricos deben satisfacer sus límites; si `whole` es verdadero,
también `min`, `max`, `step` y `default` deben ser enteros. El mínimo exclusivo
exige `valor > min`; el máximo es inclusivo. `step` regula el control visual y no
cuantiza valores escritos. Un valor inicial inválido invalida todo el paquete.
Campos de otro tipo de parámetro se rechazan; v1 no incorpora listas ni widgets
personalizados. La validación de números, texto y booleanos reutiliza el contrato
de acciones, con los límites adicionales del paquete comprobados por el host.

El comando previsto es `midi.tool <id> [valores...]`, con argumentos posicionales
en el orden declarado. Con parámetros y sin valores abre el formulario con los
valores iniciales. Si se proporcionan valores, deben estar todos; una cantidad
incorrecta produce error. Una herramienta sin parámetros se ejecuta directamente.
El catálogo usa la misma invocación que el comando. El tokenizer y las reglas de
citas actuales se conservan; los comandos no se pasan a un shell.

El host entrega `params` como una tabla por clave, con valores tipados y validados.
No interpola valores en fuente Lua. El paquete no puede registrar nombres de
comandos arbitrarios. Al migrar Acentuar en el commit 4, `midi.script.accent`
ejecutará el paquete registrado `hay.midi.accent` con `amount = 12`. Si ese paquete
no está registrado, el alias conserva su fuente integrada actual como respaldo
de compatibilidad, por la misma ruta de edición MIDI. Una invocación explícita
`midi.tool` con un ID ausente siempre da error; no usa ese respaldo.

## Ubicación y carga explícita

Los paquetes distribuidos se ubicarán en `assets/scripts/tools/midi/` bajo la raíz
del proyecto que el host conoce por `CARGO_MANIFEST_DIR`, siguiendo la convención
actual de recursos. Es la ubicación de desarrollo; distribuir una aplicación
independiente del checkout queda fuera de PR-01. El directorio de usuario será
`dirs::config_dir()/hay-daw/tools/midi/`; en Linux respeta `XDG_CONFIG_HOME`.
El núcleo headless recibe las rutas del host y no importa `dirs` ni GPUI.

La acción prevista `midi.tools.reload` lee y registra los paquetes de ambos
directorios en un worker. El usuario copia un `.lua` y solicita esa acción para
que aparezca en `[+]`. No se escanea al dibujar, abrir un formulario o ejecutar
una herramienta; tampoco hay vigilancia automática de archivos en v1.

La carga recorre solo archivos regulares `.lua` del nivel inmediato, sin seguir
enlaces simbólicos ni entrar en subdirectorios. Un directorio inexistente se
trata como vacío; otros errores de lectura rechazan la recarga. Si no existe
directorio de configuración para el usuario, se informa y solo se usa la raíz
distribuida. Se ordenan las rutas para obtener diagnósticos reproducibles.
Se inspeccionan como máximo 1024 entradas por directorio, se admiten 128 paquetes
en total y 8 MiB acumulados de fuente; superar cualquier límite rechaza la carga.

La recarga construye un registro candidato completo. Un ID duplicado entre
archivos, un prefijo reservado en la carpeta de usuario o un paquete inválido
rechaza el candidato entero: se conserva el registro anterior y se muestra el
error. En el primer intento fallido las acciones nativas siguen disponibles.
No hay precedencia silenciosa por orden de lectura ni sobrescritura de nativas.

## Registro, recarga y vida útil

1. El worker lee fuentes con los límites indicados y evalúa cada descriptor en
   una VM aislada, sin contexto MIDI. `run` solo se comprueba como función.
2. Copia metadatos validados a valores propios de Rust y conserva los bytes de
   fuente asociados. El catálogo indexa esos datos; no conserva una VM ni funciones
   Lua. Una modificación del disco solo tiene efecto tras una recarga correcta.
3. Publica el candidato completo como una nueva generación. Un archivo eliminado
   deja de ofrecerse después de una recarga correcta. Una sola recarga puede estar
   activa; otra solicitud recibe «recarga en curso», sin crear trabajos solapados.
4. Un formulario abierto conserva ID y generación. Si la generación cambia antes
   de confirmar, se rechaza su envío y se pide abrirlo de nuevo. No se reinterpretan
   sus valores usando parámetros nuevos.
5. Una edición ya enviada conserva su fuente y metadatos de origen hasta terminar.
   Puede completar con esa versión tras una recarga, incluida la retirada del
   paquete; siempre debe superar la validación del clip descrita abajo.

Las referencias `&'static` de `ActionSpec`, `CatalogEntry` y `ActionForm` actuales
no sirven para metadatos recargables. Los commits siguientes deben darles una
representación con propietario y conservar el formulario/validador compartidos.
No convertir cadenas cargadas en referencias permanentes mediante fugas de memoria
ni crear un segundo validador independiente para Lua. Las acciones nativas deben
seguir funcionando durante la adaptación.

## Ejecución y conservación de la edición

Cada invocación usa una VM nueva en un worker. Se evalúa la fuente capturada y se
comprueba que sus metadatos coinciden con los registrados antes de llamar a `run`.
El descriptor debe ser estable entre evaluaciones de una misma fuente; una
discrepancia se rechaza. El estado de una VM no persiste entre invocaciones.

`context` contiene `notes`, `controls`, `length`, `ppq` y `grid_ticks`, con las
unidades y reglas de [la API MIDI actual](LUA.md#contrato-lua). `notes` y `controls`
son copias; sin selección todas las notas tienen `selected = true`. El host
conserva las referencias a ambos arreglos: sustituirlos se rechaza. `length`,
`ppq` y `grid_ticks` son información de entrada; escribirlos no modifica el clip.
`run` modifica las filas y devuelve `nil`; otro resultado se rechaza.

Se conserva el contrato de identidad: no añadir/quitar/reordenar notas ni cambiar
sus IDs; para subdividir se usa `note.cuts`. Los controles permiten las operaciones
ya documentadas en [Controles MIDI](LUA.md#controles-midi). El host valida notas y
controles completos, crea IDs y produce el mismo `MidiEdit` que la ruta existente.
Un error en cualquiera de los dos arreglos rechaza la edición completa.

La UI aplica ese resultado por el controlador común solo si coinciden el `ClipId`
y el patrón capturados, el clip no está bloqueado y no hay un arrastre activo.
Conserva las reglas actuales de selección, silencios y descendientes de notas.
No se entrega a Lua el modelo mutable, una operación de confirmación ni recursos
de audio. La recarga no altera el contenido del clip por sí misma.

## Límites y errores

Se mantiene el sandbox actual: bibliotecas base restringida, math, string y table;
sin acceso a archivos, red, GPUI, audio, `require`, `dofile`, `loadfile`, `load`,
`pcall` ni `xpcall`. Se admite únicamente fuente de texto, no bytecode precargado.
Cada archivo tiene como máximo 256 KiB y cada VM 8 MiB de memoria Lua.

El presupuesto por paquete es 250 ms mediante hook cada 1000 instrucciones: al
registrar cubre la evaluación; al invocar cubre evaluación y `run` juntos, sin
reiniciar el plazo entre ambos. No garantiza un corte estricto de funciones C ni
incluye toda la E/S y validación del host. No es un contrato de tiempo real.
Entrada y resultado mantienen los límites de 16 384 notas y 65 536 controles.

Cada error identifica fase (`load`, `descriptor`, `arguments`, `execute`, `result`
o `stale`), ruta, ID cuando se conoce, campo cuando corresponde y explicación.
Ejemplo: `descriptor · /ruta/acento.lua · user.midi.accent · parameters[1].default:
fuera de 0–126`. El host comunica el error fuera del audio. El formato interno del
error queda a la implementación; estos datos observables son obligatorios.

## Entregas que comprobarán este contrato

| Commit de PR-01 | Capacidad y comprobación pendiente |
|---|---|
| 2 · registro | Lectura headless, tipos/campos, límites, API desconocida, IDs duplicados, publicación completa y conservación ante fallo |
| 3 · ejecución | Formulario/comando con la misma validación; errores de Lua y resultado intacto; edición concurrente rechazada; generaciones y trabajo pendiente |
| 4 · Acentuar | Paquete real con alias anterior, recarga válida/inválida; añadir una segunda herramienta sin recompilar y conservar las acciones nativas |

Este primer commit entrega el contrato y su ejemplo documental. Las comprobaciones
de comportamiento se añaden junto al registro y al ejecutor que lo implementen.
