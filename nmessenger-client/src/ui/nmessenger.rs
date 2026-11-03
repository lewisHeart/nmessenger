//! NMessenger native skin and shared interface primitives.
use crate::{
    app::MessengerApp,
    i18n::tr,
    media::{images::ImageSource, scene::Scene},
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, Stroke, StrokeKind, Vec2};
use nm_protocol::{Presence, Profile};

pub(crate) use crate::resources::{AVATAR_CATALOG, EMOJI_CATALOG};
#[cfg(not(target_os = "android"))]
pub const MAIN_SIZE: [f32; 2] = [330.0, 580.0];
#[cfg(not(target_os = "android"))]
pub const CHAT_SIZE: [f32; 2] = [886.0, 735.0];
pub const INK: Color32 = Color32::from_rgb(13, 24, 37);
pub const BLUE: Color32 = Color32::from_rgb(46, 131, 237);
pub const HEADING: Color32 = Color32::from_rgb(45, 81, 122);
pub const MUTED: Color32 = Color32::from_rgb(165, 165, 165);
pub const BORDER: Color32 = Color32::from_rgb(209, 209, 209);

mod background;
use background::mix;
pub use background::{background, set_theme};

#[cfg(not(target_os = "android"))]
pub fn app_icon() -> std::sync::Arc<egui::IconData> {
    static ICON: std::sync::OnceLock<std::sync::Arc<egui::IconData>> = std::sync::OnceLock::new();
    ICON.get_or_init(|| {
        let bytes = include_bytes!("../../assets/nmessenger/icon.svg");
        let opt = resvg::usvg::Options::default();
        if let Ok(tree) = resvg::usvg::Tree::from_data(bytes, &opt) {
            let target_size = 128.0_f32;
            let size = tree.size();
            let scale = (target_size / size.width()).min(target_size / size.height());
            let width = (size.width() * scale).round() as u32;
            let height = (size.height() * scale).round() as u32;
            if let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(width.max(1), height.max(1)) {
                let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
                resvg::render(&tree, transform, &mut pixmap.as_mut());
                let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                for pixel in pixmap.pixels() {
                    let a = pixel.alpha();
                    let (r, g, b) = if a == 0 {
                        (0, 0, 0)
                    } else {
                        let a_f = a as f32 / 255.0;
                        (
                            ((pixel.red() as f32 / a_f).round() as u8).min(255),
                            ((pixel.green() as f32 / a_f).round() as u8).min(255),
                            ((pixel.blue() as f32 / a_f).round() as u8).min(255),
                        )
                    };
                    rgba.extend_from_slice(&[r, g, b, a]);
                }
                return std::sync::Arc::new(egui::IconData {
                    rgba,
                    width,
                    height,
                });
            }
        }
        std::sync::Arc::new(egui::IconData {
            rgba: vec![0; 64 * 64 * 4],
            width: 64,
            height: 64,
        })
    })
    .clone()
}

pub fn decode_image_bytes(bytes: &[u8]) -> Option<egui::ColorImage> {
    if bytes.starts_with(b"<svg") || bytes.windows(4).any(|w| w == b"<svg") {
        let opt = resvg::usvg::Options::default();
        let tree = resvg::usvg::Tree::from_data(bytes, &opt).ok()?;
        let size = tree.size();
        let target_size = 256.0_f32;
        let scale = (target_size / size.width()).min(target_size / size.height()).min(4.0).max(0.1);
        let width = (size.width() * scale).round() as u32;
        let height = (size.height() * scale).round() as u32;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width.max(1), height.max(1))?;
        let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        Some(egui::ColorImage::from_rgba_unmultiplied(
            [pixmap.width() as usize, pixmap.height() as usize],
            pixmap.data(),
        ))
    } else {
        let image = image::load_from_memory(bytes).ok()?.to_rgba8();
        Some(egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        ))
    }
}

pub fn texture(ctx: &egui::Context, name: &str) -> Option<egui::TextureHandle> {
    let key = egui::Id::unique(("nmessenger-resource", name));
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(key)) {
        return Some(texture);
    }
    let pixels = decode_image_bytes(crate::resources::bytes(name)?)?;
    let texture = ctx.load_texture(name.to_owned(), pixels, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(key, texture.clone()));
    Some(texture)
}

pub fn badge_texture(ctx: &egui::Context, slug: &str, icon: &str) -> Option<egui::TextureHandle> {
    if !icon.is_empty() && (icon.contains('/') || icon.ends_with(".svg") || icon.ends_with(".png")) {
        if let Some(tex) = texture(ctx, icon) {
            return Some(tex);
        }
    }
    if let Some(tex) = texture(ctx, &format!("badges/{slug}.svg")) {
        return Some(tex);
    }
    if let Some(tex) = texture(ctx, &format!("badges/{slug}.png")) {
        return Some(tex);
    }
    None
}

pub fn paint(ctx: &egui::Context, painter: &egui::Painter, name: &str, rect: Rect) {
    if let Some(texture) = texture(ctx, name) {
        painter.image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

pub fn paint_rotated(
    ctx: &egui::Context,
    painter: &egui::Painter,
    name: &str,
    rect: Rect,
    angle: f32,
) {
    if let Some(texture) = texture(ctx, name) {
        if angle.abs() < 0.001 {
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            let center = rect.center();
            let half = rect.size() * 0.5;
            let (sin, cos) = angle.sin_cos();
            let rot = |dx: f32, dy: f32| -> Pos2 {
                center + Vec2::new(dx * cos - dy * sin, dx * sin + dy * cos)
            };
            let mut mesh = egui::Mesh::with_texture(texture.id());
            mesh.reserve_triangles(2);
            mesh.reserve_vertices(4);
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            mesh.vertices.push(egui::epaint::Vertex {
                pos: rot(-half.x, -half.y),
                uv: Pos2::new(0.0, 0.0),
                color: Color32::WHITE,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: rot(half.x, -half.y),
                uv: Pos2::new(1.0, 0.0),
                color: Color32::WHITE,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: rot(half.x, half.y),
                uv: Pos2::new(1.0, 1.0),
                color: Color32::WHITE,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: rot(-half.x, half.y),
                uv: Pos2::new(0.0, 1.0),
                color: Color32::WHITE,
            });
            painter.add(mesh);
        }
    }
}

pub fn image(ui: &mut egui::Ui, name: &str, size: Vec2) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    paint(ui.ctx(), ui.painter(), name, rect);
    response
}

pub fn status_name(presence: Presence) -> &'static str {
    match presence {
        Presence::Available => "disponivel",
        Presence::Busy => "ocupado",
        Presence::Away => "ausente",
        _ => "offline",
    }
}

pub fn status_label(presence: Presence) -> &'static str {
    match presence {
        Presence::Available => "Online",
        Presence::Busy => crate::i18n::tr("Busy", "Ocupado"),
        Presence::Away => crate::i18n::tr("Away", "Ausente"),
        Presence::Invisible => crate::i18n::tr("Invisible", "Invisível"),
        Presence::Offline => "Offline",
    }
}

pub fn presence_menu(ui: &mut egui::Ui, id: &str, presence: &mut Presence, login: bool) -> bool {
    let previous = *presence;
    ui.push_id(id, |ui| {
        ui.scope(|ui| {
            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
            ui.style_mut().visuals.widgets.inactive.bg_stroke = Stroke::NONE;
            ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
            ui.style_mut().visuals.widgets.hovered.bg_fill = Color32::from_rgb(219, 238, 255);
            ui.spacing_mut().button_padding = Vec2::new(3.0, 2.0);
            let label_text = format!("({})    ", status_label(*presence));
            let menu = ui.menu_button(
                egui::RichText::new(label_text).size(10.0).color(MUTED),
                |ui| {
                    ui.set_min_width(120.0);
                    ui.set_max_width(130.0);
                    for state in [
                        Presence::Available,
                        Presence::Busy,
                        Presence::Away,
                        Presence::Invisible,
                    ] {
                        let selected = *presence == state;
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(120.0, 24.0), egui::Sense::click());
                        if response.hovered() || selected {
                            ui.painter().rect_filled(
                                rect,
                                3,
                                if selected {
                                    Color32::from_rgb(205, 230, 252)
                                } else {
                                    Color32::from_rgb(230, 242, 253)
                                },
                            );
                        }
                        paint(
                            ui.ctx(),
                            ui.painter(),
                            &format!("status/{}_icone.png", status_name(state)),
                            Rect::from_center_size(
                                rect.left_center() + Vec2::new(12.0, 0.0),
                                Vec2::splat(11.0),
                            ),
                        );
                        ui.painter().text(
                            rect.left_center() + Vec2::new(26.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            status_label(state),
                            FontId::proportional(11.0),
                            if selected {
                                HEADING
                            } else {
                                Color32::from_rgb(40, 40, 40)
                            },
                        );
                        if response.clicked() {
                            *presence = state;
                            ui.close();
                        }
                    }
                },
            );
            paint(
                ui.ctx(),
                ui.painter(),
                "icons/arrow.png",
                Rect::from_center_size(
                    menu.response.rect.right_center() - Vec2::new(5.0, 0.0),
                    Vec2::splat(6.0),
                ),
            );
            menu.response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(if login {
                crate::i18n::tr("Sign-in status", "Status ao entrar")
            } else {
                crate::i18n::tr("Change status", "Alterar status")
            });
        });
    });
    previous != *presence
}

/// Inner photo corner radius matching the avatar container well.
pub fn frame_radius(inner: Rect) -> u8 {
    let size = inner.width().min(inner.height());
    (size * 0.12).clamp(3.0, 18.0).round() as u8
}

/// Seal the recessed photo with a neutral, translucent glass edge.
pub fn paint_avatar_rim(painter: &egui::Painter, inner: Rect, _presence: Presence) {
    let inner_radius = frame_radius(inner);
    painter.rect_stroke(
        inner,
        inner_radius,
        Stroke::new(0.75, Color32::from_rgba_unmultiplied(24, 40, 49, 145)),
        egui::StrokeKind::Inside,
    );
}

/// Messenger-style glass: milky upper rim, colored lower rim and soft lateral reflections.
fn avatar_glass(painter: &egui::Painter, rect: Rect, radius: f32, tint: Color32) {
    let stops = [
        (0.0, mix(tint, Color32::WHITE, 0.84), 240),
        (0.12, mix(tint, Color32::WHITE, 0.72), 210),
        (0.38, mix(tint, Color32::WHITE, 0.48), 195),
        (0.58, mix(tint, Color32::WHITE, 0.12), 220),
        (0.90, tint, 240),
        (1.0, mix(tint, Color32::BLACK, 0.12), 235),
    ];
    let color = |x: f32, progress: f32| {
        let pair = stops.windows(2).find(|pair| progress <= pair[1].0).unwrap();
        let amount = (progress - pair[0].0) / (pair[1].0 - pair[0].0);
        let rgb = mix(pair[0].1, pair[1].1, amount);
        // A continuous highlight follows the outer lip, fading toward the colored base.
        let edge = x.min(1.0 - x) * rect.width();
        let side_light = (-((edge - 1.6) / 0.9).powi(2)).exp() * (0.38 - progress * 0.16);
        let rgb = mix(rgb, Color32::WHITE, side_light);
        let alpha =
            (pair[0].2 as f32 + (pair[1].2 as f32 - pair[0].2 as f32) * amount).round() as u8;
        Color32::from_rgba_unmultiplied(rgb.r(), rgb.g(), rgb.b(), alpha)
    };
    let height = rect.height();
    let inset = |y: f32| {
        let distance = (radius - y.min(height - y)).max(0.0);
        radius - (radius * radius - distance * distance).max(0.0).sqrt()
    };
    let rows = (height * painter.ctx().pixels_per_point()).ceil().max(1.0) as usize;
    // Sample the thin side rails densely enough for the refracted highlight.
    let columns = 32;
    let mut mesh = egui::Mesh::default();
    for row in 0..=rows {
        let progress = row as f32 / rows as f32;
        let y = progress * height;
        let edge = inset(y);
        for column in 0..=columns {
            let width = rect.width() - edge * 2.0;
            let rail = 6.0_f32.min(width * 0.25);
            let x = edge
                + if column <= 8 {
                    rail * column as f32 / 8.0
                } else if column >= 24 {
                    width - rail * (columns - column) as f32 / 8.0
                } else {
                    rail + (width - 2.0 * rail) * (column - 8) as f32 / 16.0
                };
            mesh.colored_vertex(
                Pos2::new(rect.left() + x, rect.top() + y),
                color(x / rect.width(), progress),
            );
            if row > 0 && column > 0 {
                let end = (row * (columns + 1) + column) as u32;
                let above = end - (columns + 1) as u32;
                mesh.add_triangle(above - 1, above, end - 1);
                mesh.add_triangle(above, end, end - 1);
            }
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

pub fn frame(
    _ctx: &egui::Context,
    painter: &egui::Painter,
    rect: Rect,
    presence: Presence,
    role: &str,
) -> Rect {
    let size = rect.width().min(rect.height());
    let border_pad = match role {
        "login" => (size * 0.07).clamp(3.5, 9.0),
        "conversa" => (size * 0.065).clamp(3.0, 9.0),
        _ => (size * 0.07).clamp(2.5, 9.0),
    };
    let radius = frame_radius(rect.shrink(border_pad)) as f32 + border_pad;
    let r_u8 = radius.round() as u8;

    // Layered, low-opacity shadow keeps the frame floating without a heavy dark edge.
    if size >= 24.0 {
        for (spread, alpha) in [(1.5, 5), (0.75, 8), (0.0, 12)] {
            painter.rect_filled(
                rect.translate(Vec2::new(0.0, 1.0)).expand(spread),
                (radius + spread).round() as u8,
                Color32::from_black_alpha(alpha),
            );
        }
    }

    let glass_tint = match presence {
        Presence::Available => Color32::from_rgb(80, 185, 36),
        Presence::Busy => Color32::from_rgb(207, 57, 72),
        Presence::Away => Color32::from_rgb(222, 151, 25),
        _ => Color32::from_rgb(138, 155, 173),
    };
    // Faint colored halo under the bottom edge, like the original Messenger bezel.
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 1.0)).expand(1.0),
        r_u8 + 1,
        Color32::from_rgba_unmultiplied(glass_tint.r(), glass_tint.g(), glass_tint.b(), 22),
    );
    avatar_glass(painter, rect, radius, glass_tint);

    // Crisp outer bezel hairline
    painter.rect_stroke(
        rect,
        r_u8,
        Stroke::new(
            0.75,
            mix(glass_tint, Color32::from_rgb(139, 151, 135), 0.72),
        ),
        egui::StrokeKind::Inside,
    );

    // 3. Inner avatar picture well
    let inner = rect.shrink(border_pad);
    let inner_radius = frame_radius(inner);

    // White backing plate so pictures with transparent pngs or loading backgrounds look clean
    painter.rect_filled(inner, inner_radius, Color32::WHITE);

    // Thin inner bevel; the photo rim supplies the darker recessed edge.
    painter.rect_stroke(
        inner.expand(0.5),
        inner_radius + 1,
        Stroke::new(0.75, Color32::from_white_alpha(135)),
        egui::StrokeKind::Outside,
    );

    inner
}

pub fn avatar(app: &mut MessengerApp, ui: &mut egui::Ui, profile: &Profile, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
    avatar_at(
        app,
        ui,
        profile,
        rect,
        if size >= 100.0 { "conversa" } else { "perfil" },
    );
    response.on_hover_text(format!(
        "{}\n{}",
        profile.email,
        status_label(profile.presence)
    ))
}

pub fn avatar_at(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    profile: &Profile,
    rect: Rect,
    role: &str,
) {
    let inner = frame(ui.ctx(), ui.painter(), rect, profile.presence, role);
    let file = profile
        .avatar_id
        .and_then(|id| {
            app.data
                .as_ref()
                .and_then(|data| data.assets.iter().find(|a| a.id == id).map(|a| a.file_id))
        })
        .or_else(|| Scene::parse(&profile.scene).avatar_file);
    if let Some(file) = file
        && let Some((texture, size)) = app.texture(ui.ctx(), ImageSource::File(file))
    {
        egui::Image::new((texture, inner.size()))
            .uv(cover_uv(size, inner.size()))
            .corner_radius(frame_radius(inner))
            .paint_at(ui, inner);
    } else {
        paint(
            ui.ctx(),
            ui.painter(),
            "icons/person.png",
            Rect::from_center_size(inner.center(), inner.size() * 0.65),
        );
    }
    paint_avatar_rim(ui.painter(), inner, profile.presence);
}

pub fn cover_uv(source: Vec2, destination: Vec2) -> Rect {
    let source_aspect = source.x / source.y.max(1.0);
    let target_aspect = destination.x / destination.y.max(1.0);
    let size = if source_aspect > target_aspect {
        Vec2::new(target_aspect / source_aspect, 1.0)
    } else {
        Vec2::new(1.0, source_aspect / target_aspect)
    };
    Rect::from_center_size(egui::pos2(0.5, 0.5), size)
}

pub fn input(
    ui: &mut egui::Ui,
    rect: Rect,
    id: &'static str,
    value: &mut String,
    placeholder: &str,
    password: bool,
) -> Response {
    ui.painter().rect_filled(rect, 4, Color32::WHITE);
    ui.painter().rect_stroke(
        rect,
        4,
        Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Inside,
    );
    let response = ui.put(
        rect,
        egui::TextEdit::singleline(value)
            .id(egui::Id::unique(id))
            .font(FontId::proportional(11.5))
            .password(password)
            .hint_text(egui::RichText::new(placeholder).size(11.0).color(MUTED))
            .align(egui::Align2::LEFT_CENTER)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 0,
                bottom: 0,
            }))
            .min_size(rect.size()),
    );
    if response.clicked() || response.gained_focus() {
        let field_type = if password {
            "password"
        } else if id.contains("email") || placeholder.contains('@') {
            "email"
        } else {
            "text"
        };
        crate::platform::show_soft_input_type(field_type);
    }
    response.context_menu(|ui| {
        if ui.button(tr("Paste", "Colar")).clicked() {
            if let Some(text) = crate::platform::get_clipboard_text() {
                *value = text.trim_matches(['\r', '\n']).to_string();
                ui.ctx().input_mut(|i| i.events.push(egui::Event::Paste(value.clone())));
            }
            ui.close();
        }
        if !value.is_empty() && ui.button(tr("Clear", "Limpar")).clicked() {
            value.clear();
            ui.close();
        }
    });

    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            4,
            Stroke::new(1.0_f32, Color32::from_rgb(148, 163, 184)),
            egui::StrokeKind::Inside,
        );
    }
    response
}

pub fn left(ui: &mut egui::Ui, rect: Rect, widget: impl egui::Widget) -> Response {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| ui.add(widget),
    )
    .inner
}

pub fn checkbox(
    ui: &mut egui::Ui,
    rect: Rect,
    value: &mut bool,
    label: &str,
    enabled: bool,
) -> Response {
    let response = ui.interact(
        rect,
        ui.scope_id().with(label),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    if enabled && response.clicked() {
        *value = !*value;
    }
    let box_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 6.0, rect.center().y),
        Vec2::splat(10.0),
    );
    ui.painter()
        .rect_filled(box_rect, 1, Color32::from_rgb(229, 224, 234));
    ui.painter().rect_stroke(
        box_rect,
        1,
        Stroke::new(0.7_f32, Color32::from_rgb(170, 176, 183)),
        egui::StrokeKind::Inside,
    );
    if *value {
        ui.painter().line_segment(
            [
                box_rect.left_center() + Vec2::new(2.0, 0.0),
                box_rect.center() + Vec2::new(0.0, 2.0),
            ],
            Stroke::new(1.0_f32, HEADING),
        );
        ui.painter().line_segment(
            [
                box_rect.center() + Vec2::new(0.0, 2.0),
                box_rect.right_top() + Vec2::new(-2.0, 2.0),
            ],
            Stroke::new(1.0_f32, HEADING),
        );
    }
    ui.painter().text(
        Pos2::new(rect.left() + 16.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(10.0),
        if enabled { INK } else { MUTED },
    );
    response
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowControls {
    /// Full 3-button cluster: Minimize, Maximize/Restore, Close (main and chat windows)
    All,
    /// Close button only (dialogs, preferences, modals)
    CloseOnly,
    /// Minimize and Close
    #[allow(dead_code)]
    MinimizeClose,
}

fn paint_caption_button(
    painter: &egui::Painter,
    rect: Rect,
    response: &egui::Response,
    is_close: bool,
) -> (Pos2, Color32) {
    let top_half = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.center().y));

    if response.is_pointer_button_down_on() {
        if is_close {
            painter.rect_filled(rect, 3, Color32::from_rgb(175, 20, 26));
            painter.rect_stroke(
                rect,
                3,
                Stroke::new(1.0, Color32::from_rgb(125, 12, 16)),
                StrokeKind::Inside,
            );
            (rect.center() + Vec2::new(0.0, 0.5), Color32::from_rgb(240, 240, 240))
        } else {
            painter.rect_filled(rect, 3, Color32::from_rgba_unmultiplied(165, 205, 245, 240));
            painter.rect_stroke(
                rect,
                3,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(65, 130, 200, 250)),
                StrokeKind::Inside,
            );
            (rect.center() + Vec2::new(0.0, 0.5), Color32::from_rgb(10, 20, 35))
        }
    } else if response.hovered() {
        if is_close {
            painter.rect_filled(rect, 3, Color32::from_rgb(228, 52, 52));
            painter.rect_filled(
                top_half,
                3,
                Color32::from_rgba_unmultiplied(255, 130, 130, 130),
            );
            painter.rect_stroke(
                rect,
                3,
                Stroke::new(1.0, Color32::from_rgb(175, 25, 30)),
                StrokeKind::Inside,
            );
            painter.line_segment(
                [
                    Pos2::new(rect.left() + 2.0, rect.top() + 1.0),
                    Pos2::new(rect.right() - 2.0, rect.top() + 1.0),
                ],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 215, 215, 180)),
            );
            (rect.center(), Color32::WHITE)
        } else {
            painter.rect_filled(rect, 3, Color32::from_rgba_unmultiplied(230, 243, 255, 215));
            painter.rect_filled(
                top_half,
                3,
                Color32::from_rgba_unmultiplied(255, 255, 255, 140),
            );
            painter.rect_stroke(
                rect,
                3,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(95, 165, 230, 220)),
                StrokeKind::Inside,
            );
            painter.line_segment(
                [
                    Pos2::new(rect.left() + 2.0, rect.top() + 1.0),
                    Pos2::new(rect.right() - 2.0, rect.top() + 1.0),
                ],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 230)),
            );
            (rect.center(), Color32::from_rgb(15, 30, 48))
        }
    } else {
        painter.rect_filled(rect, 3, Color32::from_rgba_unmultiplied(255, 255, 255, 55));
        painter.rect_filled(
            top_half,
            3,
            Color32::from_rgba_unmultiplied(255, 255, 255, 45),
        );
        painter.rect_stroke(
            rect,
            3,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 110)),
            StrokeKind::Inside,
        );
        painter.line_segment(
            [
                Pos2::new(rect.left() + 1.0, rect.bottom()),
                Pos2::new(rect.right() - 1.0, rect.bottom()),
            ],
            Stroke::new(1.0, Color32::from_black_alpha(25)),
        );
        (rect.center(), Color32::from_rgb(55, 70, 85))
    }
}

fn draw_close_glyph(
    painter: &egui::Painter,
    center: Pos2,
    glyph_color: Color32,
    is_idle: bool,
) {
    let d = 3.5;
    if is_idle {
        let glow = Color32::from_rgba_unmultiplied(255, 255, 255, 130);
        painter.line_segment(
            [
                Pos2::new(center.x - d, center.y - d + 1.0),
                Pos2::new(center.x + d, center.y + d + 1.0),
            ],
            Stroke::new(1.2, glow),
        );
        painter.line_segment(
            [
                Pos2::new(center.x - d, center.y + d + 1.0),
                Pos2::new(center.x + d, center.y - d + 1.0),
            ],
            Stroke::new(1.2, glow),
        );
    }
    painter.line_segment(
        [
            Pos2::new(center.x - d, center.y - d),
            Pos2::new(center.x + d, center.y + d),
        ],
        Stroke::new(1.4, glyph_color),
    );
    painter.line_segment(
        [
            Pos2::new(center.x - d, center.y + d),
            Pos2::new(center.x + d, center.y - d),
        ],
        Stroke::new(1.4, glyph_color),
    );
}

fn draw_minimize_glyph(
    painter: &egui::Painter,
    center: Pos2,
    glyph_color: Color32,
    is_idle: bool,
) {
    if is_idle {
        painter.line_segment(
            [
                Pos2::new(center.x - 4.5, center.y + 4.5),
                Pos2::new(center.x + 4.5, center.y + 4.5),
            ],
            Stroke::new(1.2, Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
        );
    }
    painter.line_segment(
        [
            Pos2::new(center.x - 4.5, center.y + 3.5),
            Pos2::new(center.x + 4.5, center.y + 3.5),
        ],
        Stroke::new(1.6, glyph_color),
    );
}

fn draw_maximize_glyph(
    painter: &egui::Painter,
    center: Pos2,
    glyph_color: Color32,
    is_idle: bool,
    is_maximized: bool,
    fill_color: Color32,
) {
    if is_maximized {
        let back_r = Rect::from_center_size(center + Vec2::new(1.5, -1.5), Vec2::new(7.0, 7.0));
        let front_r = Rect::from_center_size(center + Vec2::new(-1.5, 1.5), Vec2::new(7.0, 7.0));

        painter.line_segment([back_r.left_top(), back_r.right_top()], Stroke::new(1.2, glyph_color));
        painter.line_segment([back_r.right_top(), back_r.right_bottom()], Stroke::new(1.2, glyph_color));
        painter.line_segment(
            [
                Pos2::new(back_r.left() + 2.0, back_r.top() + 1.0),
                Pos2::new(back_r.right(), back_r.top() + 1.0),
            ],
            Stroke::new(1.2, glyph_color),
        );

        painter.rect_filled(front_r, 0, fill_color);
        painter.rect_stroke(front_r, 0, Stroke::new(1.2, glyph_color), StrokeKind::Inside);
        painter.line_segment(
            [
                Pos2::new(front_r.left(), front_r.top() + 1.0),
                Pos2::new(front_r.right(), front_r.top() + 1.0),
            ],
            Stroke::new(1.4, glyph_color),
        );
    } else {
        let box_r = Rect::from_center_size(center, Vec2::new(9.0, 9.0));
        if is_idle {
            painter.rect_stroke(
                box_r.translate(Vec2::new(0.0, 1.0)),
                0,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 130)),
                StrokeKind::Inside,
            );
        }
        painter.rect_stroke(box_r, 0, Stroke::new(1.2, glyph_color), StrokeKind::Inside);
        painter.line_segment(
            [
                Pos2::new(box_r.left(), box_r.top() + 1.0),
                Pos2::new(box_r.right(), box_r.top() + 1.0),
            ],
            Stroke::new(1.6, glyph_color),
        );
    }
}

#[cfg(any(not(target_os = "android"), test))]
pub fn window_bar(ui: &mut egui::Ui, title: &str) {
    let controls = if ui.ctx().viewport_id() == egui::ViewportId::ROOT {
        WindowControls::All
    } else {
        WindowControls::CloseOnly
    };
    window_bar_with_controls(ui, title, controls);
}

pub fn window_bar_with_controls(ui: &mut egui::Ui, title: &str, controls: WindowControls) {
    let ctx = ui.ctx().clone();
    let native = super::window::apply_titlebar(&ctx);
    background(&ctx);

    let title_key = egui::Id::unique(("nmessenger-viewport-title", ctx.viewport_id()));
    let prev_title = ctx.data(|d| d.get_temp::<String>(title_key));
    if prev_title.as_deref() != Some(title) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.to_string()));
        ctx.data_mut(|d| d.insert_temp(title_key, title.to_string()));
    }

    if native || cfg!(target_os = "android") {
        return;
    }
    super::window::resize_edges(&ctx);
    egui::Panel::top(egui::Id::unique(("nmessenger-title", ctx.viewport_id())))
        .exact_size(40.0)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(Color32::TRANSPARENT)
                .inner_margin(0),
        )
        .show(ui, |ui| {
            let rect = ui.max_rect();
            let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));

            let btn_top = rect.top() + 9.0;
            let btn_bottom = btn_top + 21.0;
            let close_width = 36.0;
            let close_rect = Rect::from_min_max(
                Pos2::new(rect.right() - 8.0 - close_width, btn_top),
                Pos2::new(rect.right() - 8.0, btn_bottom),
            );

            let (cluster_left, min_info, max_info) = match controls {
                WindowControls::All => {
                    let max_width = 27.0;
                    let max_rect = Rect::from_min_max(
                        Pos2::new(close_rect.left() - 2.0 - max_width, btn_top),
                        Pos2::new(close_rect.left() - 2.0, btn_bottom),
                    );
                    let min_width = 27.0;
                    let min_rect = Rect::from_min_max(
                        Pos2::new(max_rect.left() - 2.0 - min_width, btn_top),
                        Pos2::new(max_rect.left() - 2.0, btn_bottom),
                    );
                    (min_rect.left(), Some(min_rect), Some(max_rect))
                }
                WindowControls::MinimizeClose => {
                    let min_width = 27.0;
                    let min_rect = Rect::from_min_max(
                        Pos2::new(close_rect.left() - 2.0 - min_width, btn_top),
                        Pos2::new(close_rect.left() - 2.0, btn_bottom),
                    );
                    (min_rect.left(), Some(min_rect), None)
                }
                WindowControls::CloseOnly => (close_rect.left(), None, None),
            };

            let drag_top = if is_maximized { rect.top() } else { rect.top() + 7.0 };
            let drag_left = if is_maximized { rect.left() } else { rect.left() + 7.0 };
            let drag_rect = Rect::from_min_max(
                Pos2::new(drag_left, drag_top),
                Pos2::new(cluster_left - 8.0, rect.bottom()),
            );
            let drag = ui
                .interact(
                    drag_rect,
                    ui.scope_id().with("drag"),
                    egui::Sense::click_and_drag(),
                )
                .on_hover_text(title);

            if drag.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if controls == WindowControls::All && drag.double_clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
            }

            // Left side: App icon
            let icon_center = Pos2::new(rect.left() + 20.0, rect.top() + 20.0);
            let icon_rect = Rect::from_center_size(icon_center, Vec2::splat(18.0));
            paint(&ctx, ui.painter(), "icon.svg", icon_rect);

            // Left side: Title typography with Aero Glass text glow
            let text_x = rect.left() + 34.0;
            let max_text_width = (drag_rect.max.x - text_x - 8.0).max(30.0);
            let title_font = FontId::proportional(12.5);
            let clipped_painter = ui.painter().with_clip_rect(drag_rect);
            let text_width = ui.fonts_mut(|f| {
                f.layout_no_wrap(title.to_string(), title_font.clone(), INK).size().x
            });
            let display_text = if text_width > max_text_width {
                let mut truncated = title.to_string();
                while !truncated.is_empty() {
                    truncated.pop();
                    let candidate = format!("{truncated}…");
                    let w = ui.fonts_mut(|f| {
                        f.layout_no_wrap(candidate.clone(), title_font.clone(), INK).size().x
                    });
                    if w <= max_text_width {
                        truncated = candidate;
                        break;
                    }
                }
                truncated
            } else {
                title.to_string()
            };

            let text_pos = Pos2::new(text_x, rect.top() + 20.0);
            let glow_color = Color32::from_rgba_unmultiplied(255, 255, 255, 130);
            for offset in [
                Vec2::new(-1.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, -1.0),
                Vec2::new(0.0, 1.0),
            ] {
                clipped_painter.text(
                    text_pos + offset,
                    egui::Align2::LEFT_CENTER,
                    &display_text,
                    title_font.clone(),
                    glow_color,
                );
            }
            clipped_painter.text(
                text_pos,
                egui::Align2::LEFT_CENTER,
                &display_text,
                title_font,
                INK,
            );

            // Right side: Window buttons
            let close_res = ui
                .interact(close_rect, ui.scope_id().with("close"), egui::Sense::click())
                .on_hover_text(crate::i18n::tr("Close", "Fechar"));
            let (close_glyph_pos, close_glyph_color) =
                paint_caption_button(ui.painter(), close_rect, &close_res, true);
            draw_close_glyph(
                ui.painter(),
                close_glyph_pos,
                close_glyph_color,
                !close_res.hovered() && !close_res.is_pointer_button_down_on(),
            );

            if let Some(max_rect) = max_info {
                let max_res = ui
                    .interact(max_rect, ui.scope_id().with("maximize"), egui::Sense::click())
                    .on_hover_text(if is_maximized {
                        "Restaurar"
                    } else {
                        "Maximizar"
                    });
                let (max_glyph_pos, max_glyph_color) =
                    paint_caption_button(ui.painter(), max_rect, &max_res, false);
                let front_fill = if max_res.is_pointer_button_down_on() {
                    Color32::from_rgba_unmultiplied(165, 205, 245, 255)
                } else if max_res.hovered() {
                    Color32::from_rgba_unmultiplied(230, 243, 255, 255)
                } else {
                    Color32::from_rgb(220, 235, 248)
                };
                draw_maximize_glyph(
                    ui.painter(),
                    max_glyph_pos,
                    max_glyph_color,
                    !max_res.hovered() && !max_res.is_pointer_button_down_on(),
                    is_maximized,
                    front_fill,
                );
                if max_res.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }
            }

            if let Some(min_rect) = min_info {
                let min_res = ui
                    .interact(min_rect, ui.scope_id().with("minimize"), egui::Sense::click())
                    .on_hover_text("Minimizar");
                let (min_glyph_pos, min_glyph_color) =
                    paint_caption_button(ui.painter(), min_rect, &min_res, false);
                draw_minimize_glyph(
                    ui.painter(),
                    min_glyph_pos,
                    min_glyph_color,
                    !min_res.hovered() && !min_res.is_pointer_button_down_on(),
                );
                if min_res.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            }

            if close_res.clicked() {
                if ctx.viewport_id() == egui::ViewportId::ROOT {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_photos_are_center_cropped_without_changing_aspect_ratio() {
        assert_eq!(
            cover_uv(Vec2::new(400.0, 200.0), Vec2::splat(100.0)),
            Rect::from_min_max(egui::pos2(0.25, 0.0), egui::pos2(0.75, 1.0))
        );
        assert_eq!(
            cover_uv(Vec2::new(200.0, 400.0), Vec2::splat(100.0)),
            Rect::from_min_max(egui::pos2(0.0, 0.25), egui::pos2(1.0, 0.75))
        );
        assert_eq!(
            cover_uv(Vec2::splat(96.0), Vec2::splat(100.0)),
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0))
        );
    }
}
