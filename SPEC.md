# Hay / Elevate — especificación de alto nivel

**Visión.** DAW nativo en Rust + GPUI, operable con teclado y extensible por el usuario.
Elevate permite construir herramientas musicales conectando piezas reutilizables.
Desarrollaremos el motor en pasos pequeños, revisando cada función juntos y reutilizando bibliotecas DSP.
La filosofía rectora está **siempre al inicio de [README.md](README.md)**: plugins desde la base,
componentes compuestos de primitivos, mapeos reales, acciones invocables al estilo REAPER y eventos
observables. El piano es el primer instrumento concreto; el registro formal de plugins y el bus de
acciones/eventos siguen pendientes.

## Vocabulario de Elevate

- **Plugin:** extensión que registra capacidades en el host: primitivos, componentes, parámetros y acciones.
- **Primitivo:** unidad de procesamiento o generación de señal de audio/control. El contrato actual
  implementa transformaciones de audio estéreo; las fuentes de control como LFO son un objetivo futuro.
- **Parámetro:** valor que gobierna la operación, con identificador, unidad, límites y valor inicial.
- **Componente:** composición de instancias de primitivos con conexiones y parámetros expuestos.
  Puede ser una cadena o, más adelante, un grafo; componer no implica siempre sumar señales.
- **Control / vista:** representación visual o de teclado enlazada a un parámetro o acción.
  Su apariencia pertenece al sistema de diseño; el procesamiento funciona también sin UI.
- **Mapeo:** conexión efectiva de una fuente a un parámetro direccionable, con rango, profundidad
  y relación explícita con el valor base. Ejemplo: `LFO.salida → jeyeq.filtro.res`.
- **Acción:** operación solicitada y ejecutable desde distintas entradas; **evento:** notificación
  de un resultado, cambio o error. La modulación continua pertenece al procesamiento de señales.

## Familias según lo que cambia en la onda

| Propiedad | Operaciones candidatas |
| --- | --- |
| Amplitud y su evolución | **Volume**, envolvente aplicada, compresión, limitación |
| Frecuencia / altura | Desplazamiento de frecuencia, cambio de tono |
| Timbre / espectro / forma | Filtrado, ecualización, saturación, waveshaping |
| Fase / polaridad | Inversión de polaridad, rotación de fase |
| Tiempo / duración | Retardo, estiramiento temporal |
| Distribución espacial | Paneo, balance, anchura estéreo |

Una operación puede pertenecer a varias familias. Un LFO genera control; un analizador observa.
En **jeyeq**, un filtro y un volumen podrían componer `entrada → filtro → Volume → salida`;
frecuencia y Q/resonancia son parámetros del filtro, no procesadores independientes por ser perillas.

## Contrato y primer incremento

`Volume` usa **FunDSP** para ganancia estéreo y suavizado. Expone `gain` entre `0` y `2`
(`1` = sin cambio), bypass y metadatos. `Component` conecta instancias en serie, cada una con
un ID estable local. Los eventos se dirigen a ese ID y a un parámetro. El audio se procesa por
bloques, separado de GPUI. Los componentes propios y de terceros usarán el mismo contrato;
por ahora la extensión es Rust compilado. El host/registro de plugins, el catálogo común de acciones,
la emisión de eventos y los mapeos no existen todavía. Los adaptadores WASM/C++/JS son planes posteriores.

## Interfaz y configuración

Arrangement con controles de pista a la izquierda: nombre, pan, volumen, mute/solo/arm y `|+|`
para efectos/instrumentos; timeline temporal a la derecha. En esta iteración se excluyen Browser
y Device Chain inferior. El secuenciador es un módulo del DAW. La interacción tendrá modo visible,
movimientos Vim y etiquetas visuales para saltar a controles; el teclado de computadora ya
interpreta notas en la vista Instrument. El teclado MIDI externo se definirá aparte.
`~/.jaybu.profile` concentra tokens visuales para futuros temas importables y theme picker.
`~/.hayrc` está reservado para preferencias y mapeos. Compases, tempo, subdivisiones y tresillos
pertenecen al dominio temporal musical, independiente del grid visual y de los primitivos DSP.

## Estado y evolución

**Implementado:** Arrangement con canvas GPUI (carriles, regla, cursor, zoom y scroll), importación
de un clip mono/estéreo con Symphonia, waveform, salida CPAL y controles de Volume/bypass.
Piano de muestra con RustySynth y el SoundFont Upright Piano KW, recorrido
`nota → SoundFont → Volume → Master → CPAL`, teclado de computadora y teclas clicables.
Elevate conserva su biblioteca headless, cadena serial y demo offline. También exporta
`musical`: patrones en ticks, tiempo de tempo constante y scheduler por ventanas de frames.
La vista Pattern edita notas/velocity de cuatro slots y las reproduce en loop mediante
un plan por frames, transporte en el callback y percusión sintetizada. Clip, piano y patrón
abren streams excluyentes. Mixer sigue siendo mock. El piano está desacoplado de la vista:
`src/instruments/piano.rs` posee el reproductor y las notas sostenidas, y la UI pinta el
descriptor de `elevate::catalog`. El botón `[+]`, la tecla `a` y `:add` abren un buscador
headless de instrumentos, primitivos, componentes y acciones; elegir el piano lo asigna a
la pista destino mediante `musical::track`. El código está separado en `src/ui/`,
`src/instruments/`, `src/audio/`, `src/musical/` y `src/catalog.rs`; `main.rs` solo arranca
la aplicación.
**Límites actuales:** 120 BPM, un clip en memoria, un patrón, una instancia de piano que se
mueve entre pistas, percusión sin samples, plan inmutable durante la reproducción, buscador
sin host de plugins ni creación de instancias, salida a la frecuencia original del archivo
o predeterminada del dispositivo; sin remuestreo, mezcla multipista, MIDI externo, grabación
ni guardado. Las próximas bibliotecas se elegirán junto al usuario.
**Prioridad:** secuenciador nativo inspirado en Hydrogen. Un motor musical independiente
de sus vistas, un transporte y una salida CPAL común. Permitir varias instancias de piano
por pista, unificar audio y comunicación RT, sustituir la síntesis por samples de percusión,
permitir cambios de plan en caliente y añadir loop configurable y persistencia versionada.
Hydrogen sirve de referencia; no hay integración FFI ni código suyo incorporado.
Debugger, hosting de plugins y scripting vendrán después. La inspección utilizará snapshots.

Guía de implementación y ejecución: [README.md](README.md).
Contratos actuales: [arquitectura](docs/ARCHITECTURE.md).
Siguiente trabajo: [prototipo](docs/PROTOTYPE.md).
