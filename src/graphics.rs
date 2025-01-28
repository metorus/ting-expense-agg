use egui::{*, FontFamily::Proportional, FontId};
use eframe::{App, CreationContext};

use std::collections::BTreeMap;

use crate::ecs::expense_category_slider;
use crate::crosstyping::TunedDb;


const CATEGORIES: [(&'static str, Color32, Option<&'static str>); 5] = [
    ("🍞", Color32::GREEN,     Some("food")),
    ("🏡", Color32::DARK_GRAY, Some("supplies")),
    ("🚋", Color32::ORANGE,    Some("transport")),
    ("etc", Color32::GOLD,     None),
    ("📝", Color32::BLACK,     None),
];


struct MainForm {
    spent: u64,
    comment: String,
    anim_category: f32,
    chosen_category: usize,
    spec_category: String,
}
enum CurScreen {
    Main(MainForm),
    Stats,
}

enum UiCommands {
    Go(CurScreen),
    Back,
}


pub struct Trac<D: TunedDb> {
    db: D,
    screen_buf: Vec<CurScreen>,
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
            (Monospace, FontId::new(14.0, FontFamily::Monospace)),
            (Button, FontId::new(16.0, Proportional)),
            (Small, FontId::new(15.0, Proportional)),
        ].into();
        cc.egui_ctx.all_styles_mut(move |style| style.text_styles = text_styles.clone());
        
        Trac {
            db: Default::default(),
            screen_buf: vec![CurScreen::Main(MainForm{
                spent: 0,
                comment: String::with_capacity(24),
                anim_category: 3.0,
                chosen_category: 3,
                spec_category: String::with_capacity(12),
            })],
        }
    }
}
impl<D: TunedDb> App for Trac<D> where
        std::ops::Range<<D as TunedDb>::Er>: DoubleEndedIterator<Item=<D as TunedDb>::Er> {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // We can't enable double mutability on last-screen place,
        //     but we need &mut App (or to reference all OK fields separately),
        //     so we have to pop that screen out of buffer.
        let commands = match self.screen_buf.pop() {
            None => unreachable!(),
            Some(CurScreen::Main(mut form)) => {
                 let c = self.draw_main_screen(ctx, &mut form);
                 self.screen_buf.push(CurScreen::Main(form));
                 c
            }
            Some(CurScreen::Stats) => todo!(),
        };
        
        for c in commands {
            match c {
                UiCommands::Go(to) => {
                    self.screen_buf.push(to);
                },
                UiCommands::Back => {
                    assert!(self.screen_buf.len() > 1, "cannot go back from main screen");
                    self.screen_buf.pop();
                }
            }
        }
    }
}

impl<D: TunedDb> Trac<D> where
        std::ops::Range<<D as TunedDb>::Er>: DoubleEndedIterator<Item=<D as TunedDb>::Er> {
    
    fn draw_main_screen(&mut self, ctx: &Context, form: &mut MainForm) -> Vec<UiCommands> {
        let mut cmds = vec![];
        
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
                let bigness = (form.spent as f32).ln_1p();  // 0.00 .. 11.52
                let drag_speed = 12.0 - bigness;
                
                ui.vertical_centered_justified(|mut ui| {
                    ui.spacing_mut().interact_size.y += 12.0;
                    ui.spacing_mut().item_spacing.y += 12.0;
                    
                    ui.add(widgets::DragValue::new(&mut form.spent)
                        .range(0..=100000)
                        .speed(drag_speed)
                        .prefix("Spent: "));
                    
                    expense_category_slider(&mut ui, &mut form.anim_category,
                        &mut form.chosen_category, &CATEGORIES);
                    
                    let write_in_cat = form.anim_category == 4.0;
                    CollapsingHeader::new("Specific category")
                        .open(Some(write_in_cat))
                        .show(ui, |ui| {
                            ui.add_space(3.0);
                            ui.text_edit_singleline(&mut form.spec_category);
                            ui.add_space(1.0);
                        });
                    
                    ui.add(widgets::TextEdit::multiline(&mut form.comment)
                        .desired_rows(2)
                        .hint_text("Comment"));
                    
                    if form.spent == 0 {ui.disable();}
                    if write_in_cat && form.spec_category.is_empty() {ui.disable();}
                    
                    let spent = RichText::new("Spent").size(19.0).strong().color(Color32::DARK_BLUE);
                    let spent = Button::new(spent).fill(Color32::LIGHT_BLUE);
                    if ui.add(spent).clicked() {
                        let c = if form.chosen_category == 4 {
                            Some(std::mem::take(&mut form.spec_category).into())
                        } else {
                            CATEGORIES[form.chosen_category].2.map(|s| s.into())
                        };
                        
                        self.db.insert_expense(crate::crosstyping::ClientData{
                            amount: form.spent,
                            group: c
                        });
                        
                        form.spent = 0;
                        form.comment.clear();
                        form.anim_category = 3.0;
                        form.chosen_category = 3;
                        form.spec_category.clear();
                    }
                });
            });
        
        let latest_meaning = self.db.gen_interval_last(crate::crosstyping::MONTH_LIKE);
        let latest_info = self.db.aggregate(latest_meaning, None);
        let latte = latest_info.total_amount;
        let latc = latest_info.count;
        let (a, b) = latest_info.bound;
        
        CentralPanel::default()
            .frame(Frame::side_top_panel(&ctx.style()).inner_margin(Margin::symmetric(2.0, 30.0)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().item_spacing.y += 12.0;
                    ui.heading(format!("Spending amount this month: {latte}"));
                    if latc != 0 {
                        ui.label(format!("in {latc} purchases ({:.2} on average);",
                            (latte as f32) / (latc as f32)));
                        if ui.button("Detailed statistics").clicked() {
                            cmds.push(UiCommands::Go(CurScreen::Stats));
                        }
                    }
                    ui.add_space(12.0);
                    
                    for i in (a..b).rev().take(6) {
                        let expense = self.db.load(i);
                        ui.monospace(format!("{expense}"));
                    }
                });
            });
        
        cmds
    }
}

