//! System tray (StatusNotifierItem) without any desktop toolkit dependency.
use ksni::{
    Icon, Tray,
    blocking::{Handle, TrayMethods},
    menu::{MenuItem, StandardItem, SubMenu},
};
use nm_protocol::Presence;
use std::sync::{
    Mutex, OnceLock,
    mpsc::{self, Receiver, Sender},
};

#[derive(Clone, Copy)]
pub enum Action {
    Show,
    Hide,
    Quit,
    Presence(Presence),
}

struct NmTray {
    sender: Sender<Action>,
    presence: Presence,
}

static HANDLE: OnceLock<Mutex<Option<Handle<NmTray>>>> = OnceLock::new();

fn status_item(tray: &NmTray, value: Presence, label: &str) -> MenuItem<NmTray> {
    let active = tray.presence == value;
    StandardItem {
        label: format!("{}{label}", if active { "● " } else { "    " }),
        activate: Box::new(move |tray: &mut NmTray| {
            tray.presence = value;
            let _ = tray.sender.send(Action::Presence(value));
        }),
        ..Default::default()
    }
    .into()
}

impl Tray for NmTray {
    fn id(&self) -> String {
        "nmessenger".into()
    }
    fn title(&self) -> String {
        "NMessenger".into()
    }
    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![icon(64), icon(48), icon(32), icon(24), icon(16)]
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.sender.send(Action::Show);
    }
    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        let _ = self.sender.send(Action::Show);
    }
    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Abrir NMessenger".into(),
                activate: Box::new(|tray: &mut NmTray| {
                    let _ = tray.sender.send(Action::Show);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Esconder janela".into(),
                activate: Box::new(|tray: &mut NmTray| {
                    let _ = tray.sender.send(Action::Hide);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            SubMenu {
                label: "Meu status".into(),
                submenu: vec![
                    status_item(self, Presence::Available, "Disponível"),
                    status_item(self, Presence::Busy, "Ocupado"),
                    status_item(self, Presence::Away, "Ausente"),
                    status_item(self, Presence::Invisible, "Invisível"),
                ],
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Sair".into(),
                activate: Box::new(|tray: &mut NmTray| {
                    let _ = tray.sender.send(Action::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Starts the tray in a background thread. Missing D-Bus is not fatal.
pub fn start() -> Option<Receiver<Action>> {
    let (sender, receiver) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("nm-tray".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                NmTray {
                    sender,
                    presence: Presence::Available,
                }
                .spawn()
            }));
            match result {
                Ok(Ok(handle)) => {
                    let _ = HANDLE.set(Mutex::new(Some(handle)));
                    std::thread::park();
                }
                Ok(Err(error)) => eprintln!("Bandeja do sistema indisponível: {error}"),
                Err(_) => eprintln!("Bandeja do sistema indisponível neste ambiente."),
            }
        })
        .ok()?;
    std::mem::forget(thread);
    Some(receiver)
}

pub fn presence(value: Presence) {
    if let Some(lock) = HANDLE.get()
        && let Ok(guard) = lock.lock()
        && let Some(handle) = guard.as_ref()
    {
        handle.update(|tray| tray.presence = value);
    }
}

fn icon(side: i32) -> Icon {
    let side = side.max(1);
    let bytes = include_bytes!("../assets/nmessenger/icon.svg");
    let opt = resvg::usvg::Options::default();
    if let Ok(tree) = resvg::usvg::Tree::from_data(bytes, &opt) {
        let size = tree.size();
        let scale_x = side as f32 / size.width();
        let scale_y = side as f32 / size.height();
        let scale = scale_x.min(scale_y);
        let w = (size.width() * scale).round() as u32;
        let h = (size.height() * scale).round() as u32;
        if let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(side as u32, side as u32) {
            let offset_x = (side as f32 - w as f32) / 2.0;
            let offset_y = (side as f32 - h as f32) / 2.0;
            let transform = resvg::tiny_skia::Transform::from_translate(offset_x, offset_y)
                .post_scale(scale, scale);
            resvg::render(&tree, transform, &mut pixmap.as_mut());
            let mut data = Vec::with_capacity((side * side * 4) as usize);
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
                // ksni expects ARGB32 in network byte order: [A, R, G, B]
                data.extend_from_slice(&[a, r, g, b]);
            }
            return Icon {
                width: side,
                height: side,
                data,
            };
        }
    }
    Icon {
        width: side,
        height: side,
        data: vec![0; (side * side * 4) as usize],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_icon_is_argb_with_blue_core_white_bubble_and_transparent_corners() {
        let icon = icon(32);
        assert_eq!(icon.width, 32);
        assert_eq!(icon.height, 32);
        assert_eq!(icon.data.len(), 32 * 32 * 4);
        let alpha = |x: usize, y: usize| icon.data[(y * 32 + x) * 4];
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(31, 31), 0);
        assert!(alpha(16, 16) > 200);
    }
}
