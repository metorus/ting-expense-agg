use egui::{*, FontFamily::Proportional, FontId};
use eframe::{App, CreationContext};

use std::collections::BTreeMap;

use crate::crosstyping::TunedDb;



pub struct Trac<D: TunedDb> {
    db: D,
    
    form_spent: u64,
    form_comment: String,
}
impl<D: TunedDb + Default> Trac<D> {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(Theme::Light);
        
        use egui::TextStyle::*;
        let text_styles: BTreeMap<_, _> = [
            (Heading, FontId::new(30.0, Proportional)),
            (Name("Heading2".into()), FontId::new(25.0, Proportional)),
            (Name("Context".into()), FontId::new(23.0, Proportional)),
            (Body, FontId::new(16.0, Proportional)),
            (Monospace, FontId::new(16.0, Proportional)),
            (Button, FontId::new(16.0, Proportional)),
            (Small, FontId::new(15.0, Proportional)),
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
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::bottom("status_bar")
            .min_height(48.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label("Expense Explorer by House Ting | Debug Version");
                });
            });
        
        TopBottomPanel::bottom("track")
            .frame(Frame::side_top_panel(&ctx.style()).inner_margin(Margin::same(18.0)))
            .show(ctx, |ui| {
                let bigness = (self.form_spent as f32).ln_1p();  // 0.00 .. 11.52
                let drag_speed = 12.0 - bigness;
                
                ui.vertical_centered_justified(|ui| {
                    ui.spacing_mut().interact_size.y += 12.0;
                    ui.spacing_mut().item_spacing.y += 12.0;
                    
                    ui.add(widgets::DragValue::new(&mut self.form_spent)
                        .range(0..=100000)
                        .speed(drag_speed)
                        .prefix("Spent: "));
                    
                    // TODO: smart expense-category slider.
                    
                    ui.add(widgets::TextEdit::multiline(&mut self.form_comment)
                        .desired_rows(2)
                        .hint_text("Comment"));
                    
                    if self.form_spent == 0 {ui.disable();}
                    
                    let spent = RichText::new("Spent").size(19.0).strong().color(Color32::DARK_BLUE);
                    let spent = Button::new(spent).fill(Color32::LIGHT_BLUE);
                    if ui.add(spent).clicked() {
                        self.db.insert_expense(crate::crosstyping::ClientData{
                            amount: self.form_spent,
                            group: None
                        });
                        self.form_spent = 0;
                        self.form_comment.clear();
                    }
                });
            });
        
        // const MONTH = std::time::Duration::from_days(30);
        #[allow(non_snake_case)] // until `duration_constructors` stabilize
        let MONTH = std::time::Duration::from_secs(30 * 86400);
        
        let latest_meaning = self.db.gen_interval_last(MONTH);
        let latest_info = self.db.total_spending_last(latest_meaning, None);
        let latte = latest_info.total_amount;
        
        CentralPanel::default()
            .frame(Frame::side_top_panel(&ctx.style()).inner_margin(Margin::same(30.0)))
            .show(ctx, |ui| {
                ui.vertical_centered_justified(|ui| {
                    ui.spacing_mut().item_spacing.y += 12.0;
                    ui.heading(format!("Spending amount this month: {latte}"));
                });
            });
    }
}

