# Hay Sampler · sfizz

**Insertar sampler** está disponible en `[+]`, el menú contextual de pistas y como
`:sa` (`:track.insert_sampler`). Crea y selecciona una pista llamada Hay Sampler con
el instrumento asignado y abre su panel. Respeta la inserción dentro de grupos.
Repetir la acción crea otra pista y otra instancia con parámetros y sample propios.
La acción del catálogo también se puede vincular a un icono de la barra.

En el [+] existente, busca **Hay Sampler**. También acepta `:sampler`. Se asigna a la
pista seleccionada, aparece en Track FX y abre el editor inferior. Carga un archivo con **Cargar sample…**, suéltalo
sobre el panel o usa **Desde biblioteca** con un archivo seleccionado en el explorador.

El dispositivo conserva colores, tipografía, botones, perillas y teclado de Hay. La
organización sigue la referencia de Simpler: modos a la izquierda, waveform arriba,
región/afinación/voces debajo y filtro/ADSR/salida en una segunda fila.

- **Classic:** la duración de la nota gobierna el instrumento; **Loop** repite entre el
  marcador de loop y Final mientras se sostiene, con salida de loop al liberar.
- **1-Shot:** el sample continúa al soltar la nota, hasta su final.
- Inicio, Final y Loop se expresan en porcentaje del archivo. Arrastra los marcadores
  blancos; Alt+arrastre edita el inicio del loop. Se validan orden y rango mínimo.
- Los controles aceptan arrastre vertical, Shift para precisión, rueda y doble clic
  para restaurar. Afinación en semitonos, nota raíz MIDI, voces 1–128, filtro LP 12 dB/oct,
  frecuencia Hz, resonancia dB, ADSR en ms/% y salida dB/pan.
- Volumen, pan, afinación, cutoff/resonancia y envolvente usan CC de alta resolución de
  sfizz. Los valores son efectivos según las reglas de modulación/envolventes de sfizz;
  cambiar la envolvente no reinicia una nota ya disparada.
- Región, modo, activación del filtro, nota raíz y número de voces preparan otra instancia
  en background. Al terminar se cortan las notas anteriores. En arrastres se prepara al
  soltar; las solicitudes durante una preparación se agrupan y se aplica la más reciente.
- **M** activa el teclado MIDI existente. También funcionan teclado clicable y entrada
  MIDI. Los clips MIDI de la pista asignada usan sfizz al pulsar Play junto a las pistas
  WAV y los demás samplers en el mismo reloj y salida.
- Exportar captura el sample y sus parámetros. Crea instancias independientes de sfizz
  para los clips exportados y utiliza el mismo consumidor de eventos que la audición.

## Límites de este incremento

Varias instancias sampler pueden sonar juntas en el Arrangement. No incluye Warp,
Slice, LFO ni MPE ni guardado de proyecto. La audición live conserva un adaptador
individual y al tocar interrumpe Play.
El sampler conserva su stream de audición silencioso tras Stop para permitir volver
a tocar sin cargar otra vez. El estado y el sample cargado duran la sesión. La
exportación offline de más de un sampler se rechaza explícitamente por ahora.

La carga decodifica el archivo completo con Symphonia y prepara un WAV float temporal
para sfizz. Así comparte los formatos de importación de Hay sin depender de los codecs
con los que se compiló sfizz. El recurso tiene propiedad compartida entre reproducción,
preparación y exportación; se elimina al soltar su última referencia. Los picos visuales
se reducen a un máximo de 1024 fuera de la UI. Las notas no hacen E/S en nuestro adaptador.

## Prueba rápida de PR-05

Inserta dos pistas con `:sa`, carga un WAV distinto en cada panel y crea un clip
MIDI con notas en cada pista. Importa otro WAV en una tercera pista y pulsa Play
desde Arrangement: deben oírse los tres a la vez. Mute/Solo debe afectar solo a
su pista; al cambiar el sample o un parámetro de un sampler y volver a pulsar Play,
el otro conserva su sonido. Stop corta las voces. Desde Session, Space suma el
patrón provisional al arreglo en la misma salida y mantiene el patrón en loop.

## Biblioteca nativa y compilación

Se integra **sfizz 1.2.3**, revisión
`4e70dc0bef53b41f2853ed46e26f5911114c92d0`, mediante su API C. Licencia BSD-2-Clause;
las dependencias nativas y sus licencias están descritas en el README oficial:
<https://github.com/sfztools/sfizz/tree/1.2.3>.

No se añade un crate. Se reutiliza libc para cargar la biblioteca dinámica con dueño
RAII, símbolos comprobados y synth exclusivo por hilo. La biblioteca vive hasta después
de destruir el synth. Los fallos de carga se muestran sin sustituir sfizz por un simulador.

En Linux, desde un entorno con git, CMake, make y compilador C/C++:

```sh
bash scripts/build-sfizz.sh
```

Construye solo la biblioteca, sin plugins, UI, JACK ni tests. La instala dentro de
`.local/sfizz/`, ignorado por Git. Se busca en `HAY_SFIZZ`, bibliotecas del sistema y
`.local/sfizz/lib{,64}` desde la raíz del proyecto. Si se ejecuta desde otra carpeta,
configura `HAY_SFIZZ` con la ruta absoluta a `libsfizz.so`. **Recargar** vuelve a preparar
el instrumento; también permite recuperarse de un error al abrir el dispositivo.

Verificación solicitada: compilación de sfizz, `cargo check --all-targets --locked`,
`cargo check --no-default-features --lib --locked` y build de la aplicación. No se ejecutan
tests ni se afirma validación auditiva o interactiva de esta UI.
