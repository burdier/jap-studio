//! Servidor MCP (Model Context Protocol, JSON-RPC 2.0 sobre stdio) y puente IPC Unix local.
//! Sin puertos de red ni dependencias externas: comunica clientes IA con la instancia abierta del DAW.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, BufReader, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "jap-studio";
const SERVER_VERSION: &str = "0.8.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IpcRequest {
    pub tool: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IpcResponse {
    pub ok: bool,
    pub result: Value,
}

pub(crate) struct IncomingMcpCall {
    pub request: IpcRequest,
    pub reply: mpsc::SyncSender<IpcResponse>,
}

/// Puente IPC del lado del DAW abierto: escucha en un socket Unix local y reenvía
/// cada llamada al hilo de la aplicación sin bloquear la UI.
pub(crate) struct McpHost {
    socket_path: PathBuf,
}

impl Drop for McpHost {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

pub(crate) fn socket_path() -> PathBuf {
    if let Ok(custom) = std::env::var("HAY_MCP_SOCKET")
        && !custom.trim().is_empty()
    {
        return PathBuf::from(custom);
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR")
        && !runtime.trim().is_empty()
    {
        return PathBuf::from(runtime).join("hay-daw-mcp.sock");
    }
    std::env::temp_dir().join("hay-daw-mcp.sock")
}

impl McpHost {
    pub fn start() -> Result<(Self, mpsc::Receiver<IncomingMcpCall>), String> {
        Self::start_at(socket_path())
    }

    pub fn start_at(path: PathBuf) -> Result<(Self, mpsc::Receiver<IncomingMcpCall>), String> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)
            .map_err(|e| format!("No se pudo abrir el socket MCP {}: {e}", path.display()))?;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        let (tx, rx) = mpsc::channel::<IncomingMcpCall>();
        thread::Builder::new()
            .name("mcp-ipc-host".into())
            .spawn(move || {
                for incoming in listener.incoming() {
                    let Ok(stream) = incoming else {
                        continue;
                    };
                    let tx = tx.clone();
                    thread::spawn(move || handle_ipc_client(stream, tx));
                }
            })
            .map_err(|e| format!("Hilo MCP IPC: {e}"))?;
        Ok((Self { socket_path: path }, rx))
    }
}

fn handle_ipc_client(mut stream: UnixStream, tx: mpsc::Sender<IncomingMcpCall>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_stream);
    let mut line = String::new();
    while let Ok(bytes) = reader.read_line(&mut line) {
        if bytes == 0 {
            break;
        }
        let response = match serde_json::from_str::<IpcRequest>(line.trim()) {
            Ok(request) => {
                let (reply_tx, reply_rx) = mpsc::sync_channel(1);
                if tx
                    .send(IncomingMcpCall {
                        request,
                        reply: reply_tx,
                    })
                    .is_err()
                {
                    IpcResponse {
                        ok: false,
                        result: json!("Jap Studio se cerró"),
                    }
                } else {
                    reply_rx
                        .recv_timeout(Duration::from_secs(8))
                        .unwrap_or_else(|_| IpcResponse {
                            ok: false,
                            result: json!("Tiempo de espera agotado esperando al DAW"),
                        })
                }
            }
            Err(error) => IpcResponse {
                ok: false,
                result: json!(format!("Petición IPC inválida: {error}")),
            },
        };
        if let Ok(encoded) = serde_json::to_string(&response) {
            if writeln!(stream, "{encoded}").is_err() || stream.flush().is_err() {
                break;
            }
        }
        line.clear();
    }
}

fn call_daw(path: &Path, request: &IpcRequest) -> Result<IpcResponse, String> {
    let mut stream = UnixStream::connect(path).map_err(|_| {
        format!(
            "Jap Studio no está abierto (no se encontró el socket {}). Inicia hay-daw primero.",
            path.display()
        )
    })?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    let encoded = serde_json::to_string(request).map_err(|e| e.to_string())?;
    writeln!(stream, "{encoded}").map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let n = reader.read_line(&mut line).map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("Jap Studio cerró la conexión sin responder".into());
    }
    serde_json::from_str::<IpcResponse>(line.trim())
        .map_err(|e| format!("Respuesta inválida de Jap Studio: {e}"))
}

/// Ejecuta el servidor MCP estándar sobre stdin/stdout (invocado con `hay-daw --mcp`).
pub(crate) fn run_stdio_server() -> Result<(), String> {
    let sock = socket_path();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(reply) = handle_jsonrpc_message(trimmed, &sock) {
            let encoded = serde_json::to_string(&reply).map_err(|e| e.to_string())?;
            writeln!(out, "{encoded}").map_err(|e| e.to_string())?;
            out.flush().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub(crate) fn handle_jsonrpc_message(raw: &str, sock: &Path) -> Option<Value> {
    let msg: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(err) => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": { "code": -32700, "message": format!("Parse error: {err}") }
            }));
        }
    };
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);

    // Las notificaciones JSON-RPC (sin `id`) no llevan respuesta.
    if id.is_none() || method.starts_with("notifications/") {
        return None;
    }
    let id = id.unwrap();

    match method {
        "initialize" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": SERVER_VERSION
                }
            }
        })),
        "ping" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {}
        })),
        "tools/list" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": tool_definitions()
            }
        })),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let req = IpcRequest {
                tool: name,
                arguments,
            };
            match call_daw(sock, &req) {
                Ok(resp) => {
                    let text = if let Some(s) = resp.result.as_str() {
                        s.to_owned()
                    } else {
                        serde_json::to_string_pretty(&resp.result).unwrap_or_default()
                    };
                    Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": text }],
                            "isError": !resp.ok
                        }
                    }))
                }
                Err(error) => Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": error }],
                        "isError": true
                    }
                })),
            }
        }
        other => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {other}") }
        })),
    }
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "get_project_state",
            "description": "Devuelve el estado completo del proyecto abierto en Jap Studio: tempo, compás, reproducción, pistas (nivel, pan, mute, solo, instrumento, FX), FX del Master y el secuenciador de Drums/MPC.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "set_transport",
            "description": "Controla el transporte del DAW: reproducir, detener, cambiar el BPM en vivo o activar/desactivar el metrónomo.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["play", "stop", "toggle"] },
                    "tempo_bpm": { "type": "number", "minimum": 20, "maximum": 300 },
                    "metronome": { "type": "boolean" }
                }
            }
        },
        {
            "name": "add_track",
            "description": "Crea una pista nueva en el Arrangement y opcionalmente le asigna un instrumento nativo (piano o sampler).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "instrument": { "type": "string", "enum": ["none", "piano", "sampler"] }
                }
            }
        },
        {
            "name": "set_track_mix",
            "description": "Ajusta volumen (0–100, 75 = 0 dB), panorama (0–100, 50 = centro), mute, solo o nombre de una pista por índice.",
            "inputSchema": {
                "type": "object",
                "required": ["track_index"],
                "properties": {
                    "track_index": { "type": "integer", "minimum": 0 },
                    "level": { "type": "integer", "minimum": 0, "maximum": 100 },
                    "pan": { "type": "integer", "minimum": 0, "maximum": 100 },
                    "mute": { "type": "boolean" },
                    "solo": { "type": "boolean" },
                    "name": { "type": "string" }
                }
            }
        },
        {
            "name": "add_fx",
            "description": "Inserta un efecto nativo Hey en una pista (track_index) o en el bus Master (si se omite track_index). Acepta alias cortos: eq, compressor, limiter, echo, reverb, filter, volume, convolver, o un ID exacto hay.fx.*",
            "inputSchema": {
                "type": "object",
                "required": ["fx_id"],
                "properties": {
                    "track_index": { "type": "integer", "minimum": 0 },
                    "fx_id": { "type": "string" }
                }
            }
        },
        {
            "name": "write_midi_clip",
            "description": "Crea e inserta un clip MIDI con acordes o melodía en una pista del Arrangement.",
            "inputSchema": {
                "type": "object",
                "required": ["track_index", "notes"],
                "properties": {
                    "track_index": { "type": "integer", "minimum": 0 },
                    "start_beat": { "type": "number", "default": 0 },
                    "length_beats": { "type": "number", "minimum": 0.25 },
                    "name": { "type": "string" },
                    "notes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["pitch", "start_beat", "duration_beats"],
                            "properties": {
                                "pitch": { "type": "integer", "minimum": 0, "maximum": 127 },
                                "start_beat": { "type": "number", "minimum": 0 },
                                "duration_beats": { "type": "number", "minimum": 0.05 },
                                "velocity": { "type": "integer", "minimum": 1, "maximum": 127 }
                            }
                        }
                    }
                }
            }
        },
        {
            "name": "write_drum_pattern",
            "description": "Programa pasos en el secuenciador de Drums (modo replace, merge, clear o new), opcionalmente ajustando swing (0–75) y subdivisión (8, 16 o 32).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "enum": ["replace", "merge", "clear", "new"] },
                    "swing": { "type": "integer", "minimum": 0, "maximum": 75 },
                    "grid": { "type": "integer", "enum": [8, 16, 32] },
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["lane", "step"],
                            "properties": {
                                "lane": { "type": "integer", "minimum": 0 },
                                "step": { "type": "integer", "minimum": 0 },
                                "velocity": { "type": "integer", "minimum": 1, "maximum": 127 },
                                "probability": { "type": "integer", "minimum": 5, "maximum": 100 },
                                "pitch_offset": { "type": "integer", "minimum": -24, "maximum": 24 }
                            }
                        }
                    }
                }
            }
        },
        {
            "name": "set_view",
            "description": "Cambia la vista activa de Jap Studio (arrangement, drums, mpc, mixer).",
            "inputSchema": {
                "type": "object",
                "required": ["view"],
                "properties": {
                    "view": { "type": "string", "enum": ["arrangement", "drums", "mpc", "mixer"] }
                }
            }
        },
        {
            "name": "trigger_mpc_pad",
            "description": "Dispara un pad MPC (0–15 del banco actual) en vivo para audicionar sonidos.",
            "inputSchema": {
                "type": "object",
                "required": ["pad"],
                "properties": {
                    "pad": { "type": "integer", "minimum": 0, "maximum": 15 },
                    "velocity": { "type": "integer", "minimum": 1, "maximum": 127 }
                }
            }
        },
        {
            "name": "run_command",
            "description": "Ejecuta cualquier comando de la barra de comandos (:comando) de Jap Studio (ej. 'play', 'stop', 'metro', 'mon.auto', 'track.insert').",
            "inputSchema": {
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": { "type": "string" }
                }
            }
        },
        {
            "name": "list_samples",
            "description": "Descubre archivos de audio (.wav, .flac, .mp3, .ogg, .aiff) y kits guardados (.haykit) en tus bibliotecas reales del Explorer, ~/.config/hay-daw/kits o una carpeta específica. Si pasas analyze=true o character, analiza el audio (transiente, brillo, bandas, decay, tono) para elegir con criterio musical.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Filtro opcional por nombre/ruta (ej. 'kick', 'snare', 'hat', 'vinyl', 'lofi')." },
                    "kind": { "type": "string", "enum": ["all", "audio", "kit"], "default": "all" },
                    "directory": { "type": "string", "description": "Carpeta opcional adicional donde buscar." },
                    "analyze": { "type": "boolean", "default": false, "description": "Si es true, añade la ficha acústica (transiente, brillo, bandas, decay, tono) a los resultados de audio (máx. 16)." },
                    "character": {
                        "type": "string",
                        "enum": ["soft", "punchy", "dark", "bright", "tonal", "oneshot", "loop"],
                        "description": "Filtra archivos de audio por carácter acústico real (implica análisis)."
                    },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 60 }
                }
            }
        },
        {
            "name": "browse_explorer",
            "description": "Navega el Explorador de bibliotecas exactamente como un humano: sin directory lista tus carpetas raíz configuradas en Jap Studio; con directory lista subcarpetas, samples de audio, MIDI y kits .haykit en ese nivel (opcionalmente con ficha acústica analyze=true).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": { "type": "string", "description": "Carpeta a inspeccionar. Omitir para ver las raíces del Explorer." },
                    "query": { "type": "string", "description": "Filtro opcional por nombre dentro de esa carpeta." },
                    "analyze": { "type": "boolean", "default": false, "description": "Si es true, analiza los samples de audio listados (máx. 12) para ver transiente, brillo, bandas y tono." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 60 }
                }
            }
        },
        {
            "name": "analyze_sample",
            "description": "Analiza acústicamente uno o varios archivos de audio (hasta 24) fuera del hilo RT y devuelve su carácter real: fuerza de transiente (débil/suave vs punchy/agresivo), ataque/decay en ms, brillo/centroide espectral, balance sub/graves/medios/agudos, pico/RMS/crest, estéreo, tono fundamental y resumen musical legible.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Ruta de un archivo de audio a analizar." },
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Lista de rutas (hasta 24) para comparar varios samples candidatos lado a lado."
                    }
                }
            }
        },
        {
            "name": "add_drum_piece",
            "description": "Carga un archivo de audio como nueva pieza/fila en el kit de Drums/MPC para que write_drum_pattern pueda programarla.",
            "inputSchema": {
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string" },
                    "name": { "type": "string" },
                    "choke_group": { "type": "integer", "minimum": 1, "maximum": 64 }
                }
            }
        },
        {
            "name": "load_drum_kit",
            "description": "Carga un kit completo de batería (.haykit) en Drums/MPC.",
            "inputSchema": {
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string" }
                }
            }
        },
        {
            "name": "place_drum_pattern",
            "description": "Coloca un patrón de Drums en el Arrangement en un beat concreto (creando o reutilizando carriles Drum) o en la matriz Canción de Drums.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern_id": { "type": "integer", "minimum": 1, "description": "ID del patrón (por defecto el patrón activo)." },
                    "start_beat": { "type": "number", "minimum": 0, "default": 0 },
                    "repeats": { "type": "integer", "minimum": 1, "maximum": 64, "default": 1, "description": "Número de repeticiones consecutivas del patrón." },
                    "target": { "type": "string", "enum": ["arrangement", "song"], "default": "arrangement" }
                }
            }
        },
        {
            "name": "set_fx_param",
            "description": "Ajusta parámetros, bypass o sidechain de un efecto FX existente en una pista o en el Master (consulta fx_id y nombres/IDs de parámetros en get_project_state).",
            "inputSchema": {
                "type": "object",
                "required": ["fx_id"],
                "properties": {
                    "track_index": { "type": "integer", "minimum": 0, "description": "Omitir para editar un FX del bus Master." },
                    "fx_id": { "type": "integer", "minimum": 1 },
                    "param": { "description": "ID numérico o clave/nombre del parámetro (ej. 0, 'cutoff', 'mix', 'decay', 'threshold', 'gain')." },
                    "value": { "type": "number" },
                    "params": {
                        "type": "object",
                        "description": "Mapa opcional { clave_o_id: valor } para ajustar varios parámetros a la vez."
                    },
                    "enabled": { "type": "boolean" },
                    "sidechain": { "description": "Fuente de sidechain: 'none', 'drums', o el índice numérico de pista fuente." }
                }
            }
        },
        {
            "name": "load_sampler_sample",
            "description": "Carga un archivo de audio en el Hay Sampler de una pista (si la pista no tiene Sampler, se lo asigna).",
            "inputSchema": {
                "type": "object",
                "required": ["track_index", "path"],
                "properties": {
                    "track_index": { "type": "integer", "minimum": 0 },
                    "path": { "type": "string" }
                }
            }
        },
        {
            "name": "insert_audio_clip",
            "description": "Inserta un archivo de audio (.wav, .flac, .mp3, .ogg) directamente como clip en una pista del Arrangement (ideal para texturas de vinilo, lluvia o loops).",
            "inputSchema": {
                "type": "object",
                "required": ["track_index", "path"],
                "properties": {
                    "track_index": { "type": "integer", "minimum": 0 },
                    "path": { "type": "string" },
                    "start_beat": { "type": "number", "minimum": 0, "default": 0 }
                }
            }
        },
        {
            "name": "delete_track",
            "description": "Elimina una pista del Arrangement por su índice.",
            "inputSchema": {
                "type": "object",
                "required": ["track_index"],
                "properties": {
                    "track_index": { "type": "integer", "minimum": 0 }
                }
            }
        },
        {
            "name": "delete_clip",
            "description": "Elimina un clip MIDI o de Audio del Arrangement por su clip_id (consulta los IDs en get_project_state).",
            "inputSchema": {
                "type": "object",
                "required": ["clip_id"],
                "properties": {
                    "clip_id": { "type": "integer", "minimum": 1 }
                }
            }
        },
        {
            "name": "export_audio",
            "description": "Inicia la exportación del Arrangement a un archivo de audio (.wav, .flac, .mp3, .aiff) en disco.",
            "inputSchema": {
                "type": "object",
                "required": ["filename"],
                "properties": {
                    "filename": { "type": "string", "description": "Nombre de archivo con extensión (ej. 'lofi-beat.wav' o 'lofi-beat.flac')." },
                    "output_dir": { "type": "string", "description": "Carpeta destino opcional." },
                    "sample_rate": { "type": "integer", "enum": [44100, 48000, 96000] },
                    "bits": { "type": "integer", "enum": [16, 24, 32] },
                    "lufs": { "type": "number", "description": "Normalización LUFS opcional (ej. -14 para YouTube)." }
                }
            }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn unique_sock() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "hay-mcp-test-{}-{}.sock",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn mcp_initialize_and_tools_list_return_valid_jsonrpc() {
        let sock = unique_sock();
        let init = handle_jsonrpc_message(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            &sock,
        )
        .unwrap();
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(init["result"]["serverInfo"]["name"], SERVER_NAME);

        let notif = handle_jsonrpc_message(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &sock,
        );
        assert!(notif.is_none());

        let list =
            handle_jsonrpc_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#, &sock)
                .unwrap();
        let tools = list["result"]["tools"].as_array().unwrap();
        assert!(tools.len() >= 22);
        assert!(tools.iter().any(|t| t["name"] == "get_project_state"));
        assert!(tools.iter().any(|t| t["name"] == "list_samples"));
        assert!(tools.iter().any(|t| t["name"] == "browse_explorer"));
        assert!(tools.iter().any(|t| t["name"] == "analyze_sample"));
        assert!(tools.iter().any(|t| t["name"] == "add_drum_piece"));
        assert!(tools.iter().any(|t| t["name"] == "place_drum_pattern"));
        assert!(tools.iter().any(|t| t["name"] == "set_fx_param"));
        assert!(tools.iter().any(|t| t["name"] == "export_audio"));
    }

    #[test]
    fn mcp_ipc_roundtrip_communicates_over_unix_socket() {
        let sock = unique_sock();
        let (_host, rx) = McpHost::start_at(sock.clone()).unwrap();
        let worker = thread::spawn(move || {
            let call = rx.recv_timeout(Duration::from_secs(3)).unwrap();
            assert_eq!(call.request.tool, "get_project_state");
            let _ = call.reply.send(IpcResponse {
                ok: true,
                result: json!({ "tempo_bpm": 124.0, "tracks": 2 }),
            });
        });

        let reply = handle_jsonrpc_message(
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"get_project_state","arguments":{}}}"#,
            &sock,
        )
        .unwrap();
        worker.join().unwrap();

        assert_eq!(reply["id"], 7);
        assert_eq!(reply["result"]["isError"], false);
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("124"));
    }
}
