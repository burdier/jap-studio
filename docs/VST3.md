# Host VST3: instrumentos y efectos

Inicia Hay desde el acceso del escritorio o `bash scripts/run.sh`. En una pista MIDI,
abre `[+]` → Instrumentos, busca **Pianoteq** y pulsa Enter. La carga ocurre en background;
la pista se asigna solo después de preparar el plugin y abrir la salida correctamente.
Hay abre el **editor original** en una ventana separada. La fila del instrumento
en Track FX permite reabrirlo. Cerrar la ventana conserva la instancia y su audio.
En Wayland, Hay usa el backend nativo y los editores VST3 de tipo X11 como Vital
usan XWayland.
Activa el teclado MIDI con **M** en Hay antes de tocar. Las teclas musicales siguen
llegando a Hay cuando el editor VST3 tiene el foco. Con el teclado MIDI desactivado
puedes escribir en los campos del plugin. Espacio controla el transporte.

Acciones: `instrument.vst3.load 'ruta/Plugin.vst3'` (formulario de ruta desde catálogo),
`instrument.vst3.remove`, `instrument.vst3.editor` y `instrument.vst3.editor.close`. Cada pista MIDI
puede conservar un VST3 distinto. Seleccionar una pista cambia el editor y el destino
MIDI; retirar el instrumento de una pista conserva las demás instancias. Play
procesa los VST3 cargados en el mismo transporte y salida que WAV y los demás
instrumentos MIDI, con el sonido ya editado en cada instrumento. Stop devuelve cada
VST3 a su audición en vivo.

Lua puede solicitar cargas y configurar las opciones admitidas por el host mediante
`vst3.script` y handles de pista/FX. [Contrato y ejemplos](VST3_LUA.md).

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
percusión provisional, compensación de latencia, sidechains, automatización Lua de
parámetros internos, ni persistencia del estado binario del plugin. Duplicar un nodo VST3 no
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

## Diagnóstico de StandardCLIP

StandardCLIP en `~/.vst3/yabridge/SIR Audio Tools/StandardCLIP.vst3` se carga como
efecto mediante Wine. Con Wine Staging 11.20 y yabridge 5.1.1, el editor dibujaba
los controles y los clics quedaban fuera de sus posiciones al mover la ventana.
Con la compilación de yabridge `5.1.1-72-g7529c2d6`, Gain y Ceiling respondieron en
dos posiciones distintas en XWayland. El log confirmó cambios de parámetros
recibidos por el host con el mismo binario de Jap Studio.
[Registro de cambios de yabridge](https://github.com/robbert-vdh/yabridge/blob/master/CHANGELOG.md#unreleased)

Jap Studio solicita foco X11 tras recibir `MapNotify`, cuando el gestor ya mostró la ventana.

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
  procesador. No hay acceso a GPUI ni bloqueos de UI en el callback. Las acciones
  Lua de carga y configuración del host usan el controlador compartido; los
  parámetros internos del plugin siguen fuera del contrato Lua.
## Pitch por nota

En el piano roll, activa **Pitch MPE**, selecciona una nota y arrastra su curva.
Shift ajusta en cents, el doble clic retira un punto y Alt+clic reinicia la curva.
Hay guarda la curva junto con la nota y usa canales MPE independientes para cada
nota con pitch en un VST3 que mapea pitch bend por canal. En Vital, activa
**Advanced → MPE Enabled** para usar el rango de ±48 semitonos. Cada instancia
dispone de hasta 15 canales de expresión simultáneos. Los canales usados por clips
MIDI convencionales de esa instancia quedan reservados para sus mensajes.

## Wine para plugins de Windows

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
Espacio con foco en la ventana externa solicita Play/Stop al transporte común de Hay.
El resto del teclado depende del editor; no se ha implementado el retorno de otras
teclas sin consumir a Hay. La interacción de Espacio con un plugin real sigue
pendiente de comprobación manual.

## Verificación realizada

Pianoteq 9 se copió desde `~/Games/piano/drive_c/Pianoteq 9` a
`~/.local/share/Steam/steamapps/compatdata/standardclip_test/pfx/drive_c/Pianoteq 9`,
el mismo prefijo que StandardCLIP. Se actualizó el directorio registrado en
yabridgectl y se sincronizó; el wrapper conserva `~/.vst3/yabridge/VST3/Pianoteq 9.vst3`.
El original se conserva como respaldo. El probe confirmó el prefijo nuevo y audio
con pico `0.341817`, sin error. El editor abre, pero la captura todavía muestra
zonas negras/grises y controles parcialmente dibujados; trasladar el prefijo no
ha resuelto esos defectos gráficos. La prueba con editor agotó el límite de 95 s
sin terminar el proceso; su cierre limpio sigue pendiente, aunque el probe sin
editor terminó correctamente.

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
