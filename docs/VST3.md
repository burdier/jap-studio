# Host VST3: instrumentos y efectos

Inicia Hay desde el acceso del escritorio o `bash scripts/run.sh`. En una pista MIDI,
abre `[+]` → Instrumentos, busca **Pianoteq** y pulsa Enter. La carga ocurre en background;
la pista se asigna solo después de preparar el plugin y abrir la salida correctamente.
La fila del instrumento aparece en el árbol FX y abre su **editor original** en una
ventana X11/XWayland. Al cargarlo también se solicita abrir esa ventana. Cerrarla no
elimina el instrumento ni detiene su audio.
`M` activa el teclado MIDI; el controlador físico usa la misma ruta. Espacio reproduce
el clip MIDI seleccionado y vuelve a detenerlo. Volumen, pan, mute/solo y Hay Utility
usan la mezcla y el scheduler existentes.

Acciones: `instrument.vst3.load 'ruta/Plugin.vst3'` (formulario de ruta desde catálogo),
`instrument.vst3.remove`, `instrument.vst3.editor` y `instrument.vst3.editor.close`. Cada pista MIDI
puede conservar un VST3 distinto. Seleccionar una pista cambia el editor y el destino
MIDI; retirar el instrumento de una pista conserva las demás instancias. Play
procesa los VST3 cargados en el mismo transporte y salida que WAV y los demás
instrumentos MIDI, con el sonido ya editado en cada ventana. Stop devuelve cada
VST3 a su audición en vivo.

Para comprobar la separación, carga el mismo instrumento en dos pistas MIDI,
selecciona cada una y modifica un parámetro distinto en su editor. Toca una nota
en la primera, cambia de pista antes de soltarla y verifica que la liberación
llega a la primera. Quita la segunda instancia y comprueba que la primera aún
suena. Esta comprobación requiere plugins y una salida de audio reales.

Para comprobar Play común, coloca clips MIDI en ambas pistas VST3 y un WAV en una
tercera. Pulsa Play, cambia un control en cada editor y prueba mute/solo, seek y
loop; todo debe seguir el mismo cursor. El probe sin salida física ejercita el
cambio de ruta del procesador con `cargo run --example vst3_probe --
"$HOME/.vst3/yabridge/VST3/Pianoteq 9.vst3" --shared` desde `gpui-dev`.
La prueba opcional `two_real_vst_instruments_share_one_mix_with_wav` requiere
`HAY_VST3_TEST_BUNDLE` y procesa dos instancias reales más un WAV sin abrir CPAL.

## Efectos VST3 y Cathar Restore

`[+]` descubre clases de instrumentos y efectos mediante `IPluginFactory2`, sin una
lista de nombres permitidos. Los efectos aparecen como Componentes. Cada bundle se
consulta en un proceso `--vst3-inspect` separado con timeout de 30 segundos; fallos
se registran en consola, sin cargar la fábrica en el proceso de UI durante el escaneo.
El catálogo sigue enumerando metadatos/archivos: no instancia procesadores.

Selecciona una pista, abre `[+]` y elige **Cathar Restore**. Se inserta en el contexto
actual del árbol FX, abre su interfaz original y procesa el audio de esa pista.
Doble clic/Enter abre de nuevo el editor; B y el menú permiten bypass; Delete elimina.
Serie y paralelo conservan su orden y topología. No hay controles específicos de Cathar.

Acciones: `fx.vst3.load 'ruta/Plugin.vst3'` y `fx.vst3.editor`. La primera también tiene
formulario de ruta en el catálogo. En bundles con varios efectos, la acción por ruta
elige el primero; el catálogo permite elegir cada clase por su ID. Windows requiere
el wrapper Linux de yabridge; pasar directamente una DLL muestra esa indicación.

Se copió el archivo del usuario al directorio VST3 del prefijo `.wine-vst` ya registrado
y se sincronizó yabridge. Bundle disponible: `~/.vst3/yabridge/Cathar Restore.vst3`.
No se añadieron crates ni se instaló otra biblioteca de hosting.

`audio/effects` prepara un plan inmutable de Utility/serie/paralelo/VST3 fuera del
callback, conservando instancias por FxId al renombrar, mover y cambiar bypass. RCU
publica el plan y retira el anterior en el escritor; el lector no toma mutexes ni
reserva/destruye planes. Cada instancia protege su procesador mediante CAS sin espera
ante uso concurrente accidental (silencia y marca fallo). El editor conserva su hilo
propietario y publica parámetros por la cola SPSC existente. La entrada y salida
principal de efectos son estéreo f32; buffers auxiliares desactivados. Los streams de
archivos, piano, sampler y VST reciben la ruta antes de volumen/pan/Utility master.
En Play compartido, los FX VST3 reciben posición absoluta en samples y negras, BPM,
compás y loop mediante `ProcessContext`; seek y loop se reflejan por bloque. Solo se
marcan los campos disponibles. La audición live todavía no entrega tiempo musical.
Contrato de flags: [Steinberg ProcessContext](https://steinbergmedia.github.io/vst3_doc/vstinterfaces/structSteinberg_1_1Vst_1_1ProcessContext.html).

Límites de este incremento: no buses VST3 de grupo ni FX en la audición del patrón de
percusión provisional, compensación de latencia, sidechains, automatización/Lua de
parámetros, ni persistencia del estado binario del plugin. Duplicar un nodo VST3 no
clona todavía la instancia: la validación rechaza la operación; se puede insertar una
segunda instancia desde el catálogo. Duplicar/importar una pista con IDs sin instancias
deja su cadena sin audio hasta recargar los plugins; nunca simula su procesamiento.
Cambiar sample rate requiere recargar los efectos (una ruta incompatible silencia).
La exportación se rechaza explícitamente si el proyecto contiene efectos VST3 para
no producir un archivo sin ellos. Los plugins nativos cargados todavía no están aislados
ante crashes; el aislamiento descrito arriba solo corresponde al escaneo.

Verificación manual: Cathar declara clase `436174686172526573746F7265303031`, tipo efecto.
Procesó una senoide estéreo en memoria: pico **0.105957**, todas las muestras finitas,
`failed=false`. `--editor` notificó apertura del editor original y terminó con código 0.
La acción `fx.vst3.load` también insertó Cathar en Track FX dentro de Hay y abrió
`Cathar Restore — Hay`; la captura mostró los controles originales. No equivale a
una escucha física ni a una comprobación de todos sus controles.

## Implementación

- Binding `vst3 =0.3.0`, MIT/Apache-2.0; no se incluye el código de yabridge.
- `audio/vst3`: módulo/fábrica COM, componente/controlador, buses y lifecycle VST3.
  El módulo permanece cargado hasta liberar todas las interfaces. No carga en el callback.
- Buffers de buses, eventos y parámetros preparados de antemano. Render f32 en segmentos
  de hasta 512 frames; el scheduler existente divide en las posiciones de los eventos.
- Note-on/off, velocity de liberación, CC, sustain, pitch bend y presión de canal vía
  `IMidiMapping`. No se traduce pan por nota a pan de canal. Panic libera las notas conocidas.
- Salida principal estéreo; entrada principal activada para efectos, auxiliares desactivadas, con buffers
  válidos y silenciosos para la topología declarada. Errores de process/samples no finitos
  silencian la salida y se notifican a la UI.
- `instruments/vst` conserva la ejecución sin GPUI. `ui/vst` coordina carga, asignación,
  comandos y MIDI. El catálogo continúa indexando metadatos, sin instanciar plugins.
- `audio/vst3/editor`: propietario de componente/controlador y ventana en un hilo
  separado; `IPlugView`, `IPlugFrame` e `IRunLoop` estándares, sin controles específicos
  de Pianoteq. El editor envía cambios por `IComponentHandler` y una cola SPSC al
  procesador. No hay acceso a GPUI ni bloqueos de UI en el callback. Mapeo Lua pendiente.
- En gpui-dev sin Wine, `scripts/run.sh` configura `WINELOADER=scripts/wine-host.sh`.
  El arranque del binario también prepara un lanzador en caché antes de crear hilos;
  funciona desde cargo y con lanzamiento directo. Reconoce el enlace absoluto del
  Wine del host aunque su destino no exista dentro del contenedor.
  Delega al Wine del host mediante distrobox-host-exec y conserva el prefijo elegido por
  yabridge. Como los procesos tienen namespaces de PID distintos, desactiva el watchdog
  de yabridge en ese caso. No cambia el Wine ni el bridge instalados.

## Alcance actual

No hay guardado/restauración de presets o estado VST3,
automatización de parámetros, compensación de latencia ni buses auxiliares. El instrumento
es una etapa previa a la cadena de efectos de la pista.
No se guardan la ruta del bundle ni el estado del instrumento en el proyecto: hay que
volver a cargarlo en cada sesión. La exportación que incluye MIDI de la pista VST3 se
rechaza explícitamente, para evitar sustituirlo por SoundFont. Cambiar la interfaz de
audio recarga las instancias en serie con la nueva frecuencia (se pierde su estado no persistido).
La audición VST3 usa un stream por instancia; Play silencia esos streams y usa el
motor común. El tail se acota entre 2 y 10 segundos; todavía no hay compensación
de latencia ni buses auxiliares.
Los plugins nativos se ejecutan dentro del proceso; este host no aísla crashes nativos.
El teclado de ordenador y Espacio con foco en la ventana externa dependen del editor;
no se ha implementado el retorno de teclas sin consumir a Hay.

## Verificación realizada

`cargo check --all-targets --locked`, sin ejecutar la suite de tests.
`vst3_probe` cargó el Pianoteq 9 del usuario mediante yabridge 5.1.1 y Wine Staging 11.20,
tanto desde el host como desde gpui-dev. Renderizó note-on/off en memoria, sin salida
física: pico **0.341817**, sin error de procesamiento. Esto verifica carga y síntesis;
La ventana de Hay también mostró Pianoteq 9 asignado a una pista y su fila en Track FX,
con carga exitosa en el log. También se abrió el editor original de Pianoteq con
WINELOADER inicialmente ausente. La interfaz dibuja parcialmente: faltan textos y se
observa un diálogo blanco. Inicialmente, `fc-match "Bitstream Vera Sans"` devolvía Noto
Sans; faltaba la familia que JUCE solicita bajo Wine. [Reporte del mismo problema y
solución con fuentes](https://github.com/giang17/wine/issues/13). Con autorización del
usuario se instalaron las diez variantes de Bitstream Vera 1.10 y su licencia en
`~/.local/share/fonts/bitstream-vera`, verificando el SHA-256 publicado por GNOME.
Tras actualizar fontconfig, el host resuelve la familia a `Vera.ttf`. Wine no fue
modificado; queda comprobar el dibujo al volver a cargar el plugin. No se verificaron todos los controles, cierre/reapertura limpios,
entrada MIDI física ni escucha por altavoces. El diagnóstico con editor agotó su
límite de 180 segundos durante el cierre; el cierre del plugin se mantiene fuera de GPUI.

```bash
distrobox enter gpui-dev -- cargo build --example vst3_probe --locked
target/debug/examples/vst3_probe "$HOME/.vst3/yabridge/VST3/Pianoteq 9.vst3" --editor
```
