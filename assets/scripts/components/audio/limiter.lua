-- Limiter lookahead estéreo estilo Ableton Live (Gain, Ceiling, Release, Lookahead).
return {
    api_version = 2, kind = "audio_component", id = "hay.fx.limiter",
    name = "Hay Limiter", description = "Limiter estéreo estilo Ableton con Gain, Ceiling, Release y Lookahead (1.5 / 3 / 6 ms).",
    ports = { input = { kind = "audio", channels = 2 }, output = { kind = "audio", channels = 2 } },
    graph = {
        nodes = { {
            id = "limiter", primitive = "elevate.dynamics.limiter",
            parameters = { gain = 0.0, ceiling = -0.3, release = 100.0, lookahead = 1 },
        } },
        edges = { { from = "input", to = "limiter" }, { from = "limiter", to = "output" } },
    },
    controls = {
        { id = 0, key = "gain", label = "Gain", kind = "number",
          min = -24, max = 24, default = 0, step = 0.5, unit = "dB",
          targets = { { node = "limiter", parameter = "gain" } } },
        { id = 1, key = "ceiling", label = "Ceiling", kind = "number",
          min = -24, max = 0, default = -0.3, step = 0.1, unit = "dB",
          targets = { { node = "limiter", parameter = "ceiling" } } },
        { id = 2, key = "release", label = "Release", kind = "number", scale = "log",
          min = 1, max = 1000, default = 100, step = 1, unit = "ms",
          targets = { { node = "limiter", parameter = "release" } } },
        { id = 3, key = "lookahead", label = "Lookahead", kind = "choice",
          choices = { "1.5 ms", "3.0 ms", "6.0 ms" },
          min = 0, max = 2, default = 1, step = 1, unit = "tipo",
          targets = { { node = "limiter", parameter = "lookahead" } } },
    },
}
