-- Exporta cada frame compuesto como PNG RGBA (también de sprites indexados, que Aseprite
-- exportaría como PNG indexado sin alfa parcial). Uso:
--   aseprite -b --script-param file=X.aseprite --script-param out=X_ --script frames.lua
local spr = app.open(app.params["file"])
if spr.colorMode ~= ColorMode.RGB then
  app.command.ChangePixelFormat{ format = "rgb" }
end
for i, frame in ipairs(spr.frames) do
  local img = Image(spr.spec)
  img:drawSprite(spr, frame)
  img:saveAs(app.params["out"] .. (i - 1) .. ".png")
end
