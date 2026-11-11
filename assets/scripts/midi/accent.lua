-- Ejemplo: acentúa las notas seleccionadas. Sin selección, acentúa todo el clip.
for _, note in ipairs(notes) do
    if note.selected then
        note.velocity = math.min(127, note.velocity + 12)
    end
end
