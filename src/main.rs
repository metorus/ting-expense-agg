#[cfg(feature = "graphics")] mod graphics;
#[cfg(feature = "graphics")] mod ecs;
#[cfg(feature = "graphics")] mod pie;
mod crosstyping;


#[cfg(all(feature = "graphics", not(target_arch = "wasm32")))]
fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([700.0, 600.0])
            .with_min_inner_size([600.0, 540.0])
        //  .with_icon(
        //      // NOTE: Adding an icon is optional
        //      eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
        //          .expect("Failed to load icon"),
        //  )
            ,
        ..Default::default()
    };
    eframe::run_native(
        "Expense Explorer",
        native_options,
        Box::new(|cc| Ok(Box::new(
            graphics::Trac::<crosstyping::FallbackDb>::new(cc)
        ))),
    )
}


#[cfg(all(feature = "graphics", target_arch = "wasm32"))]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    // eframe::WebLogger::init(log::LevelFilter::Debug).ok();
    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().expect("No window")
            .document().expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(
                    graphics::Trac::<crosstyping::FallbackDb>::new(cc)
                ))),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}


#[cfg(feature = "headless")]
fn main() {
    todo!();
}

