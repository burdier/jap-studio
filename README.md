> [!IMPORTANT]
> **FILOSOFÍA FUNDAMENTAL DE HAY / ELEVATE — mantener siempre al inicio de este README.**
> Hay se construye como un sistema basado en **plugins**, empezando por Volume.
> Los **componentes** componen **primitivos**; los **mapeos son conexiones reales** que afectan
> al procesamiento; toda operación ejecutable se expresa como una **acción**, al estilo REAPER,
> y sus resultados y cambios se comunican mediante **eventos**.
> Esta filosofía guía tanto las piezas ya integradas (acciones comunes, MIDI Learn/automatización
> unificados, primitivos DSP y componentes Lua) como la evolución futura hacia un registro
> general de plugins, fuentes LFO libres y eventos observables completos.

# Jap Studio (Hay / Elevate)

DAW nativo y de código abierto escrito en **Rust + GPUI**, pensado para producir, diseñar sonido,
mezclar y colaborar **sin saltar de pantalla**. Disponible en **Linux** y **Windows** (con base
multiplataforma para **macOS**).

- **Landing interactivo:** [`landing/index.html`](landing/index.html)
- **Video de release:** [`landing/video.html`](landing/video.html) (escenas animadas con los dibujos del landing);
  `python3 scripts/render_video.py` lo exporta a `dist/video/jap-studio-release.mp4` (Chrome headless + `ffmpeg`)
- **Visión original:** [`SPEC.md`](SPEC.md)
- **Documentación técnica:** [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) · [`docs/EDITING_GUIDE.md`](docs/EDITING_GUIDE.md) · [`docs/PROTOTYPE.md`](docs/PROTOTYPE.md) · [`AGENTS.md`](AGENTS.md)

---

## Instalación y uso rápido

### Linux (Flatpak único)

Empaquetado como un solo archivo `.flatpak` con el binario release, `libsfizz`, **23 bancos SoundFont**
incluidos, colaboración **P2P** activada por defecto y servidor **MCP**:

```bash
flatpak install --user dist/jap-studio-0.8.0-x86_64.flatpak
flatpak run io.github.burdier.JapStudio
```

Para construir el bundle Flatpak localmente tras compilar en release:

```bash
cargo build --release --features ui,p2p
bash scripts/build-flatpak-bundle.sh
```

### Windows x64 (paquete portable)

1. Descarga o genera `dist/JapStudio-Windows-x64.zip`.
2. Descomprime la carpeta `JapStudio-Windows-x64` y ejecuta `jap-studio.exe`.
   Usa **WASAPI** nativo, exportación **WAV** integrada y puente **MCP** por TCP local (`127.0.0.1`).

Para cross-compilar el paquete Windows desde Linux (con `cargo-xwin`):

```bash
bash scripts/build-windows.sh
```

También existe un workflow automatizado en [`.github/workflows/build-windows.yml`](.github/workflows/build-windows.yml).

### Desde el código fuente (desarrollo)

En Fedora / `gpui-dev` (con `pipewire-devel`, `clang-devel` y `alsa-lib-devel` instalados):

```bash
# Arrancar el DAW
bash scripts/run.sh

# Verificación completa (formato, tests headless, tests UI y check)
distrobox enter gpui-dev -- bash scripts/check.sh

# Solo biblioteca DSP headless (elevate)
distrobox enter gpui-dev -- cargo test --no-default-features --lib --tests --examples
distrobox enter gpui-dev -- cargo run --no-default-features --example volume
```

---

## Qué incluye hoy Jap Studio

### Instrumentos nativos y host VST3

- **Hay Synth (Voice Lab):** sintetizador nativo Hi-Fi *band-limited* (0 aliasing en RT) con controles
  de **Espacio, Cuerpo e Intención**, **Mod Matrix de 12 slots**, legato real, micro-fade anti-click,
  visor vivo de osciloscopio/transiente/ADSR y barra clara de flujo de señal.
  Importa presets y bancos **`.vital` / `.vitalbank`** y **`.fxp` (Serum)** con sus wavetables reales
  (codificadas de forma compacta y persistidas en `.loro` y P2P), con navegador separado por origen.
- **Hay Piano (SoundFont):** motor **FluidSynth** (con respaldo **RustySynth 1.3.8**), múltiples
  instancias independientes por pista, filtro estéreo LP/HP/BP con espectro RealFFT y carga diferida
  en segundo plano. Incluye **Upright Piano KW** (CC0, FreePats) y un pack de bancos FreePats en el
  bundle Flatpak (`HAY_SOUNDFONT` o **Configuración → Rutas** para añadir tus `.sf2`).
- **Hay Sampler (`:sa` / `:sampler`):** basado en **sfizz**, múltiples instancias por pista, modos
  **Classic / Loop / 1-Shot**, filtro LP/HP/BP con curva interactiva, ADSR, detección automática de
  nota raíz y pestaña **MPC Chop** (corte por transientes o rejilla 4/8/16 directo a los pads con Choke).
- **Drums y Pads MPC 4×4 (`F2`):** secuenciador nativo con **motor PCM bit-exacto sin clics**
  (fade anti-click en choke y filtro TPT SVF), vista **Pasos** con sombreado proporcional al micro-timing
  offset, **Swing** por patrón (`0–75%`), probabilidad y pitch rápidos por paso, matriz **Canción**
  estilo Hydrogen, kits `.haykit` e importación de proyectos Hydrogen (`.h2song` / `drumkit.xml`).
  Su vista hermana **Pads MPC 4×4** ofrece bancos A–D, mapeo QWERTY 4×4 (`ZXCV/ASDF/QWER/1234`),
  MIDI 36–51, **Full Level**, **16 Levels** (velocity/pitch), **Note Repeat RT** por frame y **Overdub**.
- **Host VST3:** carga instrumentos y efectos VST3 nativos (y plugins de Windows en Linux mediante
  **Wine/yabridge**, con Pianoteq y Cathar Restore validados), abre su editor original en X11/XWayland
  o Windows conservando el teclado musical del DAW, compensa latencia (**PDC**) en ramas paralelas y
  pistas, expone `last_touched` para MIDI Learn/automatización y persiste el estado en `.loro`.
  [Alcance y límites VST3](docs/VST3.md).

### Arrangement, Audio, MIDI y Hay Chord

- **Mezcla multipista real:** pistas dinámicas, grupos plegables con rack FX propio, **buses auxiliares
  Send / Return** (con *Solo Safe*), **Freeze / Unfreeze** de pistas a WAV 32-bit float y vista **Mixer**
  (`F3`) con medidores post-fader.
- **Edición de audio en contexto:** importación por arrastre o explorador (WAV/PCM, FLAC, MP3, OGG/Vorbis,
  AAC/M4A con **Symphonia 0.6.1**), **Warp por clip estilo Ableton** (`Off | Re-Pitch | Beats | Complex`
  con estimación automática de BPM y seguimiento al cambiar el tempo), **Audio → MIDI polifónico**
  (Basic Pitch en segundo plano), filas **Z** para clips solapados, **crossfades** arrastrables,
  ganancia directa sobre la onda (`Ctrl+Shift+arrastrar`), estimación de fundamental en Hz/nota/cents
  bajo el puntero y editor contextual de WAV con EQ de campana y procesos de rango.
- **Transporte vivo y grabación:** cambio de **BPM en tiempo real sin pausar**, **Tap Tempo** (`:tap`),
  **Monitoreo de entrada en vivo (`MON: Off | IN | AUTO`)** pasando por toda la cadena FX de la pista
  sin bloqueo RT (`:mon`, `:mon.auto`, `:mon.off`), grabación de audio y medidor LUFS en tiempo real.
- **Piano Roll y Hay Chord:** edición MIDI con ratón/teclado, **Pitch MPE** por nota, carriles reales
  de velocity, CC, pitch bend, presión, release velocity, probabilidad y pan por nota, empuje Lua entre
  notas y ventana acoplada o separada. Incluye **Hay Chord** con configuración individual por pad
  (color, voicing, bajo, inversión), audición en vivo y recuperación del MIDI tocado directo al clip.
- **MIDI Hardware y Mapeo unificado:** detección **hot-plug automática** de controladores USB con visor
  vivo de nota/octava (`C1..C9`) y pitch bend, más **MIDI Learn** y **Automatización** por IDs estables
  para volumen, pan, envíos Return, FX nativos/Lua y parámetros VST3 (`:midi.map`, `:learn.vst`, `:auto.vst`).

### Efectos nativos, Sidechain, Tejer y Scripting Lua

- **Suite FX nativa (Track FX en árbol serie/paralelo):**
  - **Hay Comp:** modos `Peak`/`RMS`, soft knee, curva interactiva, medidor GR y **Sidechain real**
    (`Audio From: Off | Drums | Pista …`, también enrutable al bus auxiliar de plugins VST3).
  - **Hay Limiter:** techo, release, lookahead (`1.5 / 3.0 / 6.0 ms`) y medidor GR.
  - **Hay Echo:** `Stereo` / `Ping Pong` / `Mid/Side`, modo **`Time | Sync`** al BPM, filtros HP/LP,
    saturación de cinta (`Drive`) y modulación (`Wobble`).
  - **Hay Reverb:** FDN Hadamard estéreo de 8 canales, modo **`Time | Sync`**, damping y **`Freeze`** infinito.
  - **Hay Filter:** filtro SVF FunDSP (LP/HP/BP) con respuesta en frecuencia interactiva.
  - **Hay Spectrum:** analizador RealFFT insertable en cualquier punto de la cadena (`Spectrum → VST3 → Spectrum`)
    con modos espectro, onda, espectrogramas log/lineal y nivel estéreo (`< >`). [Detalles](docs/VISUAL_MODULATION.md).
  - **Hay Utility** y **Tejer EQ:** división multibanda de una pista en filas Z con FX independientes por banda.
- **Hilos y Tejer:** agrupa 2–16 instrumentos en un Hilo con reparto en **capas** o **round robin**
  (`Tejer pistas seleccionadas`, secuencia editable `1,3,2` o script Lua opcional), tanto en MIDI en vivo
  como en clips y exportación. [Contrato Tejer](docs/TEJER.md).
- **Lua 5.4 fuera de RT:** herramientas MIDI recargables (`:midi.tools.reload`, `:midi.tool`, `:midi.script`),
  lotes atómicos de pistas (`:track.script`, `:track.template.two`), componentes de audio Lua v2
  (`:fx.components.reload`) y transformaciones previas a Play (`HAY_LUA_PREPLAY_SCRIPT`).
  [API MIDI Lua](docs/LUA.md) · [Lotes de pistas](docs/LUA_TRACK_BATCHES.md) · [Componentes DSP Lua](docs/LUA_AUDIO_COMPONENTS.md).

### Proyectos `.loro`, Colaboración P2P, Explorer acústico e IA (MCP)

- **Proyectos `.loro` (Loro CRDT):** `Ctrl+O` / `Ctrl+S` / `Ctrl+Shift+S` (`:project.open`, `:project.save`,
  `:project.save_as`, `:project.reveal`). Guarda Arrangement, pistas/grupos/Hilos, clips audio/MIDI,
  Warp, crossfades, Tejer EQ, Drums, FX nativos y VST3, Hay Piano, Hay Synth (incluidas wavetables),
  envíos Return, automatización y MIDI Learn. `:audio.collect` reúne fuentes en una carpeta autocontenida.
- **Colaboración P2P en vivo (activada por defecto):** **Archivo → Compartir sesión** genera un token
  `hay-live:...` para conectar otra ventana directamente vía **Loro CRDT + Iroh QUIC** sin servidor central.
  Transfiere automáticamente assets de audio WAV/samples (content-addressed) y wavetables, resuelve
  SoundFonts de forma portable, ofrece *ForceSync* desde el host y aplica bypass elegante en plugins VST3
  ausentes. [Detalles P2P](docs/LORO_SESSION.md).
- **Explorador de bibliotecas con analizador acústico (`e`):** árbol virtualizado con búsqueda (`Ctrl+F`),
  preescucha instantánea (`Auto` / `→`) y **analizador acústico DSP offline** (0% impacto RT) que mide
  fuerza de transiente, ataque, decay, brillo espectral, energía en 4 bandas (**Sub / Grv / Med / Agd**)
  y tono fundamental.
- **Servidor MCP nativo (`--mcp`):** expone **22 herramientas** JSON-RPC sobre stdio (conectadas al DAW
  abierto por socket Unix en Linux o TCP local en Windows) para que agentes de IA (Gemini CLI, Claude,
  etc.) exploren tus bibliotecas, analicen samples acústicamente, construyan kits/patrones/MIDI,
  ajusten parámetros de FX y exporten audio en vivo:
  ```bash
  # Linux (Flatpak)
  flatpak run io.github.burdier.JapStudio --mcp
  # Windows
  jap-studio.exe --mcp
  ```
- **Export Pro (EBU R128):** render offline en segundo plano a WAV/MP3 y stems alineados (`:export`),
  con medición `LUFS-I`, `dBTP` y `LRA`, normalización de sonoridad en 2 pasadas (`-lufs -14|off`) y
  limitador True Peak 4× (`-tp -1|off`). [Guía de exportación](docs/EXPORT.md).

### Interfaz, Audio, Rutas y Teclado

- **Audio multiplataforma:** **PipeWire** (compartiendo la interfaz con Linux) y **ALSA** en Linux;
  **WASAPI** en Windows; **CoreAudio** en macOS. Controlable desde **Configuración → Audio** o con
  `:list device` y `:use device [input|output] <número|nombre|default>`.
- **Rutas centralizadas:** **Configuración → Rutas** gestiona las carpetas de **VST3**, **VST2**,
  **CLAP**, **SoundFonts**, **Presets Synth (`.vital`/`.fxp`)**, **Samples** y **Drum Kits (`.haykit`)**
  en `<config>/hay-daw/paths.profile`.
- **Espacio de trabajo adaptable:** 4 temas integrados (**Cyberpunk Studio**, **Daylight Studio**,
  **Cuaderno Claro** y **Cuaderno Oscuro**, inspirados en [`landing/index.html`](landing/index.html)),
  contraste de selección ajustable, navegación directa por pestañas (`F1` Arrangement, `F2` Drums,
  `F3` Mixer), ampliación de panel (`f` en Normal o `F11`), paleta de comandos (`:` / `Ctrl+P`),
  buscador universal (`[+]` / `A` / `:add`) y navegación **Vim opcional** (`:vim.toggle`).
- **Créditos de SoundFont incluido:** *Upright Piano KW* es un Kawai vertical grabado en enero de 2017
  por Gonzalo y Roberto ([FreePats](http://freepats.zenvoid.org/Piano/acoustic-grand-piano.html#UprightKW)),
  con la colaboración de Inma Martínez de Miguel, publicado bajo licencia **CC0**.

---

## Principios y arquitectura de Elevate

1. **Plugins desde la base.** Un plugin aporta capacidades al DAW: primitivos, componentes,
   parámetros y acciones. Los plugins propios deben usar el mismo contrato que las extensiones.
2. **Componer en lugar de duplicar.** Los componentes conectan instancias de primitivos con identidad
   y parámetros direccionables.
3. **Primitivos y parámetros son distintos.** Un primitivo procesa señal o control; los parámetros
   gobiernan esa operación y las perillas/faders son solo su representación visual.
4. **Mapping real.** Un mapeo conecta origen y destino con rango, profundidad y valor efectivo en DSP,
   independientemente de que la UI esté abierta.
5. **Acciones como lenguaje de operación.** Teclado, ratón, barra `:`, scripts Lua y servidor MCP
   llegan a los mismos controladores y acciones.
6. **Eventos como lenguaje de observación.** Las operaciones emiten cambios y errores observables;
   la señal continua DSP/control corre en bloques de audio sin convertirse en eventos de UI por muestra.
7. **Fronteras limpias.** `src/musical/` no importa GPUI ni CPAL; `src/audio/` no importa GPUI;
   `src/instruments/` posee el estado de ejecución sin importar GPUI; `src/catalog.rs` y `src/paths.rs`
   son headless; el callback RT nunca hace E/S ni reservas costosas (`rtrb` + `Publication`).

### Diagrama conceptual — arquitectura objetivo

```text
Teclado / ratón / comandos : / scripts Lua / MCP (IA)
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
        Plugin Volume          Componente (p. ej. jeyeq / FX)
        └─ Primitivo Volume    ├─ Primitivo filtro
           └─ gain             │  ├─ frecuencia
                               │  └─ res ◄──────────────┐
                               └─ Primitivo Volume      │
                                  └─ gain               │
                                                        │
        Plugin LFO → Primitivo LFO → MAPEO REAL ────────┘
                                    rango / profundidad

    Audio: entrada → [filtro → Volume] → salida
    Control: salida LFO → mapeo → parámetro res del filtro

    Acciones ejecutadas / cambios / errores
                         │ emiten eventos
                         ▼
                UI / observadores / debugger
```

### Mapa del código y estado frente a la filosofía

| Pieza | Ubicación principal | Estado actual |
| --- | --- | --- |
| Núcleo musical headless | [`src/musical/`](src/musical/) | Tiempo, pistas, grupos, Hilos, Tejer, clips, MPE, automatización, MIDI Learn, Drums, acordes |
| Primitivos DSP (FunDSP) | [`src/primitives/`](src/primitives/) | `Volume`, `Filter` (SVF), `Compressor`, `Limiter`, `Echo`, `Reverb`; bloques `[f32; 2]` sin reservas RT |
| Dispositivos headless | [`src/devices/`](src/devices/) | `Utility`, `Filter`, `Sampler`, `Spectrum`, `Synth` (Voice Lab + `.vital`/`.fxp`), `AudioComponent` |
| Motor de audio y mezcla | [`src/audio/`](src/audio/) | Mezcla multipista, PDC, Sidechain, Send/Return, Warp, VST3, SoundFont, sfizz, PCM Drums, Export EBU R128 |
| Proyecto y P2P | [`src/project/`](src/project/), [`src/collaboration/`](src/collaboration/) | Esquema Loro CRDT, `audio.collect`, colaboración P2P en vivo con Iroh + transferencia de assets |
| Scripting Lua y MCP | [`src/scripting/`](src/scripting/), [`src/mcp.rs`](src/mcp.rs) | Herramientas MIDI, lotes de pistas, componentes DSP v2, Tejer y servidor MCP de 22 herramientas |
| Interfaz GPUI | [`src/ui/`](src/ui/) | Arrangement, Piano Roll, Hay Chord, Drums/MPC, Mixer, Track FX, Explorer acústico, 4 temas Studio/Cuaderno |
| Siguientes pasos | [`docs/PROTOTYPE.md`](docs/PROTOTYPE.md) | Registro general unificado de plugins (VST2/CLAP), LFO libre enrutable y eventos/debugger observables |

### Recorrido rápido por el motor headless (`elevate`)

| Archivo | Qué explica |
| --- | --- |
| [`src/primitives.rs`](src/primitives.rs) | Contrato `Primitive` y metadatos de parámetros |
| [`src/primitives/amplitude.rs`](src/primitives/amplitude.rs) | Implementación de `Volume` con FunDSP y suavizado sin clics |
| [`src/component.rs`](src/component.rs) | Composición de instancias con `NodeId` estable y `ControlEvent` |
| [`examples/volume.rs`](examples/volume.rs) | Demo offline de procesamiento en serie (`0.800 → 0.400 → 0.200`) |
| [`tests/elevate.rs`](tests/elevate.rs) | Pruebas del contrato DSP, bloques, suavizado, bypass y errores |
