use eframe::egui::{self, Color32, Pos2};

pub fn set_theme(ctx: &egui::Context, name: &str) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::unique("nmessenger-theme"), name.to_owned()));
}

fn top_color(ctx: &egui::Context) -> Color32 {
    let theme = ctx
        .data(|data| data.get_temp::<String>(egui::Id::unique("nmessenger-theme")))
        .unwrap_or_default();
    match theme.to_lowercase().as_str() {
        "verde" => Color32::from_rgb(200, 230, 201),
        "rosa" | "lilas" => Color32::from_rgb(252, 213, 206),
        "prata" => Color32::from_rgb(229, 229, 234),
        "purple" | "roxo" => Color32::from_rgb(193, 170, 231),
        "orange" | "laranja" => Color32::from_rgb(255, 199, 151),
        _ => Color32::from_rgb(194, 221, 244),
    }
}

fn bottom_color(ctx: &egui::Context) -> Color32 {
    let theme = ctx
        .data(|data| data.get_temp::<String>(egui::Id::unique("nmessenger-theme")))
        .unwrap_or_default();
    match theme.to_lowercase().as_str() {
        "verde" => Color32::from_rgb(238, 249, 240),
        "rosa" | "lilas" => Color32::from_rgb(254, 242, 244),
        "prata" => Color32::from_rgb(243, 244, 247),
        "purple" | "roxo" => Color32::from_rgb(242, 232, 255),
        "orange" | "laranja" => Color32::from_rgb(255, 241, 225),
        _ => Color32::from_rgb(239, 248, 250),
    }
}

pub(super) fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

/// Rounded vertical gradient using the reference's 0/15/89/100% stops.
pub fn background(ctx: &egui::Context) {
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let height = rect.height().max(1.0);
    let is_maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
    let radius = if crate::platform::is_mobile()
        || super::super::window::system_titlebars(ctx)
        || is_maximized
    {
        0.0
    } else {
        11.0_f32
    };
    let top = top_color(ctx);
    let bottom = bottom_color(ctx);
    let color = |y: f32| {
        let progress = y / height;
        if progress < 0.15 {
            mix(top, Color32::WHITE, progress / 0.15)
        } else if progress > 0.89 {
            mix(Color32::WHITE, bottom, (progress - 0.89) / 0.11)
        } else {
            Color32::WHITE
        }
    };
    let inset = |y: f32| {
        let distance = if y < radius {
            radius - y
        } else if y > height - radius {
            y - (height - radius)
        } else {
            0.0
        };
        radius - (radius * radius - distance * distance).max(0.0).sqrt()
    };
    let mut mesh = egui::Mesh::default();
    for row in 0..height.ceil() as usize {
        let y0 = row as f32;
        let y1 = (y0 + 1.0).min(height);
        let start = mesh.vertices.len() as u32;
        mesh.colored_vertex(
            Pos2::new(rect.left() + inset(y0), rect.top() + y0),
            color(y0),
        );
        mesh.colored_vertex(
            Pos2::new(rect.right() - inset(y0), rect.top() + y0),
            color(y0),
        );
        mesh.colored_vertex(
            Pos2::new(rect.left() + inset(y1), rect.top() + y1),
            color(y1),
        );
        mesh.colored_vertex(
            Pos2::new(rect.right() - inset(y1), rect.top() + y1),
            color(y1),
        );
        mesh.add_triangle(start, start + 1, start + 2);
        mesh.add_triangle(start + 1, start + 3, start + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
    super::super::glass::rim(&painter, rect, radius as u8);
}
