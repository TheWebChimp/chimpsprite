-- Un tablero de ajedrez de 2×2 en la capa actual, en todo el sprite.
local a = describe().artboards[1]
local cell = 2
local pixels = {}
for y = 0, a.height - 1 do
  for x = 0, a.width - 1 do
    if ((x // cell) + (y // cell)) % 2 == 0 then
      pixels[#pixels + 1] = { x = x, y = y, color = "#222034" }
    end
  end
end
draw_pixels{ pixels = pixels }
print(#pixels .. " pixels painted")
