//! Procesador musical Lua preparado por instancia. Aún no se invoca desde audio RT.
use crate::musical::midi::MidiMessage;
use mlua::{Function, HookTriggers, Lua, LuaOptions, StdLib, VmState, chunk::ChunkMode};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

pub const MAX_SOURCE_BYTES: usize = 64 * 1024;
pub const MAX_EVENTS_PER_BLOCK: usize = 256;
pub const MAX_BLOCK_FRAMES: u32 = 4096;
const MAX_VM_BYTES: usize = 2 * 1024 * 1024;
const MAX_PREPARATION: Duration = Duration::from_millis(250);
const MAX_EVALUATION: Duration = Duration::from_millis(2);

/// Canal MIDI 0–15 y posición relativa al bloque. El host conserva la identidad
/// de las notas y decide cómo emparejar sus liberaciones antes de publicar audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveEvent {
    pub offset: u32,
    pub channel: u8,
    pub message: MidiMessage,
}

/// La función Lua recibe `(tipo, canal, dato1, dato2, offset)` y devuelve cinco
/// enteros con el mismo formato. Tipos: 1=NoteOn, 2=NoteOff, 3=CC,
/// 4=PitchBend, 5=ProgramChange, 6=AllNotesOff. Un evento entra y uno sale;
/// la expansión y la política de propiedad de notas pertenecen a otros commits.
pub struct LiveProcessor {
    _lua: Lua,
    process: Function,
    deadline: Rc<Cell<Instant>>,
    output: Vec<LiveEvent>,
}

impl LiveProcessor {
    pub fn prepare(source: &str) -> Result<Self, String> {
        if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
            return Err("Lua live: fuente vacía o mayor de 64 KiB".into());
        }
        let lua = Lua::new_with(StdLib::MATH, LuaOptions::default())
            .map_err(|error| error.to_string())?;
        lua.set_memory_limit(MAX_VM_BYTES)
            .map_err(|error| error.to_string())?;
        for name in [
            "dofile",
            "loadfile",
            "load",
            "require",
            "print",
            "warn",
            "collectgarbage",
            "pcall",
            "xpcall",
        ] {
            lua.globals()
                .raw_set(name, mlua::Nil)
                .map_err(|error| error.to_string())?;
        }
        let deadline = Rc::new(Cell::new(Instant::now() + MAX_PREPARATION));
        let limit = deadline.clone();
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(1_000),
            move |_, _| {
                if Instant::now() >= limit.get() {
                    Err(mlua::Error::runtime("Lua live: presupuesto excedido"))
                } else {
                    Ok(VmState::Continue)
                }
            },
        )
        .map_err(|error| error.to_string())?;
        let process: Function = lua
            .load(source)
            .set_mode(ChunkMode::Text)
            .eval()
            .map_err(|error| error.to_string())?;
        // Se retiene la VM: `Function` pertenece a esta instancia y nunca a otra.
        Ok(Self {
            _lua: lua,
            process,
            deadline,
            output: Vec::with_capacity(MAX_EVENTS_PER_BLOCK),
        })
    }

    /// Solo para un hilo no RT hasta completar auditoría y medición de `mlua`.
    /// En fallo no publica un bloque parcial; el llamador conserva la entrada.
    pub fn process_block(
        &mut self,
        input: &[LiveEvent],
        frames: u32,
    ) -> Result<&[LiveEvent], String> {
        if frames == 0 || frames > MAX_BLOCK_FRAMES || input.len() > MAX_EVENTS_PER_BLOCK {
            return Err("Lua live: tamaño de bloque o número de eventos inválido".into());
        }
        self.output.clear();
        self.deadline.set(Instant::now() + MAX_EVALUATION);
        for event in input {
            if Instant::now() >= self.deadline.get() {
                self.output.clear();
                return Err("Lua live: presupuesto excedido".into());
            }
            let args = encode(*event, frames)?;
            let result: (i64, i64, i64, i64, i64) = self.process.call(args).map_err(|error| {
                self.output.clear();
                error.to_string()
            })?;
            let transformed = decode(result, frames).map_err(|error| {
                self.output.clear();
                error
            })?;
            self.output.push(transformed);
        }
        if Instant::now() >= self.deadline.get() {
            self.output.clear();
            return Err("Lua live: presupuesto excedido".into());
        }
        Ok(&self.output)
    }
}

fn encode(event: LiveEvent, frames: u32) -> Result<(i64, i64, i64, i64, i64), String> {
    if event.offset >= frames || event.channel > 15 {
        return Err("Lua live: evento de entrada fuera del bloque".into());
    }
    let (kind, a, b) = match event.message {
        MidiMessage::NoteOn { note, velocity } if note <= 127 && velocity <= 127 => {
            (1, i64::from(note), i64::from(velocity))
        }
        MidiMessage::NoteOff { note } if note <= 127 => (2, i64::from(note), 0),
        MidiMessage::ControlChange { cc, value } if cc <= 127 && value <= 127 => {
            (3, i64::from(cc), i64::from(value))
        }
        MidiMessage::PitchBend { bend } if (-8192..=8191).contains(&bend) => {
            (4, i64::from(bend), 0)
        }
        MidiMessage::ProgramChange { program } if program <= 127 => (5, i64::from(program), 0),
        MidiMessage::AllNotesOff => (6, 0, 0),
        _ => return Err("Lua live: mensaje MIDI inválido".into()),
    };
    Ok((
        kind,
        i64::from(event.channel),
        a,
        b,
        i64::from(event.offset),
    ))
}

fn decode(
    (kind, channel, a, b, offset): (i64, i64, i64, i64, i64),
    frames: u32,
) -> Result<LiveEvent, String> {
    let channel = u8::try_from(channel)
        .ok()
        .filter(|&v| v <= 15)
        .ok_or("Lua live: canal inválido")?;
    let offset = u32::try_from(offset)
        .ok()
        .filter(|&v| v < frames)
        .ok_or("Lua live: offset inválido")?;
    let midi_byte = |v| {
        u8::try_from(v)
            .ok()
            .filter(|&v| v <= 127)
            .ok_or("Lua live: dato MIDI inválido")
    };
    let message = match kind {
        1 => MidiMessage::NoteOn {
            note: midi_byte(a)?,
            velocity: midi_byte(b)?,
        },
        2 => MidiMessage::NoteOff {
            note: midi_byte(a)?,
        },
        3 => MidiMessage::ControlChange {
            cc: midi_byte(a)?,
            value: midi_byte(b)?,
        },
        4 => MidiMessage::PitchBend {
            bend: i16::try_from(a)
                .ok()
                .filter(|&v| (-8192..=8191).contains(&v))
                .ok_or("Lua live: bend inválido")?,
        },
        5 => MidiMessage::ProgramChange {
            program: midi_byte(a)?,
        },
        6 => MidiMessage::AllNotesOff,
        _ => return Err("Lua live: tipo de evento inválido".into()),
    };
    Ok(LiveEvent {
        offset,
        channel,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: &str =
        "return function(kind, channel, a, b, offset) return kind, channel, a, b, offset end";

    #[test]
    fn prepared_instances_process_typed_events_without_growing_the_host_buffer() {
        let mut processor = LiveProcessor::prepare(IDENTITY).unwrap();
        let input = [
            LiveEvent {
                offset: 0,
                channel: 2,
                message: MidiMessage::NoteOn {
                    note: 60,
                    velocity: 100,
                },
            },
            LiveEvent {
                offset: 63,
                channel: 2,
                message: MidiMessage::NoteOff { note: 60 },
            },
        ];
        assert_eq!(processor.process_block(&input, 64).unwrap(), input);
        assert_eq!(processor.output.capacity(), MAX_EVENTS_PER_BLOCK);
        assert!(
            processor
                .process_block(&vec![input[0]; MAX_EVENTS_PER_BLOCK + 1], 64)
                .is_err()
        );
    }

    #[test]
    fn bad_script_result_and_unbounded_loop_cannot_publish_a_partial_block() {
        let event = LiveEvent {
            offset: 1,
            channel: 0,
            message: MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
        };
        let mut invalid = LiveProcessor::prepare(
            "return function(kind, channel, a, b, offset) return kind, channel, a, b, 64 end",
        )
        .unwrap();
        assert!(invalid.process_block(&[event], 64).is_err());
        assert!(invalid.output.is_empty());

        let mut runaway =
            LiveProcessor::prepare("return function() while true do end end").unwrap();
        assert!(runaway.process_block(&[event], 64).is_err());
        assert!(runaway.output.is_empty());
    }
}
