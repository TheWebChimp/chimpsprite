-- Voltea la capa actual de izquierda a derecha en todos los frames.
local frames = #describe().artboards[1].frame_durations_ms
flip{ axis = "horizontal", frames = { from = 0, to = frames - 1 } }
print(frames .. " frames flipped")
