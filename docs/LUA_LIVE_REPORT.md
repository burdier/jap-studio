# PR-08: medición y decisión de alcance

## Diagnóstico reproducible

Comando: `distrobox enter gpui-dev -- cargo run --release --example lua_live_latency --locked`.
Ejecutado el 15 de noviembre de 2026 en Intel Core i5-13420H (12 CPU lógicas),
Linux 7.3.3-ogc1.1.fc45.x86_64, Rust 1.99.1 y `mlua` 0.12.2. Carga media del
sistema al consultar: 1.67/1.85/1.78; la frecuencia de CPU es dinámica. El
diagnóstico usa 48 kHz, 2 000 bloques por caso, ocho eventos por instancia y
buffers de 64/128/256/512 frames. La base hace el mismo recorrido de eventos y
registro de notas en Rust sin Lua; los valores son tiempos de pared del lote en
memoria, **no** del callback CPAL completo. La medición reserva su vector de
tiempos antes del bucle y produce la tabla después.

| Frames | Instancias | Base p50 µs | Lua p50 µs | Lua p99 µs | Lua p99.9 µs | Lua máx µs | Plazos excedidos |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 64 | 1 | 0.025 | 1.248 | 1.476 | 4.364 | 6.074 | 0 |
| 64 | 8 | 0.121 | 9.229 | 12.291 | 14.559 | 33.476 | 0 |
| 64 | 32 | 0.466 | 36.961 | 74.275 | 92.427 | 105.400 | 0 |
| 128 | 1 | 0.023 | 1.171 | 1.356 | 2.230 | 4.382 | 0 |
| 128 | 8 | 0.118 | 9.152 | 11.952 | 13.125 | 13.269 | 0 |
| 128 | 32 | 0.433 | 37.032 | 47.962 | 54.158 | 60.486 | 0 |
| 256 | 1 | 0.023 | 1.166 | 1.608 | 2.825 | 3.813 | 0 |
| 256 | 8 | 0.119 | 9.101 | 11.749 | 12.711 | 13.295 | 0 |
| 256 | 32 | 0.457 | 36.664 | 45.441 | 52.250 | 53.184 | 0 |
| 512 | 1 | 0.023 | 1.163 | 1.927 | 2.317 | 4.011 | 0 |
| 512 | 8 | 0.118 | 9.131 | 11.963 | 13.401 | 14.700 | 0 |
| 512 | 32 | 0.472 | 36.754 | 44.945 | 54.832 | 152.171 | 0 |

El plazo de un bloque de 64 frames a 48 kHz es 1 333 µs. Cero excesos en este
diagnóstico sintético no demuestra que el callback completo cumpla bajo carga ni
acota el peor caso. La muestra de p99.9 contiene solo dos observaciones por caso;
para una decisión RT harían falta sesiones más largas con audio real.

## Auditoría de la ruta ejecutada

En `mlua` 0.12.2, `Function::call` obtiene `Lua::lock()` (guardia de un mutex
reentrante), prepara la pila, llama a `lua_pcall` y traduce errores/resultados.
Dentro de Lua, scripts permitidos pueden crear tablas y provocar asignación o
GC. El límite de 2 MiB limita memoria total, pero no el trabajo ni la latencia
de una asignación o del colector. El hook de 1 000 instrucciones puede observar
un exceso después de que ocurra; el plazo de 2 ms no es un límite estricto.
Tampoco se ha medido el callback junto con instrumentos, FX y salida física.

**Decisión:** PR-08 se cierra con el alcance revisado definido en
`LUA_LIVE_PROFILE.md`. El transpositor Lua se aplica a snapshots MIDI en el
worker antes de Play; el motor recibe el patrón resultante y no ejecuta Lua en
audio. Una falla cancela la preparación completa y deja intactos los clips
editables. `LiveNoteProcessor::process_or_disable` libera los destinos de notas
ya publicadas y desactiva esa instancia, pero aún no está conectado a RT.
Lua musical live sigue pendiente y PR-09 no puede atribuirle reglas en vivo.

Verificación: `distrobox enter gpui-dev -- bash scripts/check.sh` pasó; 95
pruebas del binario aprobadas y una prueba VST3 opcional ignorada. Una ejecución
paralela previa falló en una prueba de exportación MIDI no relacionada; esa
prueba pasó aislada y toda la suite del binario pasó en serie. Por ello
`scripts/check.sh` ejecuta los tests de audio del binario en serie.

Para probar la ruta previa a Play desde la raíz del proyecto:

```bash
HAY_LUA_PREPLAY_SCRIPT=assets/scripts/components/midi/transposer.lua bash scripts/run.sh
```

Abre un clip MIDI, pulsa Play y escucha la transposición de siete semitonos.
El patrón del piano roll conserva sus tonos originales; Stop y Play vuelven a
generar el snapshot desde el archivo actual. Quita la variable para reproducir
sin transformación. La ruta es local a esta ejecución y no se guarda en Loro.
