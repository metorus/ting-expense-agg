// #[sides(client)]


use egui::{*, FontFamily::Proportional, FontId};
use eframe::{App, CreationContext};

use std::collections::BTreeMap;

use crate::crosstyping::{ClientData, Upstream};
use crate::db_slice::DbView;
use crate::widgets::*;


const CATEGORIES: [(&'static str, Color32, Option<&'static str>); 5] = [
    ("🍞", Color32::GREEN,     Some("еду")),
    ("🏡", Color32::DARK_GRAY, Some("хозтовары")),
    ("🚋", Color32::ORANGE,    Some("транспорт")),
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


struct MainForm {
    spent: u64,
    comment: String,
    anim_category: f32,
    chosen_category: usize,
    spec_category: String,
}
impl MainForm {
    fn to_default(&mut self) {
        self.spent = 0;
        self.comment.clear();
        self.anim_category = 3.0;
        self.chosen_category = 3;
        self.spec_category.clear();
    }
}
impl Default for MainForm {
    fn default() -> Self {
        MainForm {
            spent: 0,
            comment: String::with_capacity(24),
            anim_category: 3.0,
            chosen_category: 3,
            spec_category: String::with_capacity(12),
        }
    }
}


enum CurScreen {
    Main(MainForm),
    Stats,
}

enum UiCommands {
    Go(CurScreen),
    Back,
}


pub struct Trac<U: Upstream> {
    db: DbView<U>,
    screen_buf: Vec<CurScreen>,
}
impl<U: Upstream> Trac<U> {
    pub fn new(cc: &CreationContext<'_>, db: U) -> Self {
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
            db: DbView::with(db),
            screen_buf: vec![CurScreen::Main(MainForm::default())],
        }
    }
    
    fn draw_main_screen(&mut self, ctx: &Context, form: &mut MainForm) -> Vec<UiCommands> {
        let mut cmds = vec![];
        
        TopBottomPanel::bottom("status_bar")
            .min_height(48.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label("Обозреватель расходов TEA | Отладочная версия");
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
                        .prefix("Итого: "));
                    
                    expense_category_slider(&mut ui, &mut form.anim_category,
                        &mut form.chosen_category, &CATEGORIES);
                    
                    let write_in_cat = form.anim_category == 4.0;
                    CollapsingHeader::new("Другая категория")
                        .open(Some(write_in_cat))
                        .show(ui, |ui| {
                            ui.text_edit_singleline(&mut form.spec_category);
                        });
                    
                    ui.add(widgets::TextEdit::multiline(&mut form.comment)
                        .desired_rows(2)
                        .hint_text("Комментарий"));
                    
                    if form.spent == 0 {ui.disable();}
                    if write_in_cat && form.spec_category.is_empty() {
                        ui.disable();
                    }
                    
                    let spent = RichText::new("Записать").size(19.0)
                                         .strong().color(Color32::DARK_BLUE);
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
                        form.to_default();
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
                    ui.heading(format!("За месяц потрачено {latte}\u{20bd}"));
                    if latc == 0 { return; }
                    
                    ui.label(format!("в {latc} чеках (средний чек {:.2}\u{20bd});",
                                     (latte as f32) / (latc as f32)));
                    if ui.button("Подробная информация").clicked() {
                        cmds.push(UiCommands::Go(CurScreen::Stats));
                    }
                    ui.add_space(12.0);
                    
                    self.db.load_last_spendings(6).for_each(|ml| show_spending_mayload(ui, ml));
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
                    ui.label("Обозреватель расходов TEA | Отладочная версия");
                });
            });
        
        
        CentralPanel::default()
            .frame(Frame::side_top_panel(&ctx.style())
                         .inner_margin(Margin::same(18.0)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().item_spacing.y += 12.0;
                    
                    if ui.button("Назад").clicked() {
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
                    
                    ScrollArea::vertical().show_rows(ui, text_height,
                        self.db.total_live_transactions(),
                        |ui, range| {
                            self.db.load_some_spendings(range.start, range.end)
                                .for_each(|ml| show_spending_mayload(ui, ml));
                        });
                });
            });
        
        cmds
    }
}

impl<U: Upstream> App for Trac<U> {
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


#[cfg(not(target_arch = "wasm32"))]
pub fn run_app(db: impl Upstream) -> eframe::Result {
    let icon = include_bytes!("../assets/icon-32.png");
    
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Обозреватель расходов")
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
            Trac::new(cc, db)
        ))),
    )
}

#[cfg(target_arch = "wasm32")]
pub fn run_app(db: impl Upstream) {
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
                    Trac::new(cc, db)
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


