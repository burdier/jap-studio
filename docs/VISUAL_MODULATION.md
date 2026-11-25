# Visualizaciones de pista y modulación

## Incremento implementado: Hay Spectrum

En `[+]`, busca **Hay Spectrum**, o ejecuta `:fx.spectrum`. Se inserta después del
nodo seleccionado (dentro si es contenedor) y aparece en el árbol FX existente.
Puedes organizar `Spectrum → VST3 → Spectrum` y comparar ambos paneles debajo del
árbol. Renombrar, duplicar, mover, eliminar y bypass usan las mismas acciones FX.
`< >` recorre espectro, onda, espectrograma logarítmico, espectrograma lineal y
nivel peak/RMS; `↺` borra las mediciones de esa instancia. Acciones:
`fx.spectrum.next`, `fx.spectrum.previous`, `fx.spectrum.reset`.

Las vistas de espectrograma se inspiran en
[resonators](https://github.com/jhartquist/resonators); se implementan con el RealFFT
ya instalado, sin añadir esa dependencia ni afirmar equivalencia con Resonate/CQT.
FFT de 2048 muestras: la escala logarítmica no mejora su resolución en graves.
Tiempo horizontal y frecuencia vertical; rango de intensidad −80…0 dBFS, colores
del tema. La historia cubre 128 ventanas: su duración cambia con la frecuencia real.
El nivel es peak/RMS por canal, sin calibración VU ni LUFS. El piano usa el mismo panel.

Funciona en las rutas actuales de clips de audio, piano, sampler e instrumento VST3.
Se rechaza insertar en grupos porque aún no existe su bus de audio; la audición
independiente del patrón de percusión no pasa por Track FX. No añade mezcla multipista,
sidechains, vistas en cabeceras, una ruta libre LFO ni un nuevo transporte. Los nodos
se persisten con Loro; los modos, historial y captura permanecen como estado visual.
La cola omite mediciones si se llena y descarta el historial discontinuo, conservando audio.
Para cadenas Utility sin VST3 se conserva su rampa escalar de salida y se observa una
copia con las ganancias objetivo, evitando cambiar el nivel al insertar el analizador.

Compilación validada con `cargo check --all-targets --locked`; sin ejecutar tests.

## Spectro independiente (diseño de próximos incrementos)

**Spectro** será un dispositivo Hay de análisis, no una parte del piano. Se coloca en
un punto de la cadena de una pista o un bus, o se conecta como tap sin alterar el
audio. Recibe audio estéreo y, opcionalmente, una señal de control para superponer
una curva LFO/automatización. Sus módulos de medición producen snapshots de
espectro, forma de onda, peak/RMS y, después de definir integración y calibración,
LUFS. Su panel actual pasa a ser una vista del dispositivo; el mismo snapshot puede
mostrarse en cabeceras de pista, cabeceras de canal del mixer o ventana ampliada.
La gráfica de respuesta de un EQ
proviene de los parámetros del EQ y se dibuja como otra capa, no de Spectro.

La conexión se expresa con puertos tipados y ubicaciones estables:

```text
SignalPort = Audio(channels) | Note | Control | Telemetry
SignalSource = Track(TrackId) | Bus(BusId) | DeviceOutput(DeviceId, PortId)
SignalRoute { source, destination: (DeviceId, PortId) }
ProcessEvent { frame_offset, ParameterValue | ParameterModulation | Note | Transport }
```

La edición de rutas y topología ocurre fuera del callback. El motor recibe un plan
preparado, procesa por bloques/frames y publica telemetría acotada hacia la UI; no
envía GPUI ni memoria dinámica al audio. `Device::process(&mut [StereoFrame])` y la
cadena FX actuales no tienen puertos ni entradas múltiples: habilitar rutas libres,
sidechains y Spectro en cualquier punto **sí exige ampliar esa infraestructura**.
El primer incremento puede ser un tap estéreo pre/post FX sin introducir el grafo
general completo.

Este contrato interno se inspira en los buses/eventos de **CLAP** y **VST3**, pero no
usa sus tipos dentro de `musical` o de la UI. El host traduce puertos de audio,
eventos de nota/transport y parámetros de cada formato al contrato de Hay. CLAP
distingue valor base y modulación de parámetro; en VST3 el adaptador entregará los
cambios mediante sus colas de parámetros del bloque. Los IDs nativos del plugin se
guardan en el adaptador, vinculados a IDs estables del proyecto.

## Decisión de interfaz

Un espacio visual opcional por cabecera de pista. El usuario elige **Ninguno**, **VU/peak**,
**Espectro**, **Forma de onda**, **Curva EQ** o **Curva de modulación** desde el menú de pista.
Los modos comparten el marco y los controles del panel `ui/components/spectrum.rs`, pero
cada uno recibe datos distintos. El espacio se oculta al compactar una pista y puede
ampliarse con la altura de pista existente. La elección es por `TrackId`, no por fila.

El panel no consulta CPAL, `Piano`, `FxChain` ni el callback. Consume un snapshot visual:
modo, valores, unidades, color, estado de disponibilidad y revisión. Una pista sin toma
de audio disponible muestra «Sin señal disponible», nunca un medidor ficticio.
Las preferencias de aspecto global van al perfil; la elección por pista pertenece al
proyecto cuando exista persistencia de proyecto. Mientras tanto vive en
`TrackPresentation` durante la sesión.

La misma ranura compacta existe **dentro del Track header y en la cabecera del canal
del mixer**: `Track header [Nivel/Visualizador]` y `Mixer channel [Insertar FX visual]`.
**Insertar FX visual** añade Spectro a la cadena de esa pista y muestra su vista;
si ya está presente, enlaza la vista existente. Cada superficie guarda su modo y
tamaño, pero ambas se suscriben al mismo `DeviceId` y snapshot. Cambiar de Espectro
a Nivel no crea otro procesador ni altera el audio. El botón de inserción utiliza
la acción/catálogo FX ya existentes, y más adelante el adaptador de host permitirá
mostrar la telemetría de plugins CLAP/VST3 cuando la expongan; la UI conserva el
mismo contrato interno de puertos y eventos.

## Datos de audio

- **VU/peak:** peak y RMS por canal, medidos después de la cadena de la pista. Un VU
  necesita referencia/calibración y tiempo de integración propios; hasta definirlos,
  la etiqueta visible debe ser «Nivel»/«Peak», no VU.
- **Espectro/forma de onda:** reutilizar `audio/analyzer` y su cola acotada; la FFT y la
  reducción de datos quedan fuera del callback. Activar tomas solo para vistas visibles.
- **Curva EQ/filtro:** calcular respuesta desde los parámetros del procesador, aun sin
  audio. El espectro medido puede aparecer detrás como otra capa opcional.
- El piano ya proporciona espectro. Otros tipos de pista esperan una ruta de mezcla
  común con medición por pista; no conectar una cabecera a la salida de otra pista.

## Contrato de modulación

El MIDI Learn actual ya asigna notas y CC a varios controles del mixer y del piano.
Para ampliar el mapeo, un botón físico dispara una acción en su flanco de pulsación
y una perilla escribe el valor base de una dirección de parámetro. Un LFO puede
apuntar a esa misma dirección con su propia profundidad. Loro guardará la ruta,
la división rítmica y la profundidad. Cada motor evaluará la curva con su reloj de
audio, también al exportar. Así bastará enviar una edición al cambiar el LFO.

Una fuente LFO no es un efecto de audio ni una visualización. Vive en el estado de
proyecto y produce un control bipolar normalizado `[-1, 1]` según frames del transporte.
Su identidad es estable (`ModSourceId`), igual que la dirección de destino:

```text
ParameterAddress { track: TrackId, device: Instrument(InstrumentId) | Fx(FxId) | Track,
                   parameter: ParameterId }
ModRoute { source: ModSourceId, target: ParameterAddress,
           depth: -1..1, enabled: bool }
Lfo { shape, rate: Hertz | BeatDivision, phase, retrigger, polarity }
```

El parámetro conserva un **valor base**. En cada intervalo de control, el motor suma la
modulación en dominio normalizado, limita al rango permitido y convierte a unidades
reales. Para cutoff la conversión debe ser logarítmica; booleanos y enumeraciones no
se modulan en el primer incremento. Los cambios de rutas se preparan fuera del hilo
de audio y se publican con el mecanismo seguro del motor. La UI dibuja por separado
el valor base, el rango modulado y el valor efectivo.

Para una curva dibujada por el usuario, la forma LFO puede ser una lista inmutable de
puntos `(fase 0..1, valor -1..1)` con interpolación declarada. La edición crea una
nueva versión; el motor no recibe puntos mutables desde GPUI. La gráfica del panel
puede mostrar la forma prevista sin esperar muestras de audio.

## Interacción propuesta

1. Crear **Hay LFO** en el árbol de dispositivos/moduladores de la pista.
2. Arrastrar su manejador al control de un parámetro o elegir **Mapear LFO** desde el
   menú del parámetro. Ambas entradas ejecutan la misma acción de ruta.
3. Mostrar un aro/indicador junto al control; clic abre profundidad bipolar,
   velocidad/división, forma y opción de quitar la ruta.
4. Seleccionar **Curva de modulación** en el espacio visual de la cabecera muestra
   la forma y el recorrido del parámetro elegido. La vista solo observa el estado.

## Librería LFO

`fundsp 0.23`, ya dependencia del proyecto, ofrece `prelude32::lfo`, un generador de
control basado en una función del tiempo, con interpolación entre muestras de control.
Es útil para prototipar formas libres, y su licencia es MIT/Apache-2.0. No incluye la
identidad de rutas, la conversión de parámetros ni la sincronización al transporte de
Hay. No añadir otra dependencia ahora; evaluar `fundsp` frente a un evaluador pequeño
propio cuando se implemente la primera ruta sincronizada por frames.

## Orden de implementación

1. Extraer el panel actual a una variante compacta y añadir la elección visual por
   `TrackId` en el menú de pista. Conectar solo datos realmente disponibles.
2. Incorporar medición peak/RMS a cada pista cuando el motor tenga una ruta de mezcla
   común; después habilitar Nivel, espectro y forma de onda en todas las cabeceras.
3. Definir `ParameterAddress` y conversión normalizada de parámetros; implementar un
   LFO seno libre → cutoff del filtro como primera ruta completa.
4. Añadir divisiones musicales, formas adicionales y editor de curva; habilitar el
   mapeo visual y la curva de modulación en cabecera y en Hey EQ.
