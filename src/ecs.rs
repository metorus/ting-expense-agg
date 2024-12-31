use egui::*;

//----------------------------------------------------------------------------//

// ( )----(.)-*--( )----( )----( )
// Food   House  Tran.  Stuff  sel

// grey point '.' stands for held option
// all labels are colored

const NO_STROKE: (f32, Color32) = (0.0, Color32::PLACEHOLDER);
const WHITE: Color32 = Color32::WHITE;

pub fn expense_category_slider(ui: &mut Ui, pos: &mut f32, held: &mut usize,
        options: &[(&str, Color32)]) -> Response {
    
    let n = options.len();
    let nf = n as f32;
    
    let mark_diam = ui.spacing().interact_size.y / 2.0;
    let slot_diam = mark_diam * 1.9;
    
    let w = ui.available_width();
//----------------------------------------------------------------------------//
    // SECTION 1: Slider.
    let sl_size = vec2(w, mark_diam);
    let (slider_rect, mut re) = ui.allocate_exact_size(sl_size,
        Sense::click_and_drag());
    
    let segment = (w - nf * slot_diam) / (nf - 1.0);
    let offset = slot_diam + segment;
    
    // Utility function to check where a slider marker/slot should be drawn.
    let point_at = |i: f32| -> Pos2 {
        let x = slider_rect.left() + offset * i + slot_diam / 2.0;
        pos2(x, slider_rect.center().y)
    };
    
    // Drawing slots.
    for i in (0..n).map(|x| x as f32) {
        ui.painter().circle(point_at(i), slot_diam / 2.0, WHITE, NO_STROKE);
    }
    
    // Drawing slider base line.
    let (first_pos, second_pos) = (point_at(0.0), point_at(nf - 1.0));
    ui.painter().line(vec![first_pos, second_pos], (mark_diam, WHITE));
    
//----------------------------------------------------------------------------//
    // SECTION 1.5: Moving parts of slider.
    if re.drag_stopped() {
        *pos = *held as f32;  // need to snap
    } else if re.clicked() {
        if let Some(p) = re.ctx.input(|i| i.pointer.latest_pos()) {
            *pos = remap_clamp(p.x, slider_rect.x_range(), 0.0..=(nf-1.0))
                .round();
            let want = *pos as usize;
            if want != *held {
                *held = want; re.mark_changed();
            }
        }
    } else if re.is_pointer_button_down_on() {
        if let Some(p) = re.ctx.input(|i| i.pointer.latest_pos()) {
            *pos = remap_clamp(p.x, slider_rect.x_range(), 0.0..=(nf-1.0));
            let mut want = *held;
            
            if pos.fract() < 0.12 {
                *pos = pos.floor(); want = *pos as usize;
            } else if pos.fract() > 1. - 0.12 {
                *pos = pos.ceil();  want = *pos as usize;
            }
            
            if want != *held {
                *held = want; re.mark_changed();
            }
        }
    }
    
    if pos.fract() == 0.0 {
        // Drawing colored point.
        let pos = point_at(*pos);
        ui.painter().circle(pos, mark_diam / 2.0, options[*held].1, NO_STROKE);
    } else {
        let pos_held = point_at(*held as f32);
        let pos_move = point_at(*pos);
        
        // Drawing slightly-colored and gray points.
        ui.painter().circle(
            pos_held, mark_diam / 2.0,
            options[*held].1.gamma_multiply(0.2), NO_STROKE
        );
        ui.painter().circle(
            pos_move, mark_diam / 2.0,
            Color32::LIGHT_GRAY, (1.0, Color32::DARK_GRAY)
        );
    }
    
    re
}
//----------------------------------------------------------------------------//

/*
fn toggle_ui_compact(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, "")
    });

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let visuals = ui.style().interact_selectable(&response, *on);
        let rect = rect.expand(visuals.expansion);
        let radius = 0.5 * rect.height();
        ui.painter()
            .rect(rect, radius, visuals.bg_fill, visuals.bg_stroke);
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter()
            .circle(center, 0.75 * radius, visuals.bg_fill, visuals.fg_stroke);
    }

    response
}
*/

