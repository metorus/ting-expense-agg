// #[sides(client,server)]

#[cfg(feature = "graphics")] mod db_client_view;
#[cfg(feature = "graphics")] mod graphics;
#[cfg(feature = "graphics")] mod ecs;
#[cfg(feature = "graphics")] mod pie;
#[cfg(feature = "headless")] mod serv2;
mod dbs2;
mod crosstyping;
mod dbs;


#[cfg(feature = "graphics")]
type Db = dbs::SingleUserSqlite;
// type Db = crosstyping::PseudoUpstream;


#[cfg(all(feature = "graphics", not(target_arch = "wasm32")))]
fn main() -> eframe::Result {
    let icon = include_bytes!("../assets/icon-32.png");
    
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Expense Explorer")
            .with_inner_size([700.0, 600.0])
            .with_min_inner_size([600.0, 540.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(&icon[..])
                    .expect("Failed to load icon"),
            )
            ,
        ..Default::default()
    };
    eframe::run_native(
        "ton.ting.ExpenseExplorer",
        native_options,
        Box::new(|cc| Ok(Box::new(
            graphics::Trac::<Db>::new(cc, Db::default())
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
                    graphics::Trac::<Db>::new(cc, Db::default())
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
                        "<p> App crashed, see details in console. </p>",
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

