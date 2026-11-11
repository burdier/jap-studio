-- Fija la velocidad MIDI de las notas seleccionadas (o de todas si no hay selección).
for _, note in ipairs(notes) do
    if note.selected then
        note.velocity = value
    end
end
