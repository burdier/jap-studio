# Hay Sampler · sfizz

**Insertar sampler** está disponible en `[+]`, el menú contextual de pistas y como
`:sa` (`:track.insert_sampler`). Crea y selecciona una pista llamada Hay Sampler con
el instrumento asignado y abre su panel. Respeta la inserción dentro de grupos.
Repetir la acción crea otra pista y otra instancia con parámetros y sample propios.
La acción del catálogo también se puede vincular a un icono de la barra.

En el [+] existente, busca **Hay Sampler**. También acepta `:sampler`. Se asigna a la
pista seleccionada, aparece en Track FX y abre el editor inferior. Carga un archivo con **Cargar sample…**, suéltalo
sobre el panel o usa **Desde biblioteca** con un archivo seleccionado en el explorador.

En el Explorador, **Detectar raíz** propone una nota y su desviación en cents a
partir de varios tramos de la muestra. Puedes elegir la raíz con **− / +**.
**Usar como raíz** carga el archivo en el Hay Sampler de la pista activa y
compensa los cents detectados. **Preescuchar con la frase** usa el clip MIDI
seleccionado, su recorte y el tempo del proyecto. La preescucha arranca desde
el inicio del clip en una salida independiente y dura hasta 30 segundos.
**Añadir al Hilo** incorpora el sample afinado como rama con su propio fader.
Desde una pista raíz con instrumento crea un Hilo en modo Capas. En un Hilo
existente conserva su modo y mantiene la identidad del clip MIDI. La edición
estructural detiene Play. Guardar conserva la ruta del sample y los parámetros,
incluida la raíz.

El dispositivo conserva colores, tipografía, botones, perillas y teclado de Hay. La
organización sigue la referencia de Simpler: modos a la izquierda, waveform arriba,
región/afinación/voces debajo y filtro/ADSR/salida en una segunda fila.

- **Drum Kit (Simpler para drums):** atajo que activa `1-Shot`, `Keytrack` apagado (cualquier tecla MIDI dispara el golpe a su tono original), `Choke` (`group=1 off_by=1`) para que cada golpe corte la cola del anterior (p. ej. hi-hats), envolvente **AHD** (`Attack`, `Hold`, `Decay`, `Sustain = 0`), filtro activo y sensibilidad de velocidad.
- **Classic:** la duración de la nota gobierna el instrumento; **Loop** repite entre el
  marcador de loop y Final mientras se sostiene, con salida de loop al liberar.
- **1-Shot:** el sample continúa al soltar la nota, hasta su final o el cierre de la envolvente AHD.
- **Keytrack / Choke / Reverse:** botones independientes para fijar o cromatizar el tono (`pitch_keytrack`), cortar voces previas de la instancia o invertir el sample (`direction=reverse`).
- Inicio, Final y Loop se expresan en porcentaje del archivo. Arrastra los marcadores
  blancos; Alt+arrastre edita el inicio del loop. Se validan orden y rango mínimo.
- **Filtro estilo Hay Filter (`CurveOverlay`):** modos **LP / HP / BP** (`lpf_2p`, `hpf_2p`, `bpf_2p`) con curva interactiva 2D donde el arrastre horizontal controla la frecuencia (escala log continua) y el vertical o la rueda controla la resonancia (dB), con `Shift` para ajuste fino y doble clic para restaurar.
- Los controles incluyen además **Hold**, **Pitch Env** (semitonos), **Pitch Decay** (ms) para dar ataque percusivo a kicks/toms, **Vel → Vol** (`amp_veltrack`) y **Vel → Cut** (`fil_veltrack`) para que las curvas/rampas/ondas de Velocity del Piano Roll modulen dinámica y brillo del filtro.
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
Slice, LFO ni MPE. La audición live conserva un adaptador
individual y al tocar interrumpe Play.
El sampler conserva su stream de audición silencioso tras Stop para permitir volver
a tocar sin cargar otra vez. El proyecto guarda la referencia al archivo y restaura
el sampler al abrirlo.

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


## Envolvente de Drums Latex

Cada pieza usa su propia envolvente de amplitud y conserva esos ajustes al guardar.
**Cola completa** selecciona one-shot y pone Sustain al 100 %. Los pasos disparan
la muestra, y su duración audible se ajusta con el archivo y la envolvente.
**Cortar al repetir** hace que el siguiente golpe de la misma pieza apague
el anterior, incluso al cambiar de tono. **Grupo de corte** permite enlazar piezas.
Las muestras nuevas traen activados Cola completa y Cortar al repetir.
Para una muestra ya guardada, actívalos desde **Ajustes** de esa pieza.
La ganancia inicial de una muestra nueva es 0 dB. Jap Studio inicializa
la salida de sfizz y el volumen MIDI a ganancia unitaria. El nivel de
cada golpe sigue respondiendo a la velocidad y al fader de la pieza.
Los proyectos guardados conservan el valor de ganancia de cada muestra,
aunque el nivel resultante puede subir al quitarse el recorte interno de sfizz.
