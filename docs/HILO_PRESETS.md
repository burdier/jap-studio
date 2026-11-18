# Preset de Hilo: contrato de archivo (PR-10)

Un preset describe **un grupo Hilo y sus ramas**. No es un proyecto: no guarda
clips, posición del transporte, armados MIDI, voces sonando ni ajustes de la
salida física. Su carga prepara instancias nuevas y publica el grupo completo
solo después de validar recursos, plugins, FX y conexiones.

## Archivo y versión

La extensión prevista es `.hayhilo`. El archivo contendrá una cabecera con
firma `HAYHILO`, versión mayor `1`, longitud del manifiesto y longitudes de
bloques binarios. El manifiesto usa un documento Loro, igual que la proyección
musical existente. Los estados opacos de plugins viven en bloques separados;
ningún blob se interpreta como Lua ni se copia a un callback de audio.
Una versión mayor desconocida se rechaza sin modificar la sesión. Los límites
de tamaño y el checksum se comprueban antes de reservar o cargar una instancia.

## Identidad y contenido

| Campo | Contrato |
|---|---|
| `group` | Nombre, modo (`layer`/`round_robin`) y secuencia Tejer de 1–64 pasos |
| `branches` | Lista ordenada de 2–16 ramas con nombre, tipo de pista, nivel, pan, mute, solo y FX |
| `instrument.type` | Identificador estable del tipo (`hay.piano`, `hay.sampler` o CID VST3), nunca el `InstrumentId` de la sesión |
| `instrument.state` | Parámetros serializables y referencia a un bloque de estado VST3 cuando el plugin lo admite |
| `resource` | Referencia a SoundFont, sample o bundle con ruta y huella; el archivo externo no se incorpora implícitamente |
| `lua` | ID y versión del paquete, más parámetros permitidos; no se guarda una VM ni código de origen ajeno al paquete |
| `connections` | Índices locales de rama y puertos tipados; se validan ciclos, tipos y límites antes de publicar |

Los `TrackId`, `InstrumentId` y `FxId` del archivo son **locales al preset**.
Cada carga asigna IDs nuevos y remapea todas las referencias internas. Duplicar
el Hilo usa la misma ruta de carga en memoria, de modo que las dos copias nunca
comparten estado mutable de instrumentos, FX o scripts. El orden de ramas es
parte del sonido: cambiarlo cambia el significado de la secuencia Tejer.

## Captura y restauración

La captura de VST3 se solicita al hilo propietario del componente/controlador
con el procesador parado. `getState` del componente y del controlador se guardan
por separado; en la restauración se cargan antes de activar el procesador y se
sincroniza el controlador. Una operación no consulta un `IComponent` mientras
el callback lo procesa. Un plugin que no implemente el estado requerido debe
declararlo y bloquear el preset completo, sin reemplazarlo por otro instrumento.

La carga verifica versión, tamaño, referencias, plugins, recursos y topología;
prepara cada instancia fuera del callback; asigna IDs; y valida un candidato
completo antes de sustituir el grupo. Si algo falla, informa el recurso o tipo
que falta y conserva el grupo actual. Guardar escribe un temporal en el mismo
directorio y publica el archivo completo mediante rename atómico.

## Estado actual

El formato es un contrato para el siguiente incremento. `ProjectDoc` ya guarda
modo, secuencia y estructura musical de un Hilo, pero la interfaz todavía no
guarda presets ni sesiones completas. El host VST3 todavía no expone captura y
restauración de estado; sus recursos externos y las instancias Lua necesitan la
ruta de preparación descrita arriba.
