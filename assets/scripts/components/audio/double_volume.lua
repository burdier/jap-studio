return {
    api_version = 1,
    kind = "audio_component",
    id = "hay.fx.double_volume",
    name = "Hay Double Volume",
    description = "Dos etapas Volume con una macro de ganancia.",
    ports = {
        input = { kind = "audio", channels = 2 },
        output = { kind = "audio", channels = 2 },
    },
    graph = {
        nodes = {
            { id = "first", primitive = "elevate.amplitude.volume", parameters = { gain = 1.0 } },
            { id = "second", primitive = "elevate.amplitude.volume", parameters = { gain = 1.0 } },
        },
        edges = {
            { from = "input", to = "first" },
            { from = "first", to = "second" },
            { from = "second", to = "output" },
        },
    },
    controls = {
        { id = 0, key = "gain", label = "Ganancia", kind = "number",
          min = 0.0, max = 2.0, default = 1.0, step = 0.01, unit = "x",
          targets = {
              { node = "first", parameter = "gain" },
              { node = "second", parameter = "gain" },
          } },
    },
}
