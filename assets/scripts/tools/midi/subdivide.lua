return {
    api_version = 1,
    id = "hay.midi.subdivide",
    name = "Subdividir notas",
    description = "Corta las notas seleccionadas en los límites de la rejilla actual.",
    parameters = {},
    run = function(context, _params)
        local step = context.grid_ticks
        for _, note in ipairs(context.notes) do
            if note.selected then
                note.cuts = {}
                local boundary = (note.start // step + 1) * step
                while boundary < note.start + note.duration do
                    note.cuts[#note.cuts + 1] = boundary
                    boundary = boundary + step
                end
            end
        end
    end,
}
