#!/usr/bin/env python3
"""Renderiza landing/video.html a MP4, fotograma a fotograma.

Sin dependencias de Python: sirve landing/ por HTTP local, abre Chrome/Chromium
headless (binario del sistema o el Flatpak com.google.Chrome), lo controla por
DevTools Protocol con un cliente WebSocket mínimo y entrega cada captura a ffmpeg.
Cada fotograma llama a render(t), así que el resultado no depende de la
velocidad de la máquina.

    python3 scripts/render_video.py
    python3 scripts/render_video.py --theme dark --fps 60 --audio exports/video-sfx.wav
    python3 scripts/render_video.py --daw-clip dist/video/daw-sfx.mp4 --audio exports/video-sfx.wav
    python3 scripts/render_video.py --preview          # solo sirve la vista previa
"""

import argparse
import base64
import functools
import http.server
import json
import os
import shutil
import socket
import struct
import subprocess
import sys
import threading
import time
import urllib.parse
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LANDING = os.path.join(ROOT, "landing")
WIDTH, HEIGHT = 1920, 1080


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass


def serve(port):
    handler = functools.partial(QuietHandler, directory=LANDING)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


class DevTools:
    """Cliente WebSocket (RFC 6455) justo para mandar y recibir mensajes CDP."""

    def __init__(self, ws_url):
        url = urllib.parse.urlparse(ws_url)
        self.sock = socket.create_connection((url.hostname, url.port), timeout=60)
        key = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall(
            f"GET {url.path} HTTP/1.1\r\nHost: {url.hostname}:{url.port}\r\n"
            f"Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n".encode()
        )
        self.reader = self.sock.makefile("rb")
        status = self.reader.readline()
        if b" 101 " not in status:
            raise RuntimeError(f"DevTools rechazó el WebSocket: {status!r}")
        while self.reader.readline() not in (b"\r\n", b""):
            pass
        self.next_id = 0

    def _send(self, text):
        data = text.encode()
        head = bytearray([0x81])
        n = len(data)
        if n < 126:
            head.append(0x80 | n)
        elif n < 1 << 16:
            head += bytes([0x80 | 126]) + struct.pack(">H", n)
        else:
            head += bytes([0x80 | 127]) + struct.pack(">Q", n)
        mask = os.urandom(4)
        masked = bytes(b ^ mask[i % 4] for i, b in enumerate(data))
        self.sock.sendall(bytes(head) + mask + masked)

    def _read(self, n):
        data = self.reader.read(n)
        if data is None or len(data) < n:
            raise RuntimeError("DevTools cerró la conexión")
        return data

    def _recv(self):
        chunks = []
        while True:
            b0, b1 = self._read(2)
            n = b1 & 0x7F
            if n == 126:
                n = struct.unpack(">H", self._read(2))[0]
            elif n == 127:
                n = struct.unpack(">Q", self._read(8))[0]
            payload = self._read(n)
            opcode = b0 & 0x0F
            if opcode == 0x8:
                raise RuntimeError("DevTools cerró la conexión")
            if opcode in (0x9, 0xA):
                continue
            chunks.append(payload)
            if b0 & 0x80:
                return json.loads(b"".join(chunks))

    def call(self, method, **params):
        self.next_id += 1
        self._send(json.dumps({"id": self.next_id, "method": method, "params": params}))
        while True:
            msg = self._recv()
            if msg.get("id") != self.next_id:
                continue
            if "error" in msg:
                raise RuntimeError(f"{method}: {msg['error']}")
            return msg.get("result", {})

    def evaluate(self, expression, await_promise=False):
        result = self.call("Runtime.evaluate", expression=expression,
                           awaitPromise=await_promise, returnByValue=True)
        if "exceptionDetails" in result:
            raise RuntimeError(result["exceptionDetails"].get("exception", {}).get("description", result))
        return result.get("result", {}).get("value")


def chrome_command():
    for name in ("chromium", "chromium-browser", "google-chrome", "google-chrome-stable"):
        if shutil.which(name):
            return [name]
    if shutil.which("flatpak"):
        for app in ("com.google.Chrome", "org.chromium.Chromium"):
            if subprocess.run(["flatpak", "info", app], capture_output=True).returncode == 0:
                return ["flatpak", "run", app]
    sys.exit("No encuentro Chrome/Chromium (ni en el sistema ni en Flatpak).")


def open_chrome(port):
    args = chrome_command() + [
        "--headless=new", f"--remote-debugging-port={port}", "--remote-allow-origins=*",
        "--user-data-dir=/tmp/jap-video-chrome", "--no-first-run", "--no-default-browser-check",
        "--hide-scrollbars", "--mute-audio", "--force-device-scale-factor=1",
        f"--window-size={WIDTH},{HEIGHT}", "about:blank",
    ]
    proc = subprocess.Popen(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    deadline = time.time() + 30
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=2) as r:
                pages = [p for p in json.load(r) if p.get("type") == "page"]
            if pages:
                return proc, pages[0]["webSocketDebuggerUrl"]
        except OSError:
            pass
        time.sleep(.3)
    proc.terminate()
    sys.exit("Chrome no abrió el puerto de DevTools a tiempo.")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("-o", "--out", default=os.path.join(ROOT, "dist", "video", "jap-studio-release.mp4"))
    ap.add_argument("--fps", type=int, default=30)
    ap.add_argument("--theme", choices=("light", "dark"), default="light")
    ap.add_argument("--audio", help="pista principal de audio/SFX (wav/mp3); se recorta y se funde al final")
    ap.add_argument("--music", help="pista musical opcional de fondo (se mezcla debajo de --audio)")
    ap.add_argument("--music-volume", type=float, default=0.35, help="volumen relativo de --music (0.0–1.0)")
    ap.add_argument("--daw-clip", help="grabación real del DAW (mp4/mkv/webm) para la escena 'Detrás del sonido'")
    ap.add_argument("--daw-start", type=float, default=0.0, help="segundo inicial del --daw-clip")
    ap.add_argument("--start", type=float, default=0.0, help="segundo inicial (pruebas)")
    ap.add_argument("--end", type=float, help="segundo final (pruebas)")
    ap.add_argument("--http-port", type=int, default=8765)
    ap.add_argument("--cdp-port", type=int, default=9333)
    ap.add_argument("--preview", action="store_true", help="solo servir la vista previa")
    opts = ap.parse_args()

    if opts.daw_clip and not os.path.exists(opts.daw_clip):
        sys.exit(f"No existe --daw-clip: {opts.daw_clip}")

    server = serve(opts.http_port)
    page = f"http://127.0.0.1:{opts.http_port}/video.html?theme={opts.theme}"
    if opts.preview:
        print(f"Vista previa: {page}  (Ctrl+C para salir)")
        try:
            threading.Event().wait()
        except KeyboardInterrupt:
            return

    proc, ws = open_chrome(opts.cdp_port)
    ffmpeg = None
    try:
        cdp = DevTools(ws)
        cdp.call("Emulation.setDeviceMetricsOverride", width=WIDTH, height=HEIGHT, deviceScaleFactor=1, mobile=False)
        capture_url = page + "&capture=1" + ("&dawOverlay=1" if opts.daw_clip else "")
        cdp.call("Page.navigate", url=capture_url)
        meta = None
        for _ in range(100):
            try:
                meta = cdp.evaluate("window.__ready ? window.__ready.then(() => window.VIDEO_META) : null", True)
            except RuntimeError as err:
                sys.exit(f"video.html falló al construir: {err}")
            if meta:
                break
            time.sleep(.2)
        if not meta:
            sys.exit("video.html no terminó de cargar.")

        total = float(meta["total"])
        end = min(opts.end or total, total)
        frames = max(1, round((end - opts.start) * opts.fps))
        length = frames / opts.fps
        os.makedirs(os.path.dirname(os.path.abspath(opts.out)), exist_ok=True)

        cmd = ["ffmpeg", "-y", "-loglevel", "error", "-f", "image2pipe", "-framerate", str(opts.fps),
               "-c:v", "mjpeg", "-i", "-"]
        next_idx = 1
        daw_idx = None
        if opts.daw_clip:
            daw_idx = next_idx
            next_idx += 1
            cmd += ["-ss", str(opts.daw_start), "-i", opts.daw_clip]
        audio_idx = None
        if opts.audio:
            audio_idx = next_idx
            next_idx += 1
            cmd += ["-ss", str(opts.start), "-i", opts.audio]
        music_idx = None
        if opts.music:
            music_idx = next_idx
            next_idx += 1
            cmd += ["-ss", str(opts.start), "-i", opts.music]

        filters = []
        v_map = "0:v"
        if daw_idx is not None and "daw" in meta.get("scenes", {}):
            daw_a, daw_b = meta["scenes"]["daw"]
            rect = meta["dawRect"]
            rel_a = max(0.0, daw_a - opts.start)
            rel_b = max(0.0, daw_b - opts.start)
            daw_dur = max(0.1, rel_b - rel_a)
            if rel_b > 0.0 and rel_a < length:
                x, y, rw, rh = int(rect["x"]), int(rect["y"]), int(rect["w"]), int(rect["h"])
                fade_d = min(0.45, daw_dur / 3.0)
                filters.append(
                    f"[{daw_idx}:v]trim=duration={daw_dur:.3f},setpts=PTS-STARTPTS+{rel_a:.3f}/TB,"
                    f"scale={rw}:{rh}:force_original_aspect_ratio=decrease,"
                    f"pad={rw}:{rh}:(ow-iw)/2:(oh-ih)/2:color=#161513,format=yuva420p,"
                    f"fade=t=in:st={rel_a:.3f}:d={fade_d:.3f}:alpha=1,"
                    f"fade=t=out:st={max(rel_a, rel_b - fade_d):.3f}:d={fade_d:.3f}:alpha=1[dawv];"
                    f"[0:v][dawv]overlay={x}:{y}:eof_action=pass:shortest=0[vout]"
                )
                v_map = "[vout]"

        a_map = None
        fade_out_st = max(0.0, length - 2.2)
        if audio_idx is not None and music_idx is not None:
            filters.append(
                f"[{audio_idx}:a]atrim=duration={length:.3f},asetpts=PTS-STARTPTS,"
                f"afade=t=in:d=0.15,afade=t=out:st={fade_out_st:.2f}:d=2.2[a_sfx];"
                f"[{music_idx}:a]atrim=duration={length:.3f},asetpts=PTS-STARTPTS,"
                f"volume={opts.music_volume:.3f},"
                f"afade=t=in:d=0.5,afade=t=out:st={fade_out_st:.2f}:d=2.2[a_mus];"
                f"[a_sfx][a_mus]amix=inputs=2:duration=longest:normalize=0[aout]"
            )
            a_map = "[aout]"
        elif audio_idx is not None:
            filters.append(
                f"[{audio_idx}:a]atrim=duration={length:.3f},asetpts=PTS-STARTPTS,"
                f"afade=t=in:d=0.15,afade=t=out:st={fade_out_st:.2f}:d=2.2[aout]"
            )
            a_map = "[aout]"
        elif music_idx is not None:
            filters.append(
                f"[{music_idx}:a]atrim=duration={length:.3f},asetpts=PTS-STARTPTS,"
                f"afade=t=in:d=0.4,afade=t=out:st={fade_out_st:.2f}:d=2.2[aout]"
            )
            a_map = "[aout]"

        if filters:
            cmd += ["-filter_complex", ";".join(filters)]
        cmd += ["-map", v_map]
        if a_map:
            cmd += ["-map", a_map, "-c:a", "aac", "-b:a", "192k", "-shortest"]
        cmd += ["-c:v", "libx264", "-preset", "slow", "-crf", "18", "-pix_fmt", "yuv420p",
                "-r", str(opts.fps), "-movflags", "+faststart", opts.out]
        ffmpeg = subprocess.Popen(cmd, stdin=subprocess.PIPE)

        started = time.time()
        for i in range(frames):
            t = opts.start + i / opts.fps
            cdp.evaluate(f"render({t:.5f})")
            shot = cdp.call("Page.captureScreenshot", format="jpeg", quality=95)
            ffmpeg.stdin.write(base64.b64decode(shot["data"]))
            if i % opts.fps == 0 or i == frames - 1:
                eta = (time.time() - started) / (i + 1) * (frames - i - 1)
                print(f"\r{i + 1}/{frames} fotogramas · t={t:5.1f}s · faltan ~{eta:4.0f}s", end="", flush=True)
        print()
        ffmpeg.stdin.close()
        if ffmpeg.wait() != 0:
            sys.exit("ffmpeg falló.")
        try:
            cdp.call("Browser.close")
        except RuntimeError:
            pass
        print(f"Listo: {opts.out}")
    finally:
        if ffmpeg and ffmpeg.poll() is None:
            ffmpeg.kill()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.terminate()
        server.shutdown()


if __name__ == "__main__":
    main()
