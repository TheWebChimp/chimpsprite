//! En Windows, el ícono de la app (16, 24, 32 y 48 px, dibujados a mano) va adentro del `.exe`: es el
//! que muestran el Explorador y la barra de tareas.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/chimpsprite.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon/chimpsprite.ico");
        if let Err(e) = res.compile() {
            // Sin el SDK de Windows (rc.exe) la app compila igual, sin ícono en el `.exe`.
            println!("cargo:warning=no se pudo incrustar el ícono: {e}");
        }
    }
}
