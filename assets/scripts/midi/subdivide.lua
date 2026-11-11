-- Cortes absolutos sobre la rejilla del patrón; conserva ambos extremos.
-- Rust valida los cortes, crea IDs nuevos y propaga selección y silencio.
for _, note in ipairs(notes) do
    if note.selected then
        note.cuts = {}
        local boundary = (note.start // grid_ticks + 1) * grid_ticks
        while boundary < note.start + note.duration do
            note.cuts[#note.cuts + 1] = boundary
            boundary = boundary + grid_ticks
        end
    end
end
