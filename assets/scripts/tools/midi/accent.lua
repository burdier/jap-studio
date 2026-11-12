return {
    api_version = 1,
    id = "hay.midi.accent",
    name = "Acentuar notas",
    description = "Aumenta la velocity de las notas seleccionadas.",
    parameters = {
        {
            key = "amount", label = "Acento", kind = "number",
            default = 12, min = 0, max = 126, step = 1,
            whole = true, unit = "velocity",
        },
    },
    run = function(context, params)
        for _, note in ipairs(context.notes) do
            if note.selected then
                note.velocity = math.min(127, note.velocity + params.amount)
            end
        end
    end,
}
