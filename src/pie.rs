use egui::*;
use std::f32::consts::TAU;

//----------------------------------------------------------------------------//

/// Creates an `egui` pie chart widget with a legend to the right.
///
/// Example data format:
/// ```
/// let data = vec![
///     ("Category A", 30.0, Color32::RED),
///     ("Category B", 50.0, Color32::BLUE),
///     ("Category C", 20.0, Color32::GREEN),
/// ];
/// ```
pub fn pie_chart_with_legend(ui: &mut Ui, data: &[(&str, f32, Color32)]) -> Response {
    let desired_pie_size = Vec2::splat(120.0);
    let legend_width = 160.0;
    let x_spacing = ui.spacing().item_spacing.x;
    let desired_size = vec2(desired_pie_size.x + legend_width + x_spacing,
                            desired_pie_size.y);

    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let pie_rect = Rect::from_min_size(rect.min, desired_pie_size);
        let legend_rect = Rect::from_min_size(
            Pos2::new(pie_rect.max.x + x_spacing, rect.min.y),
            Vec2::new(legend_width, desired_pie_size.y),
        );

        draw_pie_chart(&painter, pie_rect, data);
        draw_legend(&painter, legend_rect, data, ui);
    }

    response
}


fn draw_pie_chart(painter: &Painter, rect: Rect, data: &[(&str, f32, Color32)]) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0;

    let total_value: f32 = data.iter().map(|(_, value, _)| value).sum();
    if total_value <= 0.0 {
        return;  // Don't draw corrupt or absent data.
    }

    let mut start_angle = 0.0;
    
//----------------------------------------------------------------------------//
    // SECTION 1: Workaround for poor epaint tesselator functionality.
    let mut top_color = None;
    for (_, value, color) in data {
        if value / total_value > 0.5 {top_color = Some(color);}
    }
    if let Some(top_color) = top_color {  // Explained below (*).
        painter.circle(center, radius, *top_color,
                                       Stroke::new(2.0, Color32::WHITE));
    }
    
//----------------------------------------------------------------------------//
    // SECTION 2: [Most] pie slices.
    for (_, value, color) in data {
        let percentage = value / total_value;
        let delta_angle = TAU * percentage; // TAU is 2 * PI
        let end_angle = start_angle + delta_angle;

        /* Drawing pie slice (arc). It would be nice if the following method
        //   were available, but it is not.
        painter.arc(
            center,
            radius,
            start_angle,
            end_angle,
            color,
            Stroke::new(2.0, Color32::WHITE),
        );
        */
        
        // This means we shall list the points by hand.
        // We can only use convex polygons but there might be one concave,
        //   if the percentage is more than 0.5. (*) We drew it as the whole
        //   circle before anything else.
        if percentage > 0.5 {
            start_angle = end_angle; continue;
        }
        
        let mut points = Vec::with_capacity(32);
        points.push(center);
        let num_segments = 30;  // Approximation.
        let step = delta_angle / num_segments as f32;
        for i in 0..=num_segments {
            let angle = start_angle + step * i as f32;
            points.push(center + Vec2::angled(angle) * radius);
        }
        points.push(center);

        // Draw the prepared pie slice
        painter.add(Shape::convex_polygon(
            points, *color, Stroke::new(2.0, Color32::WHITE)
        ));

        start_angle = end_angle;
    }
}


fn draw_legend(painter: &Painter, rect: Rect, data: &[(&str, f32, Color32)], ui: &Ui) {
    let text_height = ui.fonts(|r| r.row_height(&FontId::default()));
    let color_box_size = Vec2::splat(text_height * 0.8);
    let x_spacing = ui.spacing().item_spacing.x;
    let y_spacing = 4.0; // ui.spacing().item_spacing.y;
    let text_color = ui.style().visuals.text_color();

    let mut current_pos = Pos2::new(rect.min.x + x_spacing, rect.min.y + y_spacing);

    for (label, _, color) in data {
        // Color box showing how we displayed that category,
        let color_rect = Rect::from_min_size(current_pos, color_box_size);
        painter.rect_filled(color_rect, Rounding::same(0.0), *color);

        // and the category label.
        let text_pos = Pos2::new(
            color_rect.max.x + x_spacing,
            current_pos.y + color_box_size.y / 2.0, // Vertically align text with color box center
        );
        painter.text(
            text_pos,
            Align2::LEFT_CENTER,
            label,
            FontId::default(),
            text_color,
        );

        current_pos.y += text_height + y_spacing;
        if current_pos.y + text_height > rect.max.y {
            // Basic vertical overflow handling - stop drawing legend items
            break;
        }
    }
}

