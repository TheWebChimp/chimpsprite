-- Tres copias del frame actual después de él y un tag "Walk" con los cuatro.
local first = current_frame()
for i = 0, 2 do
  add_frame{ duplicate = first + i }
end
set_tag{ name = "Walk", from = first, to = first + 3, color = "#e27285" }
print("Walk: frames " .. (first + 1) .. " to " .. (first + 4))
