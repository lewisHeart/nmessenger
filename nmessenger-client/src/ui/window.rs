use eframe::egui::{self, Pos2, Rect, Vec2};
use std::time::{Duration, Instant};
use std::collections::HashMap;

#[derive(Clone)]
struct WindowSizes {
    values: HashMap<String, [f32; 2]>,
    loaded: Instant,
    changed: Option<Instant>,
}

fn sizes_id() -> egui::Id { egui::Id::unique("nmessenger-window-sizes") }

fn valid_size(size: [f32; 2]) -> bool {
    size.iter().all(|value| value.is_finite() && (200.0..=8192.0).contains(value))
}

pub fn load_sizes(ctx: &egui::Context, sizes: HashMap<String, [f32; 2]>) {
    let values: HashMap<_, _> = sizes.into_iter().filter(|(_, size)| valid_size(*size)).collect();
    if !crate::platform::is_mobile() && let Some(size) = values.get("main") {
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::InnerSize(Vec2::new(size[0], size[1])));
    }
    ctx.data_mut(|data| data.insert_temp(sizes_id(), WindowSizes { values, loaded: Instant::now(), changed: None }));
}

pub fn initial_size(ctx: &egui::Context, key: &str, default: [f32; 2]) -> [f32; 2] {
    let id = egui::Id::unique(("nmessenger-initial-window-size", key));
    ctx.data_mut(|data| {
        if let Some(size) = data.get_temp::<[f32; 2]>(id) { return size; }
        let size = data.get_temp::<WindowSizes>(sizes_id()).and_then(|sizes| sizes.values.get(key).copied()).unwrap_or(default);
        data.insert_temp(id, size);
        size
    })
}

pub fn has_viewport_initialized(ctx: &egui::Context, key: &str) -> bool {
    ctx.data(|data| data.get_temp::<bool>(egui::Id::unique(("nmessenger-viewport-init", key)))).unwrap_or(false)
}

pub fn mark_viewport_initialized(ctx: &egui::Context, key: &str) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::unique(("nmessenger-viewport-init", key)), true));
}

pub fn forget_initial_size(ctx: &egui::Context, key: &str) {
    ctx.data_mut(|data| {
        data.remove::<[f32; 2]>(egui::Id::unique(("nmessenger-initial-window-size", key)));
        data.remove::<bool>(egui::Id::unique(("nmessenger-viewport-init", key)));
    });
}

pub fn record_size(ctx: &egui::Context, key: &str) {
    if crate::platform::is_mobile() { return; }
    let size = ctx.input(|input| {
        let viewport = input.viewport();
        if viewport.maximized.unwrap_or(false) || viewport.fullscreen.unwrap_or(false) { None }
        else { viewport.inner_rect.map(|rect| [rect.width().round(), rect.height().round()]) }
    });
    let Some(size) = size.filter(|size| valid_size(*size)) else { return; };
    let changed = ctx.data_mut(|data| {
        let Some(mut state) = data.get_temp::<WindowSizes>(sizes_id()) else { return false; };
        if state.loaded.elapsed() < Duration::from_secs(1) || state.values.get(key) == Some(&size) { return false; }
        state.values.insert(key.to_owned(), size);
        state.changed = Some(Instant::now());
        data.insert_temp(sizes_id(), state);
        true
    });
    if changed {
        // Do NOT request immediate repaint of ROOT during interactive mouse drag!
        // Schedule ROOT repaint after the 750ms debounce window so ROOT stays idle during resize.
        ctx.request_repaint_after_for(Duration::from_millis(800), egui::ViewportId::ROOT);
    }
}

pub fn take_size_changes(ctx: &egui::Context) -> Option<HashMap<String, [f32; 2]>> {
    ctx.data_mut(|data| {
        let mut state = data.get_temp::<WindowSizes>(sizes_id())?;
        if state.changed.is_none_or(|changed| changed.elapsed() < Duration::from_millis(750)) { return None; }
        state.changed = None;
        let values = state.values.clone();
        data.insert_temp(sizes_id(), state);
        Some(values)
    })
}

#[derive(Clone)]
struct Shake {
    origin: Option<Pos2>,
    started: Instant,
}

pub fn configure(ctx: &egui::Context, system_titlebars: bool) {
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::unique("nmessenger-system-titlebars"),
            system_titlebars,
        )
    });
}

pub fn scale(ctx: &egui::Context, scale: f32) {
    let scale = if scale.is_finite() {
        scale.clamp(0.75, 2.0)
    } else {
        1.0
    };
    let key = egui::Id::unique("nmessenger-ui-scale");
    if ctx.data(|data| data.get_temp::<f32>(key)) != Some(scale) {
        ctx.set_zoom_factor(scale);
        ctx.data_mut(|data| data.insert_temp(key, scale));
    }
}

pub fn system_titlebars(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(egui::Id::unique("nmessenger-system-titlebars")))
        .unwrap_or(false)
}

pub fn apply_titlebar(ctx: &egui::Context) -> bool {
    let native = system_titlebars(ctx);
    let key = egui::Id::unique(("nmessenger-applied-titlebar", ctx.viewport_id()));
    if ctx.data(|data| data.get_temp::<bool>(key)) != Some(native) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(native));
        ctx.data_mut(|data| data.insert_temp(key, native));
    }
    native
}

pub fn minimum(ctx: &egui::Context, size: Vec2) {
    let key = egui::Id::unique(("nmessenger-minimum", ctx.viewport_id()));
    ctx.data_mut(|data| data.insert_temp(key, size));
}

pub fn shake(ctx: &egui::Context, started: Option<Instant>) -> Vec2 {
    let key = egui::Id::unique(("nmessenger-window-shake", ctx.viewport_id()));
    let previous = ctx.data(|data| data.get_temp::<Shake>(key));
    let Some(started) = started.filter(|time| time.elapsed() < Duration::from_millis(800)) else {
        if let Some(previous) = previous {
            if let Some(origin) = previous.origin {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(origin));
            }
            ctx.data_mut(|data| data.remove::<Shake>(key));
        }
        return Vec2::ZERO;
    };
    let state = match previous {
        Some(previous) if previous.started == started => previous,
        previous => {
            let origin = previous.and_then(|state| state.origin).or_else(|| {
                ctx.input(|input| {
                    input
                        .viewport()
                        .outer_rect
                        .or(input.viewport().inner_rect)
                        .map(|rect| rect.min)
                })
            });
            let state = Shake { origin, started };
            ctx.data_mut(|data| data.insert_temp(key, state.clone()));
            state
        }
    };
    let elapsed = started.elapsed().as_secs_f32();
    let amplitude = (1.0 - elapsed / 0.8).clamp(0.0, 1.0);
    let offset = Vec2::new((elapsed * 90.0).sin() * 9.0, (elapsed * 73.0).cos() * 6.0) * amplitude;
    if let Some(origin) = state.origin {
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(origin + offset));
    }
    ctx.request_repaint_after(Duration::from_millis(16));
    offset
}

/// Read border gestures directly, so child panels cannot swallow the resize press.
pub fn resize_edges(ctx: &egui::Context) {
    use egui::{CursorIcon as Cursor, ResizeDirection as Direction};
    if ctx.input(|input| input.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let r = ctx.content_rect();
    let (edge, corner) = (7.0, 14.0);
    let regions = [
        (
            Rect::from_min_max(r.min, r.min + Vec2::splat(corner)),
            Direction::NorthWest,
            Cursor::ResizeNwSe,
        ),
        (
            Rect::from_min_max(
                egui::pos2(r.right() - corner, r.top()),
                egui::pos2(r.right(), r.top() + corner),
            ),
            Direction::NorthEast,
            Cursor::ResizeNeSw,
        ),
        (
            Rect::from_min_max(
                egui::pos2(r.left(), r.bottom() - corner),
                egui::pos2(r.left() + corner, r.bottom()),
            ),
            Direction::SouthWest,
            Cursor::ResizeNeSw,
        ),
        (
            Rect::from_min_max(r.max - Vec2::splat(corner), r.max),
            Direction::SouthEast,
            Cursor::ResizeNwSe,
        ),
        (
            Rect::from_min_max(
                r.min + Vec2::new(corner, 0.0),
                egui::pos2(r.right() - corner, r.top() + edge),
            ),
            Direction::North,
            Cursor::ResizeVertical,
        ),
        (
            Rect::from_min_max(
                egui::pos2(r.left() + corner, r.bottom() - edge),
                r.max - Vec2::new(corner, 0.0),
            ),
            Direction::South,
            Cursor::ResizeVertical,
        ),
        (
            Rect::from_min_max(
                r.min + Vec2::new(0.0, corner),
                egui::pos2(r.left() + edge, r.bottom() - corner),
            ),
            Direction::West,
            Cursor::ResizeHorizontal,
        ),
        (
            Rect::from_min_max(
                egui::pos2(r.right() - edge, r.top() + corner),
                r.max - Vec2::new(0.0, corner),
            ),
            Direction::East,
            Cursor::ResizeHorizontal,
        ),
    ];
    let pointer = ctx.input(|input| input.pointer.interact_pos().or(input.pointer.hover_pos()));
    let pressed = ctx.input(|input| input.pointer.primary_pressed());
    let hit = if pressed {
        ctx.input(|input| input.pointer.press_origin()).or(pointer)
    } else {
        pointer
    };
    if let Some(hit) = hit
        && let Some((_, direction, cursor)) = regions.iter().find(|(rect, _, _)| rect.contains(hit))
    {
        ctx.set_cursor_icon(*cursor);
        if pressed {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(*direction));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn saved_window_sizes_are_restored_and_invalid_sizes_are_ignored() {
        let ctx = egui::Context::default();
        load_sizes(&ctx, HashMap::from([
            ("profile".to_owned(), [640.0, 720.0]),
            ("invalid".to_owned(), [f32::NAN, 300.0]),
        ]));
        assert_eq!(initial_size(&ctx, "profile", [480.0, 580.0]), [640.0, 720.0]);
        assert_eq!(initial_size(&ctx, "invalid", [480.0, 580.0]), [480.0, 580.0]);
        ctx.data_mut(|data| {
            let mut sizes = data.get_temp::<WindowSizes>(sizes_id()).unwrap();
            sizes.values.insert("profile".into(), [700.0, 800.0]);
            data.insert_temp(sizes_id(), sizes);
        });
        assert_eq!(initial_size(&ctx, "profile", [480.0, 580.0]), [640.0, 720.0]);
        forget_initial_size(&ctx, "profile");
        assert_eq!(initial_size(&ctx, "profile", [480.0, 580.0]), [700.0, 800.0]);
    }

    #[test]
    fn resize_is_recorded_and_debounced_for_persistence() {
        let ctx = egui::Context::default();
        load_sizes(&ctx, HashMap::new());
        ctx.data_mut(|data| {
            let mut sizes = data.get_temp::<WindowSizes>(sizes_id()).unwrap();
            sizes.loaded = Instant::now() - Duration::from_secs(2);
            data.insert_temp(sizes_id(), sizes);
        });
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 500.0)));
        input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().inner_rect = input.screen_rect;
        let mut output = ctx.run_ui(input, |ui| record_size(ui.ctx(), "main"));
        output.textures_delta.clear();
        assert!(take_size_changes(&ctx).is_none());
        ctx.data_mut(|data| {
            let mut sizes = data.get_temp::<WindowSizes>(sizes_id()).unwrap();
            sizes.changed = Some(Instant::now() - Duration::from_secs(1));
            data.insert_temp(sizes_id(), sizes);
        });
        assert_eq!(take_size_changes(&ctx).unwrap().get("main"), Some(&[640.0, 500.0]));
        assert!(take_size_changes(&ctx).is_none());
    }

    #[test]
    fn shake_returns_zero_when_not_started() {
        let ctx = egui::Context::default();
        let offset = shake(&ctx, None);
        assert_eq!(offset, Vec2::ZERO);
    }

    #[test]
    fn overlaid_label_does_not_block_hover_on_profile_field() {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(egui::pos2(20.0, 20.0), Vec2::new(120.0, 24.0));
        let input = |events| egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(300.0))),
            events,
            ..Default::default()
        };
        let draw = |ui: &mut egui::Ui| {
            ui.put(rect, egui::Label::new("Profile field"));
            let response = ui.interact(rect, ui.scope_id().with("field"), egui::Sense::click());
            response.hovered()
        };
        let mut output = ctx.run_ui(input(vec![]), |ui| { draw(ui); });
        output.textures_delta.clear();
        let mut hovered = false;
        let mut output = ctx.run_ui(input(vec![egui::Event::PointerMoved(rect.center())]), |ui| { hovered = draw(ui); });
        output.textures_delta.clear();
        assert!(hovered, "the field should react to pointer movement without a click");
    }

    #[test]
    fn shake_calculates_decaying_offset_even_without_native_origin() {
        let ctx = egui::Context::default();
        let now = Instant::now();
        let offset = shake(&ctx, Some(now));
        // On headless / Wayland where outer_rect and inner_rect are None,
        // shake must still generate an oscillation vector instead of bailing out early.
        assert_ne!(offset, Vec2::ZERO);

        let expired = now - Duration::from_millis(900);
        let reset_offset = shake(&ctx, Some(expired));
        assert_eq!(reset_offset, Vec2::ZERO);
    }
}
