# Plan de PRs: kit Lua, multipista e Hilos

Fecha: 2026-11-11. Estado: PR-01 y PR-02 implementados; PR-03 tiene sus cinco
incrementos implementados, pruebas headless y funcionamiento confirmado por el usuario
en la aplicación. No se registraron pasos ni mediciones de la audición.
Acentuar y Subdividir usan paquetes recargables.
Los números identifican entregas de este plan, no PRs abiertos en GitHub. El repositorio
todavía no tiene remoto configurado.

Entrega complementaria: **Hay Acent**, cuatro presets de scripts MIDI en `1fc521f`
([uso](LUA.md#hay-acent)). Usa el ejecutor de scripts de archivo; su formato es
independiente del registro de paquetes. Tiempo desde la primera lectura registrada hasta el commit: 3 min 42 s
(2026-11-11, 06:40:06–06:43:48 UTC), incluidas comprobaciones y espera de permisos,
sin el commit posterior de documentación/seguimiento. Pasaron las 4 pruebas de
Hay Acent y las 7 del ejecutor existente; `scripts/check.sh` se detuvo en diferencias
de formato previas de archivos ajenos. No se hizo audición manual.

## Objetivo

Poder crear herramientas, componentes, configuraciones de pistas y comportamientos
musicales principalmente en Lua, sobre un motor que reproduzca varias pistas y aloje
instancias independientes de plugins. «Hilos» será el contenedor de cadenas y ramas;
«Tejer» distribuirá notas entre sus instrumentos mediante round robin.

Cada PR debe dejar una capacidad que se pueda usar o escuchar. Las abstracciones nuevas
se introducen con su primer consumidor real. No se considera una entrega terminada
una colección de interfaces, módulos vacíos o botones sin comportamiento.

## Punto de partida comprobado

| Existe hoy | Límite relevante para este plan |
|---|---|
| Lua 5.4 mediante `mlua`, edición de notas/CC y lógica de empuje | Las herramientas integradas se registran en Rust; no existe un kit general de componentes |
| Pistas, clips e IDs; cadenas FX recursivas serie/paralelo | El paralelo de audio ejecuta las ramas consecutivamente en CPU |
| Host VST3 de instrumentos y efectos, editores y escaneo separado | Un instrumento VST3 por sesión; faltan estado persistido, PDC y buses auxiliares |
| CPAL, `rtrb`, planes de FX preparados y publicación con retiro fuera del callback | Los reproductores de audición siguen siendo separados; falta mezcla multipista live |
| Exportación offline con mezcla de fuentes compatibles | Usa otro recorrido de render y rechaza VST3 |
| Documento Loro de pistas, FX y MIDI | No representa una sesión completa con audio y mezcla; no habilitar Abrir/Guardar global a partir de ese subconjunto |

Referencias: [guía de edición](EDITING_GUIDE.md), [arquitectura](ARCHITECTURE.md),
[estado](PROTOTYPE.md), [filosofía de componentes](../README.md), [Lua](LUA.md),
[VST3](VST3.md) y [puertos/modulación previstos](VISUAL_MODULATION.md).
Hay secciones históricas en esos documentos: contrastar sus afirmaciones con el código
al iniciar cada PR y corregir las que afecte el cambio.

## Decisiones que guían las tareas

- **Lua como herramienta principal de extensión.** Metadatos, acciones, composición,
  presets, reglas musicales y presentación declarativa pertenecen al kit. Rust aporta
  operaciones reutilizables, validación, adaptadores y ejecución del motor.
- **Rendimiento medido.** No trasladar una regla musical a Rust solo por suponer que Lua
  será lento. El PR-08 introduce una prueba utilizable y mediciones del recorrido real.
  Lua general no obtiene garantías de tiempo real solo por tener un timeout.
- **Dos contratos de ejecución explícitos.** Edición/preparación permite Lua general
  dentro del sandbox existente. Procesamiento live exige un perfil acotado, preparado
  y verificado. No enviar una nota a un worker y esperar su respuesta desde el callback.
- **Un motor y un reloj.** GPUI representa el estado. El transporte avanza por frames;
  los adaptadores de audición se van integrando, sin mantener dos transportes activos.
- **Identidad independiente.** Separar tipo de plugin, instancia, pista, puerto, parámetro
  y nota. Dos instancias del mismo plugin tienen estado diferente. Un «Hilo» musical
  no exige crear un hilo del sistema operativo.
- **Contrato independiente de VST3/CLAP.** Puertos tipados, eventos con offset de frame,
  contexto temporal, parámetros, latencia, colas de sonido y estado serializable. Los
  tipos de cada formato permanecen en su adaptador. Prever valor base y modulación
  como conceptos distintos, sin prometer capacidades que el plugin no exponga.
- **Recursos preparados.** Carga, compilación del grafo, E/S y destrucción costosa fuera
  del audio; buffers y colas con capacidad definida. Los desbordamientos deben tener
  una respuesta explícita que preserve las liberaciones de notas.
- **Dependencias.** Reutilizar CPAL, `rtrb`, FunDSP, `mlua`, Loro y `vst3`. CLAP se aborda
  en PR-15; cualquier dependencia nueva necesita la revisión y autorización previstas
  en `AGENTS.md`. El plan no autoriza instalarla por adelantado.

Una descripción de grafo producida por Lua puede convertirse en un plan nativo. Esto
no significa compilar cualquier programa Lua a código nativo. El contrato live de
PR-08 se evalúa por separado.

## Secuencia y valor de cada PR

El orden de la tabla es el recorrido recomendado. La columna de dependencias expresa
prerrequisitos técnicos; no autoriza empezar varios cambios a la vez.

| PR | Entrega visible | Depende de |
|---|---|---|
| PR-01 | Añadir una herramienta Lua al catálogo sin recompilar | — |
| PR-02 | Crear y configurar varias pistas con un script | PR-01 |
| PR-03 | Insertar un componente de audio definido en Lua y mover sus controles | PR-01 |
| PR-04 | Escuchar clips de varias pistas juntos | PR-03 |
| PR-05 | Escuchar instrumentos internos independientes en varias pistas | PR-04 |
| PR-06 | Usar varias instancias VST3 con el mismo transporte | PR-05 |
| PR-07 | Combinar instrumentos y efectos dentro de Hilos | PR-02, PR-03, PR-06 |
| PR-08 | Aplicar un procesador musical Lua y medir su coste real | PR-01, PR-05 |
| PR-09 | Tejer notas entre instrumentos con reglas Lua | PR-07, PR-08 |
| PR-10 | Guardar, recuperar y duplicar un Hilo con sus plugins | PR-09 |
| PR-11 | Mantener alineadas pistas y ramas con plugins que añaden latencia | PR-07 |
| PR-12 | Mezclar por buses y conectar envíos y sidechains | PR-11 |
| PR-13 | Exportar el mismo procesamiento que se escucha | PR-10, PR-12 |
| PR-14 | Aprovechar varios núcleos en proyectos con ramas independientes | PR-08, PR-12, PR-13 |
| PR-15 | Insertar plugins CLAP en las mismas pistas e Hilos | PR-06, PR-10, PR-12, PR-13, PR-14 |

## Estimación inicial

Rangos de esfuerzo para una persona con experiencia en Rust y audio, trabajando a tiempo
completo. Incluyen implementación, comprobaciones localizadas, integración y documentación
propia del PR. No incluyen espera por revisión, dependencias externas, disponibilidad de
plugins de prueba ni bloqueos del entorno. Son una primera estimación y deben recalibrarse
con el trabajo real de cada entrega; no son fechas comprometidas.

| PR | Jornadas-persona |
|---|---:|
| PR-01 | 2–4 |
| PR-02 | 3–5 |
| PR-03 | 4–7 |
| PR-04 | 6–10 |
| PR-05 | 5–8 |
| PR-06 | 5–9 |
| PR-07 | 4–8 |
| PR-08 | 5–10 |
| PR-09 | 4–8 |
| PR-10 | 5–9 |
| PR-11 | 4–8 |
| PR-12 | 4–7 |
| PR-13 | 5–9 |
| PR-14 | 5–10 |
| PR-15 | 7–14 |
| **Total secuencial** | **68–127 (aprox. 14–26 semanas)** |

El cálculo secuencial supone una sola persona y cinco jornadas por semana. La concurrencia
puede reducir el tiempo de calendario solo cuando las dependencias estén satisfechas; la
integración y resolución de conflictos añaden trabajo y el esfuerzo total no desaparece.

Primer hito: PR-01 a PR-03 hacen útil el kit Lua. PR-04 a PR-07 entregan multipista
y composición de instrumentos. PR-09 entrega Tejer. Los últimos PRs amplían fidelidad,
interoperabilidad y capacidad de procesamiento.

## Instrucciones comunes para la IA que implemente cada PR

1. Leer `AGENTS.md` y las filas aplicables de `docs/EDITING_GUIDE.md`. Revisar `git status`
   y el estado de los prerrequisitos: actualmente hay muchos cambios locales previos.
   No incluirlos en un commit del PR ni sobrescribirlos.
2. Implementar un único PR seleccionado. Si ya está parcialmente resuelto, reutilizar
   lo existente. Las rutas de abajo son puntos de entrada, no una obligación de crear
   todos los archivos citados ni un diseño cerrado.
3. Mantener `musical` y `catalog` headless; `audio` e `instruments` no importan GPUI.
   La UI y Lua llaman las mismas operaciones del modelo/controlador.
4. Añadir solo pruebas que protejan un contrato o un fallo real. Una prueba del grafo
   debe verificar sonido, eventos o tiempos; contar nodos no demuestra procesamiento.
5. Hacer una demostración del comportamiento indicado. Registrar qué se comprobó en
   memoria, qué se escuchó y qué sigue pendiente; no intercambiar esas afirmaciones.
6. Ejecutar primero comprobaciones localizadas y después la verificación requerida por
   el proyecto. Distinguir fallos previos de regresiones introducidas; no arreglar todo
   el árbol como parte de un PR pequeño.
7. Actualizar mapa de archivos, contratos y estado cuando cambien. Hacer commits
   pequeños que incluyan solo el alcance elegido.
8. Cada elemento de la lista de commits del tablero `index.html` representa un commit
   planeado. Tras integrar ese commit, el coordinador marca el elemento correspondiente
   con `data-status="done"` y actualiza el tablero en un commit separado `docs(progress)`.
   Así el commit funcional mantiene una sola intención y varios agentes no editan el
   tablero concurrentemente desde sus ramas. Si se cambia la secuencia de commits, actualizar
   también el número y descripción del tablero. No marques un commit solo por haberlo creado
   en una rama: debe estar integrado en la rama acordada.
9. No marcar el PR como completo si solo existe infraestructura o una alternativa que
   cambia su valor anunciado. Si necesita dividirse, conservar una demostración útil
   en cada entrega y actualizar aquí sus dependencias.

## PR-01 — Registrar herramientas Lua sin recompilar

**Título sugerido:** `feat(lua): cargar herramientas desde paquetes declarativos`

**Avance:** 4 de 4 commits: contrato `5bec9fe`, registro `7ce9d63`, ejecución
`0fd3849` y migración de herramientas `8d6b0aa`. [Contrato v1](LUA_PACKAGES.md).
Tiempo transcurrido hasta el primer commit documental: **12 min 50 s**
(2026-11-11, 05:45:16–05:58:06 UTC), incluida la espera de permisos. El sondeo previo
y el commit posterior de seguimiento del tablero no están incluidos. Es una medición
del paso documental; no extrapolar ese tiempo a los commits de implementación.

**Valor:** añadir una herramienta MIDI nueva copiando un paquete Lua, con entrada en
`[+]`, comando y formulario generado desde sus parámetros.

El commit 3 (`0fd3849`) conecta `midi.tools.reload` y
`midi.tool <id>` a catálogo, formulario y controlador MIDI. La recarga conserva
el registro ante errores; los formularios antiguos se rechazan y los trabajos
enviados conservan su fuente. [Uso y estado](LUA.md#herramientas-registradas).

Comprobaciones del commit 3: 91 pruebas headless aprobadas, compilación correcta y
62/63 pruebas del binario. Persiste el fallo anterior de perfiles (ancho 180 frente
a 237); `scripts/check.sh` se detiene en formato previo. Se añadieron 16 pruebas.
No hubo comprobación interactiva. Tiempo desde la marca registrada hasta el commit:
48 min 14 s (22:53:14–23:41:28 UTC), incluidas comprobaciones y espera de permisos;
sin exploración inicial ni seguimiento documental posterior.

El commit 4 (`8d6b0aa`) distribuye `hay.midi.accent` y `hay.midi.subdivide` como
paquetes; mantiene `midi.script.accent`, `midi.subdivide`, el botón y los gestos del
piano roll. Los alias usan el registro publicado y conservan la implementación
integrada hasta la primera recarga válida. No se ejecutaron pruebas ni compilación
en este incremento; tampoco hubo comprobación interactiva.

**Puntos de entrada:** `src/scripting/packages/`, `src/scripting/{notes,controls,output,target}.rs`,
`src/catalog/{entry,tools}.rs`, `src/actions{,/owned}.rs`,
`src/ui/{lua_tools,scripting,action_form,palette}.rs`, `assets/scripts/tools/midi/`,
`assets/scripts/midi/`.

**Tareas**

- [x] Definir paquete v1: ID estable, versión de API, nombre, parámetros y función de
  edición. Documentar su ubicación y carga explícita; evitar escanear disco al dibujar.
- [x] Crear un registro de metadatos dinámicos que conviva con las acciones nativas.
  El catálogo no instancia plugins ni conserva una VM como descriptor.
- [x] Adaptar el formulario y despacho actuales para usar el mismo contrato validado.
- [x] Migrar Acentuar y Subdividir como paquetes y conservar los alias actuales.
  Recargar publica ambas herramientas; un error conserva el último registro válido.
- [x] Mantener límites, validación por IDs y descarte de resultados obsoletos de la
  edición MIDI. Un paquete duplicado o con API incompatible produce un error claro.

**Aceptación:** una segunda herramienta aparece y transforma una selección sin tocar
Rust ni recompilar. Un script inválido deja intacto el clip. Las acciones existentes
siguen resolviendo sus parámetros por la misma ruta. La implementación está lista;
queda comprobarla en la aplicación.

## PR-02 — Automatizar pistas desde Lua

**Título sugerido:** `feat(lua): editar pistas mediante lotes validados`

**Contrato e implementación:** [snapshot, operaciones, revisión, handles, errores,
Loro y límites](LUA_TRACK_BATCHES.md). `track.script` y `track.template.two`
están disponibles; Loro sigue sin conexión persistente a la UI.

**Valor:** ejecutar una plantilla Lua que cree dos pistas, las nombre y configure sus
niveles o cadenas, sin repetir operaciones a mano.

**Puntos de entrada:** `src/musical/track{,/management}.rs`, `src/musical/fx/`,
`src/ui/{track_actions,audio_actions}.rs`, `src/scripting/`, `src/project/`.

**Tareas**

- [x] Exponer un snapshot con IDs y operaciones tipadas para pistas, mezcla y FX ya
  soportados. Compartir la validación con las operaciones de teclado y ratón.
- [x] Hacer que Lua produzca un lote de cambios. Validar el resultado completo contra
  la revisión de origen antes de publicarlo; no dejar medio lote aplicado.
- [x] Resolver referencias entre pistas nuevas del mismo lote mediante handles
  temporales y devolver sus IDs definitivos al confirmar.
- [x] Añadir una plantilla incluida y errores asociados a la operación que falló.
  Conservar clips, selección y mapeos al editar la estructura.

**Aceptación:** la plantilla produce las dos pistas configuradas. Un destino inexistente,
un ciclo o una edición concurrente incompatible rechaza todo el lote. Esta atomicidad
no se anuncia como undo/redo global, que todavía no existe.

## PR-03 — Crear un componente audible desde Lua

**Título sugerido:** `feat(lua): componer dispositivos de audio con primitivas existentes`

**Contrato inicial:** [descriptor, puertos, grafo, macro, límites y recarga](LUA_AUDIO_COMPONENTS.md).
Implementación headless y en Track FX disponible. La prueba de audio del rack y la
recarga pasan; el usuario confirmó que funciona en la aplicación y aprobó el estilo.
No se registraron pasos ni mediciones de la audición.

**Valor:** insertar desde `[+]` un componente Lua con dos etapas Volume y una macro de
ganancia que realmente modifica el audio.

**Puntos de entrada:** `src/devices/{mod,utility}.rs`, `src/component.rs`,
`src/primitives.rs`, `src/musical/fx/`, `src/audio/effects/`, `src/ui/fx/`.

**Tareas**

- [x] Extender el paquete con puertos, parámetros de ID estable, valores iniciales,
  grafo y controles declarativos; componer primitivas existentes en serie/paralelo.
- [x] Validar tipos, ciclos, límites de tamaño y referencias antes de preparar instancias.
  Mantener separadas descripción editable y memoria de procesamiento.
- [x] Introducir el contrato común de procesador con el mínimo necesario para este
  componente: preparación, bloques, parámetros, reset y latencia cero declarada.
  Extenderlo con consumidores reales en los siguientes PRs.
- [x] Generar controles con los componentes GPUI actuales y compartir el direccionamiento
  de parámetros con comandos. Aplicar suavizado donde corresponda.
- [x] Cargar dos instancias independientes. Preparar una recarga fuera del callback y
  conservar el componente anterior si falla.

**Aceptación:** el componente suena, su macro altera ambas etapas y el bypass funciona.
Cambiar una instancia no afecta a la otra. El audio coincide con la composición nativa
equivalente tras el suavizado; no se ejecuta Lua por muestra para construir esa cadena.

## PR-03B — Filtro nativo reutilizable y kit Lua v2

Incremento aprobado antes de PR-04: SVF de FunDSP existente, descriptor Lua v2
compatible con v1, targets por nodo/parámetro, controles choice/log y respuesta
visual. Paquete Hay Filter en Track FX; sin nuevas dependencias.

Implementado: `dcdfc0e` (núcleo/contrato/pruebas) y `04080be` (integración UI/uso).
`scripts/check.sh` completo pasó: formato, pruebas headless, 66 pruebas del
binario y compilación. Las cuatro pruebas específicas verifican sonido, curva,
aislamiento y validación. La audición interactiva queda pendiente. [Uso y límites](LUA_AUDIO_COMPONENTS.md#incremento-pr-03b-hay-filter-y-kit-v2).

Corrección posterior `d3941aa`: el piano compartido aplica el rack de la pista
del clip MIDI durante carga/reproducción; vuelve al de su pista asignada para
notas en vivo. Regresión comprobada con audio por Routing y cambios de corte.
`scripts/check.sh` completo pasó, incluidas 67 pruebas del binario.

En PR-04 comprobar dos pistas con el filtro solo en una, respetando mute/solo/bypass,
y preparar los FX con la frecuencia **real** del dispositivo. Con el transporte
compartido se podrá añadir el LFO sincronizado; este incremento no lo implementa.

## PR-04 — Mezclar clips de audio bajo un único transporte

**Avance:** 2 de 4 commits (`3592aa4`, `549688e`). Fuente de clip sin stream integrada en
el reproductor actual; conserva preparación, remuestreo, cancelación y FX.
`scripts/check.sh` completo pasó, incluidas pruebas de la fuente sin dispositivo
para recortes, particiones, seek y muestras remuestreadas. Audición manual pendiente.

Segundo incremento: Play en contexto de audio toma todos los clips de Arrangement,
prepara fuentes por pista y suma en un stream. FX por pista antes del master;
controles existentes redirigidos por TrackId como integración mínima. El commit 3
completará la validación de mute/solo/nivel/pan y sus interacciones. Pruebas de suma
con margen, protección única, offsets/bloques y filtro aislado pasan. Suite completa
correcta (72 pruebas del binario); audición manual pendiente. MIDI, Session,
loop/seek en caliente y preparación FX a frecuencia real siguen pendientes.

**Título sugerido:** `feat(audio): reproducir varias pistas de audio en un motor compartido`

**Valor:** pulsar Play y escuchar dos clips en pistas distintas, con mute, solo, volumen
y paneo independientes.

**Puntos de entrada:** `src/audio/{playback,output,assets}.rs`, `src/audio/effects/`,
`src/ui/{file_playback,audio_actions,transport}.rs`, `src/musical/{time,scheduler}.rs`.

**Tareas**

- [x] Extraer una fuente de clip que procese bloques sin poseer su propio stream CPAL.
  Reutilizar decodificación, remuestreo y cancelación de preparación actuales.
- [x] Crear el motor con un stream, posición por frames y suma en flotante con margen
  interno; aplicar protección de salida una sola vez en el master.
- [ ] Preparar fuera del callback fuentes, buffers, rutas y plan de ejecución. Reutilizar
  la publicación existente cuando encaje y retirar recursos fuera del audio.
- [ ] Procesar todos los clips del intervalo, incluidos solapamientos, offsets, recortes
  y silencios. Insertar los FX existentes en su pista correcta.
- [ ] Conectar Play, Stop, seek y loop al mismo transporte. Durante la transición,
  las audiciones antiguas se detienen antes de iniciar este motor y viceversa.

**Aceptación:** dos clips simultáneos se mezclan con la suma esperada; mute/solo no
silencia pistas ajenas. Cambiar el tamaño de bloque no desplaza inicios ni loops.
La UI refleja frames del motor y el callback no decodifica ni prepara recursos.

## PR-05 — Instancias internas y MIDI por pista

**Título sugerido:** `feat(audio): reproducir instrumentos independientes por pista`

**Valor:** dos pistas con el mismo tipo de instrumento interpretan clips distintos a
la vez, junto a pistas de audio.

**Puntos de entrada:** `src/instruments/{piano,sampler}.rs`,
`src/audio/piano/{backend,prepare,render}.rs`, `src/audio/sampler/`,
`src/musical/{track,scheduler,midi}.rs`, `src/ui/{audio_actions,sampler,palette}.rs`.

**Tareas**

- [ ] Sustituir la asignación global por instancias con ID y estado de ejecución propios.
  Extraer la síntesis del reproductor que actualmente posee el stream.
- [ ] Integrar piano y sampler en el motor por bloques; compartir recursos inmutables
  cuando sea seguro, conservando voces y controles independientes.
- [ ] Añadir eventos con offset de frame y contexto del transporte. Programar todos
  los clips MIDI activos, manteniendo el scheduler musical existente.
- [ ] Enrutar teclado y MIDI físico por ID de instancia. Resolver liberaciones al
  cambiar foco, detener, buscar otra posición o eliminar una instancia.
- [ ] Integrar la audición provisional de percusión en la misma salida/reloj, preservando
  su condición de síntesis de audición y su plan inmutable hasta un cambio explícito.

**Aceptación:** dos samplers con muestras diferentes y un clip de audio suenan juntos;
cambiar preset, sustain o notas de uno no modifica el otro. Stop no deja notas colgadas
y el cambio de foco conserva el destino correcto de cada liberación.

## PR-06 — VST3 como procesador del motor común

**Título sugerido:** `feat(vst3): integrar instancias independientes en el motor multipista`

**Valor:** insertar dos instrumentos VST3 del mismo tipo y efectos por pista, con audio
y transporte sincronizados.

**Puntos de entrada:** `src/audio/vst3/{mod,lifecycle,buffers,events,parameters,process}.rs`,
`src/audio/effects/`, `src/instruments/vst.rs`, `src/ui/vst/`, `src/catalog/plugins.rs`.

**Tareas**

- [ ] Adaptar el host existente al contrato común y a IDs de instancia. Conservar su
  escaneo y el ciclo de vida de editores; no escribir un segundo host.
- [ ] Pasar posición absoluta, tempo, compás, estado de reproducción y loop desde el
  motor, indicando solamente los campos válidos del contexto VST3.
- [ ] Traducir notas y cambios de parámetros con offsets correctos. Representar buses,
  latencia y cola de sonido en el adaptador aunque inicialmente se use el bus principal.
- [ ] Preparar y destruir fuera del callback. Una instancia se procesa una sola vez
  por bloque y nunca se comparte simultáneamente entre rutas de ejecución.
- [ ] Hacer accesibles desde Lua las operaciones de creación/conexión/configuración
  soportadas, mediante el controlador de carga y handles, sin exponer interfaces COM.

**Aceptación:** dos instancias tienen audio y parámetros independientes; un plugin
sincronizado a tempo sigue Play/Stop/seek. Una carga fallida no reemplaza otra instancia.
Separar pruebas deterministas con procesador de prueba de la compatibilidad manual con
plugins reales; mantener visibles los límites de exportación y estado aún pendientes.

## PR-07 — Hilos con capas de instrumentos y efectos

**Título sugerido:** `feat(hilos): componer ramas de instrumentos y efectos`

**Valor:** una pista puede disparar dos instrumentos en capas, con efectos y nivel por
rama, usando un Hilo definido desde Lua o desde el árbol existente.

**Puntos de entrada:** `src/musical/fx/`, `src/musical/track.rs`,
`src/audio/effects/`, `src/ui/fx/`, `src/scripting/` y motor de PR-04 a PR-06.

**Tareas**

- [ ] Ampliar la composición actual para alojar eventos, instrumentos y audio en
  puertos tipados. Conservar IDs y semántica de las cadenas FX existentes.
- [ ] Implementar ramificación de eventos, procesamiento por rama y suma de salidas.
  Diferenciar una rama de audio de una rama que contiene un instrumento.
- [ ] Exponer la misma composición en Lua y en Track FX, con controles de nivel/bypass.
- [ ] Compilar las dependencias y buffers fuera del callback. Inicialmente ejecutar
  el plan de forma secuencial y rechazar conexiones incompatibles o ciclos.
- [ ] Definir eliminación/bypass de ramas con notas activas y preservar colas de sonido
  según una política explícita; ninguna rama puede liberar notas de otra.

**Aceptación:** un Hilo de dos instrumentos produce una capa audible; editar el nivel
de una rama conserva la otra. Una ruta inválida mantiene el último plan válido.
No anunciar ejecución multinúcleo en este PR.

## PR-08 — Procesador musical Lua y medición de ejecución

**Título sugerido:** `feat(lua): incorporar procesamiento musical con presupuesto verificable`

**Valor previsto:** insertar un transpositor Lua que procese notas en vivo y permita
medir el coste de Lua dentro de un recorrido musical real.

**Puntos de entrada:** `src/scripting.rs`, `src/scripting/`, motor común,
`src/musical/{scheduler,midi}.rs`, `examples/midi_edit_latency.rs` como referencia de diagnóstico.

**Tareas**

- [ ] Definir un perfil live separado del sandbox de edición: VM preparada por instancia,
  estado con capacidad fijada y acceso solo a operaciones de coste acotado.
- [ ] Revisar el camino efectivo de `mlua` y Lua: llamadas, asignaciones, GC, locks,
  errores y destrucción. Reutilizar buffers/eventos; evitar tablas nuevas por nota,
  E/S, carga de módulos y llamadas C de duración no acotada durante el procesamiento.
- [ ] Proporcionar un procesador de notas genérico, sin lógica Rust específica del
  transpositor. El host mantiene identidades y correspondencia entre notas originales
  y transformadas; Lua expresa la transformación.
- [ ] Asegurar que un error o exceso de presupuesto no deja notas colgadas: conservar
  liberaciones, desactivar la instancia afectada y notificar fuera del callback.
- [ ] Medir en release con 1, 8 y 32 instancias, ráfagas MIDI y buffers 64/128/256/512.
  Registrar equipo, frecuencia, carga, p50/p99/p99.9, máximo observado y plazos incumplidos,
  comparando con el mismo recorrido sin script. Recolectar métricas sin E/S en audio.

**Aceptación:** el transpositor recibe notas live, devuelve eventos con el mismo offset
y libera correctamente notas solapadas. El informe incluye trabajo y memoria acotados
y resultados de carga; un promedio bajo por sí solo no demuestra aptitud para RT.

**Si el perfil no cumple:** entregar como alcance revisado un procesador Lua aplicado
a clips antes de reproducir, con su medición y limitación explícitas. No marcar como
resuelto el procesamiento live. Revisar PR-09 para ejecutar reglas expresables mediante
operaciones acotadas del kit; no atribuir ejecución Lua live a esa alternativa ni añadir
un runtime distinto sin autorización. La medición es la que justifica esa decisión.

## PR-09 — Tejer mediante round robin

**Título sugerido:** `feat(hilos): distribuir notas con el modo Tejer`

**Valor:** tocar una secuencia y oír cómo las notas alternan entre instrumentos, con
patrones de distribución configurables en Lua.

**Puntos de entrada:** Hilos de PR-07, procesadores musicales de PR-08,
`src/musical/{midi,note_expression}.rs`, `src/ui/fx/` y paquetes Lua.

**Tareas**

- [ ] Implementar Tejer como paquete del kit, reutilizando el procesador de eventos.
  Por defecto, avanzar una posición por NoteOn aceptado; desempatar eventos simultáneos
  de forma estable. Cambiar reglas/secuencias no requiere modificar Rust.
- [ ] Mantener un registro acotado de notas activas: identidad, canal, tono y rama
  elegida. Un NoteOff y su expresión siguen su destino original aunque cambie el turno.
- [ ] Definir emparejamiento de notas repetidas sin ID en MIDI 1.0 y remapeo a los IDs
  que soporte cada plugin. No usar solamente el tono como identidad de una voz.
- [ ] Definir sustain, CC y pitch bend: controles de canal alcanzan las ramas que los
  necesitan; una rama recién elegida recibe el estado vigente. Stop/panic libera todas.
- [ ] Conservar las colas de las ramas que dejan de recibir NoteOn. Definir reinicio
  del contador en Play/seek y comportamiento al retirar una rama durante una nota.

**Aceptación:** con tres instrumentos, seis notas recorren A-B-C-A-B-C. Acordes,
repeticiones del mismo tono, sustain y liberaciones fuera de orden no dejan voces
colgadas ni cortan las colas. El resultado no cambia al particionar el mismo flujo
en bloques distintos. Si PR-08 revisó su alcance, explicar antes el contrato usado aquí.

## PR-10 — Presets y duplicación con estado independiente

**Título sugerido:** `feat(presets): guardar y restaurar componentes e Hilos`

**Valor:** guardar un Hilo con sus instrumentos, reglas Lua y parámetros, volver a
cargarlo y duplicarlo conservando su sonido sin compartir estado mutable.

**Puntos de entrada:** `src/project/`, `src/audio/vst3/`, `src/ui/fx/`,
`src/scripting/` y controladores de carga de dispositivos.

**Tareas**

- [ ] Definir un preset versionado de componente/Hilo: topología, IDs de tipos, versión
  de paquetes Lua, parámetros, recursos referenciados y estado serializable permitido.
- [ ] Implementar captura/restauración de estado VST3 respetando su ciclo de vida e
  hilos permitidos. No consultar estado mutable del procesador concurrentemente.
- [ ] Preparar instancias y recursos fuera del audio; asignar IDs nuevos al duplicar
  y remapear conexiones internas. No guardar voces, streams, punteros ni una VM completa.
- [ ] Publicar solo una restauración válida. Conservar información recuperable de
  plugins/paquetes ausentes y explicar el fallo sin sustituirlos por otro sonido.
- [ ] Exponer guardar/cargar/duplicar mediante operaciones comunes a Lua y UI. Guardar
  archivos de forma atómica y cargar sin ejecutar scripts fuera de su contrato.

**Aceptación:** guardar, quitar y restaurar recupera configuración y estado del plugin;
una copia puede editarse sin modificar el original. Un preset inválido no daña la
sesión activa. El preset no se presenta como guardado completo de proyecto: audio,
mezcla y apertura global requieren su propio incremento posterior.

## PR-11 — Compensación de latencia

**Título sugerido:** `feat(audio): alinear pistas y ramas mediante compensación de latencia`

**Valor:** mezclar un camino directo con otro que contiene un plugin con latencia sin
desalinearlos por ese retraso de procesamiento.

**Puntos de entrada:** compilador/ejecutor del grafo, contrato común de procesador,
`src/audio/vst3/`, `src/audio/effects/`.

**Tareas**

- [ ] Recoger latencias declaradas y calcular el retraso acumulado por camino. Insertar
  compensaciones preasignadas antes de cada punto de suma.
- [ ] Separar latencia de procesamiento y duración de colas; transportar eventos y
  contexto de forma coherente con la línea temporal del grafo.
- [ ] Preparar cambios de latencia fuera del audio, incluidos cambios de configuración
  de plugin, bypass y sample rate. Documentar qué cambios requieren detener/repreparar.
- [ ] Mostrar la latencia resultante y definir la política para entrada MIDI monitorizada;
  no prometer compensar un evento live antes de que llegue.

**Aceptación:** impulsos de dos pistas y dos ramas se alinean al compensar un procesador
de retraso conocido. Cambiar su latencia actualiza el plan sin asignar buffers en audio.
Incluir bloques parciales, bypass y cambio de frecuencia en las comprobaciones.

## PR-12 — Buses, envíos y sidechain

**Título sugerido:** `feat(routing): procesar buses y entradas auxiliares`

**Valor:** enviar dos pistas a un bus con FX común y usar una tercera como sidechain de
un efecto compatible.

**Puntos de entrada:** `src/musical/track.rs`, modelo de rutas, compilador del grafo,
`src/audio/vst3/{buffers,lifecycle}.rs`, `src/ui/fx/`, kit Lua de pistas.

**Tareas**

- [ ] Convertir grupos con función de mezcla en buses reales, con suma y efectos una
  sola vez por bloque. Mantener separada la presentación visual de esa función DSP.
- [ ] Añadir envíos pre/post fader con nivel y destinos por IDs/puertos, expuestos al kit.
- [ ] Negociar y conectar entradas auxiliares del plugin; validar número de canales y
  capacidades. Mantener los buses inactivos en el estado correcto del formato.
- [ ] Extender orden de ejecución y PDC a esas dependencias. Rechazar feedback en este
  incremento y explicar la conexión que forma el ciclo.

**Aceptación:** ambas pistas pasan por el FX del bus una sola vez. Un procesador de
prueba responde a su entrada auxiliar y un plugin real compatible confirma sidechain;
la señal auxiliar no se suma accidentalmente al master. Una ruta cíclica no se publica.

## PR-13 — Exportar el mismo grafo

**Título sugerido:** `feat(export): renderizar Hilos y plugins con el motor compartido`

**Valor:** exportar una mezcla con Tejer, instrumentos y efectos VST3, incluyendo sus
colas y la compensación de latencia.

**Puntos de entrada:** `src/audio/export/{mod,sources,render,job}.rs`, `src/ui/export.rs`,
motor común y mecanismos de estado de PR-10.

**Tareas**

- [ ] Sustituir las diferencias de procesamiento live/offline por el mismo contrato
  de grafo, con instancias separadas y snapshot coherente. Exportar no comparte voces,
  VMs ni procesadores mutables con la reproducción.
- [ ] Configurar el modo offline de los plugins cuando proceda; conservar tiempos,
  eventos, automatización admitida, rutas y parámetros de mezcla.
- [ ] Definir pre-roll, recortes de rango, colas, PDC y reinicio/semilla de reglas Lua.
  No omitir dispositivos no soportados para producir un archivo aparentemente válido.
- [ ] Reutilizar codecs, cancelación y publicación atómica actuales; mantener exportación
  sin abrir dispositivo físico.

**Aceptación:** una sesión determinista con Hilos produce audio equivalente en render
live capturado y offline dentro de una tolerancia definida, con independencia del bloque.
Verificar estados/tiempos en plugins no deterministas sin exigir igualdad bit a bit.
Cancelar no modifica la reproducción ni deja una exportación final incompleta.

## PR-14 — Procesamiento multinúcleo por dependencias

**Título sugerido:** `perf(audio): repartir ramas independientes entre trabajadores de audio`

**Valor:** un proyecto con varias pistas o ramas pesadas reduce el tiempo necesario
para procesar cada bloque usando más de un núcleo.

**Puntos de entrada:** compilador/ejecutor del motor y mediciones de PR-08/PR-13.

**Tareas**

- [ ] Generar tareas desde las dependencias reales del grafo. Ejecutar fuentes antes
  de sus consumidores, incluidos buses y sidechains; procesar cada instancia una vez.
- [ ] Preparar un conjunto acotado de trabajadores, buffers y coordinación fuera del
  callback. Respetar restricciones de hilos y ciclo de vida de cada formato/plugin.
- [ ] Dar propiedad exclusiva de procesadores y VMs durante cada tarea. Mantener un
  orden estable de mezcla y de eventos; no compartir una VM global entre trabajadores.
- [ ] Conservar ejecución secuencial para grafos pequeños y como referencia. Medir
  coste de coordinación y definir respuesta ante sobrecarga sin reutilizar buffers
  que todavía esté escribiendo un trabajador.
- [ ] Comparar carga representativa y peor caso con 1/2/4 trabajadores, registrando
  tiempos altos y plazos incumplidos. No asumir que más trabajadores siempre mejora.

**Aceptación:** el proyecto de demostración muestra una mejora medida en equipo con
varios núcleos y mantiene audio/eventos equivalentes. Una cadena completamente serial
no recibe una promesa de aceleración. El callback no crea hilos ni usa un pool general
de tareas cuya política de bloqueo/asignación se desconozca.

## PR-15 — CLAP mediante el mismo contrato

**Título sugerido:** `feat(clap): alojar instrumentos y efectos en pistas e Hilos`

**Valor:** insertar un plugin CLAP, usarlo en Tejer y exportarlo sin crear otra clase de
pista ni otro sistema de routing.

**Puntos de entrada:** `src/catalog/plugins.rs`, adaptadores de plugin, kit Lua,
estado/presets y grafo común. Crear el módulo CLAP junto a su integración funcional.

**Tareas**

- [ ] Revisar API, mantenimiento y licencia de `clack-host` y extensiones necesarias;
  solicitar autorización antes de añadir dependencias.
- [ ] Implementar descubrimiento, ciclo de vida, buses, notas, parámetros y contexto
  del host. Preservar IDs de plugin dentro del adaptador y IDs de instancia en Hay.
- [ ] Integrar estado, latencia y exportación con las operaciones existentes. Negociar
  extensiones: no anunciar modulación por nota o thread pool sin implementarlas.
- [ ] Conectar el editor mediante el mecanismo que corresponda a la plataforma y
  exponer parámetros estándar cuando el plugin no tenga editor.
- [ ] Usar el mismo paquete Lua para construir un Hilo con VST3 y CLAP; documentar las
  diferencias de capacidades sin filtrarlas al modelo musical como tipos del SDK.

**Aceptación:** un instrumento y un efecto CLAP compatibles funcionan en la misma
sesión con VST3, restauran su estado y exportan. La falta de una extensión opcional no
impide usar capacidades básicas. El procesamiento multinúcleo respeta el contrato CLAP.

## Verificación y entrega

Comandos existentes, desde la raíz. Elegir los localizados según el cambio:

```bash
distrobox enter gpui-dev -- cargo test --no-default-features --test scripting --locked
distrobox enter gpui-dev -- cargo test --no-default-features --test musical --locked
distrobox enter gpui-dev -- cargo test --no-default-features --test utility --locked
distrobox enter gpui-dev -- cargo test --no-default-features --test catalog --locked
distrobox enter gpui-dev -- cargo check --all-targets --locked
distrobox enter gpui-dev -- bash scripts/check.sh
```

Las pruebas de código alojado en `src/audio` pueden necesitar el target del binario;
revisar la guía y los módulos afectados antes de elegir filtros. Los plugins comerciales
del usuario no deben ser requisito para pruebas automáticas reproducibles.

Cada entrega debe incluir:

- El problema concreto resuelto y una demostración reproducible.
- Qué tareas/criterios se completaron, comandos ejecutados y resultados reales.
- Límites que afecten al uso; mediciones cuando el PR afirme mejorar rendimiento.
- Archivos/contratos actualizados y dependencias del siguiente PR.

Este documento solo planifica trabajo. No afirma que se haya ejecutado la suite ni
que alguna de las capacidades propuestas esté implementada.

## Prompt para encargar un PR a una IA

```text
Trabaja en /var/home/burdier/Projects/daw-gpui.
Lee AGENTS.md, docs/EDITING_GUIDE.md y docs/PLAN_LUA_MULTITRACK_HILOS.md.
Implementa únicamente PR-XX de ese plan, reutilizando el estado actual del repositorio.
Comprueba sus prerrequisitos y respeta los cambios locales ajenos.
Entrega el comportamiento usable y los criterios de aceptación del PR, no solo interfaces.
Lua debe expresar la lógica extensible; cualquier traslado a Rust por rendimiento
necesita una razón concreta y medición cuando corresponda.
No añadas dependencias sin autorización. Usa las verificaciones aplicables y actualiza
los documentos que describan capacidades o fronteras modificadas.
Haz commits pequeños con tu alcance. Al terminar resume la demostración, verificación
y limitaciones. No avances automáticamente al siguiente PR ni publiques en un remoto.
```

## Referencias de las bibliotecas

- [mlua](https://github.com/mlua-rs/mlua): integración Rust/Lua ya utilizada.
- [Manual Lua 5.4](https://www.lua.org/manual/5.4/manual.html): memoria, GC y API de Lua.
- [Scripting de Ardour](https://manual.ardour.org/lua-scripting/): referencia de integración
  de acciones, sesión y procesadores Lua; no implica que nuestro runtime actual tenga
  las mismas garantías o capacidades.
- [vst3-rs](https://github.com/coupler-rs/vst3-rs): bindings del host VST3 existente.
- [Clack](https://github.com/prokopyl/clack): candidato para el adaptador CLAP.
