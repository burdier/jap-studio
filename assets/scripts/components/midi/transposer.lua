-- PR-08: ejemplo de transformación musical. Cambia este intervalo antes de cargar.
-- La identidad y el emparejamiento NoteOn/NoteOff pertenecen al host.
local semitones = 7

return function(kind, channel, data1, data2, offset)
    if kind == 1 or kind == 2 then
        data1 = data1 + semitones
        if data1 < 0 or data1 > 127 then
            error("Transpositor: nota fuera del rango MIDI")
        end
    end
    return kind, channel, data1, data2, offset
end
