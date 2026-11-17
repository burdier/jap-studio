# Tejer: reparto de notas entre pistas

## Uso normal

Selecciona dos o más pistas con instrumentos diferentes usando Ctrl+clic o
Shift+clic y pulsa **Tejer pistas seleccionadas** en el menú de pista o en `[+]`.
Hay crea el Hilo, activa su entrada **MIDI** y distribuye cada NoteOn a la
siguiente rama. Para reproducir un clip,
insértalo en el carril del grupo y pulsa Play. No hace falta una variable de
entorno ni un archivo Lua para este flujo.

Si ya tienes un grupo, elige **Tejer · automático** en su menú; también activa
su entrada MIDI. Sin una regla
personalizada, el orden es A-B-C-A-B-C para tres instrumentos.

## Regla Lua opcional

Para personalizar el orden de ramas, el archivo Lua devuelve una tabla con
`steps`: una lista de posiciones de las pistas hijas que tienen instrumento,
numeradas desde 1 en su orden visible. La lista se repite por cada NoteOn aceptado.

```lua
return { steps = { 1, 3, 2, 1, 3, 2 } }
```

El ejemplo reparte seis notas A-C-B-A-C-B. Hay un ejemplo A-B-C en
`assets/scripts/hilos/tejer.lua`. Para usarlo desde la raíz del proyecto:

```bash
HAY_WEAVE_RULE_SCRIPT=assets/scripts/hilos/tejer.lua bash scripts/run.sh
```

Este comando es para quien edita scripts. La interfaz no solicita esa variable.
El archivo de ejemplo requiere tres ramas con instrumento.

La VM Lua se ejecuta una sola vez al abrir la aplicación. El resultado se valida
y se convierte en una secuencia de 1–64 pasos dirigida a 2–16 ramas. La selección
de rama por nota consulta esa secuencia; no ejecuta Lua, no asigna memoria y no
depende del tamaño del bloque de audio. Los clips se preparan fuera del callback;
el MIDI en vivo usa el servidor MIDI de la interfaz. Ambos usan el mismo orden
de pasos y conservan el destino del NoteOff. El plan de clips desempata por frame,
tipo de evento, clip e ID de nota. Stop, cambio de foco o modo y seek reinician
el turno y liberan notas activas.

Si la regla es inválida o nombra una rama que no existe, Play se rechaza y el
enrutamiento MIDI en vivo queda sin destinos, con el error en la barra de estado.
El archivo tiene un límite de 16 KiB y usa el sandbox Lua existente. Hay que
reiniciar la aplicación para cargar cambios del archivo.

**Alcance actual:** se carga una regla global para los Hilos. Todavía no hay
editor ni persistencia de reglas por Hilo; tampoco se ejecuta Lua en el callback.
El MIDI físico sin ID empareja notas por canal y tono; si se repiten dentro de
esa pareja, usa orden inverso. NoteOff conserva la rama elegida aunque cambie
el turno o se retire otra rama. El registro acotado, la expresión por voz,
el sustain y las colas al retirar ramas requieren el cierre restante de PR-09.
