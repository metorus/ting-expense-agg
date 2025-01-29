use egui::{*, FontFamily::Proportional, FontId};
use eframe::{App, CreationContext};

use std::collections::BTreeMap;
use std::ops::Deref;

use crate::db_client_view::{DbView, MayLoad};
use crate::ecs::expense_category_slider;
use crate::pie::pie_chart_with_legend;
use crate::crosstyping::ClientData;


const CATEGORIES: [(&'static str, Color32, Option<&'static str>); 5] = [
    ("🍞", Color32::GREEN,     Some("food")),
    ("🏡", Color32::DARK_GRAY, Some("supplies")),
    ("🚋", Color32::ORANGE,    Some("transport")),
    ("etc", Color32::GOLD,     None),
    ("📝", Color32::BLACK,     None),
];
fn color_cat(a: &str) -> Color32 {
    match a {
        "food"      => Color32::GREEN,
        "supplies"  => Color32::DARK_GRAY,
        "transport" => Color32::ORANGE,
        _           => Color32::GOLD,
    }
}


fn show_mayload(ui: &mut Ui, ml: MayLoad<'_>) {
    match ml {
        Ok(e)   => ui.monospace(e.to_string()),
        Err(()) => ui.monospace("------------------------------"),
    };
}


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


pub struct Trac<U> {
    db: DbView<U>,
    screen_buf: Vec<CurScreen>,
}
impl<U> Trac<U> where DbView<U>: Default {
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

impl<U> App for Trac<U> {
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
            Some(CurScreen::Stats) => {
                 let c = self.draw_stat_screen(ctx);
                 self.screen_buf.push(CurScreen::Stats);
                 c
            }
        };
        
        for c in commands {
            match c {
                UiCommands::Go(to) => {
                    self.screen_buf.push(to);
                },
                UiCommands::Back => {
                    assert!(self.screen_buf.len() > 1, "no screens to go back");
                    self.screen_buf.pop();
                }
            }
        }
    }
}

impl<U> Trac<U> {
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
            .frame(Frame::side_top_panel(&ctx.style())
                         .inner_margin(Margin::same(18.0)))
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
                            ui.text_edit_singleline(&mut form.spec_category);
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
                        
                        self.db.insert_expense(ClientData{
                            amount: form.spent,
                            group: c,
                            revoked: false,
                        });
                        
                        form.spent = 0;
                        form.comment.clear();
                        form.anim_category = 3.0;
                        form.chosen_category = 3;
                        form.spec_category.clear();
                    }
                });
            });
        
        let (latte, latc) = self.db.month_transactions_info();
        
        CentralPanel::default()
            .frame(Frame::side_top_panel(&ctx.style())
                         .inner_margin(Margin::symmetric(2.0, 30.0)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().item_spacing.y += 12.0;
                    ui.heading(format!("Spending amount this month: {latte}"));
                    if latc == 0 { return; }
                    
                    ui.label(format!("in {latc} purchases ({:.2} on average);",
                                     (latte as f32) / (latc as f32)));
                    if ui.button("Detailed statistics").clicked() {
                        cmds.push(UiCommands::Go(CurScreen::Stats));
                    }
                    ui.add_space(12.0);
                    
                    self.db.load_last_spendings(6)
                           .for_each(|ml| show_mayload(ui, ml));
                });
            });
        
        cmds
    }
    
    fn draw_stat_screen(&mut self, ctx: &Context) -> Vec<UiCommands> {
        let mut cmds = vec![];
        
        TopBottomPanel::bottom("status_bar")
            .min_height(48.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label("Expense Explorer by House Ting | Debug Version");
                });
            });
        
        
        CentralPanel::default()
            .frame(Frame::side_top_panel(&ctx.style())
                         .inner_margin(Margin::same(18.0)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().item_spacing.y += 12.0;
                    
                    if ui.button("Back").clicked() {
                        cmds.push(UiCommands::Back);
                    }
                    
                    // 1. displaying aggregate
                    
                    pie_chart_with_legend(
                        ui,
                        self.db.month_pie().into_iter()
                               .map(|(group, value)| {
                                   (group, *value as f32, color_cat(&group))
                               })
                    );
                    
                    // 2. displaying spendings
                    
                    let font = FontId::default();
                    let text_height = ui.fonts(|r| r.row_height(&font));
                    
                    /*
                    ScrollArea::vertical().show_rows(ui, text_height, b-a,
                        |ui, row_range| {
                            for row in row_range {
                                let expense = self.db.load(row);
                                ui.monospace(format!("{expense}"));
                            }
                        });
                    */
                });
            });
        
        cmds
    }
}

