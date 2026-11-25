#!/usr/bin/env python3
"""Prueba de cancelación del máster de Play frente a Exportar en Linux."""

import argparse
import array
import contextlib
import json
import math
import os
from pathlib import Path
import secrets
import select
import shutil
import signal
import subprocess
import sys
import time
import wave

RATE = 48_000
SECONDS = 4
REPO = Path(__file__).resolve().parents[1]


def wait_for(check, error, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if result := check():
            return result
        time.sleep(0.025)
    raise RuntimeError(error)


def fixtures(directory):
    for name, frequency, period, offset, decay, pan in [
        ("kick", 65, 0.5, 0, 0.055, (0.8, 1.0)),
        ("bass", 43, 0.75, 0.125, 0.19, (1.0, 0.6)),
    ]:
        pcm = array.array("h")
        for frame in range(SECONDS * RATE):
            t = frame / RATE - 1.0 - offset
            voice = 0.0
            if t >= 0 and frame < (SECONDS - 0.5) * RATE:
                hit = int(t / period)
                phase = t - hit * period
                amplitude = 0.23 + 0.04 * (hit % 3)
                voice = amplitude * math.exp(-phase / decay) * math.sin(math.tau * frequency * phase)
            pcm.extend(round(32767 * voice * gain) for gain in pan)
        if sys.byteorder != "little":
            pcm.byteswap()
        with wave.open(str(directory / f"{name}.wav"), "wb") as handle:
            handle.setparams((2, 2, RATE, 0, "NONE", "not compressed"))
            handle.writeframes(pcm.tobytes())


def start(stack, output, command, **kwargs):
    log = stack.enter_context(output.open("wb"))
    process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, **kwargs)

    def terminate():
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()

    stack.callback(terminate)
    return process


def capture(directory, binary):
    fixtures(directory)
    runtime = directory / "pipewire"
    runtime.mkdir(mode=0o700)
    config = directory / "config"
    config.mkdir()
    env = os.environ | {
        "PIPEWIRE_RUNTIME_DIR": str(runtime),
        "PIPEWIRE_REMOTE": "pipewire-0",
        "XDG_CONFIG_HOME": str(config),
        "XDG_STATE_HOME": str(directory / "state"),
    }
    env.pop("WAYLAND_DISPLAY", None)

    fragment = config / "pipewire" / "pipewire.conf.d" / "test.conf"
    fragment.parent.mkdir(parents=True)
    fragment.write_text("""
context.properties = {
    default.clock.rate = 48000
    default.clock.allowed-rates = [ 48000 ]
    default.clock.quantum = 256
}
context.objects = [
    { factory = adapter
      args = {
        factory.name = support.null-audio-sink
        node.name = jap-null-sink
        media.class = Audio/Sink
        audio.position = [ FL FR ]
        adapter.auto-port-config = { mode = dsp monitor = true position = preserve }
      }
    }
]
""")
    with contextlib.ExitStack() as stack:
        pipewire = start(stack, directory / "pipewire.log", ["pipewire"], env=env)
        wait_for(lambda: (runtime / "pipewire-0").is_socket() and pipewire.poll() is None,
                 "PipeWire privado no inició")
        start(stack, directory / "wireplumber.log", ["wireplumber", "-p", "policy"], env=env)

        read_fd, write_fd = os.pipe()
        try:
            start(stack, directory / "display.log",
                  ["Xwayland", "-displayfd", str(write_fd), "-geometry", "1400x920"], pass_fds=(write_fd,))
            os.close(write_fd)
            write_fd = None
            if not select.select([read_fd], [], [], 10)[0]:
                raise RuntimeError("Xwayland no publicó un display")
            display = os.read(read_fd, 32).decode().strip()
            if not display.isdigit():
                raise RuntimeError("Xwayland no inició")
        finally:
            os.close(read_fd)
            if write_fd is not None:
                os.close(write_fd)
        env["DISPLAY"] = f":{display}"

        app = start(stack, directory / "daw.log",
                    ["bwrap", "--ro-bind", "/", "/", "--dev-bind", "/dev", "/dev",
                     "--proc", "/proc", "--bind", str(directory), str(directory),
                     "--tmpfs", str(Path.home()), "--ro-bind", str(REPO), str(REPO),
                     "--ro-bind", str(binary), str(directory / "jap-studio"),
                     "--chdir", str(REPO), str(directory / "jap-studio")], env=env)

        def xdo(*args):
            return subprocess.run(["xdotool", *args], env=env, capture_output=True, text=True, timeout=10)

        def find_window():
            if app.poll() is not None:
                raise RuntimeError("Jap Studio cerró al iniciar")
            ids = xdo("search", "--onlyvisible", "--name", "^Jap Studio$").stdout.split()
            return ids[0] if ids else None

        window = wait_for(find_window, "Jap Studio no abrió la ventana", timeout=20)
        xdo("windowfocus", window).check_returncode()
        subprocess.run(["xmodmap", "-e", "keycode 200 = colon"], env=env, check=True, timeout=5)

        def command(line):
            xdo("key", "Escape", "200").check_returncode()
            time.sleep(0.1)
            xdo("type", "--clearmodifiers", "--delay", "1", line).check_returncode()
            xdo("key", "Return").check_returncode()
            time.sleep(0.35)

        time.sleep(0.6)
        command(f"file.import {directory / 'kick.wav'}")
        xdo("key", "Down").check_returncode()
        command(f"file.import {directory / 'bass.wav'}")
        command(f"export -n export.wav -out {directory} -sr {RATE} -bits 32f -master")
        export = directory / "export.wav"
        wait_for(export.is_file, "Jap Studio no generó export.wav", timeout=20)

        live = directory / "play.wav"
        recorder = start(stack, directory / "record.log",
                         ["pw-record", "--target=jap-null-sink", "-P",
                          "{ node.name = jap-null-record stream.capture.sink = true }",
                          "--channels=2", "--channel-map=FL,FR", f"--rate={RATE}", "--format=f32",
                          str(live)], env=env)

        def monitor_linked():
            links = subprocess.run(["pw-link", "-l"], env=env, capture_output=True, text=True,
                                   check=True, timeout=4).stdout
            return all(f"jap-null-record:input_{channel}" in links for channel in ("FL", "FR"))

        wait_for(monitor_linked, "La grabación no pudo conectarse al monitor")
        xdo("key", "space").check_returncode()
        time.sleep(SECONDS + 0.3)
        recorder.send_signal(signal.SIGINT)
        recorder.wait(timeout=5)
    return export, live


def wave_samples(path):
    streams = json.loads(subprocess.check_output(
        ["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries",
         "stream=sample_rate,channels", "-of", "json", str(path)], timeout=10,
    ))["streams"]
    if streams != [{"sample_rate": str(RATE), "channels": 2}]:
        raise RuntimeError(f"{path.name} debe ser estéreo a {RATE} Hz")
    data = subprocess.check_output(
        ["ffmpeg", "-v", "error", "-i", str(path), "-map", "0:a:0", "-f", "f32le", "-"],
        timeout=15,
    )
    pcm = array.array("f")
    pcm.frombytes(data)
    if sys.byteorder != "little":
        pcm.byteswap()
    if not pcm or len(pcm) % 2 or any(not math.isfinite(sample) for sample in pcm):
        raise RuntimeError(f"PCM estéreo inválido en {path.name}")
    return pcm


def compare(reference, live):
    def onset(pcm):
        for index in range(0, len(pcm), 2):
            if max(abs(pcm[index]), abs(pcm[index + 1])) >= 1e-4:
                return index // 2
        raise RuntimeError("La referencia o la captura está en silencio")

    expected_onset, actual_onset = map(onset, (reference, live))
    # La captura puede comenzar tras el primer bloque silencioso. Solo se
    # corrige un desplazamiento entero para los dos canales, sin tocar la ganancia.
    lag = actual_onset - expected_onset
    if len(live) // 2 - lag < len(reference) // 2:
        raise RuntimeError("La captura terminó antes que la referencia")
    if lag < 0 and any(reference[:-lag * 2]):
        raise RuntimeError("Faltan muestras audibles al principio de la captura")

    peak = squared = 0.0
    difference = array.array("f")
    first = min(0, lag) * 2
    last = max(len(live), len(reference) + lag * 2)
    for index in range(first, last):
        target = index - lag * 2
        expected = reference[target] if 0 <= target < len(reference) else 0.0
        captured = live[index] if 0 <= index < len(live) else 0.0
        residual = captured - expected
        peak = max(peak, abs(residual))
        squared += residual * residual
        difference.append(residual)
    return lag, peak, math.sqrt(squared / len(reference)), difference


def validate_fixture(directory, reference):
    kick, bass = (wave_samples(directory / filename) for filename in ("kick.wav", "bass.wav"))
    if len(reference) != len(kick) or len(reference) != len(bass) or any(
        abs(rendered - (a + b)) > 1e-6 for rendered, a, b in zip(reference, kick, bass)
    ):
        raise RuntimeError("El máster exportado difiere de la suma esperada del kick y el bajo")


def save_residual(directory, pcm):
    if sys.byteorder != "little":
        pcm.byteswap()
    subprocess.run(
        ["ffmpeg", "-v", "error", "-f", "f32le", "-ar", str(RATE), "-ac", "2",
         "-i", "-", "-c:a", "pcm_f32le", str(directory / "residuo.wav")],
        input=pcm.tobytes(), check=True, timeout=10,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", type=Path, default=REPO / ".local/bin/hay-daw")
    parser.add_argument("--limit-dbfs", type=float, default=-100.0)
    parser.add_argument("--keep", action="store_true", help="Conservar WAV y registros en /tmp")
    args = parser.parse_args()
    binary = args.bin.resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error(f"No encontré el release en {binary}")
    if not math.isfinite(args.limit_dbfs) or args.limit_dbfs >= 0:
        parser.error("El límite debe ser un número negativo en dBFS")
    for name in ["Xwayland", "bwrap", "xdotool", "xmodmap", "pipewire", "wireplumber",
                 "pw-record", "pw-link", "ffmpeg", "ffprobe"]:
        if not shutil.which(name):
            parser.error(f"Falta la herramienta {name}")

    directory = Path("/tmp") / f"jap-null-{secrets.token_hex(8)}"
    directory.mkdir(mode=0o700)
    result = 1
    try:
        exported, captured = capture(directory, binary)
        reference, live = map(wave_samples, (exported, captured))
        lag, peak, rms, residual = compare(reference, live)
        db = lambda value: f"{20 * math.log10(value):.2f}" if value else "−∞"
        result = int(peak > 10 ** (args.limit_dbfs / 20))
        if args.keep or result:
            save_residual(directory, residual)
        validate_fixture(directory, reference)
        print(f"{'FALLÓ' if result else 'OK'} · alineación {lag:+} muestras · "
              f"pico {db(peak)} dBFS · RMS {db(rms)} dBFS · límite {args.limit_dbfs:g} dBFS")
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Error en la prueba de cancelación: {error}", file=sys.stderr)
        result = 2
    finally:
        if args.keep or result:
            print(f"Archivos de la prueba: {directory}")
        else:
            shutil.rmtree(directory)
    return result


if __name__ == "__main__":
    sys.exit(main())
