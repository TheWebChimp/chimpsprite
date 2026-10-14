//! Punto de entrada: ventana nativa en escritorio, `<canvas>` en el navegador.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    use eframe::egui;

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ChimpSprite")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "ChimpSprite",
        options,
        Box::new(|cc| Ok(Box::new(cs_ui::ChimpApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast;

    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    let web_options = eframe::WebOptions {
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    wasm_bindgen_futures::spawn_local(async move {
        let document = eframe::web_sys::window()
            .and_then(|w| w.document())
            .expect("no document");
        let canvas = document
            .get_element_by_id("chimpsprite")
            .expect("missing <canvas id=\"chimpsprite\">")
            .dyn_into::<eframe::web_sys::HtmlCanvasElement>()
            .expect("#chimpsprite is not a canvas");
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(cs_ui::ChimpApp::new(cc)))),
            )
            .await;
        if let Some(loading) = document.get_element_by_id("loading") {
            match result {
                Ok(()) => loading.remove(),
                Err(e) => {
                    log::error!("ChimpSprite could not start: {e:?}");
                    loading.set_inner_html(
                        "<p><strong>ChimpSprite could not start.</strong></p>\
                         <p>Your browser needs WebGPU or WebGL2. Try an up-to-date \
                         Chrome, Edge, Firefox or Safari.</p>",
                    );
                }
            }
        }
    });
}
