use eframe::egui::{self, Color32};

pub struct Scroll(egui::ScrollArea);
pub fn vertical() -> Scroll {
    Scroll(
        egui::ScrollArea::vertical()
            .scroll_source(egui::scroll_area::ScrollSource {
                scroll_bar: true,
                mouse_wheel: true,
                drag: if crate::platform::is_mobile() {
                    egui::scroll_area::DragScroll::Always
                } else {
                    egui::scroll_area::DragScroll::OnTouch
                },
            })
    )
}
pub fn horizontal() -> Scroll {
    Scroll(
        egui::ScrollArea::horizontal()
            .scroll_source(egui::scroll_area::ScrollSource {
                scroll_bar: true,
                mouse_wheel: true,
                drag: if crate::platform::is_mobile() {
                    egui::scroll_area::DragScroll::Always
                } else {
                    egui::scroll_area::DragScroll::OnTouch
                },
            })
    )
}

impl Scroll {
    pub fn id_salt(mut self, salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.0 = self.0.id_salt(salt);
        self
    }
    pub fn max_height(mut self, height: f32) -> Self {
        self.0 = self.0.max_height(height);
        self
    }
    pub fn auto_shrink(mut self, shrink: [bool; 2]) -> Self {
        self.0 = self.0.auto_shrink(shrink);
        self
    }
    pub fn stick_to_bottom(mut self, stick: bool) -> Self {
        self.0 = self.0.stick_to_bottom(stick);
        self
    }
    pub fn show_rows<R>(
        self,
        ui: &mut egui::Ui,
        row_height: f32,
        total_rows: usize,
        contents: impl FnOnce(&mut egui::Ui, std::ops::Range<usize>) -> R,
    ) -> egui::scroll_area::ScrollAreaOutput<R> {
        let original = ui.style().clone();
        ui.scope(|ui| {
            let visuals = &mut ui.style_mut().visuals;
            visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(173, 191, 207);
            visuals.widgets.hovered.fg_stroke.color = Color32::from_rgb(127, 158, 185);
            visuals.widgets.active.fg_stroke.color = Color32::from_rgb(91, 137, 177);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(242, 247, 251);
            self.0.show_rows(ui, row_height, total_rows, |ui, range| {
                ui.set_style(original);
                contents(ui, range)
            })
        })
        .inner
    }
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::scroll_area::ScrollAreaOutput<R> {
        let original = ui.style().clone();
        ui.scope(|ui| {
            let visuals = &mut ui.style_mut().visuals;
            visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(173, 191, 207);
            visuals.widgets.hovered.fg_stroke.color = Color32::from_rgb(127, 158, 185);
            visuals.widgets.active.fg_stroke.color = Color32::from_rgb(91, 137, 177);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(242, 247, 251);
            self.0.show(ui, |ui| {
                ui.set_style(original);
                contents(ui)
            })
        })
        .inner
    }
}
