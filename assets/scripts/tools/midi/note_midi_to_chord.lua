return {
    api_version = 1,
    id = "hay.midi.note_midi_to_chord",
    name = "Fundamentales MIDI a acordes",
    description = "Añade terceras y quintas diatónicas según la escala activa del piano roll.",
    parameters = {},
    run = function(context, _params)
        local function in_scale(pitch)
            local interval = (pitch - context.scale.root) % 12
            return math.floor(context.scale.intervals / (2 ^ interval)) % 2 == 1
        end

        local function scale_step_above(pitch, steps)
            local found = 0
            for candidate = pitch + 1, 127 do
                if in_scale(candidate) then
                    found = found + 1
                    if found == steps then
                        return candidate
                    end
                end
            end
            error("No hay suficientes tonos de la escala sobre esta fundamental")
        end

        local function note_key(instrument, start, duration, pitch)
            return instrument .. ":" .. start .. ":" .. duration .. ":" .. pitch
        end

        local existing = {}
        for _, note in ipairs(context.notes) do
            existing[note_key(note.instrument, note.start, note.duration, note.pitch)] = true
        end

        for _, root in ipairs(context.notes) do
            if root.selected then
                if not in_scale(root.pitch) then
                    error("La fundamental seleccionada no pertenece a la escala activa")
                end

                local third = scale_step_above(root.pitch, 2)
                local fifth = scale_step_above(root.pitch, 4)
                for _, pitch in ipairs({ third, fifth }) do
                    local key = note_key(root.instrument, root.start, root.duration, pitch)
                    if not existing[key] then
                        context.new_notes[#context.new_notes + 1] = {
                            source_id = root.id,
                            start = root.start,
                            duration = root.duration,
                            pitch = pitch,
                            selected = false,
                        }
                        existing[key] = true
                    end
                end
            end
        end
    end,
}
