//! Light Aero-style depth: soft outer shadow, glass rim and top gloss.
use eframe::egui::{self, Color32, Pos2, Stroke, StrokeKind};

pub fn rim(painter: &egui::Painter, rect: egui::Rect, radius: u8) {
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, Color32::from_black_alpha(48)),
        StrokeKind::Inside,
    );
    if radius > 0 {
        painter.rect_stroke(
            rect.shrink(1.0),
            radius.saturating_sub(1),
            Stroke::new(1.0_f32, Color32::from_white_alpha(180)),
            StrokeKind::Inside,
        );
    }
}

/// A subtle highlight inside the top of a card, painted before its contents.
pub fn gloss_line(ui: &egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    if rect.width() < 8.0 {
        return;
    }
    let left = rect.left() + 1.0;
    let right = rect.right() - 1.0;
    let y = rect.top();
    ui.painter().line_segment(
        [Pos2::new(left, y), Pos2::new(right, y)],
        Stroke::new(1.0_f32, Color32::from_white_alpha(205)),
    );
    ui.painter().line_segment(
        [Pos2::new(left, y + 1.0), Pos2::new(right, y + 1.0)],
        Stroke::new(1.0_f32, Color32::from_black_alpha(9)),
    );
}
