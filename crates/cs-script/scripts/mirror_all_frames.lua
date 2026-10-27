-- Voltea la capa actual de izquierda a derecha en todos los frames.
local frames = 0
for _, a in ipairs(describe().artboards) do
  if a.id == current_artboard() then frames = #a.frame_durations_ms end
end
flip{ axis = "horizontal", frames = { from = 0, to = frames - 1 } }
print(frames .. " frames flipped")
