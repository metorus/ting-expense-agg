mod ecs;
mod pie;

pub use ecs::expense_category_slider;
pub use pie::pie_chart_with_legend;


/// Creates a representation of given Ok(expense) or Err(fact that it's not
/// loaded yet) on given ui, using single widget.
pub fn show_spending_mayload(ui: &mut egui::Ui, ml: crate::db_slice::MayLoad<'_>) {
    use time::format_description::well_known::Rfc3339;
    use crate::crosstyping::UNCLASSIFIED;
    use crate::db_slice::MayLoad::*;
    
    match ml {
        Confirmed(e) => ui.monospace(e.to_string()),
        NotLoaded    => ui.monospace("------------------------------"),
        Provisional{data, temp_time} =>
            ui.monospace(format!("[не синхронизировано!] - {} - {}\u{20bd} на {}",
                temp_time.format(&Rfc3339).unwrap(),
                data.amount, data.group.as_deref().unwrap_or(UNCLASSIFIED))),
    };
}



