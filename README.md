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

## Interfaz de audio

Pianoteq mediante Wine/yabridge: `[+]` → Instrumentos → Pianoteq. Carga y reproducción
MIDI integradas; inicia con `bash scripts/run.sh` o el acceso del escritorio.
El catálogo también identifica efectos VST3 genéricos y los inserta en Track FX
con su editor original (`fx.vst3.load`). Cathar Restore validado mediante yabridge.
[Alcance del host VST3 y límites actuales](docs/VST3.md).

`:list device` abre Configuración → Audio con las interfaces detectadas.
`:use device` abre el selector; `:use device 2` o `:use device 'Nombre exacto'`
aplica y guarda la salida. Para entrada: `:use device input 2`.
Los números corresponden a la última lista; se persiste el ID, no el número.
`:use device default` vuelve a la salida predeterminada.

El panel permite elegir driver, canales, sample rate y buffer, con las capacidades
reportadas por CPAL. En Linux se usa **PipeWire** cuando no hay una selección previa.
En **Configuración → Audio**, elige **PipeWire** (o «Compartir audio con Linux» si
estabas en ALSA), selecciona **SSL 2+ Mk II Line Outputs 3 + 4** y pulsa
**Aplicar y guardar**. Linux puede seguir usando **Line Outputs 1/L + 2/R**: Hay
no cambia la salida predeterminada del sistema. El selector **Salida estéreo** muestra
las rutas hermanas con sus nombres reales; cada ruta estéreo usa dos canales locales.
ALSA directo sigue disponible y puede requerir acceso exclusivo a toda la interfaz.
**Aplicar y guardar** detiene la reproducción y actualiza piano,
sampler, percusión y clips. El tono de prueba dura dos segundos a −24 dBFS; el medidor
solo comprueba la entrada, sin grabar ni monitorizarla. Para cancelar una prueba,
pulsa de nuevo el botón o cierra el panel. Las preferencias de exportación son separadas.

Para compilar el backend nativo en Fedora/gpui-dev: `sudo dnf install pipewire-devel clang-devel`.
CPAL activa su feature `pipewire`; no se necesita modificar la configuración de ALSA
ni la salida de otras aplicaciones.

## Espacio de trabajo y teclado

Pulsa **f en Normal** sobre el panel activo para ampliarlo; repite **f** o pulsa
**Esc** para restaurar la distribución acoplada. También existe el botón **Ampliar**,
**F11** y la acción `view.maximize`. En ventanas GPUI separadas se maximiza la ventana.

En **Configuración → Atajos → Navegación Vim** puedes desactivarlo y reactivarlo.
La misma operación está en `[+]` como **Vim · activar/desactivar** (`vim.toggle`).
La preferencia se guarda; MIDI, Espacio, flechas, Delete y edición con ratón se mantienen.

## Spectrum en la cadena FX

Busca **Hay Spectrum** en `[+]` o usa `:fx.spectrum`. Arrastra sus nodos para colocar
`Spectrum → VST3 → Spectrum`; los inspectores bajo el árbol permiten comparar señal.
`< >` cambia entre espectro, onda, espectrogramas log/lineal y nivel estéreo. El piano
comparte el mismo componente. [Alcance y acciones](docs/VISUAL_MODULATION.md).

## Explorador de bibliotecas

El explorador de bibliotecas se abre con **e en modo Normal**, a la izquierda del Arrangement.
Su separador permite reducirlo hasta ocultarlo; e lo recupera. «Añadir biblioteca» guarda
accesos a carpetas hasta quitarlos explícitamente. Doble clic o Enter sobre audio/MIDI lo
inserta en la pista activa, en el cursor; la operación también está en el catálogo y los
iconos como `explorer.insert` o `file.import` (con ruta). «Recargar» actualiza los archivos
tras cambios externos. El árbol headless y su entidad visual están separados del transporte;
lecturas en background, filas virtualizadas y vista cacheada. Véase `docs/ARCHITECTURE.md`.

Los formularios de acciones usan [GPUI Kit](https://gpui-kit.com/) 0.9.0 (Apache-2.0): campos
de texto, cantidades musicales, casillas y botones. El adaptador está en
`src/ui/components/parameter_field.rs`; el tema Hay se proyecta desde `src/ui/kit.rs`.
El contrato de parámetros y las operaciones del DAW siguen siendo independientes de la UI.

## Hilos: capas y round robin

Selecciona dos o más pistas con instrumento y usa **Tejer pistas seleccionadas**
desde el menú de pista o el buscador `[+]`. Hay crea el grupo, activa su entrada
MIDI y alterna una nota
por instrumento automáticamente. También puedes agrupar primero y elegir
**Tejer · automático** en el menú del grupo; **Hilo · capas** toca todos los
instrumentos con cada nota. Coloca el clip MIDI
en el carril del grupo o activa el botón **MIDI** del grupo para tocar en vivo.
Cada hija conserva su volumen, panorama y FX. `track.script` admite
`{ op="hilo.mode", track=<TrackId o handle>, mode="round_robin" }`.
En la barra de comandos: `:track.group` agrupa la selección, `:hilo.rr` cambia
el grupo seleccionado a round robin, `:hilo.layer` activa capas y `:track.midi`
activa su entrada MIDI. `:hilo.off` desactiva el Hilo cuando no tiene clips.
Para crear el grupo sin seleccionar pistas con el ratón, usa
`:track.template.hilo`; la plantilla agrupa las dos primeras pistas raíz que ya
tengan instrumento y añade Hay Utility a cada rama.
El modo queda en el documento de proyecto; la UI todavía no guarda una sesión
completa al salir. [Estado y límites](docs/ARCHITECTURE.md#pr-07-primer-recorrido-de-hilos-sobre-grupos).

No hace falta configurar Lua para usar Tejer. Un archivo Lua puede cambiar el
orden de ramas como opción avanzada. [Contrato y prueba](docs/TEJER.md).
Puedes insertar **Hay Piano** en varias pistas: cada inserción crea una instancia
independiente, con sus propios controles y voces.

## Lua musical previo a Play

Para oír el transpositor Lua de ejemplo, inicia Hay desde la raíz con
`HAY_LUA_PREPLAY_SCRIPT=assets/scripts/components/midi/transposer.lua bash scripts/run.sh`,
abre un clip MIDI y pulsa Play. El worker aplica el script a una copia de los
clips; el piano roll conserva sus notas. Editar `local semitones = 7` en el
script cambia el intervalo para el siguiente Play. Un error cancela la
preparación y aparece en el estado del DAW. La ruta no se guarda en el proyecto
ni ejecuta Lua en audio. [Medición y alcance](docs/LUA_LIVE_REPORT.md).

## Track FX sin cambiar Add

El **[+]**, **A** y **:add** siguen abriendo el mismo catálogo general. Elegir **Hay Utility**
inserta un efecto en la pista, dentro del contenedor FX seleccionado o después del efecto activo.
El botón **FX** junto al [+], el menú de pista o **:fx.show** abren el árbol. Archivos y FX
comparten el mismo componente visual, colores, filas, sangría y lista virtualizada.

- **A / Add en el árbol:** catálogo existente, inicialmente en Componentes; Tab conserva las
  demás categorías. El encabezado indica pista y contenedor destino.
- **Doble clic / Enter:** parámetros. **B:** bypass. **F2:** nombre. **Ctrl+D:** duplicar.
- **Delete:** quitar. Clic derecho ofrece las mismas acciones y contenedores serie/paralelo.
- **Arrastrar:** bordes para ordenar; centro de contenedor para entrar; raíz para sacar.
- Selección, expansión y scroll se recuerdan por pista; el panel es redimensionable.

Utility tiene procesamiento real en Arrangement/piano y exportación. En serie se multiplican
sus ganancias; en paralelo se suman ramas con la misma entrada. VST3 y otros efectos del
catálogo todavía necesitan su procesador/host; no se simulan. El Utility de salida sigue en
**:utility**, independiente del árbol. El proyecto aún no guarda estas cadenas al reiniciar.

## Principios que guían el desarrollo

Las herramientas Lua pueden transformar notas MIDI desde `[+]` o `:midi.script`.
El ejemplo `:midi.script.accent` acentúa la selección. Lua corre fuera del hilo de
audio y Rust valida cada resultado. [API y límites](docs/LUA.md).

Para automatizar pistas, usa `[+]` → **Lua · plantilla de dos pistas** o
`:track.template.two`. También puedes ejecutar un archivo propio con
`:track.script '/ruta/script.lua'`. El script devuelve operaciones que Rust
valida y aplica como un solo lote; se conservan clips y mapeos. Los cambios FX
por lote admiten Hay Utility y contenedores serie/paralelo. [Contrato y ejemplo](docs/LUA_TRACK_BATCHES.md).

Para efectos de audio, `[+]` ofrece **Hay Filter** (SVF LP/HP/BP nativo FunDSP,
paquete Lua v2 con frecuencia, Q y curva de respuesta) y **Hay Double Volume**. Su macro controla dos
primitivos Volume nativos en Track FX; otros descriptores Lua se recargan con
`:fx.components.reload`. [Uso y contrato](docs/LUA_AUDIO_COMPONENTS.md).

Al escribir `:` aparecen sugerencias de comandos y acciones. Usa `↑/↓` para elegir,
Tab o clic para completar, Enter para ejecutar y Esc para cancelar. El texto con
argumentos se ejecuta tal como se escribió; los comandos sin argumentos que requieren
datos abren el formulario existente.

1. **Plugins desde la base.** Un plugin aporta capacidades al DAW: primitivos, componentes,
   parámetros y acciones. Los plugins propios deben usar el mismo contrato que las extensiones.
   El primer caso registra y utiliza el Volume existente mediante ese contrato.
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
| `Volume` con FunDSP | Primitivo de ganancia usado por Hay Utility | Más capacidades reutilizables |
| `Hay Utility` (`hay.utility`) | Primer dispositivo interno: fábrica → Component → Volume; slot real de salida | Host de efectos generales y persistencia |
| `Primitive` y metadatos de parámetros | Base del contrato DSP | Contratos de fuentes de control y conexiones |
| `Component` y `NodeId` | Composición serial e identidad local de instancias | Direccionamiento completo entre componentes/plugins |
| `ControlEvent` | Actualmente solicita cambios de parámetro o bypass | Separar solicitudes/acciones de eventos que notifican resultados |
| GPUI, canvas y reproducción CPAL | Interfaz y recorrido de audio existentes | Enrutar sus operaciones por acciones comunes |
| Ganancia/bypass por atómicos | Puente actual entre UI y Volume | Integrarlo con el contrato de parámetros, acciones y mapeos |
| Piano con RustySynth | Primer instrumento concreto: notas → SoundFont → Volume → Master → CPAL | Registro formal como plugin y eventos de notas observables |

**Prioridad actual:** prototipo de secuenciador nativo inspirado en Hydrogen, con un único
sistema de tiempo musical e independiente de sus vistas. La filosofía anterior se mantiene;
el registro general de plugins, LFO y jeyeq son extensiones futuras; el filtro SVF
ya está disponible como primitiva nativa mediante el kit Lua v2. Avanzamos en pasos pequeños,
consultando antes de añadir bibliotecas. El MVP incluye pruebas temporales headless.

# Hay / Elevate

Prototipo de DAW en Rust + GPUI. [Visión del proyecto](SPEC.md).

## Empezar a editar

- [Arquitectura y contratos](docs/ARCHITECTURE.md).
- [Mapa tarea → archivo y verificaciones](docs/EDITING_GUIDE.md).
- [Estado del prototipo y siguientes incrementos](docs/PROTOTYPE.md).
- [Instrucciones para agentes](AGENTS.md).

`main.rs` solo arranca la aplicación. `src/ui/` contiene vistas/controladores,
`src/instruments/` el estado de ejecución de los instrumentos, `src/audio/` los adaptadores
y `src/musical/` el núcleo headless exportado por Elevate. `src/catalog.rs` es el índice
headless de instrumentos, primitivos, componentes y acciones.
`src/project.rs` inicia la gestión de proyecto con un documento Loro headless para
identidad, orden, datos básicos de pistas, cadenas FX y clips MIDI. Aún no incluye audio;
la UI no ofrece abrir/guardar un proyecto incompleto. Véase `docs/ARCHITECTURE.md`.
La vista **2 / Pattern** edita notas y velocity y las reproduce en loop con percusión
sintetizada; sus samples reales siguen pendientes. `examples/sequencer.rs` muestra la
programación de eventos offline.

## Canvas del Arrangement

Clic derecho en una cabecera abre las propiedades y acciones de pista: insertar, renombrar,
duplicar, agrupar/desagrupar, mute, solo, color y eliminar. **Ctrl+clic** selecciona varias
pistas y **Shift+clic** un rango. Los grupos reúnen sus hijos y muestran `[activas/total]`;
en Arrangement, **Delete** o **dd** en modo **Normal** eliminan las pistas seleccionadas
y sus clips. **Esc** cancela la primera `d`. Los grupos incluyen sus pistas hijas al eliminar;
no contienen clips propios. Al eliminar todas las pistas, el **+** central abre el buscador
de **A / :add** para insertar una pista o elegir un instrumento. Las cadenas de Utility por
pista/grupo se administran en Track FX; el host VST3 y los buses generales siguen pendientes.

La flecha **▾/▸** de la cabecera pliega o despliega un grupo en una sola fila. Para ahorrar
espacio vertical, selecciona pistas y usa clic derecho → **Altura · compacta**, **predeterminada**,
**amplia** o **personalizada**. Solo cambia su presentación; los clips y el audio se conservan.
Estos ajustes son de sesión y acompañan las pistas al reordenar o duplicar.

[`src/ui/timeline/`](src/ui/timeline/) usa `gpui::canvas`, `Window::paint_quad` y el sistema de
texto de la revisión de GPUI fijada en Cargo. Dibuja carriles continuos, regla y cursor de edición.
Los headers y carriles comparten el desplazamiento vertical de GPUI; la regla permanece fija.

- **Click** en un carril: seleccionar pista y colocar el cursor; en la regla: colocar el cursor.
- **Rueda**: desplazar pistas; **Shift + rueda** o gesto horizontal: desplazar el tiempo.
- **Ctrl + rueda**: zoom alrededor del puntero. Sobre la regla, la rueda sola desplaza el tiempo.
- **Touchpad:** pellizcar acerca/aleja alrededor del puntero; deslizar dos dedos mueve
  tiempo y pistas, también en diagonal. Usa los gestos nativos de GPUI (Wayland/X11
  cuando el sistema los entrega), sin bibliotecas adicionales.
- En el **piano roll**, los mismos gestos mueven tiempo y registro de notas; **0** o
  **Ajustar vista** restaura el encuadre inicial. También funcionan en su ventana separada.
- **Windows/Super + dos dedos hacia abajo:** filas más altas en el piano roll.
  **Alt + dos dedos hacia arriba:** notas más anchas. El gesto contrario reduce cada
  dimensión; solo cambia la escala visual, conservando tono y duración.
- En modo NORMAL: **h/l** o flechas horizontales mueven el cursor un pulso; **j/k** o flechas
  verticales seleccionan pista; **+ / −** hacen zoom; **Home** vuelve al inicio.
- **R** o **:reload** recargan `~/.jaybu.profile`. `TIMELINE_PIXELS_PER_BEAT` define la escala
  inicial; alturas, colores y tipografía de la regla también proceden del perfil.
- **Configuración → Apariencia → Contraste de selección** ajusta y guarda la intensidad.
  En el perfil: `SELECTION_CONTRAST=0.18` (rango `0.0–0.4`; mayor valor, mayor contraste).
- **Ableton Live Dark** está en Configuración → Apariencia. Su perfil completo está en
  [`assets/themes/ableton-live-dark.profile`](assets/themes/ableton-live-dark.profile): la
  paleta y los tamaños que la UI reconoce se aplican al elegir el tema y se guardan en
  `~/.jaybu.profile`. Algunas claves tipográficas y de gaps del archivo siguen reservadas.

El proyecto comienza en **4/4 a 120 BPM**. Haz clic en el BPM o compás del transporte
para editarlos; también puedes usar `:tempo 95` y `:meter 6/8` (o abrir sus formularios
con `:tempo` y `:meter`). El click es libre, sin snapping;
el grid adapta su densidad al zoom. El cursor azul es de edición y el verde muestra la posición
enviada al dispositivo durante la reproducción (sin compensación de latencia de salida).
El efecto visual del playhead usa `particula-rs` y `fastrand`, aislados en
`ui/timeline/particles.rs`. Sus [tokens](docs/PROTOTYPE.md#partículas-tokens-opcionales)
se leen del perfil visual; no interviene en el transporte musical.

## Primer archivo reproducible

### Exportar el Arrangement

```text
:export -n 93.[mp3,wave] -master
:export -n 93.wav -individuals -sr 48000 -bits 24
```

Render offline en segundo plano mediante **libsndfile + LAME + libsoxr**; audio y MIDI,
stems alineados, rango/cola, mono/estéreo, frecuencia, profundidad y MP3 CBR/VBR configurables.
Por defecto escribe en `exports/` y respeta mute/solo y niveles de pista. `:export.help`
muestra las opciones y `:export.cancel` cancela. [Guía de exportación](docs/EXPORT.md).
Los Hilos con Hay Piano o Hay Sampler se exportan como master o stems por rama;
las ramas y cadenas VST3 todavía requieren render offline de plugins.

Al ejecutar **Exportar audio** desde el buscador o su icono aparece un formulario flotante
con los parámetros de esa exportación y botón **OK**. Parte de las preferencias guardadas;
cancelarlo o editar sus campos no cambia esas preferencias. Las cantidades muestran sus
unidades (beats, segundos, Hz), admitiendo fracciones cuando corresponde.
El contrato reutilizable está en `src/actions.rs`; **Utility · ajustar ganancia** usa el mismo
formulario para una cantidad en ×. Las invocaciones escritas siguen ejecutándose directamente.

### Importar y escuchar

Arrastra **un archivo mono o estéreo** sobre un carril. Se coloca en el punto donde lo sueltas,
se decodifica en segundo plano mediante **Symphonia 0.6.1** y aparece su waveform.
Este incremento mantiene un único clip: importar otro sustituye al anterior cuando termina de decodificarse.
Formatos habilitados: WAV/PCM, FLAC, MP3, OGG/Vorbis y AAC/M4A.

- **Space** en modo NORMAL/INSERT o **▶**: reproducir desde el cursor de edición.
  También funciona con foco en el explorador, piano roll o instrumento, incluidas sus ventanas
  separadas. Mantenerlo pulsado no repite Play/Stop; al editar texto conserva su función de espacio.
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
samples. El BPM se edita desde el transporte y se aplica al volver a reproducir;
los detalles y límites están en
[`docs/PROTOTYPE.md`](docs/PROTOTYPE.md).

## Buscador de instrumentos, componentes y acciones

El botón **`[+]`** de cada cabecera de pista, la tecla **a** o el comando **`:add`** abren
un buscador superpuesto al estilo rofi. Escribe para filtrar; **↑/↓** mueven la selección,
**Enter** abre la entrada y **Esc** cierra. La búsqueda ignora acentos.

El catálogo headless vive en [`src/catalog.rs`](src/catalog.rs) y distingue cuatro
categorías: instrumento, primitivo, componente y acción. El overlay y su estado están en
`ui/views/palette.rs` y `ui/palette.rs`; las acciones se traducen a los comandos existentes.

- **Upright Piano KW** asigna el piano a la pista seleccionada y lo muestra en Track FX;
  comparte el catálogo con Hay Sampler. La cabecera muestra `Upright Piano KW` en esa pista.
- **Volume** y **Componente serial** aparecen como metadatos: todavía no se instancian
  desde el buscador ni se asignan a una pista.
- **Reproducción**, **Detener**, **Patrón**, **Arrangement**, **Mixer** y **Recargar diseño**
  son acciones ejecutables.

El `[+]` de una cabecera apunta a esa pista; **a** y **:add** apuntan a la pista
seleccionada. Como solo existe una instancia de piano, asignarlo a otra pista lo mueve.
El buscador todavía no crea instancias nuevas ni es un host de plugins. La asignación
vive en `elevate::musical::track`, no en la vista.

Haz clic en **◈ Hay** para abrir **Configuración**. Allí se eligen el **tema de color** y
el **efecto del cursor**, con guardado automático en `~/.jaybu.profile` para conservarlos
al reiniciar y recargar. Estos controles ya no ocupan la barra superior. El panel también
incluye la ayuda de atajos y motions Vim-like; **Esc** lo cierra.

## Hay Sampler

Usa **Insertar sampler** en `[+]` o en el menú de pistas, o escribe `:sa`, para crear
una pista con Hay Sampler y abrir sus controles. Si ya existe una pista con el
instrumento, la acción abre esa pista: por ahora hay una instancia por sesión.

Desde el [+] de una pista elige **Hay Sampler** (o `:sampler`) y carga/arrastra audio al
panel. Waveform, Classic/1-Shot, inicio/final/loop, afinación, voces, filtro LP12 y ADSR,
con controles y estética de Hay. Las notas del teclado y piano roll utilizan sfizz;
la exportación captura el mismo sample y parámetros. Una instancia por sesión; Warp,
Slice y mezcla multipista live siguen pendientes. [Uso y construcción nativa](docs/SAMPLER.md).

## Primer instrumento: piano

El filtro del piano muestra un panel `<` `>` que alterna espectro de salida con RealFFT
y visualizador de forma de onda. La guía de frecuencia de corte aparece en Espectro.
El panel es reutilizable para futuros dispositivos y cabeceras de pista.

El piano aparece como instrumento de la pista en Track FX. Al seleccionarlo se abre su
editor bajo el Arrangement; la tecla **4** abre ese editor para la pista activa.
Usa **FluidSynth** cuando está disponible, con **RustySynth 1.3.8** como respaldo,
y el banco **Upright Piano KW** de FreePats,
incluido en [`assets/soundfonts/UprightPianoKW-20220221.sf2`](assets/soundfonts/) bajo **CC0**.

**Créditos:** *Upright Piano KW* es un Kawai vertical grabado en enero de 2017 por Gonzalo y Roberto
(proyecto [FreePats](http://freepats.zenvoid.org/Piano/acoustic-grand-piano.html#UprightKW)),
con la colaboración de Inma Martínez de Miguel, que cedió su piano. Publicado bajo CC0.
RustySynth es solo el motor: no incluye instrumentos propios.
El banco y el motor se preparan en segundo plano **al abrir el editor o solicitar audio**;
el arranque no carga SoundFonts. `HAY_SOUNDFONT` añade un banco al descubrimiento.

El panel sigue la línea visual de **Hay Sampler**: fuente/preset, afinación y voces,
filtro, efectos y salida, con perillas compactas y teclado. **Bancos** y **Créditos**
despliegan información adicional. Arrastra o usa la rueda; **Shift** permite precisión
y **doble clic** restablece valores. Los controles conservan MIDI Learn.

El bloque **Filtro estéreo** ofrece **LP 12 / HP 12 / BP 12**, frecuencia y resonancia;
su botón ON/OFF permite comparar. Procesa la salida del instrumento, también al exportar.
Durante la primera preparación aparece «Preparando SoundFont…»: solo se disparan las notas
que sigan pulsadas al terminar; Stop cancela un Play pendiente.

El editor no conoce RustySynth ni CPAL: pinta el descriptor del catálogo y envía notas a
[`src/instruments/piano.rs`](src/instruments/piano.rs), que posee el reproductor, las notas
sostenidas y la nota del ratón. La primera nota abre el dispositivo.

```text
Acción: Note On / Note Off
             │
             ▼
  instruments::Piano (notas sostenidas)
             │
             ▼
  audio/piano (RustySynth)
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
- **Space** reproduce/detiene el transporte; **Esc** libera las notas del instrumento.
  **b** compara bypass y **[/]** ajustan Volume.
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

### Mezcla de clips de audio (PR-04)

Play en contexto de audio reproduce los clips de Arrangement en una salida compartida.
Cada pista suma sus clips antes de su cadena FX; el master conserva margen flotante
hasta la salida física. Para probar, coloca dos WAV en pistas distintas al mismo beat,
selecciona un clip de audio y reproduce desde el inicio. MIDI y el patrón Session aún
usan sus rutas separadas. `:transport.seek 8` salta a la negra 8 durante Play;
`:transport.loop 0 8` repite el rango y `:transport.loop off` lo desactiva. El botón
**↻ Audio** alterna el loop de todo el arreglo; Home/⏮ vuelven a cero y Stop conserva
la posición observada. Los componentes Lua se preparan a la frecuencia real de salida,
también en Auto. Detalles en `docs/PROTOTYPE.md`.
