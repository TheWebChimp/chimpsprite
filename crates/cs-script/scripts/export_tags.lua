-- Un GIF (×4) por cada tag del artboard actual, en la carpeta del documento.
local dir = export_folder()
local artboard
for _, a in ipairs(describe().artboards) do
  if a.id == current_artboard() then artboard = a end
end
if #artboard.tags == 0 then
  print("This artboard has no tags: nothing to export")
else
  for _, t in ipairs(artboard.tags) do
    local path = dir .. "/" .. artboard.name .. "_" .. t.name .. ".gif"
    export_sprite{ kind = "gif", tag = t.name, path = path, scale = 4 }
    print(path)
  end
end
