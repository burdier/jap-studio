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
4. Un archivo por responsabilidad. Como referencia, 100–250 líneas; separar por concepto,
   no cortar funciones arbitrariamente para cumplir un número.
5. `src/musical/` no importa GPUI, CPAL, RustySynth ni tipos de `ui/` o `audio/`.
6. `src/audio/` no importa GPUI. Decodificar y preparar recursos fuera del callback.
7. Las vistas llaman controladores; teclado y ratón comparten la misma operación.
8. No crear módulos vacíos, sistemas de plugins prematuros ni un segundo transporte.
9. No confundir el piano clicable con un piano roll, el mixer mock con mezcla real,
   ni la percusión sintetizada de audición con un sampler de samples implementado.
10. No usar temporizadores de GPUI para programar notas. El audio avanza por frames.
11. No hacer E/S, reservas/destrucciones costosas o trabajo sin límite en el futuro
    motor RT. `audio/drums/stream.rs` no debe asignar memoria por bloque ni por nota.
    El adaptador `audio/piano` aún usa mpsc: su sustitución está pendiente.
12. El plan de percusión es inmutable. Editar el patrón no actualiza el audio activo;
    para escucharlo hay que detener y volver a pulsar Play. No ocultar esa limitación.
13. Actualizar el mapa de archivos y el estado si cambia una frontera o una capacidad.

## Verificación

Desde la raíz: `distrobox enter gpui-dev -- bash scripts/check.sh`.
Para un cambio localizado usar primero los comandos específicos de la guía.
La suite temporal del MVP está habilitada; no añadir tests que solo repitan la implementación.

Este directorio es un repositorio Git. Haz commits pequeños y revisa `git status` antes de
cambios amplios. El remoto todavía no está configurado.
