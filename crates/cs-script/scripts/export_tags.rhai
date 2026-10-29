// Un GIF (×4) por cada tag del artboard actual, en la carpeta del documento.
let dir = export_folder();
let artboard = ();
for a in describe().artboards {
    if a.id == current_artboard() { artboard = a; }
}
if artboard.tags.is_empty() {
    print("This artboard has no tags: nothing to export");
} else {
    for t in artboard.tags {
        let path = `${dir}/${artboard.name}_${t.name}.gif`;
        export_sprite(#{ kind: "gif", tag: t.name, path: path, scale: 4 });
        print(path);
    }
}
