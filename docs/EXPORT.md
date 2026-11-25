# Exportación de audio

Exporta el **Arrangement** en segundo plano con libsndfile, LAME y libsoxr.
El comando y la acción «Exportar audio» del catálogo usan el mismo controlador.
Abre **Archivo → Exportar audio…** y elige **Mezcla completa** o **Pistas separadas**.
Puedes marcar varios formatos, escribir el nombre y elegir la carpeta.
**Más opciones** reúne la selección de pistas, el rango y los ajustes de muestreo.
Al terminar, **Mostrar en carpeta** te lleva al primer archivo generado.
La acción también está disponible en el botón **+** de la barra.
**Hay → Configuración → Exportación** permite editar y guardar sus valores predeterminados.
La acción «Configurar exportación» (`:export.settings`) abre directamente esa categoría.
Los cambios se mantienen como borrador hasta pulsar **Guardar valores predeterminados**;
se validan y persisten en `EXPORT_DEFAULTS` de `~/.jaybu.profile`. **Ctrl+S** también guarda.
Nombre y carpeta admiten espacios; clic selecciona el campo, Ctrl+A selecciona su contenido,
Ctrl+V pega, Tab recorre los campos y Enter termina la edición.
Los cambios posteriores en clips, niveles, mute/solo, parámetros de piano y Utility no
alteran el snapshot que ya se está exportando. La reproducción puede continuar.

En Linux, la suma de ramas paralelas usa la latencia en muestras que publica cada
VST3 tras restaurar su preset. La duración automática incluye el tiempo de paso
por el rack y la cola que declaran sus efectos. Un rango explícito termina en la
posición solicitada más los segundos de `-tail`.

```text
:export -n 93.[mp3,wave] -master
:export -n 93.[wav,flac] -individuals
:export -n "Mi Mezcla.[wav,mp3]" -sr 44100 -bits 24 -bitrate 192 -out "~/Music/Exports"
:export -n Piano.aiff -individuals -tracks 2 -sr 96000 -bits 32f
:export -n Loop.wav -start 8 -length 16 -tail 2 -mono -master-fx off
:export -n Preview.mp3 -vbr 2
:export.cancel
```

## Prueba de cancelación en Linux

Con el release instalado, ejecuta `python3 scripts/null-test.py --keep`.
El script genera un kick y un bajo en dos pistas, exporta el máster como WAV
de 32 bits flotantes y captura Play en un monitor privado de PipeWire.
Alinea las dos señales por muestras, resta ambos canales y mide el pico y RMS
del residuo. El límite de pico es −100 dBFS por defecto. Un residuo por encima
del límite devuelve código 1 y un fallo al montar la prueba devuelve código 2.
`--keep` conserva la captura, la exportación y `residuo.wav` en la carpeta indicada.
`--bin` permite elegir otro ejecutable. Usa Python 3, Xwayland, xdotool, xmodmap,
bubblewrap, PipeWire, WirePlumber y FFmpeg. Ejecuta el script desde una sesión
gráfica de Linux con Wayland.

## Opciones

| Opción | Valores / comportamiento |
|---|---|
| `-n` | Nombre y formato (`93.wav`) o lista (`93.[mp3,wave]`). `wave` es alias de `wav`; `aif` de `aiff`. No es tempo. |
| `-master` | Una mezcla de las pistas incluidas; predeterminado. |
| `-individuals` | Un archivo por pista MIDI/audio con clips y por pieza programada del kit. Igual inicio y duración para todos. |
| `-tracks 1,3` | Restringe ambos modos a esas pistas, numeradas desde 1. |
| `-out "ruta"` | Carpeta de destino. Por defecto `exports/` bajo el directorio de ejecución; admite `~/` desde la UI. |
| `-sr 48000` | Frecuencia final en Hz enteros; predeterminada 48000. PCM: 8000–384000, según codec/sintetizador. MP3: 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100 o 48000. |
| `-bits 24` | PCM entero `16`, `24`, `32`, o float `32f`; predeterminado 24. FLAC: 16/24. No afecta a MP3. |
| `-bitrate 320` | MP3 CBR, kbps; predeterminado 320. Los bitrates disponibles dependen de la frecuencia MPEG. |
| `-vbr 2` | MP3 VBR, calidad 0 (mejor) a 9; alternativa a `-bitrate`. |
| `-mono` / `-stereo` | Mono `(L+R)/2` o estéreo; predeterminado estéreo. |
| `-start 0` | Inicio en beats desde cero, no en compases. Predeterminado 0. |
| `-length 16` | Duración en beats. Por defecto hasta el último clip de las pistas incluidas. |
| `-tail 2` | Segundos adicionales para las colas del instrumento. Predeterminado 0. Al terminar el rango se liberan notas/sustain y no entran clips/notas nuevos. |
| `-master-fx on` / `off` | Incluir/excluir Utility. Por defecto incluido en master y excluido en individuales. |

Sin argumentos, `:export` utiliza los valores guardados en Configuración. Antes de personalizarlos,
escribe `exports/export.wav`: master estéreo a 48 kHz/24 bits. Los argumentos explícitos
sustituyen únicamente sus opciones; `-tracks all`, `-length all` y `-master-fx auto`
restauran respectivamente todas las pistas, hasta el último clip y Utility según el modo de salida.
`:export.help` muestra un resumen. Los nombres conservan mayúsculas y aceptan comillas.
En modo Command, `Ctrl+V` pega texto del portapapeles.
Opciones desconocidas, combinaciones incompatibles y archivos ya existentes producen error;
no se sustituyen por otros valores ni se sobrescribe el destino. Para repetir, elige otro nombre/carpeta.

Individuales: `93-01-Nombre.wav`, `93-02-Nombre.wav`, etc. El índice evita colisiones entre
pistas con el mismo nombre. Todos los formatos de una salida reciben el mismo render.
WAV/AIFF float conservan valores por encima de ±1; PCM entero y MP3 saturan al convertir.
La barra de estado informa cuántas muestras excedieron 0 dBFS. No se normaliza ni limita
automáticamente y todavía no hay dither.

## Qué se renderiza

- Todos los clips conservados de audio y MIDI del Arrangement, con posiciones, silencios,
  recortes, solapamientos, ganancia/mute de clip, nivel/mute/solo y balance de pista.
- Audio se convierte con **libsoxr VHQ**, compensando el retardo mediante su API offline.
- Play y Exportar usan la misma voz por pista de Hay Piano o Hay Sampler y la misma mezcla por bloques.
- Drums Latex conserva las capas, el choke y la mezcla del kit. Los stems de las piezas se toman después del fader y paneo del bus.
- Los clips MIDI de un **Hilo** usan el mismo reparto por rama de Play (capas o Tejer),
  con una instancia offline de Hay Piano o Hay Sampler por rama. Cada rama se suma en el bus
  del Hilo. Elegir solo una rama exporta las notas que le corresponden en la secuencia completa.
- El máster pasa por los FX y el fader de cada grupo. Los stems individuales de las hijas
  se toman antes del grupo. Para incluir también un archivo con el grupo procesado,
  selecciona su número con `-tracks` en una exportación `-individuals`.
- Un rango con inicio posterior a cero se prepara renderizando lo anterior sin escribirlo,
  para conservar notas sostenidas y estado de efectos.
- Utility se construye con su estado capturado y se aplica después de la mezcla.

Desde Drums Latex se exporta la fuente activa (Patrón, Canción o Arrangement). Un patrón
repetido se imprime durante el rango solicitado; con solo el kit se toma un ciclo y se deja
caer el último one-shot. En Arrangement se incluyen los clips de batería del proyecto.
`-tracks` filtra los clips de Arrangement por pista; para incluir un patrón o canción
independiente usa todas las pistas. En el audio, loop interno y transposición siguen pendientes.
Retornos, automatización, render como loop, normalización, LUFS y dither son
incrementos posteriores.
Los instrumentos y efectos VST3 de pistas de audio y MIDI usan copias privadas de las
instancias abiertas, preparadas a la frecuencia de exportación. Al pulsar Exportar,
cada editor captura el estado del procesador y del controlador. El máster y los stems
recorren las cadenas con el orden, bypass y ramas de Play. Un error al recuperar el estado
o durante el procesamiento cancela el lote. Los componentes de audio Lua siguen pendientes.

MP3 escribe la etiqueta LAME de delay/padding; la alineación exacta depende de que el lector
la respete. Usa PCM/FLAC para intercambio de stems sample-accurate.

## Dependencias nativas y verificación

Se cargan dinámicamente, siguiendo el patrón de FluidSynth. No se necesita un ejecutable
externo ni nuevas dependencias Cargo. Runtime Linux:

- `libsndfile.so.1`: libsndfile 1.2.2, LGPL-2.1-or-later, WAV/AIFF/FLAC.
- `libmp3lame.so.0`: LAME 3.100, LGPL, MP3 CBR/VBR.
- `libsoxr.so.0`: SoX Resampler 0.1.3, LGPL-2.1-or-later, remuestreo VHQ.

En Fedora (`gpui-dev`): `sudo dnf install libsndfile lame-libs soxr`.
Solo se carga el backend solicitado; una biblioteca ausente produce un error descriptivo.
Los adaptadores comprueban escrituras y cierre. Los archivos se preparan junto al destino
y se publican después de terminar el lote, sin sobrescribir; error/cancelación limpia temporales.
La publicación sin sobrescritura usa hard links en el mismo filesystem.

```bash
distrobox enter gpui-dev -- cargo test --no-default-features --test export --locked
distrobox enter gpui-dev -- cargo test --bin daw-gpui audio::export --locked
```

Las pruebas nativas necesitan esas tres bibliotecas y el SoundFont incluido, pero no ventana
ni dispositivo. Comprueban mezcla, recortes, rango/cola, stems, MIDI, frecuencias distintas,
PCM entero/float, MP3 CBR/VBR, decodificación con Symphonia y limpieza ante errores/cancelación.

Los WAV afinados desde el editor de muestras usan el mismo PCM estéreo preparado para preescucha, Arrangement y exportación. Los ajustes en semitonos y cents conservan el número de frames de la fuente.
