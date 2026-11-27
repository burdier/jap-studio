#!/usr/bin/env python3
"""Genera one-shots Foley orgánicos (papel, lápiz, tachón, clic de tecla, pop) en assets/video-sfx/.

Se cargan como clips de audio / Sampler dentro de Jap Studio para combinarlos
con Hay Synth, Hay Piano, Drums y efectos nativos al producir el audio del video.
"""

import math
import os
import random
import struct
import wave

SR = 48000
OUT_DIR = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "assets",
    "video-sfx",
)


def write_wav(name, samples_l, samples_r=None):
    if samples_r is None:
        samples_r = samples_l
    os.makedirs(OUT_DIR, exist_ok=True)
    path = os.path.join(OUT_DIR, name)
    peak = max(
        1e-6,
        max(abs(x) for x in samples_l),
        max(abs(x) for x in samples_r),
    )
    scale = 0.88 / peak if peak > 0.88 else 1.0
    with wave.open(path, "wb") as wf:
        wf.setnchannels(2)
        wf.setsampwidth(2)
        wf.setframerate(SR)
        buf = bytearray()
        for l, r in zip(samples_l, samples_r):
            il = max(-32767, min(32767, int(l * scale * 32767.0)))
            ir = max(-32767, min(32767, int(r * scale * 32767.0)))
            buf += struct.pack("<hh", il, ir)
        wf.writeframes(buf)
    return path


def bandpass_step(x, f, q, state):
    """Filtro biquad SVF simple para moldear ruido orgánico."""
    g = math.tan(math.pi * min(f, SR * 0.42) / SR)
    k = 1.0 / max(q, 0.25)
    a1 = 1.0 / (1.0 + g * (g + k))
    a2 = g * a1
    a3 = g * a2
    v3 = x - state[1]
    v1 = a1 * state[0] + a2 * v3
    v2 = state[1] + a2 * state[0] + a3 * v3
    state[0] = 2.0 * v1 - state[0]
    state[1] = 2.0 * v2 - state[1]
    return v1, v2


def gen_pencil_stroke(dur=0.65, seed=11, freq_lo=1600.0, freq_hi=3600.0):
    rng = random.Random(seed)
    n = int(SR * dur)
    st_l, st_r = [0.0, 0.0], [0.0, 0.0]
    out_l, out_r = [], []
    for i in range(n):
        t = i / SR
        u = t / dur
        env = math.sin(math.pi * u) ** 0.7
        grain = 0.65 + 0.35 * math.sin(2 * math.pi * (18.0 * t + 0.5 * math.sin(9.0 * t)))
        freq = freq_lo + (freq_hi - freq_lo) * (0.5 + 0.5 * math.sin(math.pi * u))
        nl = rng.uniform(-1.0, 1.0)
        nr = 0.7 * nl + 0.3 * rng.uniform(-1.0, 1.0)
        bp_l, _ = bandpass_step(nl, freq, 2.2, st_l)
        bp_r, _ = bandpass_step(nr, freq * 1.05, 2.2, st_r)
        out_l.append(bp_l * env * grain * 0.75)
        out_r.append(bp_r * env * grain * 0.75)
    return out_l, out_r


def gen_red_strike(dur=0.42, seed=23):
    """Tachón enérgico de rotulador/lápiz grueso con cuerpo grave y roce agudo."""
    rng = random.Random(seed)
    n = int(SR * dur)
    st_hi = [0.0, 0.0]
    st_lo = [0.0, 0.0]
    out_l, out_r = [], []
    for i in range(n):
        t = i / SR
        u = t / dur
        env = (min(1.0, u / 0.08)) * ((1.0 - u) ** 1.35)
        raw = rng.uniform(-1.0, 1.0)
        f_hi = 1400.0 + 3200.0 * math.sin(math.pi * u)
        hi, _ = bandpass_step(raw, f_hi, 1.6, st_hi)
        lo, _ = bandpass_step(raw, 420.0 + 260.0 * (1.0 - u), 1.4, st_lo)
        whoosh = math.sin(2 * math.pi * (240.0 * t - 90.0 * t * t)) * 0.22 * math.exp(-t * 9.0)
        sig = (hi * 0.85 + lo * 0.75 + whoosh) * env
        pan = -0.35 + 0.7 * u
        out_l.append(sig * (0.8 - 0.35 * pan))
        out_r.append(sig * (0.8 + 0.35 * pan))
    return out_l, out_r


def gen_page_flip(dur=0.36, seed=37):
    """Vuelta suave de hoja de cuaderno."""
    rng = random.Random(seed)
    n = int(SR * dur)
    st1, st2 = [0.0, 0.0], [0.0, 0.0]
    out_l, out_r = [], []
    for i in range(n):
        t = i / SR
        u = t / dur
        env = math.sin(math.pi * u) ** 1.2 * (1.0 + 0.35 * math.exp(-((u - 0.62) / 0.12) ** 2))
        raw = rng.uniform(-1.0, 1.0)
        f1 = 700.0 + 1800.0 * u
        f2 = 2800.0 - 1100.0 * u
        b1, _ = bandpass_step(raw, f1, 1.1, st1)
        b2, _ = bandpass_step(raw, f2, 1.4, st2)
        sig = (b1 * 0.7 + b2 * 0.55) * env
        pan = -0.5 + 1.0 * u
        out_l.append(sig * (0.75 - 0.3 * pan))
        out_r.append(sig * (0.75 + 0.3 * pan))
    return out_l, out_r


def gen_bubble_pop(dur=0.16, base_hz=310.0, end_hz=760.0):
    """Pop orgánico tipo burbuja/zanahoria para píldoras y diálogos."""
    n = int(SR * dur)
    out = []
    phase = 0.0
    for i in range(n):
        t = i / SR
        u = t / dur
        freq = base_hz + (end_hz - base_hz) * (1.0 - math.exp(-u * 6.5))
        phase += 2 * math.pi * freq / SR
        env = min(1.0, u / 0.05) * math.exp(-u * 6.2)
        s = (math.sin(phase) + 0.18 * math.sin(2 * phase)) * env * 0.85
        out.append(s)
    return out, out


def gen_carrot_hop(dur=0.22):
    """Saltito elástico y cálido para la zanahoria."""
    n = int(SR * dur)
    out = []
    phase = 0.0
    for i in range(n):
        t = i / SR
        u = t / dur
        freq = 210.0 + 360.0 * math.sin(math.pi * min(1.0, u * 1.1))
        phase += 2 * math.pi * freq / SR
        env = min(1.0, u / 0.04) * math.exp(-u * 4.8)
        s = (math.sin(phase) + 0.25 * math.sin(2.0 * phase) * math.exp(-u * 8.0)) * env * 0.8
        out.append(s)
    return out, out


def gen_key_click(dur=0.055, seed=71):
    """Clic táctil de tecla mecánica para la terminal del cierre."""
    rng = random.Random(seed)
    n = int(SR * dur)
    st = [0.0, 0.0]
    out = []
    for i in range(n):
        t = i / SR
        u = t / dur
        click = math.sin(2 * math.pi * (1900.0 - 900.0 * u) * t) * math.exp(-t * 140.0)
        thock = math.sin(2 * math.pi * 340.0 * t) * math.exp(-t * 85.0)
        noise, _ = bandpass_step(rng.uniform(-1.0, 1.0), 3400.0, 1.5, st)
        sig = click * 0.55 + thock * 0.45 + noise * math.exp(-t * 110.0) * 0.65
        out.append(sig)
    return out, out


def gen_terminal_typing(dur=2.4, chars=22, seed=91):
    """Ráfaga natural de tecleo mecánico sincronizada con el bloque de instalación."""
    rng = random.Random(seed)
    n = int(SR * dur)
    out_l = [0.0] * n
    out_r = [0.0] * n
    for c in range(chars):
        t_hit = (c / chars) * (dur - 0.12) + rng.uniform(-0.015, 0.018)
        t_hit = max(0.0, min(dur - 0.06, t_hit))
        idx = int(t_hit * SR)
        kl, _ = gen_key_click(0.052, seed=seed + c * 7)
        gain = rng.uniform(0.55, 0.95)
        pan = rng.uniform(-0.25, 0.25)
        for j, s in enumerate(kl):
            if idx + j < n:
                out_l[idx + j] += s * gain * (0.8 - pan)
                out_r[idx + j] += s * gain * (0.8 + pan)
    return out_l, out_r


def main():
    files = {
        "pencil-stroke.wav": gen_pencil_stroke(0.75, seed=11),
        "pencil-short.wav": gen_pencil_stroke(0.32, seed=17, freq_lo=2000.0, freq_hi=4100.0),
        "red-strike.wav": gen_red_strike(0.42, seed=23),
        "strike-short.wav": gen_red_strike(0.20, seed=29),
        "page-flip.wav": gen_page_flip(0.36, seed=37),
        "bubble-pop.wav": gen_bubble_pop(0.15, 310.0, 780.0),
        "carrot-hop.wav": gen_carrot_hop(0.22),
        "key-click.wav": gen_key_click(0.055, seed=71),
        "terminal-typing.wav": gen_terminal_typing(2.4, chars=24, seed=91),
    }
    for name, (l, r) in files.items():
        path = write_wav(name, l, r)
        print(f"Generado: {os.path.relpath(path)} ({len(l) / SR:.2f}s)")


if __name__ == "__main__":
    main()
