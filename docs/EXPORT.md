# Exportación de audio

Exporta el **Arrangement** en segundo plano con libsndfile, LAME y libsoxr.
El comando y la acción «Exportar audio» del catálogo usan el mismo controlador.
La acción está disponible al principio de la lista del botón SVG **+** de la barra.
**Hay → Configuración → Exportación** permite editar y guardar sus valores predeterminados.
La acción «Configurar exportación» (`:export.settings`) abre directamente esa categoría.
Los cambios se mantienen como borrador hasta pulsar **Guardar valores predeterminados**;
se validan y persisten en `EXPORT_DEFAULTS` de `~/.jaybu.profile`. **Ctrl+S** también guarda.
Nombre y carpeta admiten espacios; clic selecciona el campo, Ctrl+A selecciona su contenido,
Ctrl+V pega, Tab recorre los campos y Enter termina la edición.
Los cambios posteriores en clips, niveles, mute/solo, parámetros de piano y Utility no
alteran el snapshot que ya se está exportando. La reproducción puede continuar.

```text
:export -n 93.[mp3,wave] -master
:export -n 93.[wav,flac] -individuals
:export -n "Mi Mezcla.[wav,mp3]" -sr 44100 -bits 24 -bitrate 192 -out "~/Music/Exports"
:export -n Piano.aiff -individuals -tracks 2 -sr 96000 -bits 32f
:export -n Loop.wav -start 8 -length 16 -tail 2 -mono -master-fx off
:export -n Preview.mp3 -vbr 2
:export.cancel
```

## Opciones

| Opción | Valores / comportamiento |
|---|---|
| `-n` | Nombre y formato (`93.wav`) o lista (`93.[mp3,wave]`). `wave` es alias de `wav`; `aif` de `aiff`. No es tempo. |
| `-master` | Una mezcla de las pistas incluidas; predeterminado. |
| `-individuals` | Un archivo por pista que contiene clips, incluyendo las silenciadas como silencio. Igual inicio y duración para todos. |
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
- MIDI usa el SoundFont, preset y parámetros capturados del piano actual. Cada clip tiene
  voces independientes para no cortar notas de otro clip solapado; todavía no hay varias
  instancias de instrumento configurables por pista. Reutiliza backend y scheduler de audición.
- Un rango con inicio posterior a cero se prepara renderizando lo anterior sin escribirlo,
  para conservar notas sostenidas y estado de efectos.
- Utility se construye con su estado capturado y se aplica después de la mezcla.

El patrón de percusión de la vista Pattern no forma parte del Arrangement y no se añade
implícitamente. Loop interno/transposición de clips aún no tienen procesador y se rechazan
si están activados. Retornos, automatización, render como loop, normalización, LUFS, dither
son incrementos posteriores.

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
