-- Filtro FunDSP nativo; Lua describe el grafo y sus controles.
return {
    api_version = 2, kind = "audio_component", id = "hay.fx.filter",
    name = "Hay Filter", description = "Filtro estéreo LP/HP/BP con frecuencia y resonancia; DSP FunDSP.",
    ports = { input = { kind = "audio", channels = 2 }, output = { kind = "audio", channels = 2 } },
    graph = {
        nodes = { { id = "filter", primitive = "elevate.filter.svf",
                    parameters = { cutoff = 1000, resonance = 0.707, mode = 0 } } },
        edges = { { from = "input", to = "filter" }, { from = "filter", to = "output" } },
    },
    response_node = "filter",
    controls = {
        { id = 0, key = "cutoff", label = "Frecuencia", kind = "number", scale = "log",
          min = 20, max = 20000, default = 1000, step = 1, unit = "Hz",
          targets = { { node = "filter", parameter = "cutoff" } } },
        { id = 1, key = "resonance", label = "Resonancia", kind = "number",
          min = 0.5, max = 10, default = 0.707, step = 0.01, unit = "Q",
          targets = { { node = "filter", parameter = "resonance" } } },
        { id = 2, key = "mode", label = "Tipo", kind = "choice", choices = { "LP", "HP", "BP" },
          min = 0, max = 2, default = 0, step = 1, unit = "tipo",
          targets = { { node = "filter", parameter = "mode" } } },
    },
}
