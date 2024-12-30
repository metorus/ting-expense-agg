mod crosstyping;
#[cfg(feature = "graphics")] mod graphics;


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


#[cfg(feature = "headless")]
fn main() {
    todo!();
}

