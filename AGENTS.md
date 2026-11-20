# Hay / Elevate — instrucciones de edición

## Leer primero

- `docs/EDITING_GUIDE.md`: mapa tarea → archivo y comandos de verificación.
- `docs/ARCHITECTURE.md`: fronteras y contratos actuales.
- `docs/PROTOTYPE.md`: qué funciona y siguiente incremento.
- `README.md`: filosofía de componentes, primitivas, acciones y eventos.

## Reglas del proyecto

1. Comunicar en español. Avanzar por incrementos comprensibles.
2. Preguntar antes de añadir dependencias; comprobar API, licencia y encaje.
3. Mantener `main.rs` como arranque. `mod.rs` declara módulos, fachada y estado compartido.
4. Mantener juntas las operaciones con una misma responsabilidad. Divide un archivo
   cuando la separación aclare qué hace cada parte y cómo se comunican.
5. `src/musical/` no importa GPUI, CPAL, RustySynth ni tipos de `ui/` o `audio/`.
6. `src/audio/` no importa GPUI. Decodificar y preparar recursos fuera del callback.
7. `src/instruments/` no importa GPUI. Posee el estado de ejecución (notas, ratón,
   reproductor); las vistas no deben importar `audio::piano` ni `rustysynth`.
8. `src/catalog.rs` es headless: no importa GPUI ni CPAL. Indexa metadatos y comandos;
   no instancia plugins. La asignación instrumento → pista vive en `src/musical/track.rs`
   y la coordina `ui/palette.rs`.
9. Las vistas llaman controladores; teclado y ratón comparten la misma operación.
10. No crear módulos vacíos, sistemas de plugins prematuros ni un segundo transporte.
11. No confundir el piano clicable con un piano roll, el mixer mock con mezcla real,
    ni la percusión sintetizada de audición con un sampler de samples implementado.
    El buscador `[+]` tampoco es todavía un host de plugins.
12. No usar temporizadores de GPUI para programar notas. El audio avanza por frames.
13. No hacer E/S, reservas/destrucciones costosas o trabajo sin límite en el futuro
    motor RT. `audio/drums/` reutiliza buffers y entrega las voces retiradas al hilo de control.
    El adaptador `audio/piano` aún usa mpsc: su sustitución está pendiente.
14. Compilar y publicar las ediciones de percusión fuera del callback. Un plan de notas
    nuevo conserva el reloj y las voces que están sonando.
15. Actualizar el mapa de archivos y el estado si cambia una frontera o una capacidad.

## Refactorización

Simplifica el código preservando su comportamiento, rendimiento y determinismo musical.
[Guía de refactorización de PufferLib](https://github.com/PufferAI/PufferLib/blob/5.0/SKILL_ISSUES.md)

1. Empieza por la entrada normal de la función y revisa sus llamadores. Identifica los
   contratos que deben conservarse, como el orden de eventos, los IDs de clips y notas
   y la apertura de proyectos guardados con versiones anteriores.
2. Integra un auxiliar de un solo uso cuando la operación se lea mejor en el llamador.
   Conserva las funciones que delimitan una operación musical o una frontera entre la
   interfaz, el proyecto y el motor.
3. Reduce el anidamiento con retornos tempranos y estados explícitos. Si cambias una
   función compartida, revisa todos sus llamadores en el mismo cambio.
4. Valida archivos, plugins y dispositivos en sus fronteras. Devuelve los errores
   recuperables con `Result` para que la UI pueda explicarlos. Reserva las aserciones
   para invariantes internas fuera del callback de audio.
5. Reutiliza las acciones y los componentes existentes. Prepara recursos y planes fuera
   del callback, conserva los límites de sus bloques y publica el estado cuando esté listo.
6. Deja los nombres y el formato para la última pasada. Usa `rustfmt` y elige una
   comprobación que cubra el riesgo concreto del cambio.

## Entregas en paralelo

Para cada entrega, registra el objetivo, las dependencias, el commit previsto, los archivos
a cargo del agente y la comprobación de cierre. Parte de una base común y asigna a una
persona la integración de los archivos compartidos. Concentra la comprobación conjunta al
integrar.

## Verificación

Desde la raíz: `distrobox enter gpui-dev -- bash scripts/check.sh`.
Para un cambio localizado usar primero los comandos específicos de la guía.
La suite temporal del MVP está habilitada; no añadir tests que solo repitan la implementación.

Este directorio es un repositorio Git. Haz commits pequeños y revisa `git status` antes de
cambios amplios. El remoto todavía no está configurado.
