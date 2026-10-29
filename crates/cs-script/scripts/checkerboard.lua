-- Un tablero de ajedrez de 2×2 en la capa actual, en todo el sprite.
local cell = 2
local pixels = {}
for y = 0, height() - 1 do
  for x = 0, width() - 1 do
    if ((x // cell) + (y // cell)) % 2 == 0 then
      pixels[#pixels + 1] = { x = x, y = y, color = "#222034" }
    end
  end
end
draw_pixels{ pixels = pixels }
print(#pixels .. " pixels painted")
