//! Adaptador aislado de entrada MIDI hardware (ALSA/PipeWire vía `midir`) + cola `rtrb`.
//! No importa GPUI. Convierte bytes MIDI entrantes en `MidiMessage` sin bloquear el hilo MIDI.
use elevate::musical::midi::MidiMessage;
use midir::{Ignore, MidiInput, MidiInputConnection};
use rtrb::{Consumer, RingBuffer};

const QUEUE_CAPACITY: usize = 512;

struct PortStream {
    name: String,
    _connection: MidiInputConnection<()>,
    consumer: Consumer<MidiMessage>,
}

pub(crate) struct MidiHardwareIn {
    streams: Vec<PortStream>,
}

impl Default for MidiHardwareIn {
    fn default() -> Self {
        Self::connect_all()
    }
}

impl MidiHardwareIn {
    pub fn connect_all() -> Self {
        let mut this = Self {
            streams: Vec::new(),
        };
        this.reconnect();
        this
    }

    pub fn reconnect(&mut self) {
        self.streams.clear();
        let Ok(scanner) = MidiInput::new("hay-daw-scan") else {
            return;
        };
        for port in scanner.ports() {
            let Ok(name) = scanner.port_name(&port) else {
                continue;
            };
            if name.to_lowercase().contains("midi through") {
                continue;
            }
            let Ok(mut input) = MidiInput::new("hay-daw-in") else {
                continue;
            };
            input.ignore(Ignore::ActiveSense);
            let (mut producer, consumer) = RingBuffer::new(QUEUE_CAPACITY);
            let port_label = name.clone();
            if let Ok(connection) = input.connect(
                &port,
                "hay-midi-port",
                move |_timestamp, bytes, ()| {
                    if let Some(msg) = MidiMessage::from_bytes(bytes) {
                        let _ = producer.push(msg);
                    }
                },
                (),
            ) {
                self.streams.push(PortStream {
                    name: port_label,
                    _connection: connection,
                    consumer,
                });
            }
        }
    }

    pub fn port_names(&self) -> Vec<&str> {
        self.streams.iter().map(|s| s.name.as_str()).collect()
    }

    pub fn has_ports(&self) -> bool {
        !self.streams.is_empty()
    }

    pub fn drain(&mut self) -> Vec<MidiMessage> {
        let mut out = Vec::new();
        for stream in &mut self.streams {
            while let Ok(msg) = stream.consumer.pop() {
                out.push(msg);
            }
        }
        out
    }
}
