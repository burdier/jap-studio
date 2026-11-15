# PR-08: perfil de Lua musical en vivo

Este documento fija el contrato previo a implementar y medir el procesador. El
sandbox actual de `src/scripting/sandbox.rs` transforma clips fuera del callback:
crea una VM para la operación, limita su memoria a 8 MiB y comprueba un plazo de
250 ms mediante un hook cada 1 000 instrucciones. Esos límites **no** hacen apta
esa VM para audio en tiempo real. `examples/midi_edit_latency.rs` mide edición de
clips; tampoco mide el callback.

## Ciclo de vida y API prevista

- Una instancia musical tiene una VM y un script propios. Carga, validación,
  inicialización, reservas y destrucción ocurren fuera del callback. El host
  publica solo una instancia preparada y un estado con capacidades fijas. El
  primer `LiveProcessor` admite fuentes de hasta 64 KiB, VM de 2 MiB, 256
  eventos por bloque y bloques de hasta 4 096 frames; usa 250 ms para preparar
  y un plazo de 2 ms por bloque procesado.
- El procesador recibe eventos MIDI ordenados por offset de frame y devuelve
  eventos para el mismo bloque. Conserva el offset salvo que una transformación
  declare explícitamente otro offset válido dentro del bloque. El host conserva
  los IDs de origen y la correspondencia de cada NoteOn con sus NoteOff; Lua
  expresa la transformación, por ejemplo el intervalo del transpositor.
- El script no accede a archivos, red, reloj, carga de módulos, UI ni plugins.
  Tampoco puede llamar código C arbitrario. El transporte y la mezcla existentes
  siguen siendo los únicos dueños del tiempo y la salida.
- Antes de activar una instancia en audio hay que fijar también el máximo de
  notas activas y verificar que el plazo de 2 ms sirve para cada tamaño de
  buffer. Si no caben los eventos, se informa desbordamiento y se preservan las
  liberaciones; nunca se trunca un NoteOff silenciosamente.

## Revisión obligatoria del camino RT

El siguiente commit debe inspeccionar el recorrido real de `mlua`/Lua: conversión
de argumentos y resultados, tablas y strings, asignador, GC, locks internos,
hooks, manejo de errores y `Drop`. No basta con limitar instrucciones: un hook
puede ejecutarse después de una asignación o una llamada lenta. No se crean
tablas por nota ni se ejecuta GC o destrucción costosa en el callback. Se
reutilizan buffers y almacenamiento de eventos. Si no se puede demostrar una
ruta acotada, el procesador se prepara y ejecuta antes de Play sobre clips, sin
anunciar Lua live.

El segundo commit solo aporta `src/scripting/live.rs`: una función Lua preparada
por instancia que transforma un evento tipado en otro, con buffer Rust
preasignado y validación de rango. Su método `process_block` se invoca
explícitamente fuera de RT; `mlua` todavía puede asignar y su plazo basado en
hook no garantiza un tiempo máximo en el callback. El emparejamiento de
NoteOn/NoteOff y la integración de audio pertenecen a los siguientes commits.

Si el script falla o excede el presupuesto, el host desactiva solo esa instancia,
conserva los NoteOff necesarios para sus notas ya enviadas y publica el aviso
fuera del callback. Seek, Stop, cambio de script y retiro de un Hilo liberan los
destinos originales antes de descartar su estado. El caso de notas repetidas del
mismo pitch y la expresión asociada deben estar cubiertos por pruebas.

## Medición y criterio de decisión

Medir en `--release` el mismo recorrido musical con Lua desactivado y con 1, 8
y 32 instancias; usar buffers de 64, 128, 256 y 512 frames, ráfagas de notas
solapadas y CC, a la frecuencia real de salida. Registrar equipo, CPU, sistema,
versión de Rust/Lua, frecuencia, carga y número de iteraciones. Recoger tiempos
en una estructura preasignada y analizarlos fuera del callback; allí calcular
p50, p99, p99.9, máximo y plazos de bloque incumplidos. Informar el tiempo total
del callback y el incremento frente al recorrido sin script. El presupuesto de
cada bloque es `frames / sample_rate` segundos; un promedio bajo o un p50 bajo
no prueban que se cumpla el plazo.

El cierre de PR-08 requiere que la peor carga medida no incumpla plazos y que
se pueda justificar un límite de trabajo y memoria para la ruta ejecutada en
audio. Si falla cualquiera de las dos condiciones, se entrega la transformación
previa a Play como alcance revisado y se deja explícitamente pendiente Lua live.
No se añade otro runtime ni se marca PR-09 como procesamiento live por inferencia.
