> [!IMPORTANT]
> Remove this line to confirm you've reviewed this PR before submitting.

> [!IMPORTANT]
> **FILOSOFÍA FUNDAMENTAL DE HAY / ELEVATE — mantener siempre al inicio de este README.**
> Hay se construye como un sistema basado en **plugins**, empezando por Volume.
> Los **componentes** componen **primitivos**; los **mapeos son conexiones reales** que afectan
> al procesamiento; toda operación ejecutable se expresa como una **acción**, al estilo REAPER,
> y sus resultados y cambios se comunican mediante **eventos**.
> Esta filosofía guía las próximas implementaciones. Ahora estamos organizando la arquitectura;
> el registro de plugins, el sistema común de acciones/eventos y los mapeos aún no están implementados.

## Principios que guían el desarrollo

1. **Plugins desde la base.** Un plugin aporta capacidades al DAW: primitivos, componentes,
   parámetros y acciones. Los plugins propios deben usar el mismo contrato que las extensiones.
   El primer caso será registrar y utilizar el Volume existente mediante ese contrato.
   «Plugin» describe la extensión y su integración; el formato de carga se decidirá después.
2. **Componer en lugar de duplicar.** `jeyeq` es un componente. En su interior conecta instancias
   de primitivos, por ejemplo un filtro y Volume. Cada instancia tiene identidad y parámetros
   direccionables. La composición exacta de jeyeq se definirá juntos.
3. **Primitivos y parámetros son distintos.** Volume transforma amplitud; un filtro transforma
   espectro; un LFO genera una señal de control. `gain`, frecuencia y `res`/resonancia son
   parámetros de sus respectivos primitivos. La perilla es una representación de un parámetro.
4. **Mapping real.** Conectar `LFO.salida → jeyeq.filtro.res` debe modular realmente la resonancia
   en el DSP, incluso sin interfaz abierta. El mapeo identifica origen y destino, y define rango,
   profundidad y cómo combina la modulación con el valor base. El valor base y el efectivo se
   distinguen para poder entender qué está haciendo la conexión.
5. **Acciones como lenguaje de operación.** Reproducir, mover un clip, insertar un componente,
   cambiar una ganancia, crear un mapeo o activar bypass son acciones identificables e invocables.
   Teclado, ratón y futuras automatizaciones/scripts deben llegar al mismo sistema de acciones,
   en lugar de mantener implementaciones de comportamiento independientes.
6. **Eventos como lenguaje de observación.** Una acción expresa «hacer algo»; un evento informa
   «esto ocurrió». Componentes, primitivos, mapeos y transporte exponen sus cambios y resultados,
   incluidos los errores. La interfaz y el futuro debugger observan esos eventos.
   La señal continua de un LFO se procesa en el dominio de audio/control: no se convierte cada
   muestra en una acción de interfaz. Crear o editar esa conexión sí es una acción.
7. **La interfaz controla y representa.** GPUI permite operar el sistema y visualizar su estado;
   la identidad de un plugin, sus conexiones y su procesamiento no dependen de un widget.

### Diagrama conceptual — arquitectura objetivo

```text
Teclado / ratón / automatización / scripts
                     │ invocan
                     ▼
             SISTEMA DE ACCIONES
    Play · Mover clip · Cambiar parámetro · Mapear
                     │ ejecuta
                     ▼
                  HAY HOST
       Registro e instancias de plugins
          │                       │
          ▼                       ▼
    Plugin Volume          Plugin de componente jeyeq
    └─ Primitivo Volume    └─ Componente jeyeq
       └─ gain                ├─ Primitivo filtro
                              │  ├─ frecuencia
                              │  └─ res ◄──────────────┐
                              └─ Primitivo Volume     │
                                 └─ gain              │
                                                      │
    Plugin LFO → Primitivo LFO → MAPEO REAL ────────────┘
                                rango / profundidad

    Audio: entrada → [filtro → Volume] → salida
    Control: salida LFO → mapeo → parámetro res del filtro

    Acciones ejecutadas / cambios / errores
                     │ emiten eventos
                     ▼
            UI / observadores / debugger
```

### Cómo encaja lo que ya tenemos

| Pieza actual | Papel en esta filosofía | Pendiente |
| --- | --- | --- |
| `Volume` con FunDSP | Primer primitivo que expondrá un plugin | Registro e instanciación mediante el host |
| `Primitive` y metadatos de parámetros | Base del contrato DSP | Contratos de fuentes de control y conexiones |
| `Component` y `NodeId` | Composición serial e identidad local de instancias | Direccionamiento completo entre componentes/plugins |
| `ControlEvent` | Actualmente solicita cambios de parámetro o bypass | Separar solicitudes/acciones de eventos que notifican resultados |
| GPUI, canvas y reproducción CPAL | Interfaz y recorrido de audio existentes | Enrutar sus operaciones por acciones comunes |
| Ganancia/bypass por atómicos | Puente actual entre UI y Volume | Integrarlo con el contrato de parámetros, acciones y mapeos |
| Piano con RustySynth | Primer instrumento concreto: notas → SoundFont → Volume → Master → CPAL | Registro formal como plugin y eventos de notas observables |

**Prioridad actual:** prototipo de secuenciador nativo inspirado en Hydrogen, con un único
sistema de tiempo musical e independiente de sus vistas. La filosofía anterior se mantiene;
el registro de plugins, LFO, filtro y jeyeq son extensiones futuras. Avanzamos en pasos pequeños,
consultando antes de añadir bibliotecas. El MVP incluye pruebas temporales headless.

# Hay / Elevate

Prototipo de DAW en Rust + GPUI. [Visión del proyecto](SPEC.md).

## Empezar a editar

- [Arquitectura y contratos](docs/ARCHITECTURE.md).
- [Mapa tarea → archivo y verificaciones](docs/EDITING_GUIDE.md).
- [Estado del prototipo y siguientes incrementos](docs/PROTOTYPE.md).
- [Instrucciones para agentes](AGENTS.md).

`main.rs` solo arranca la aplicación. `src/ui/` contiene vistas/controladores,
`src/audio/` los adaptadores y `src/musical/` el núcleo headless exportado por Elevate.
La vista **2 / Pattern** edita notas y velocity y las reproduce en loop con percusión
sintetizada; sus samples reales siguen pendientes. `examples/sequencer.rs` muestra la
programación de eventos offline.

## Canvas del Arrangement

[`src/ui/timeline/`](src/ui/timeline/) usa `gpui::canvas`, `Window::paint_quad` y el sistema de
texto de la revisión de GPUI fijada en Cargo. Dibuja carriles continuos, regla y cursor de edición.
Los headers y carriles comparten el desplazamiento vertical de GPUI; la regla permanece fija.

- **Click** en un carril: seleccionar pista y colocar el cursor; en la regla: colocar el cursor.
- **Rueda**: desplazar pistas; **Shift + rueda** o gesto horizontal: desplazar el tiempo.
- **Ctrl + rueda**: zoom alrededor del puntero. Sobre la regla, la rueda sola desplaza el tiempo.
- En modo NORMAL: **h/l** o flechas horizontales mueven el cursor un pulso; **j/k** o flechas
  verticales seleccionan pista; **+ / −** hacen zoom; **Home** vuelve al inicio.
- **R** o **:reload** recargan `~/.jaybu.profile`. `TIMELINE_PIXELS_PER_BEAT` define la escala
  inicial; alturas, colores y tipografía de la regla también proceden del perfil.

Esta vista usa pulsos en **4/4 a 120 BPM**. El click es libre, sin snapping;
el grid adapta su densidad al zoom. El cursor azul es de edición y el verde muestra la posición
enviada al dispositivo durante la reproducción (sin compensación de latencia de salida).
El efecto visual del playhead usa `particula-rs` y `fastrand`, aislados en
`ui/timeline/particles.rs`. Sus [tokens](docs/PROTOTYPE.md#partículas-tokens-opcionales)
se leen del perfil visual; no interviene en el transporte musical.

## Primer archivo reproducible

Arrastra **un archivo mono o estéreo** sobre un carril. Se coloca en el punto donde lo sueltas,
se decodifica en segundo plano mediante **Symphonia 0.6.1** y aparece su waveform.
Este incremento mantiene un único clip: importar otro sustituye al anterior cuando termina de decodificarse.
Formatos habilitados: WAV/PCM, FLAC, MP3, OGG/Vorbis y AAC/M4A.

- **Space** en modo NORMAL o **▶**: reproducir desde el cursor de edición.
- **Arrastrar el clip con el botón izquierdo**: moverlo en el tiempo y entre pistas, sin snapping.
  Conserva el punto de agarre y detiene la reproducción al comenzar.
- **Space** de nuevo o **■**: detener. Un click en el timeline detiene y recoloca el cursor.
- **[ / ]**, los botones **− / +** junto a Volume o la rueda sobre la barra **V** del track
  del clip modifican la ganancia real entre `0` y `2`.
- **b** o **FX ON / BYPASS** permiten comparar el procesamiento con la señal original.

[`src/audio/playback.rs`](src/audio/playback.rs) conecta las muestras decodificadas a **Volume de Elevate** y a
**CPAL 0.18.2**. La comunicación de ganancia, bypass y posición usa atómicos; el callback
procesa bloques acotados sin decodificar ni reservar buffers. La salida se limita a ±1 al convertir
al dispositivo. El archivo completo y sus picos se conservan en memoria.

La salida predeterminada debe admitir estéreo a la frecuencia original del archivo: todavía
no hay remuestreo ni selector de dispositivo. Los errores se muestran en la barra de estado.
Para compilar CPAL en `gpui-dev` se instaló `alsa-lib-devel`. Los demás controles de pista y
Mixer siguen siendo mock. No hay mezcla multipista, recorte de clips ni guardado.

## Audición del patrón

La vista **2 / Pattern** (o **:pattern**) contiene cuatro instrumentos y un patrón de
16 pasos iniciales. **Click** o **Enter** alterna una nota; la **rueda** sobre una celda
ajusta su velocity. El modelo guarda ticks reales, no columnas: la longitud y la resolución
pueden cambiar sin convertir las notas a una rejilla fija.

- **Space** inicia o detiene la audición en loop desde la vista Pattern.
- El paso que suena se resalta en verde; la posición procede de los frames procesados.
- Clip, piano y patrón son excluyentes: iniciar uno detiene el anterior. La audición
  del patrón también se detiene al salir de la vista Pattern.
- El plan se compila al pulsar Play. Si editas mientras suena, detén y vuelve a iniciar
  para escuchar el cambio.

```text
Pattern (ticks) → PreparedPattern (frames, offsets)
                         │
                 Transport por frames
                         │ loop: frame % duración
                         ▼
              DrumKit (4 voces sintetizadas)
                         │
                 Volume de Elevate
                         │
                       CPAL
```

Esta audición usa síntesis provisional para validar el scheduler; todavía no carga
samples ni permite cambiar el BPM desde la interfaz. Los detalles y límites están en
[`docs/PROTOTYPE.md`](docs/PROTOTYPE.md).

## Primer instrumento: piano

La vista **INSTRUMENT / PIANO** (tecla **4** o **Tab**) contiene un piano de dos octavas.
Usa **RustySynth 1.3.8** como motor SoundFont y el banco **Upright Piano KW** de FreePats,
incluido en [`assets/soundfonts/UprightPianoKW-20220221.sf2`](assets/soundfonts/) bajo **CC0**.
El SoundFont se precarga en segundo plano; se puede sustituir con la variable `HAY_SOUNDFONT`.

```text
Acción: Note On / Note Off
             │
             ▼
     Piano (RustySynth)
       SoundFont 48 kHz
             │ L/R
             ▼
   Volume de Elevate (0…2, bypass)
             │
             ▼
   Master de este incremento
   (una fuente + protección ±1)
             │
             ▼
        CPAL → ALSA → PipeWire → altavoces
```

- **a w s e d f t g y h u j k**: C4…C5 cromático; mantener una tecla sostiene la nota.
- **Click** sobre una tecla blanca o negra: la sostiene hasta soltar el botón.
- **Space** libera todas las notas. **b** compara bypass y **[/]** ajustan Volume.
- La nota y el evento resultante aparecen en la barra de estado; la tecla pulsada se ilumina.
- La primera nota abre el dispositivo a la frecuencia predeterminada (48 kHz en el entorno actual).
  El callback aplica las notas, renderiza bloques acotados y mantiene la polifonía dentro del hilo de audio.

El SoundFont no se decodifica en el callback. El teclado de la interfaz y el estado de las notas
viven en GPUI; las órdenes llegan al callback por un canal y la ganancia/bypass por atómicos.
Todavía no hay MIDI externo, grabación, piano roll, registro de plugins ni mezcla multipista:
el Master actual es la etapa final de una sola fuente. En `gpui-dev` se instaló `pipewire-alsa`
para que ALSA del contenedor llegue al PipeWire del sistema; sin ese puente el dispositivo
predeterminado aparece ocupado.

## Primer paso del motor: Volume

Elevate comienza como una biblioteca Rust **headless**: transforma muestras sin depender de
GPUI, una ventana o un dispositivo de audio. Usa [FunDSP 0.23](https://docs.rs/fundsp/0.23.0/fundsp/).

- **Primitivo:** una operación sobre una señal, por ejemplo cambiar su amplitud.
- **Parámetro:** un valor que gobierna esa operación, por ejemplo su ganancia.
- **Componente:** conecta instancias de primitivos para producir un comportamiento compuesto.
- **Control visual:** permite editar un parámetro; el mismo volumen puede tener fader, perilla o comando.

Componer no siempre significa sumar señales: `Volume(0.5) → Volume(0.5)` aplica ambas
operaciones en serie, por lo que la ganancia total es `0.25`.

### Recorrido para leer juntos

| Archivo | Pregunta que resuelve |
| --- | --- |
| [`src/primitives.rs`](src/primitives.rs) | ¿Qué contrato tiene un primitivo y cómo describe sus parámetros? |
| [`src/primitives/amplitude.rs`](src/primitives/amplitude.rs) | ¿Cómo implementa FunDSP el primer volumen? |
| [`src/component.rs`](src/component.rs) | ¿Cómo se conectan las instancias y a cuál llega cada evento? |
| [`examples/volume.rs`](examples/volume.rs) | ¿Qué sucede al pasar una onda por uno o dos volúmenes? |
| [`tests/elevate.rs`](tests/elevate.rs) | ¿Cómo verificamos el sonido procesado y los límites del contrato? |

### La señal y el control

```text
audio:    entrada L/R → Volume → salida L/R
control:  SetParameter { node, parameter: Volume::GAIN, value: 0.5 }
```

La relación, una vez estabilizada la ganancia, es `salida = entrada × ganancia`:

| Ganancia | Resultado |
| --- | --- |
| `0` | Silencio |
| `0.5` | Mitad de amplitud, aproximadamente −6.02 dB |
| `1` | Sin cambio, 0 dB |
| `2` | Doble amplitud, aproximadamente +6.02 dB |

El rango inicial es `0..=2`. Los valores inválidos (incluidos NaN e infinitos) devuelven
errores y conservan el ajuste anterior. El volumen no recorta muestras a ±1: ese procesamiento
pertenecería a otro primitivo. Reducir amplitud a la mitad no equivale a la mitad del volumen percibido.

`shared → var → follow → split` genera una ganancia suavizada y común a ambos canales;
`multipass * ganancia` aplica el DSP. `follow(0.005)` tarda aproximadamente 5 ms en alcanzar
la mitad del cambio. La transición continúa entre bloques. Bypass lleva suavemente la
ganancia efectiva a `1`, conservando el ajuste para cuando vuelva a activarse.

### Contrato inicial

- Audio estéreo en bloques de frames `[f32; 2]`, modificados **in-place**. La frecuencia de
  muestreo se valida mediante `ProcessSpec`; también se aceptan bloques vacíos o de distinta longitud.
- `prepare` configura recursos antes de procesar. `reset` reinicia el historial DSP cuando
  se reinicia el procesamiento; conserva los parámetros y no debe llamarse entre bloques normales.
- `process` en `Volume` y `Component` no reserva memoria, toma mutexes ni hace E/S.
  Un primitivo de terceros debe cumplir ese mismo contrato; el trait por sí solo no lo garantiza.
- Los parámetros tienen ID, clave, unidad, límites y valor inicial. Esto permitirá enlazar
  controles configurables sin acoplar el DSP a `~/.jaybu.profile`.
- `NodeId` identifica una instancia **dentro de su componente**, permanece estable al quitar
  otras y no se reutiliza. `ControlEvent` lleva IDs y valores, sin objetos de GPUI.
- `Component` implementa una cadena estéreo en serie. Se construye/modifica fuera del procesamiento;
  los eventos se aplican síncronamente antes de cada bloque. Las colas entre hilos, marcas temporales
  para automatización y conexiones en paralelo son pasos posteriores del motor.

### Ejecutar la demo y las pruebas

Desde `~/Projects/daw-gpui`:

```bash
distrobox enter gpui-dev -- cargo run --no-default-features --example volume
distrobox enter gpui-dev -- cargo test --no-default-features --lib --tests --examples
distrobox enter gpui-dev -- cargo run --bin daw-gpui
```

La demo genera una senoide de 440 Hz en memoria y muestra picos `0.800 → 0.400 → 0.200`.
Esta demo es offline; la aplicación GPUI también permite reproducir un archivo a través de Volume.
Las pruebas cubren estéreo, ganancia, silencio, cambios suaves, bypass, partición en bloques,
frecuencias de muestreo, errores y direccionamiento de eventos.

El mock GPUI sigue siendo una aplicación separada de esta biblioteca. `cargo watch -x run`
recompila y reinicia la aplicación; ese ciclo no conserva su estado como un hot-patch.
