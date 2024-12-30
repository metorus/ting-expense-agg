use egui::{*, FontFamily::Proportional, FontId, TextStyle::*};
use eframe::{App, CreationContext, Frame};

use std::collections::BTreeMap;

use crate::crosstyping::{TunedDb, LastInfo};



pub struct Trac<D: TunedDb> {
    db: D,
    
    form_spent: u64,
    form_comment: String,
}
impl<D: TunedDb + Default> Trac<D> {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(Theme::Light);
        
        let text_styles: BTreeMap<_, _> = [
            (Heading, FontId::new(30.0, Proportional)),
            (Name("Heading2".into()), FontId::new(25.0, Proportional)),
            (Name("Context".into()), FontId::new(23.0, Proportional)),
            (Body, FontId::new(14.0, Proportional)),
            (Monospace, FontId::new(14.0, Proportional)),
            (Button, FontId::new(14.0, Proportional)),
            (Small, FontId::new(13.0, Proportional)),
        ].into();
        cc.egui_ctx.all_styles_mut(move |style| style.text_styles = text_styles.clone());
        
        Trac {
            db: Default::default(),
            form_spent: 0,
            form_comment: String::with_capacity(24),
        }
    }
}
impl<D: TunedDb> App for Trac<D> {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        TopBottomPanel::bottom("status_bar")
            .min_height(30.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label("Expense Explorer by House Ting | Debug Version");
                });
            });
        
        TopBottomPanel::bottom("track")
            .show(ctx, |ui| {
                let bigness = (self.form_spent as f32).ln_1p();  // 0.00 .. 11.52
                let drag_speed = 12.0 - bigness;
                
                ui.vertical_centered_justified(|ui| {
                    ui.spacing_mut().window_margin = Margin::same(12.0);
                    ui.spacing_mut().interact_size.y += 8.0;
                    ui.spacing_mut().item_spacing.y += 8.0;
                    
                    ui.add(widgets::DragValue::new(&mut self.form_spent)
                        .range(0..=100000)
                        .speed(drag_speed)
                        .prefix("Spent: "));
                    
                    ui.text_edit_multiline(&mut self.form_comment);
                    
                    if self.form_spent == 0 {ui.disable();}
                    if ui.button("Spent").clicked() {
                        self.db.insert_expense(crate::crosstyping::ClientData{
                            amount: self.form_spent,
                            group: None
                        });
                        self.form_spent = 0;
                    }
                });
            });
        
        // const MONTH = std::time::Duration::from_days(30);
        #[allow(non_snake_case)] // until `duration_constructors` stabilize
        let MONTH = std::time::Duration::from_secs(30 * 86400);
        
        let latest_meaning = self.db.gen_interval_last(MONTH);
        let latest_info = self.db.total_spending_last(latest_meaning, None);
        
        CentralPanel::default()
            .show(ctx, |ui| {
                ui.heading(format!("{} last month", latest_info.total_amount));
            });
    }
}

