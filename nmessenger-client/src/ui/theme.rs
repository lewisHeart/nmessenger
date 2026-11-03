use crate::app::MessengerApp;
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use nm_protocol::{Presence, Profile};

pub const INK: Color32 = super::nmessenger::INK;
pub const MUTED: Color32 = super::nmessenger::MUTED;
pub const BLUE: Color32 = super::nmessenger::BLUE;
pub const SKY: Color32 = Color32::from_rgb(205, 227, 246);
pub const GREEN: Color32 = Color32::from_rgb(74, 163, 51);

pub fn install(ctx: &egui::Context) {
    super::assets::install_fonts(ctx);
    ctx.options_mut(|options| {
        options.input_options.surrender_focus_on = egui::SurrenderFocusOn::Never;
    });
    let mut style = egui::Style::default();
    style.interaction.selectable_labels = false;
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = Color32::TRANSPARENT;
    style.visuals.window_fill = Color32::from_rgb(250, 253, 255);
    style.visuals.override_text_color = None;
    style.visuals.widgets.noninteractive.fg_stroke.color = INK;
    style.visuals.hyperlink_color = BLUE;
    style.visuals.selection.bg_fill = Color32::from_rgb(205, 227, 246);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(166, 185, 205));
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(250, 252, 255);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(195, 212, 230));
    style.visuals.widgets.inactive.expansion = 0.0;

    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(232, 244, 255);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(238, 247, 255);
    style.visuals.widgets.hovered.bg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(120, 180, 240));
    style.visuals.widgets.hovered.expansion = 0.5;

    // Active / Onholding state: distinct tactile emphasis when pressed and held down!
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(168, 212, 252);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(185, 222, 252);
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.2_f32, Color32::from_rgb(40, 115, 205));
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0, 35, 115));
    style.visuals.widgets.active.expansion = -0.6;
    style.visuals.window_corner_radius = egui::CornerRadius::same(8);
    for widgets in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.noninteractive,
    ] {
        widgets.corner_radius = egui::CornerRadius::same(4);
    }
    style.spacing.item_spacing = Vec2::new(5.0, 4.0);
    style.spacing.button_padding = Vec2::new(7.0, 4.0);
    style.spacing.interact_size = Vec2::new(24.0, 22.0);
    style.spacing.icon_width = 12.0;
    style.spacing.window_margin = egui::Margin::same(12);
    let is_mobile = crate::platform::is_mobile();
    style.spacing.scroll = egui::style::ScrollStyle::floating();
    style.spacing.scroll.bar_width = 7.0;
    style.spacing.scroll.floating_width = 4.0;
    style.spacing.scroll.floating_allocated_width = 8.0;
    style.spacing.scroll.handle_min_length = 26.0;
    style.spacing.scroll.bar_inner_margin = 2.0;
    style.spacing.scroll.bar_outer_margin = 2.0;
    style.spacing.scroll.foreground_color = true;
    style.spacing.scroll.dormant_background_opacity = 0.0;
    style.spacing.scroll.dormant_handle_opacity = if is_mobile { 0.35 } else { 0.0 };
    style.spacing.scroll.active_background_opacity = 0.15;
    style.spacing.scroll.active_handle_opacity = 0.55;
    style.spacing.scroll.interact_background_opacity = 0.35;
    style.spacing.scroll.interact_handle_opacity = 0.85;
    style.animation_time = 0.16;
    for (kind, size) in [
        (egui::TextStyle::Body, 12.0),
        (egui::TextStyle::Button, 11.0),
        (egui::TextStyle::Heading, 16.0),
        (egui::TextStyle::Small, 10.0),
    ] {
        style
            .text_styles
            .insert(kind, egui::FontId::proportional(size));
    }
    ctx.all_styles_mut(move |target| *target = style.clone());
}

pub fn gradient(painter: &egui::Painter, rect: egui::Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(egui::Shape::mesh(mesh));
}

pub fn avatar(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    profile: &Profile,
    size: f32,
) -> egui::Response {
    super::nmessenger::avatar(app, ui, profile, size)
}

pub fn primary_button_with_icon(
    ui: &mut egui::Ui,
    icon: Option<egui::load::SizedTexture>,
    text: RichText,
    min_size: Vec2,
) -> egui::Response {
    ui.scope(|ui| {
        ui.style_mut().visuals.widgets.inactive.bg_fill = SKY;
        ui.style_mut().visuals.widgets.inactive.bg_stroke =
            Stroke::new(1.0, Color32::from_rgb(160, 200, 235));
        ui.style_mut().visuals.widgets.hovered.bg_fill = Color32::from_rgb(185, 222, 252);
        ui.style_mut().visuals.widgets.hovered.bg_stroke =
            Stroke::new(1.0, Color32::from_rgb(105, 175, 240));
        ui.style_mut().visuals.widgets.hovered.expansion = 0.5;
        // Onholding / active state
        ui.style_mut().visuals.widgets.active.bg_fill = Color32::from_rgb(140, 195, 245);
        ui.style_mut().visuals.widgets.active.bg_stroke =
            Stroke::new(1.2, Color32::from_rgb(30, 105, 195));
        ui.style_mut().visuals.widgets.active.fg_stroke =
            Stroke::new(1.0, Color32::from_rgb(0, 25, 100));
        ui.style_mut().visuals.widgets.active.expansion = -0.6;

        let btn = if let Some(st) = icon {
            egui::Button::image_and_text(st, text)
        } else {
            egui::Button::new(text)
        };
        ui.add(btn.corner_radius(4).min_size(min_size))
    })
    .inner
}

pub fn primary_button(ui: &mut egui::Ui, text: RichText, min_size: Vec2) -> egui::Response {
    primary_button_with_icon(ui, None, text, min_size)
}

pub fn danger_button(ui: &mut egui::Ui, text: RichText, min_size: Vec2) -> egui::Response {
    ui.scope(|ui| {
        ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::from_rgb(255, 245, 245);
        ui.style_mut().visuals.widgets.inactive.bg_stroke =
            Stroke::new(1.0, Color32::from_rgb(240, 210, 210));
        ui.style_mut().visuals.widgets.hovered.bg_fill = Color32::from_rgb(254, 230, 230);
        ui.style_mut().visuals.widgets.hovered.bg_stroke =
            Stroke::new(1.0, Color32::from_rgb(225, 140, 140));
        ui.style_mut().visuals.widgets.hovered.expansion = 0.5;
        // Onholding / active state
        ui.style_mut().visuals.widgets.active.bg_fill = Color32::from_rgb(245, 185, 185);
        ui.style_mut().visuals.widgets.active.bg_stroke =
            Stroke::new(1.2, Color32::from_rgb(195, 50, 50));
        ui.style_mut().visuals.widgets.active.fg_stroke =
            Stroke::new(1.0, Color32::from_rgb(140, 15, 15));
        ui.style_mut().visuals.widgets.active.expansion = -0.6;

        ui.add(egui::Button::new(text).corner_radius(4).min_size(min_size))
    })
    .inner
}

pub fn primary(ui: &mut egui::Ui, label: &str) -> egui::Response {
    primary_button(
        ui,
        RichText::new(label)
            .color(Color32::from_rgb(1, 45, 147))
            .strong(),
        Vec2::new(84.0, 27.0),
    )
}

#[allow(dead_code)]
pub fn status_icon(ui: &mut egui::Ui, presence: Presence, size: f32) {
    super::nmessenger::image(
        ui,
        &format!(
            "status/{}_icone.png",
            super::nmessenger::status_name(presence)
        ),
        Vec2::splat(size),
    );
}

pub fn status_dot(ui: &mut egui::Ui, presence: Presence) {
    let status_color = match presence {
        Presence::Available => Color32::from_rgb(52, 168, 83),
        Presence::Busy => Color32::from_rgb(234, 67, 53),
        Presence::Away => Color32::from_rgb(251, 188, 4),
        _ => Color32::from_rgb(140, 145, 150),
    };
    let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 12.0), egui::Sense::hover());
    ui.painter().circle_filled(dot_rect.center(), 3.0, status_color);
}

pub fn close_button(ui: &mut egui::Ui, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, 3, Color32::from_rgb(205, 30, 40));
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 3, Color32::from_rgb(232, 17, 35));
    }
    let stroke_color = if resp.hovered() || resp.is_pointer_button_down_on() {
        Color32::WHITE
    } else {
        MUTED
    };
    let pad = (size * 0.28).round();
    let p = ui.painter();
    p.line_segment(
        [rect.min + Vec2::splat(pad), rect.max - Vec2::splat(pad)],
        Stroke::new(1.3, stroke_color),
    );
    p.line_segment(
        [
            egui::pos2(rect.max.x - pad, rect.min.y + pad),
            egui::pos2(rect.min.x + pad, rect.max.y - pad),
        ],
        Stroke::new(1.3, stroke_color),
    );
    resp
}

pub use super::nmessenger::WindowControls;

#[cfg(any(not(target_os = "android"), test))]
pub fn window_bar(ui: &mut egui::Ui, title: &str) {
    super::nmessenger::window_bar(ui, title);
}

pub fn window_bar_with_controls(ui: &mut egui::Ui, title: &str, controls: WindowControls) {
    super::nmessenger::window_bar_with_controls(ui, title, controls);
}
