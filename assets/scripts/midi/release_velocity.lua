-- Velocidad de liberación: el instrumento decide su respuesta audible.
for _, note in ipairs(notes) do
    if note.selected then note.release_velocity = value end
end
