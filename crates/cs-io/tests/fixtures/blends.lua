-- Un frame por modo de mezcla: fondo con colores y alfas variados y una capa con el modo encima
-- (con opacidad parcial). Uso: aseprite -b --script-param out=blends.aseprite --script blends.lua
local modes = {
  BlendMode.NORMAL, BlendMode.MULTIPLY, BlendMode.SCREEN, BlendMode.OVERLAY,
  BlendMode.DARKEN, BlendMode.LIGHTEN, BlendMode.ADDITION, BlendMode.SUBTRACT,
  BlendMode.DIFFERENCE,
}
local W, H = 16, 16
local spr = Sprite(W, H, ColorMode.RGB)
app.activeSprite = spr
for i = 2, #modes do spr:newEmptyFrame(i) end

local back = Image(W, H, ColorMode.RGB)
for y = 0, H - 1 do
  for x = 0, W - 1 do
    back:drawPixel(x, y, Color{ r = x * 16, g = 255 - y * 16, b = (x * y) % 256, a = y * 17 })
  end
end
local top = Image(W, H, ColorMode.RGB)
for y = 0, H - 1 do
  for x = 0, W - 1 do
    top:drawPixel(x, y, Color{ r = 255 - x * 12, g = y * 14, b = 128 + x * 4, a = 255 - x * 15 })
  end
end

local base = spr.layers[1]
base.name = "Back"
for i = 1, #modes do
  local layer = spr:newLayer()
  layer.name = "Mode " .. i
  layer.blendMode = modes[i]
  layer.opacity = 200
  spr:newCel(base, i, back, Point(0, 0))
  spr:newCel(layer, i, top, Point(0, 0))
end
spr:saveAs(app.params["out"])
