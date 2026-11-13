# Estado del prototipo y siguiente incremento

Plan de visualizaciones por pista y rutas LFO: [VISUAL_MODULATION.md](VISUAL_MODULATION.md).

## Disponible

- **PR-04, cuarto incremento:** reloj `musical::time::Transport` compartido por
  todos los clips de audio. `transport.seek <negras>` salta durante Play;
  Home y ⏮ comparten la operación de volver a cero. `transport.loop <inicio> <fin>`
  define el rango, `transport.loop off` lo desactiva y el botón **↻ Audio** alterna
  el loop de todo el arreglo. Stop conserva el frame observado como cursor de edición.
  Pedidos publicados fuera de RT; la UI espera su confirmación antes de dar por finalizado
  un stream. Loop se divide en segmentos exactos, incluidos loops de un frame.
  Los FX Lua se preparan por frecuencia real y una preparación cancelada no altera
  la variante activa. VST3 con frecuencia incompatible rechaza el arranque de la mezcla.
  Fuentes inmutables durante Play; cambios de clips requieren detener/reproducir.
  Loop global de audio, sin mezclar aún MIDI/Session; no cambia el loop individual de clips.

- **PR-04, tercer incremento:** resolución común headless de mute/solo/nivel/pan.
  Prueba de audio cubre mute, múltiples solos, mute sobre solo, nivel, balance y
  reordenación sin cambiar los destinatarios por TrackId. La UI usa esa resolución.

- **PR-04, segundo incremento:** Play en contexto de audio mezcla los clips de
  Arrangement en un stream. Suma por pista antes de FX y protección única en master.
  Recursos preparados en worker; Stop o cambios durante preparación cancelan el arranque.
  Ganancia de clip antes de FX; nivel/pan/rack direccionados por TrackId.
  Prueba manual: dos WAV en pistas distintas desde el mismo beat, seleccionar audio,
  colocar cursor al inicio y Play. Deben oírse ambos; el filtro de una pista deja
  intacta la otra. El usuario confirmó reproducción de dos WAV. MIDI/Session siguen
  separados; loop/seek se añaden en el cuarto incremento.

- **PR-04, primer incremento:** fuente de clip extraída de CPAL y usada por el
  reproductor actual. Render por frames absolutos, recortes y buffers ya remuestreados;
  pruebas sin dispositivo. El segundo incremento conecta estas fuentes a una salida compartida.

- **Hay Filter (PR-03B):** paquete Lua v2 con SVF FunDSP nativo LP/HP/BP, frecuencia
  logarítmica, Q, selector y curva de respuesta en Track FX. Controles, bypass y
  duplicación por instancia; sin LFO/Drive/Mix. Los clips MIDI aplican los FX
  de su propia pista durante reproducción/carga; al volver a notas en vivo, el
  piano compartido recupera la ruta de su pista asignada. Prueba interactiva pendiente.
  PR-04 prepara el FX a la frecuencia real del stream, incluida Auto. [Contrato y uso](LUA_AUDIO_COMPONENTS.md#incremento-pr-03b-hay-filter-y-kit-v2).

- **Componentes de audio Lua** (PR-03): `[+]` inserta Hay Double Volume en Track FX.
  El grafo estéreo de primitivas Volume se valida fuera de audio y procesa en el
  rack FX existente; macro, comando `fx.parameter`, bypass, duplicación y recarga
  explícita conservan instancias independientes. Pruebas headless de sonido y
  recarga; el usuario confirmó funcionamiento y estilo en la aplicación, sin
  registrar pasos ni mediciones de audición. El rack prepara el suavizado a la
  frecuencia real de salida desde PR-04. Export offline y
  persistencia global de UI siguen pendientes. [Uso](LUA_AUDIO_COMPONENTS.md).

- **Lotes Lua de pistas** (PR-02): `track.script 'archivo.lua'` y
  `track.template.two` desde comandos o `[+]` crean/configuran pistas en una
  confirmación local. Snapshot con revisión, handles, niveles, panorama y cadenas
  Hay Utility; validación estructural compartida con `TrackEdit` y Loro. Un fallo
  de referencia, FX o revisión descarta el lote; clips, foco y MIDI Learn se
  remapean por identidad. `ProjectDoc` se valida como candidato, todavía sin
  conexión persistente de UI, mezcla/audio en el esquema, red ni undo/redo global.
  [Uso y límites](LUA_TRACK_BATCHES.md).

- Las asignaciones de iconos y comandos personalizados del Transporte y Piano Roll se
  guardan en `~/.jaybu.profile` y se restauran al abrir el DAW.

- **Paquetes Lua v1 en catálogo y formulario** (PR-01, commit 4):
  `midi.tools.reload` publica la carga completa desde un worker; `midi.tool <id>`
  abre parámetros, y con todos los valores ejecuta la herramienta. `[+]` y las
  sugerencias usan la misma invocación. Una recarga fallida conserva el registro;
  una válida invalida formularios anteriores, mientras los trabajos enviados
  conservan su fuente. Sandbox y validación MIDI compartidos; cambios concurrentes
  del clip, bloqueo o arrastre descartan el resultado. Acentuar y Subdividir se
  distribuyen como paquetes; los alias anteriores siguen disponibles antes de la
  primera recarga correcta. Sin dependencias añadidas. [Uso](LUA.md#herramientas-registradas).

- **Fundamentales MIDI a acordes**: paquete Lua que añade terceras y quintas diatónicas
  a las raíces seleccionadas según la escala activa del piano roll. Conserva instrumento,
  expresión, inicio y duración; el host valida las notas y asigna sus IDs. Requiere
  `:midi.tools.reload` y una selección explícita. [Uso](LUA.md#herramientas-registradas).

- **Hay Acent**: presets Lua Pulso, Alternado, Creciente y Clic 4/4, ejecutables
  sobre el clip MIDI con `midi.script`/Lua… sin recompilar. El clic transforma cuatro
  notas existentes en una guía a negras; usa el instrumento de la pista, sin
  metrónomo global. [Uso y límites](LUA.md#hay-acent).

- Acción **Nota general…** desde `[+]`, `:note` o `:note.edit`. Abre el mismo apunte
  multilínea para editarlo y guardarlo en los datos del usuario (`hay-daw/general-note.md`);
  no queda atado a una pista o proyecto. Ctrl+Enter guarda y Esc cierra.

- Subdivisión MIDI desde el paquete recargable `hay.midi.subdivide`; el alias y los
  gestos del piano roll conservan selección explícita, IDs, mute y selección de
  fragmentos. Si no hay un paquete registrado, queda disponible la implementación
  integrada anterior, incluida su vía inmediata para selecciones pequeñas.

- `f` en Normal amplía el panel con foco (piano roll, Arrangement, instrumento,
  Track FX o explorador) y otra pulsación restaura el layout sin cambiar anchos ni
  divisiones. Esc restaura el panel acoplado; F11, botón Ampliar y acción
  `view.maximize` también funcionan sin Vim. En ventanas GPUI separadas se alterna
  la maximización nativa. Sin ventanas nuevas ni copias del editor.
- Configuración → Atajos ofrece Navegación Vim, con acción `vim.toggle` en `[+]`
  y `:vim.toggle` / `:vim toggle`. Se persiste `VIM_ENABLED` en el perfil existente.
  Al desactivar Vim se cancelan `dd` y el modo/comando pendiente; flechas, Delete,
  ratón, atajos de herramientas, controlador MIDI y transporte siguen disponibles.
  El indicador cambia a VIM OFF; se reactiva desde la UI sin necesitar `:`.

- Hay Spectrum insertable desde `[+]` o `:fx.spectrum`, con identidad por nodo y
  múltiples inspectores en Track FX. Comparación antes/después de VST3, reordenamiento,
  serie/paralelo, duplicación y bypass mediante el árbol existente. El mismo componente
  del piano ofrece espectro, onda estéreo, espectrogramas log/lineal y peak/RMS.
  RealFFT existente, sin dependencias nuevas. Análisis fuera de RT; captura solo con
  Track FX visible. Posición/nombre/bypass se guardan con el proyecto; modo e historial
  pertenecen a la sesión. Pendientes buses de grupo, audición de patrón de percusión,
  cabeceras/mixer y modulación LFO. Ver [VISUAL_MODULATION.md](VISUAL_MODULATION.md).

- VST3 genérico: catálogo por metadatos de fábrica en proceso separado, clases de
  instrumentos y efectos, entrada/salida estéreo, editor original y efectos en el
  árbol existente (serie/paralelo, bypass, orden y eliminación). Cathar Restore de
  Windows instalado mediante yabridge y procesando audio de diagnóstico. Acciones
  `fx.vst3.load/editor`. Sin nuevas dependencias. Compilación y diagnóstico manual,
  sin suite de tests; límites de export/estado/duplicación/grupos en [VST3.md](VST3.md).

- Arrangement y piano roll alternan un fondo tenue por compás, con divisiones y
  números más claros. Componente compartido, adaptado al tema y la firma de compás;
  mantiene la alternancia al desplazar y hacer zoom, sin cambiar notas ni snap.

- Daynight Studio: variante oscura de Daylight Studio, con paneles diferenciados,
  texto claro y acentos mates. Disponible en Apariencia y mediante `THEME=daynight`;
  conserva dimensiones, controles y contraste de selección del usuario.

- Los 16 temas integrados usan acentos desaturados y superficies más neutras.
  Obsidiana adopta grises de estudio inspirados en REAPER; Cyberpunk Studio conserva
  su familia violeta/rosa. Notas con halo discreto y clips con bordes del tema, sin
  coral fijo. Selección, avisos y tiradores siguen diferenciados. Los perfiles con
  colores personalizados conservan prioridad; volver a elegir el tema aplica la paleta.

- `[+]` → Instrumentos carga Pianoteq VST3 mediante yabridge en background. Una instancia
  por sesión, fila en Track FX, MIDI en vivo y clips con scheduler existente, mezcla de
  pista y salida principal estéreo. Comandos `instrument.vst3.load/remove`. El acceso
  del escritorio usa el Wine del host desde gpui-dev. Carga/síntesis real comprobadas;
  editor original VST3 abierto desde Track FX. Bitstream Vera instalada para corregir
  textos ausentes bajo Wine; falta revisar el dibujo tras recargar el plugin.
  Estado/presets, mapeo Lua y exportación VST3 pendientes. [Alcance](VST3.md).

- M activa/desactiva el controlador también desde OFF. El icono asignado a teclado MIDI
  muestra ON/OFF y fondo activo desde el mismo estado que el instrumento.
- Shift + arrastrar dentro del cuerpo de clips de audio o MIDI desplaza su contenido
  sin mover pista, inicio ni duración; reutiliza `trim_start`/`trim_len`, sin modificar
  la fuente. Desplazamiento continuo, acotado a la fuente y respetando bloqueo. Si el
  clip muestra toda la fuente hay que recortar antes un borde; el estado lo indica.
  Cabecera y Ctrl-selección conservan sus gestos. Validación: compilación sin tests.

- Instrumento/FX: MIDI del ordenador activo tiene prioridad sobre atajos de árbol y
  explorador; Espacio comparte Play/Stop en editor y ventana separada. Cambiar el foco
  a FX no libera notas si el destino MIDI sigue siendo el mismo. La ventana separada
  también drena MIDI físico. Formularios/menús y campos de texto conservan sus teclas.
- Playhead visual interpolado entre publicaciones del audio, extrapolación máxima de
  80 ms y reinicio inmediato en Stop, saltos y vueltas del loop. Cursor de edición
  con convergencia breve; Play sigue usando la posición exacta elegida. Dibujo compartido
  entre regla, Arrangement y piano roll. Validación: compilación; pendiente revisión
  visual/interactiva por el usuario.

- Configuración → Audio: driver disponible, entrada/salida por ID CPAL, pareja física de
  salida, canal mono de entrada, frecuencia y buffer admitidos, estimación por bloque,
  tono de prueba y medidor. `:list device` lista interfaces; `:use device 2` selecciona
  y guarda una salida (`:use device input 2` selecciona entrada). `:use device` abre el
  panel y enfoca el selector. Borrador/Aplicar/Revertir; valores `AUDIO_*` persistidos en el perfil.
  Select compartido de Kit con búsqueda y menú flotante (sin desplazar el panel),
  Tab/Shift-Tab y navegación de opciones. Interfaces ocupadas o con error de consulta
  permanecen visibles. Backend PipeWire nativo habilitado para compartir la interfaz
  con Linux. Salidas hermanas seleccionables por nombre real (SSL Line Outputs
  1/L + 2/R y Line Outputs 3 + 4), con canales locales por endpoint. Los perfiles ALSA
  existentes se conservan y ofrecen cambio explícito a PipeWire desde Audio.
  Verificado en SSL: stream silencioso de ocho segundos en 3/4 (765952 muestras)
  mientras 1/L + 2/R seguía RUNNING en Linux; ambas salidas RUNNING simultáneamente,
  sin errores durante el stream. No equivale a una comprobación auditiva.
  Piano, sampler, percusión y clips comparten esta selección. Los archivos se remuestrean
  con SoXR fuera de UI/RT cuando corresponde. La entrada sirve al medidor; grabación a
  pistas y monitorización todavía pendientes. Comprobado mediante compilación; falta
  comprobación auditiva con la interfaz física del usuario.

- Modo `:` con sugerencias visibles del catálogo y alias. `↑/↓` eligen, Tab completa,
  Enter ejecuta el texto o la selección explícita, Esc cancela y clic completa. Los
  comandos que requieren parámetros abren el formulario existente. Funciona en la
  ventana principal y en el piano roll separado. Validación: compilación.

- Lua 5.4 mediante `mlua` para transformar notas del clip MIDI activo fuera de la UI y
  del hilo de audio. El formulario `[+]` acepta un archivo Lua; `:midi.script` hace lo
  mismo, y **Lua · acentuar notas** / `:midi.script.accent` / acción de catálogo **Lua · acentuar notas** prueban un
  ejemplo integrado; el botón **Lua…** abre un archivo propio. El resultado se valida como patrón completo y se descarta si el
  clip cambia. Solo edita notas existentes; no hay todavía API de displays ni undo de
  proyecto. [Contrato y límites](LUA.md). Validación: compilación.

- Documento Loro headless para nombre, IDs, orden, grupos, instrumento, mute, solo, FX y MIDI de
  pistas. Reconcilia cambios sin recrear el orden CRDT, exporta/importa snapshots y
  actualizaciones, y guarda/carga archivos binarios con reemplazo atómico. Valida el
  esquema y la jerarquía al importar. Las cadenas FX conservan nodos, parámetros,
  bypass y contenedores anidados. Los clips MIDI guardan posición, recorte, propiedades
  y notas con IDs originales; se reconstruyen con el índice de carril actual.
  Todavía no guarda clips de audio ni controles de mezcla;
  por eso no hay comando de abrir/guardar proyecto en la UI. Validación: compilación.

- Piano roll: controles de la barra registrados en catálogo y comandos, formularios de
  parámetros musicales y transformaciones Lua (Q/H/Subdividir/Gate/Swing/Velocity/Random/Clamp).
  Selector real de clip, longitud no destructiva, canal MIDI 1–16, escala/tónica, duración,
  copy/cut/paste múltiple, Paint por arrastre, color picker, Follow y repetición por frames.
  Pointer (V) selecciona/mueve con clic; doble clic en vacío crea con duración de rejilla,
  y sobre una nota la elimina. Región selecciona, Delete/Backspace borra la selección,
  y Borrar región elimina al soltar (Esc cancela). Ctrl+A selecciona todas;
  Ctrl+Shift+A despeja. Acciones registradas y suavizado visual breve de mover/recortar
  y del ajuste final al grid, sin cambiar el resultado musical. Validado por compilación;
  pendiente valorar la sensación con ratón/touchpad en la app.
  Notas con borde de 0,7 px y sombra tenue. Empuje Lua al mover una nota o alargar su
  borde derecho: recorta vecinas del mismo tono, desde el lado del contacto; funciona
  al mover a izquierda/derecha. Mantiene un tick mínimo y detiene el avance al alcanzarlo.
  Retroceder restaura mientras dura el gesto; al soltar se confirma; Esc cancela todo.
  VM reutilizada por gesto, sin pasar controles ni ejecutar Lua durante paint/audio.
  Validación: compilación; interacción visual pendiente de probar en la app.
  Botones, teclado e iconos comparten operaciones. Las preferencias sobreviven cambios de
  clip durante la sesión; Edited compara con el estado al abrir. Velocity y los carriles CC/bend/presión editan datos reales; estos últimos tienen puntos
  independientes, scheduler, importación SMF, Loro y API Lua. Release velocity, probabilidad
  y pan por nota tienen carril, acción Lua y persistencia. Probabilidad se evalúa por vuelta;
  sfizz recibe release velocity y FluidSynth aplica pan por voz. Los otros backends
  conservan esas expresiones sin aplicar las que no soportan.
  También siguen pendientes undo y guardado completo de sesión. Inventario y validación:
  [PIANO_ROLL_ACTIONS.md](PIANO_ROLL_ACTIONS.md).

- Analizador RealFFT reutilizable para gráficas de efectos: el filtro del piano muestra
  un panel `<` `>` que alterna espectro de salida y visualizador de forma de onda.
  Espectro incluye guía de corte; la FFT corre en la UI con buffers
  preparados; el callback solo entrega muestras mediante una cola acotada.

- Piano reorganizado con la línea visual de Hay Sampler: cabecera/fuente compactas,
  bloques de perillas, bancos y créditos desplegables y teclado de 135 px. Arrastre/rueda,
  Shift para precisión, doble clic para reset y MIDI Learn conservado. Filtro estéreo real
  LP/HP/BP con corte logarítmico 20 Hz–20 kHz y resonancia Q 0,5–10, también en exportación.
  Descubrimiento de bancos y preparación del motor bajo demanda en un worker; sin cargar
  SoundFonts al arrancar. Stop/NoteOff cancelan solicitudes pendientes y un cambio de banco
  descarta preparaciones antiguas. Sin dependencias nuevas. Verificación completa con
  `scripts/check.sh`: formato, 104 tests y compilación; incluye filtro en exportación.
  Interacción visual/auditiva pendiente de sesión gráfica.

- Piano y Hay Sampler aparecen como fila de instrumento en Track FX. Insertarlos
  conserva Arrangement y abre el editor inferior; la vista central Instrument ya
  no forma parte de la navegación.

- Acción **Insertar sampler** en `[+]`, menú de pistas y `:sa`: crea una pista con
  Hay Sampler asignado o abre la existente (una instancia por sesión). Asignable a la barra.

- Hay Sampler basado en sfizz, accesible desde el [+] actual y `:sampler`. Panel compacto
  inspirado en Simpler con waveform, región/loop, Classic/1-Shot, nota raíz/afinación,
  polifonía, filtro LP12, ADSR y salida. Reutiliza botones, perillas y teclado de Hay.
  Carga por selector, drop externo o archivo seleccionado en biblioteca; prepara en background.
  Parámetros continuos por CC de alta resolución; cambios estructurales reemplazan el motor
  al terminar y cortan notas. Piano roll, controlador MIDI y exportación usan sfizz en la
  pista asignada. Scheduler y reproductor compartidos con el piano; no se añade otro reloj.
  Una instancia por sesión, sin Warp/Slice/LFO/MPE, persistencia ni mezcla multipista live.
  sfizz 1.2.3 compilado localmente; no se ejecutaron tests ni una sesión gráfica/auditiva.
  [Uso, arquitectura y construcción](SAMPLER.md).

- Track FX Tree junto al Arrangement, con el mismo TreeView extraído del explorador de archivos.
  [+] / A / :add conservan el catálogo general; FX añade un panel contextual, sin sustituir Add.
  Hay Utility se inserta en la pista/contenedor indicado. Serie/paralelo admiten anidamiento;
  selección, plegado y scroll por pista; bypass independiente, nombre, duplicar, quitar y drag/drop
  con rechazo de ciclos. Enter/doble clic: parámetros; B: bypass; F2: nombre; Ctrl+D: duplicar;
  Delete: quitar; A: Add; Espacio: transporte. Menú y formulario son los componentes existentes.
  `fx.show/add/serial/parallel/rename/parameters/duplicate/bypass/remove` están en catálogo/comandos.
  IDs estables; al duplicar nodos o pistas se generan identidades nuevas. Editar nombre/bypass
  no reconstruye filas; el árbol está cacheado y virtualizado. Su panel se puede reducir a cero.
  Utility procesa en audio/piano y exportación, mediante compilación lineal de serie/paralelo
  fuera del callback y publicación atómica suavizada. Un grupo compone su ganancia con sus hijos.
  Paralelo suma sin normalizar; vacío/bypass pasa la entrada. No añade EQ/Reverb ni efectos VST3,
  buses generales, mezcla multipista live, FX de Session ni persistencia/undo del proyecto.
  Validación: compilación; sin ejecutar tests ni validar interacción/audio en una sesión gráfica.

- Espacio inicia/detiene el transporte desde Arrangement, piano roll, explorador e instrumento,
  también en ventanas separadas y en modo Insert. Reutiliza Play/Stop existentes; Session conserva
  la audición del patrón. Mantener la tecla pulsada no alterna repetidamente. Campos de texto,
  comandos y diálogos conservan sus controles. Esc libera notas en el instrumento.
  Validación de este ajuste: compilación, sin ejecutar tests ni comprobación interactiva.

- GPUI Kit 0.9.0 integrado como biblioteca de UI, con runtime/plataformas 0.3.11 alineados.
  Primer uso: formulario de parámetros de acciones (exportar, importar por ruta, nombre/altura
  de pista y ganancia Utility). Campos de texto con selección, IME, copiar/cortar/pegar y
  undo/redo del campo; cantidades con botones y ↑/↓ según unidad/incremento del contrato;
  booleanos con casillas y botones OK/Cancelar de Kit. Tab/Shift+Tab recorren el diálogo,
  Enter desde un editor o Ctrl+Enter confirman, Esc cancela y devuelve el foco. Los errores
  conservan el borrador. El formulario aparece en la ventana que lo abrió, incluido el piano
  roll separado. Colores y contraste proceden del perfil Hay; el motor de audio no cambia.
  Este historial de texto no implementa undo/redo del proyecto. Validación: compilación,
  sin ejecutar tests ni comprobación interactiva de la UI.

- Explorador de bibliotecas independiente a la izquierda del Arrangement y su editor, con
  separación visual y ancho arrastrable hasta ocultarse. `e` en modo Normal lo abre/cierra
  y recupera el panel reducido a cero; con foco dentro, `e`/Esc lo ocultan. En el piano roll,
  Borrar queda disponible con Shift+E. Árbol con sangría y guías, carpetas primero, iconos
  audio/MIDI y selección con el contraste del perfil. Flechas/j/k/h/l navegan el árbol.
  «Añadir biblioteca» permite elegir carpetas; sus accesos sobreviven al reinicio en
  `~/.config/hay-daw/libraries` (o la carpeta de configuración del sistema). «Quitar» olvida
  el acceso sin borrar archivos. Bibliotecas desconectadas se conservan y muestran error.
  «Recargar» consulta cambios externos; todavía no hay vigilancia automática del disco.
  Doble clic/Enter insertan audio o MIDI en la pista activa y el cursor del Arrangement.
  `explorer.toggle`, `explorer.insert` y `file.import` están en catálogo e iconos; esta última
  recoge una ruta con el formulario declarativo. La importación conserva TrackId al reordenar.
  Modelo headless, entidad GPUI cacheada, lista virtual y E/S en background, incluido MIDI.
  Sin nuevas dependencias. Validación de este incremento: compilación, sin ejecutar tests.

- Grupos plegables con ▸/▾ en su cabecera y «Plegar/Desplegar grupo» en el menú contextual.
  Al plegar queda una única fila de grupo con su contador; sus hijos, clips y audio se conservan.
  Al entrar en un grupo, las pistas pasan a altura compacta (40 px), también al insertar
  una pista nueva dentro de él. Después se pueden ampliar manualmente; duplicar conserva
  la altura elegida en el original.
  Las cabeceras de los hijos tienen 12 px adicionales de sangría para marcar la jerarquía.
  Flechas/j/k y Shift+clic recorren únicamente las filas visibles. El menú de pistas ofrece
  altura Compacta (40 px), Predeterminada (perfil), Amplia (120 px) y Personalizada (32–240 px)
  para toda la selección. La personalizada reutiliza el formulario de acciones. Las filas
  compactas conservan nombre, mute, solo y + según la configuración de controles; las mayores
  recuperan el resto. Cabeceras, waveforms, MIDI, recorte y drag/drop comparten geometría variable.
  Comandos: `:track.fold`, `:track.height 40`, `:track.height.compact/default/tall`; también catálogo
  e iconos. Estado visual de sesión, sin alterar audio ni exportaciones; no se guarda al reiniciar.

- Selección con mayor contraste, separada del hover: cabeceras y carriles de todas las
  pistas seleccionadas, clips, filas de menús, buscador y selectores compartidos. En
  Configuración → Apariencia, «Contraste de selección» ofrece Suave/Medio/Alto y una vista
  comparativa. Se guarda automáticamente como `SELECTION_CONTRAST` en `~/.jaybu.profile`;
  por defecto `0.18`, rango `0.0–0.4`. Los fondos se adaptan al texto del tema para funcionar
  tanto en temas claros como oscuros; los temas existentes conservan la intensidad elegida,
  mientras que Ableton Live Dark aplica `0.22` de su preset.

- Pistas dinámicas: insertar, eliminar con sus clips, duplicar selección/clips, renombrar,
  color, mute/solo y grupos básicos de un nivel. Ctrl+clic selecciona varias cabeceras;
  Shift+clic selecciona un rango. El menú derecho usa las mismas filas planas y selector de
  color que los clips. En Arrangement, Delete o dos pulsaciones de `d` en modo Normal eliminan
  las pistas seleccionadas (incluidos hijos/clips de grupos); Esc u otra tecla cancela `d`.
  Arrastrar una cabecera separa visualmente la pista y sus clips del grid; flotan con el
  puntero mientras las demás filas muestran el nuevo orden y el hueco de destino.
  El orden musical se cambia al soltar. Las hijas se reordenan entre hermanas y los grupos
  se mueven con sus hijos.
  Mantener pulsada la tecla no dispara eliminaciones sucesivas. Los campos y overlays conservan
  su entrada de texto. Las acciones también se encuentran en el catálogo.
  Las acciones `track.insert/delete/duplicate/group/ungroup/rename/mute/solo`
  están en catálogo, comandos y selector de iconos. Renombrar usa el formulario de parámetros.
  Agrupar mueve las pistas juntas; desagrupar conserva los hijos. Eliminar un grupo elimina
  también sus hijos/clips; duplicarlo copia la estructura con IDs nuevos. La única instancia
  de piano conserva su asignación original: duplicar clips no crea un segundo instrumento.
  Un Arrangement sin pistas muestra fondo oscuro y un + central que abre el mismo buscador
  de `:add`/A. Desde allí se puede insertar una pista vacía o elegir el piano para crearla.
- Grupos como pistas contenedoras de audio/MIDI, sin clips ni grabación propios. El título
  muestra `[activas/total]` según mute. Mute/solo del grupo y nivel se heredan por los hijos;
  el balance suma el desplazamiento del grupo al de cada hijo y limita a L/R. También se
  aplica al snapshot de exportación; seleccionar un grupo exporta sus hijos (stems por hijo).
  Son controles jerárquicos básicos, no un bus de mezcla en vivo. `Track.chain` usa ahora el árbol FX recursivo descrito arriba.
  Los grupos de pistas siguen limitados a un nivel; no hay efectos VST3 de grupo ni bus DSP general. El proyecto sigue
  sin guardado/undo. En este incremento se valida únicamente compilación, sin ejecutar tests.

- Acciones con parámetros declarativos: texto, booleanos y cantidades con unidad/límites.
  «Exportar audio» desde catálogo o icono abre un formulario flotante con nombre/formato,
  carpeta, pistas, inicio/duración en beats, cola en segundos, frecuencia en Hz, mono y
  archivos individuales. Parte de los valores guardados; profundidad, calidad MP3 y Utility
  mantienen las preferencias. «Utility · ajustar ganancia» reutiliza el formulario en ×.
  Tab recorre los controles, ↑/↓ ajustan cantidades, Espacio alterna bool, Enter en un editor
  o Ctrl+Enter/OK ejecutan y Esc cancela. Los errores conservan el borrador; confirmar no
  modifica preferencias. Los comandos manuales mantienen su ejecución directa.

- Configuración organizada por categorías laterales: Apariencia, Pistas, Exportación y Atajos.
  Exportación permite guardar nombre/carpeta, varios formatos, master o archivos por pista,
  frecuencia, profundidad PCM, MP3 CBR/VBR, canales, Utility, pistas y rango/cola.
  El borrador se valida antes de guardar en `EXPORT_DEFAULTS` del perfil personal; reiniciar
  o `:reload` recupera los valores. «Exportar audio» y `:export` los usan; los argumentos
  explícitos del comando prevalecen. En el selector de iconos, ahora un botón SVG «+»,
  «Exportar audio» y «Configurar exportación» aparecen al principio.

- Regla de tiempo fija al pie del Arrangement, alineada con el grid: minutos/segundos,
  subdivisiones adaptadas al zoom y fracciones de segundo al acercarse. Comparte cursor,
  playhead, desplazamiento y zoom con la regla musical; clic coloca el cursor de edición.
  La conversión usa el BPM actual del Arrangement, editable desde el transporte o `:tempo`.
  El compás editable (`:meter`, por ejemplo `:meter 6/8`) gobierna las líneas y etiquetas
  de compás, la vista de piano y el redondeo de clips MIDI importados o capturados.
  Las posiciones internas siguen en negras a PPQ=960. Cambiar BPM detiene la audición
  activa para preparar los reproductores con el nuevo tempo; no hay mapa de tempo.
  El patrón de 16 pasos de Session conserva su duración independiente.

- Al arrastrar un archivo MIDI o audio sobre Arrangement se muestra una sombra con su nombre
  y una línea en el punto de inserción, usando el zoom y scroll actuales. Desaparece al salir,
  cancelar o soltar. Su ancho es una guía de un compás; la duración real se obtiene al importar.
  Mantiene la colocación libre existente y la importación de un archivo por gesto.

- **Exportación offline del Arrangement**: `:export -n 93.[mp3,wave] -master` o
  `-individuals`, también desde catálogo. WAV/AIFF/FLAC con libsndfile, MP3 CBR/VBR con LAME,
  conversión VHQ con libsoxr. Frecuencia, bits, mono, pistas, rango, cola, Utility y destino
  configurables por comando; progreso y `:export.cancel`. Snapshot independiente de cambios
  en UI, mezcla de clips superpuestos y stems alineados. Pruebas reales de codecs/MIDI sin
  dispositivo. [Opciones, dependencias y límites](EXPORT.md). Esta mezcla es offline;
  la audición en vivo aún usa reproductores excluyentes.

- **Hay Utility**: primer dispositivo interno de Elevate (`hay.utility`), construido por
  descriptor/fábrica como `Component → Volume → FunDSP`. Ganancia 0–2x, bypass suavizado,
  reset e inserción/retirada en un único slot de **salida de audición** mediante `:utility`.
  Buscar «Hay Utility» en Add inserta ahora una instancia en el Track FX Tree. `:utility.gain 0.5`, `:utility.bypass`,
  `:utility.reset` y `:utility.remove` comparten operaciones con el panel, transporte y MIDI.
  Los atajos `[`/`]` y `b` actúan ahora sobre este dispositivo; insertan el slot si está vacío.
  Audio, piano y percusión preparan su procesador antes de abrir el stream y reciben un snapshot
  atómico coherente. Utility se aplica después del nivel de pista y el paneo: bypass nunca
  cancela Mute/Solo. Retirar restablece sus valores y lleva suavemente a ganancia unitaria.
  Abrir :utility de nuevo reutiliza ese mismo slot de salida, independiente del árbol FX.
  La persistencia del proyecto sigue pendiente.

- **Materialize MIDI** (`:materialize`, `:clip.materialize`, menú contextual y buscador):
  clic selecciona un MIDI; Ctrl+clic añade/quita clips y sus marcos indican la selección.
  Con uno, consolida el recorte visible en un patrón nuevo; con varios de la misma pista,
  crea un único clip desde el primer inicio al último final, conservando silencios y notas
  solapadas. Sustituye solo los seleccionados después de preparar el resultado completo.
  Requiere clips desbloqueados con igual ganancia, mute, pitch y loop, que se conservan;
  nombre/color proceden del primero cronológico. El resultado queda sin recorte: es la nueva
  fuente MIDI, no un render de audio ni un guardado del proyecto. Undo/redo sigue pendiente.

- Arrangement: clips de audio/MIDI conservados en sus carriles, arrastre, recorte no destructivo
  desde ambos bordes, cursor, zoom y scroll. Los clips se pueden solapar sin borrarse; en pantalla
  el activo queda encima y MIDI se dibuja sobre audio. La reproducción simultánea aún no está disponible.
- La categoría **Acciones** del buscador incluye operaciones de clip: duplicar, dividir en cursor,
  restaurar recorte, mute, bloqueo, ganancia, movimiento entre pistas y eliminación. Renombrar y
  color abren sus opciones contextuales. El objetivo es el clip bajo el cursor de edición; en un
  solapamiento audio/MIDI, MIDI tiene prioridad visual y de selección.
  Las mismas opciones se invocan en modo Command (`:`) con `:clip.duplicate`, `:clip.split`,
  `:clip.delete`, `:clip.mute`, `:clip.lock`, `:clip.restore_trim`, `:clip.gain.down`,
  `:clip.gain.reset`, `:clip.gain.up`, `:clip.track.previous`, `:clip.track.next`,
  `:clip.rename`, `:clip.gain` y `:clip.color`; aparecen en la ayuda.
- Reproducción de archivo y piano SoundFont, cada uno con Volume/bypass.
- Touchpad en Arrangement y piano roll: pellizcar hace zoom horizontal anclado al
  puntero; dos dedos desplazan ambos ejes, incluidos movimientos diagonales y deltas
  fraccionales. En Arrangement desplaza tiempo/pistas; en el piano roll, tiempo/registro
  MIDI (0–127). Ctrl+rueda hace zoom y Shift+rueda desplaza horizontalmente.
  En el piano roll, Windows/Super + dos dedos hacia abajo aumenta la altura de las
  filas; Alt + dos dedos hacia arriba aumenta el ancho. El gesto contrario reduce
  cada dimensión. Ambas escalas son independientes, mantienen el punto musical bajo
  el puntero y no editan tono, duración ni posición de notas. La escala vertical
  permite desde mostrar las 128 notas MIDI hasta ampliar ocho veces la altura inicial.
  El piano roll permite volver al encuadre inicial con «Ajustar vista» o `0`, también
  en ventana separada. Los gestos solo cambian la vista, no clips, notas ni transporte.
  Se utiliza `PinchEvent` de la revisión actual de GPUI: el backend y el sistema deben
  entregar el gesto (Wayland con pointer-gestures, X11 con XInput 2.4). La dirección
  natural y la inercia recibida proceden del sistema; no se añade un reloj de gestos.
- Menú de clips MIDI/audio con el mismo estilo del menú de inserción: renombrar,
  duplicar a continuación, dividir en el cursor, restaurar recorte, silenciar,
  bloquear posición/tamaño, color, ganancia, mover de pista y eliminar. Solo MIDI
  ofrece «Editar en Piano Roll». Las opciones son texto plano, sin cajas ni sombras,
  con resaltado discreto al navegar. El color se elige en un selector visual al pie
  del menú (siete muestras y «A» para heredar el de la pista), compartido con las pistas.
  Izquierda/derecha recorren sus muestras y Enter aplica. Ganancia tiene página secundaria.
  Flechas y Enter navegan/ejecutan; Escape vuelve o cierra. Al renombrar, escribir
  sustituye el nombre seleccionado; Ctrl+A selecciona todo, Enter guarda y Escape cancela.
  Clic derecho sobre un clip conserva el cursor de edición para dividir en él;
  las dos partes deben medir al menos 0,25 beats. El bloqueo impide dividir,
  restaurar, mover y redimensionar. Restaurar conserva la alineación temporal
  sin extender antes del beat cero. WAV conserva su nombre de archivo original.
  Loop y transposición no se ofrecen en el menú mientras su reproducción siga pendiente.
- Efecto de partículas del playhead aislado en UI, con particula-rs y fastrand.
- Vista `2` / `:pattern`: cuatro slots, 16 pasos iniciales, click/Enter para alternar
  notas y rueda para velocity. Modelo real en ticks, conservado al cambiar de vista.
- Audición del patrón en loop con cuatro percusiones sintetizadas (sin samples externos),
  plan compilado al pulsar Play, offsets por frame y transporte por frames. Space
  inicia/detiene desde la vista Pattern; el paso que suena se resalta.
- Biblioteca musical sin GPUI/CPAL: patrón variable, notas simultáneas, tempo constante,
  transporte en frames y plan NoteOn/NoteOff con offsets por buffer.
- Buscador tipo rofi con `[+]`, `a` o `:add`: filtra instrumentos, primitivos, componentes
  y acciones sin distinguir acentos. Navegación con ↑/↓, Enter y Escape.
- Selector de iconos con todas las acciones ejecutables del catálogo (incluido Materialize,
  edición de clips y vistas), más controles de transporte, pistas, instrumento y zoom.
  El Piano Roll ofrece sus controles visuales, ajuste de vista y zoom por eje; las herramientas
  de edición pendientes conservan la etiqueta «mock». Iconos y acciones tienen búsquedas
  independientes (Tab alterna, Enter asigna) y listas desplazables. Las acciones nuevas del
  catálogo aparecen automáticamente en Transporte; las asignaciones duran la sesión.
- El `[+]` asigna el piano a su pista y la cabecera muestra `Upright Piano KW`; Track FX
  muestra el instrumento de la pista activa. `a`/`:add` usan la pista seleccionada.
- Piano desacoplado de la vista: `src/instruments/piano.rs` posee reproductor, notas
  sostenidas y ratón; la vista pinta el descriptor de `elevate::catalog`.
- Demo offline `sequencer` y contratos de fronteras, IDs, errores e invariancia por tamaño de buffer.

- Piano separable en ventana propia del sistema (botón o `:detach` / `:dock`): sigue
  sonando al cambiar de vista; teclado y ratón funcionan en esa ventana.

- Teclado del ordenador como controlador MIDI reutilizable (`M` / botón / `:midi`):
  octava Z/X, velocity C/V, desactivado por defecto; indicador en la barra de estado.

- Parámetros del piano (RustySynth): Master, Volume/Expression/Pan/Modulación, sends de
  Reverb y Chorus, Sustain, Pitch bend, rango de bend, afinación fina/gruesa (RPN) y, al
  abrir el piano, Polifonía y Reverb+chorus. Click en la etiqueta restaura el valor por defecto.
- Selector de bancos SoundFont (`.sf2`) y presets (Bank/Program MIDI) dentro del editor
  de instrumento: descubre automáticamente `assets/soundfonts/` y `assets/soundfonts/freepats/`,
  muestra el nombre oficial y los créditos/licencia del autor leídos de cada `.readme.txt`.
- Componente `MidiClip` y grabación/recuperación MIDI (`MidiCapture`): mapea todo lo tocado
  (por controlador o teclado gráfico) a `NoteEvent` en ticks (`Pattern`), permite **Recuperar MIDI**
  retrospectivamente o **Grabar MIDI** (`●` / `:record` / `:capture`), inserta el clip en el carril
  del Arrangement y lo reproduce de forma sample-accurate a través del instrumento SoundFont.
- Track Headers configurables estilo REAPER TCP (`TrackHeaderConfig` en `src/ui/views/tracks.rs`)
  con perillas rotatorias en GPU (`src/ui/components/knob.rs`): Pan bipolar y Volumen unipolar en
  forma de perilla (`knobs`, activo por defecto) o barras (`bars`), arrastre vertical o rueda para ajustar y
  clic derecho para restaurar al centro/defecto; tamaño por defecto 28 px y arrastre vertical
  continuo, además de presets (`standard`, `compact`, `mixer`,
  `minimal`) y conmutadores de elementos en `◈ Hay` o `~/.jaybu.profile` (`TRACK_CONTROL_STYLE`,
  `TRACK_HEADER_LAYOUT`, `TRACK_HEADER_HIDE`, `KNOB_SIZE`). La cabecera agrupa visualmente
  identidad, mezcla, estado e instrumentos/FX; la configuración organiza los elementos con
  las mismas asociaciones. El color de pista se concentra en la franja lateral y los controles
  inactivos tienen menos contraste. Los bordes exteriores usan un valor neutro más firme y
  los espacios entre paneles y filas usan una superficie más oscura derivada del tema.
- Los controles M/S/R de los headers son operativos: M silencia, S deja audible solo la pista
  seleccionada (combinable con M) y R inicia/detiene la captura MIDI dirigida a esa pista.
  El contrato de audibilidad vive en `musical::track`; el stream existente recibe la ganancia
  mediante su adaptador, sin procesamiento ni GPUI en el callback.
- Pan y volumen de pista responden a rueda y click; el paneo se aplica en el adaptador estéreo
  (`audio::stereo_pan`) después de Volume, leyendo un valor atómico por bloque y sin asignar.
- Al abrir un clip MIDI, Arrangement queda arriba y el piano roll acoplado debajo. Separarlo
  en una ventana muestra el Mixer mock en el panel inferior; cerrar la ventana lo vuelve a acoplar.
- Puntero inmersivo en GPU (`src/ui/cursor_fx.rs` con `particula-rs` + `fastrand`): cinco modos
  seleccionables desde Configuración (`◈ Hay`), comandos (`:cursor fire|sparks|neon|stars|off`)
  o `~/.jaybu.profile` (`CURSOR_FX=fire`), incluyendo el efecto **Fueguito** por defecto.
- Tema y efecto del cursor se guardan automáticamente al seleccionarlos, también por comando.
  Configuración concentra estos controles; ya no están en la barra superior ni en las acciones
  asignables al Transporte. El perfil conserva dimensiones, claves ajenas y comentarios;
  se actualizan también los colores para que una paleta explícita antigua no anule el tema
  al reiniciar o recargar. Los errores de guardado se muestran en Configuración y en el estado.

## Limitaciones de esta audición

- Polifonía, Reverb+chorus y el cambio de archivo `.sf2` solo los lee RustySynth al crearse:
  cambiarlos con el piano abierto lo cierra (corta notas) y se reabre en el siguiente note-on.
  Cambiar de preset dentro del mismo banco sí se aplica en vivo por MIDI Bank/Program.
- Los parámetros del instrumento no se guardan al cerrar la app; el tema y el cursor sí.

- El plan activo es inmutable. Editar notas mientras suena no cambia el audio hasta
  detener y volver a pulsar Play. Los cambios de plan en caliente siguen pendientes; el loop global de audio está disponible en PR-04.
- Los cuatro instrumentos son síntesis provisional; todavía no cargan samples del usuario.
- Clip, piano y percusión abren streams excluyentes. Al iniciar uno se detiene el anterior;
  la audición del patrón se detiene al salir de su vista.
- Solo existe una instancia de piano. El buscador la asigna a una pista y la mueve al
  reasignarla; todavía no crea instancias nuevas. `Volume` y `Componente serial` son metadatos.
- Las acciones del catálogo usan comandos string provisionales; no son todavía el
  sistema de acciones definitivo.
- No hay chase de notas sostenidas, tempo editable desde la UI ni map de tempo.

## Todavía pendiente

- Docking estilo Visual Studio: paneles acoplables con imán/gravedad por tamaño,
  generalizar `DetachedPanel` a cualquier componente con salida de audio y recordar posición.

- Sustituir la percusión sintetizada por un sampler de cuatro samples con pool de voces.
- Un stream persistente común: el piano, el clip y la percusión aún son excluyentes.
- Cola RT acotada y snapshots coherentes; el piano conserva su canal mpsc provisional;
  la percusión aún no recibe actualizaciones de plan durante la reproducción.
- Tempo map, tempo editable en UI, chase de notas y cambios de plan en caliente; loop global de audio disponible.
- Guardado global `.dawproj`, undo/redo e integración multipista de instrumentos; remuestreo y mezcla de clips de audio disponibles.
- Edición MIDI avanzada del piano roll, launcher, automatización, MIDI físico y hosting CLAP/VST3.
- Mezcla por pista de audio con mute/solo/nivel/balance; buses generales y mezcla con instrumentos pendientes.

## Orden recomendado

1. **Varias instancias por pista:** la asignación única y el `[+]` ya están conectados;
   falta crear instancias independientes, decidir si cada una abre su stream o comparte
   motor y generalizar el catálogo. Separar identidad, datos de ejecución y vista.
2. **Motor único:** extraer procesadores del clip, piano y percusión, abrir una salida y
   definir el contrato `procesar bloque + eventos`. Resolver frecuencia de samples y motor.
3. **Puente RT:** aprobar `rtrb`, limitar comandos por bloque y decidir saturación,
   prioridad de Stop/AllOff y devolución de recursos fuera del callback.
4. **Samples reales:** sustituir la síntesis provisional por cuatro samples cargados
   fuera del callback y un pool de voces acotado, conservando los offsets verificados.
5. **Transporte conectado:** GPUI observa snapshots. Loop configurable y cambios de plan
   en caliente; BPM/compás editables ya llegan a las rutas actuales, pero falta un
   transporte único con cambios de plan sin detener la reproducción.
6. **Persistencia:** aprobar Serde/JSON y guardado temporal; versionar datos musicales
   separadamente del layout, nunca serializar voces/streams.
7. Swing/probabilidad/microtiming, más vistas y extensiones sobre esos contratos.

La futura arquitectura completa se mantiene como dirección; estos puntos son tareas
concretas, no módulos vacíos ya creados.

## Partículas: tokens opcionales

Añadir a `~/.jaybu.profile` y recargar con `R` o `:reload`:

```ini
PLAYHEAD_PARTICLE_RATE=120
PLAYHEAD_PARTICLE_LIFE=0.6
PLAYHEAD_PARTICLE_SIZE=2
PLAYHEAD_PARTICLE_SPEED=70
PLAYHEAD_PARTICLE_GRAVITY=60
PLAYHEAD_PARTICLE_COLOR=52c77a
CURSOR_FX=fire
TRACK_CONTROL_STYLE=knobs
KNOB_SIZE=28
TRACK_HEADER_LAYOUT=standard
# Ejemplo TCP a medida por filas: TRACK_HEADER_LAYOUT=name,mute,solo,add|vol,pct,pan
# Ejemplo ocultar elementos: TRACK_HEADER_HIDE=pan,pct
```

Rate en partículas/segundo; vida en segundos; tamaño/velocidad/gravedad en píxeles,
píxeles/segundo y píxeles/segundo². Rate 0 desactiva emisión. Pool máximo: 512.
Solo emite al avanzar el playhead visible; al detenerse la estela se desvanece.
Su reloj visual está separado del transporte musical.
