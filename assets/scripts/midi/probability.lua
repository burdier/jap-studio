-- Guarda la probabilidad: el reproductor decide el disparo por vuelta.
for _, note in ipairs(notes) do
    if note.selected then note.probability = amount end
end
