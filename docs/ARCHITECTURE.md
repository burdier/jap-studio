# Arquitectura del prototipo

## Dos targets, un paquete

- Biblioteca `elevate` (`src/elevate.rs`): DSP y núcleo musical headless.
- Binario `daw-gpui` (`src/main.rs`): adaptadores de audio + interfaz GPUI.
- `--no-default-features` permite trabajar en la biblioteca sin compilar GPUI/CPAL.
- No se necesita un workspace de ocho crates para mantener estas fronteras.

```text
main.rs → ui::run
             │
        DawApp (ui/mod.rs)
          ├── input.rs ─────────┐
          ├── views/* ─────────┼→ pattern_editor.rs → musical/model.rs
          │                    └→ audio_actions.rs → audio/*
          ├── design.rs                     │
          └── timeline/*                    ├→ Symphonia / RustySynth / DrumKit
                                            └→ Elevate Volume → CPAL

Biblioteca headless:
musical/model.rs → scheduler::compile(TimeBase) → PreparedPattern
                                                   │
Transport::advance(frames) → FrameWindow → events_in()
                                                   ↓
                                     ScheduledEvent { sample_offset, ... }
                                                   │
                                   audio/drums/stream.rs → DrumKit
                                                   ↓
                                         Volume → CPAL
```

La ruta del patrón ya está conectada al callback de percusión. Clip y piano conservan
sus adaptadores separados; unificarlos es el siguiente incremento. `examples/sequencer.rs`
sigue mostrando el plan sin abrir audio.

## Responsabilidades y propiedad del estado

| Estado | Propietario actual | Restricción |
|---|---|---|
| Notas del patrón | `Pattern`, contenido en `ui::PatternEditor` | Sin coordenadas visuales ni voces |
| Selección, modo, vista | `DawApp` | Estado de workspace, no proyecto musical |
| Zoom/cursor/arrastre | `Timeline` | Convierte píxeles ↔ beats, no gobierna audio |
| Clip cargado | `Timeline::clip`, provisional | Un único clip; mover al proyecto al ampliar |
| Archivo decodificado | `Arc<AudioFile>` | Preparado fuera del callback |
| Voces del piano | RustySynth dentro del callback | No pasan por GPUI |
| Voces de percusión | `DrumKit` en `audio/drums/voice.rs` | Banco fijo; sin asignaciones por nota |
| Plan de notas | `PreparedPattern` | Inmutable; percusión lo compila en Play |
| Posición de transporte | `Transport` en cada callback / demo headless | Avanzar con frames procesados |
| Posición del patrón | `AtomicU64` en `DrumState` | Snapshot de solo lectura para la UI |
| Partículas | `ParticleTrail` | Tiempo visual; no afecta al scheduler |

## Contrato musical actual

- PPQ = 960 ticks por negra. `Tick` admite enteros con signo; este prototipo valida
  posiciones de patrón no negativas. La resolución de la vista no modifica PPQ.
- Patrón de longitud arbitraria positiva. Varias notas pueden compartir tick e instrumento.
- IDs de nota estables, locales al patrón y no reutilizados. No son UUID de proyecto.
- Notas con duración positiva, pitch 0–127 y velocity 1–127; terminan dentro del patrón.
- `compile` genera NoteOn/NoteOff, redondeando posiciones absolutas a frames; mínimo
  un frame de duración. No suma duraciones redondeadas paso a paso.
- `PreparedPattern::length_frames()` mide una vuelta, con mínimo de un frame; el stream
  de percusión divide cada bloque en tramos que no cruzan el final del loop.
- `events_in([start, end))` no reserva memoria: búsqueda binaria + eventos del bloque.
- Los NoteOff preceden a NoteOn coincidentes. Un NoteOff justo al final del patrón
  pertenece a la siguiente ventana: el consumidor debe procesarlo antes de cerrar voces.
  La percusión los ignora porque cada voz es un one-shot con envolvente propia.
- El transporte usa frames, no reloj de pared. Stop conserva posición; seek es explícito.
- La audición del patrón mapea `frame_global % length_frames` a tiempo local. Los eventos
  se consultan por segmento, así que el offset dentro del buffer sigue siendo exacto.
- No hay chase de notas sostenidas. Ante stop/seek/cambio de plan, el futuro host
  deberá liberar voces y definir la política de reanudación.
- El plan activo de percusión es inmutable: editar notas no lo actualiza. Detener y
  volver a pulsar Play recompila. Loop configurable y cambios de tempo necesitan
  segmentación y una política de reemplazo; no implementar un segundo reloj.
- Hay límites de representación explícitos y errores; comprobarlos antes de publicar un plan.

## Audio existente y siguiente frontera

`audio/output.rs` comparte selección de formatos. `playback.rs`, `piano/` y `drums/`
poseen streams separados y excluyentes. El clip exige la frecuencia del archivo;
piano y percusión siguen la del dispositivo. El master actual es un clamp, no un limitador.

`drums/mod.rs` compila el plan y abre el stream. `drums/stream.rs` avanza `Transport`
por frames, segmenta en el loop y entrega offsets exactos a `DrumKit`; `Volume` procesa
el bloque antes de CPAL. La síntesis es deliberadamente provisional (sin samples) y el
plan se recompila solo al iniciar la reproducción.

La siguiente integración extraerá procesadores del clip, piano y percusión, abrirá un solo
stream y consumirá eventos entre subbloques. El transporte vivirá ahí. GPUI recibirá
snapshots; editará comandos y planes preparados. `rtrb` sigue pendiente de aprobación.
También se debe definir dónde se liberan planes/samples retirados: nunca delegar
esa destrucción costosa al callback por accidente. Para percusión, el siguiente paso
es un sampler con cuatro samples y reemplazo de plan en caliente.

## Extensibilidad

Elevate mantiene `Primitive`, `Component`, parámetros y `NodeId`. `ControlEvent`
es todavía una solicitud síncrona de cambio, no un bus de notificaciones.
`PatternAction` es una operación compartida de UI, no el catálogo global de acciones.
El futuro host conectará eventos musicales y DSP sin importar tipos de GPUI.
