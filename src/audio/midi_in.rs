//! Adaptador aislado de entrada MIDI hardware (ALSA/PipeWire vía `midir`) + cola `rtrb`.
//! No importa GPUI. Convierte bytes MIDI entrantes en `MidiMessage` sin bloquear el hilo MIDI.
use elevate::musical::midi::{MidiInputEvent, MidiMessage};
use midir::{Ignore, MidiInput, MidiInputConnection};
use rtrb::{Consumer, RingBuffer};
use std::collections::HashSet;
use std::time::{Duration, Instant};

const QUEUE_CAPACITY: usize = 512;
const HOTPLUG_SCAN_INTERVAL: Duration = Duration::from_millis(1200);

struct PortStream {
    name: String,
    _connection: MidiInputConnection<()>,
    consumer: Consumer<MidiInputEvent>,
}

pub(crate) struct MidiHardwareIn {
    streams: Vec<PortStream>,
    last_scan: Instant,
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
            last_scan: Instant::now() - HOTPLUG_SCAN_INTERVAL,
        };
        this.reconnect();
        this
    }

    fn connect_port(scanner: &MidiInput, port: &midir::MidiInputPort, name: String) -> Option<PortStream> {
        let mut input = MidiInput::new("hay-daw-in").ok()?;
        input.ignore(Ignore::ActiveSense);
        let (mut producer, consumer) = RingBuffer::new(QUEUE_CAPACITY);
        let connection = input
            .connect(
                port,
                "hay-midi-port",
                move |_timestamp, bytes, ()| {
                    if let Some(msg) = MidiMessage::input_from_bytes(bytes) {
                        let _ = producer.push(msg);
                    }
                },
                (),
            )
            .ok()?;
        Some(PortStream {
            name,
            _connection: connection,
            consumer,
        })
    }

    pub fn reconnect(&mut self) {
        self.streams.clear();
        self.last_scan = Instant::now();
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
            if let Some(stream) = Self::connect_port(&scanner, &port, name) {
                self.streams.push(stream);
            }
        }
    }

    /// Hot-plug diferencial: conecta puertos nuevos y quita los desconectados sin cortar notas activas.
    pub fn poll_hotplug(&mut self) -> Option<(Vec<String>, Vec<String>)> {
        if self.last_scan.elapsed() < HOTPLUG_SCAN_INTERVAL {
            return None;
        }
        self.last_scan = Instant::now();
        let Ok(scanner) = MidiInput::new("hay-daw-scan") else {
            return None;
        };
        let mut available: Vec<(String, midir::MidiInputPort)> = Vec::new();
        let mut available_names: HashSet<String> = HashSet::new();
        for port in scanner.ports() {
            let Ok(name) = scanner.port_name(&port) else {
                continue;
            };
            if name.to_lowercase().contains("midi through") {
                continue;
            }
            if available_names.insert(name.clone()) {
                available.push((name, port));
            }
        }

        // 1. Quitar solo puertos que desaparecieron del sistema.
        let mut removed = Vec::new();
        self.streams.retain(|s| {
            if available_names.contains(&s.name) {
                true
            } else {
                removed.push(s.name.clone());
                false
            }
        });

        // 2. Conectar solo puertos nuevos sin tocar las conexiones existentes.
        let existing: HashSet<String> = self.streams.iter().map(|s| s.name.clone()).collect();
        let mut added = Vec::new();
        for (name, port) in available {
            if existing.contains(&name) {
                continue;
            }
            if let Some(stream) = Self::connect_port(&scanner, &port, name.clone()) {
                self.streams.push(stream);
                added.push(name);
            }
        }

        if added.is_empty() && removed.is_empty() {
            None
        } else {
            Some((added, removed))
        }
    }

    pub fn port_names(&self) -> Vec<&str> {
        self.streams.iter().map(|s| s.name.as_str()).collect()
    }

    pub fn has_ports(&self) -> bool {
        !self.streams.is_empty()
    }

    pub fn drain(&mut self) -> Vec<MidiInputEvent> {
        let mut out = Vec::new();
        for stream in &mut self.streams {
            while let Ok(msg) = stream.consumer.pop() {
                out.push(msg);
            }
        }
        out
    }
}
