use eframe::{App, CreationContext, Frame};
use egui::*;

use crate::crosstyping::{TunedDb, LastInfo};


pub struct Trac<D: TunedDb> {
    db: D
}
impl<D: TunedDb + Default> Trac<D> {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(Theme::Light);
        
        Trac {
            db: Default::default()
        }
    }
}
impl<D: TunedDb> App for Trac<D> {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        TopBottomPanel::bottom("status_bar")
            .min_height(30.0)
            .show(ctx, |ui| {
                ui.label("Expense Explorer by House Ting");
            });
        
        
    }
}

