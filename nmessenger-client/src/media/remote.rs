//! Screen sharing and opt-in X11 remote input for an accepted activity.
use nm_protocol::{ClientEvent, MediaStream};
use serde::{Deserialize, Serialize};
use std::{
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

pub fn available() -> bool {
    cfg!(any(target_os = "windows", target_os = "android")) || control_available()
}

pub fn control_available() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_some()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Input {
    Click { x: f32, y: f32, button: u8 },
    Scroll { up: bool },
    Text { text: String },
    Key { key: Key },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    Enter,
    Backspace,
    Tab,
    Escape,
    Left,
    Right,
    Up,
    Down,
    Delete,
}

fn dimensions() -> Result<(u32, u32), String> {
    let result = Command::new("xdotool")
        .arg("getdisplaygeometry")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("xdotool: {e}"))?;
    if !result.status.success() {
        return Err("Não foi possível consultar o display X11.".into());
    }
    let text = String::from_utf8(result.stdout).map_err(|e| e.to_string())?;
    let numbers: Vec<u32> = text
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    match numbers.as_slice() {
        [w, h] if *w > 0 && *h > 0 && *w <= 32_768 && *h <= 32_768 => Ok((*w, *h)),
        _ => Err("Dimensões do display inválidas.".into()),
    }
}

pub fn capture(
    id: Uuid,
    stop: Arc<AtomicBool>,
    outgoing: Sender<ClientEvent>,
    observe: impl Fn(&[u8]) + Send + 'static,
) -> Result<(), String> {
    if !available() {
        return Err("Compartilhamento de tela indisponível nesta plataforma ou sessão gráfica.".into());
    }
    #[cfg(target_os = "android")]
    return crate::android::capture_screen(stop, move |jpeg| {
        use base64::{Engine, engine::general_purpose::STANDARD};
        observe(jpeg);
        let _ = outgoing.try_send(ClientEvent::Media {
            activity_id: id, stream: MediaStream::Screen,
            sequence: super::next_sequence(), data: STANDARD.encode(jpeg),
        });
    });
    #[cfg(target_os = "linux")]
    let input = {
        let display = std::env::var("DISPLAY").map_err(|e| e.to_string())?;
        let (w, h) = dimensions()?;
        vec!["-f".into(), "x11grab".into(), "-framerate".into(), "5".into(),
             "-video_size".into(), format!("{w}x{h}"), "-i".into(), display]
    };
    #[cfg(target_os = "windows")]
    let input = vec!["-f".into(), "gdigrab".into(), "-framerate".into(), "5".into(), "-i".into(), "desktop".into()];
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "android")))]
    let input: Vec<String> = Vec::new();
    #[cfg(not(target_os = "android"))]
    super::video::capture(
        super::video::CaptureSpec {
            activity: id,
            stream: MediaStream::Screen,
            input,
            width: 640,
            fps: 5,
        },
        stop,
        outgoing,
        observe,
    )
}

fn arguments(input: Input, size: (u32, u32)) -> Result<Vec<String>, String> {
    Ok(match input {
        Input::Click { x, y, button } => {
            if !x.is_finite()
                || !y.is_finite()
                || !(0.0..=1.0).contains(&x)
                || !(0.0..=1.0).contains(&y)
                || !(1..=3).contains(&button)
            {
                return Err("Entrada remota inválida.".into());
            }
            vec![
                "mousemove".into(),
                ((size.0 - 1) as f32 * x).round().to_string(),
                ((size.1 - 1) as f32 * y).round().to_string(),
                "click".into(),
                button.to_string(),
            ]
        }
        Input::Scroll { up } => vec!["click".into(), if up { "4" } else { "5" }.into()],
        Input::Text { text } => {
            if text.is_empty() || text.chars().count() > 256 || text.chars().any(char::is_control) {
                return Err("Texto remoto deve ter 1–256 caracteres sem controles.".into());
            }
            vec![
                "type".into(),
                "--clearmodifiers".into(),
                "--delay".into(),
                "0".into(),
                "--".into(),
                text,
            ]
        }
        Input::Key { key } => vec![
            "key".into(),
            "--clearmodifiers".into(),
            match key {
                Key::Enter => "Return",
                Key::Backspace => "BackSpace",
                Key::Tab => "Tab",
                Key::Escape => "Escape",
                Key::Left => "Left",
                Key::Right => "Right",
                Key::Up => "Up",
                Key::Down => "Down",
                Key::Delete => "Delete",
            }
            .into(),
        ],
    })
}

pub fn execute(
    input: Input,
    permission: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    if !control_available() { return Err("Remote control requires Linux/X11.".into()); }
    if !permission.load(Ordering::Relaxed) || stop.load(Ordering::Relaxed) {
        return Ok(());
    }
    let args = arguments(input, dimensions()?)?;
    if !permission.load(Ordering::Relaxed) || stop.load(Ordering::Relaxed) {
        return Ok(());
    }
    let status = Command::new("xdotool")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Falha ao aplicar entrada remota.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_input_rejects_unbounded_coordinates_and_control_characters() {
        assert!(
            arguments(
                Input::Click {
                    x: f32::NAN,
                    y: 0.5,
                    button: 1
                },
                (800, 600)
            )
            .is_err()
        );
        assert!(
            arguments(
                Input::Click {
                    x: 1.2,
                    y: 0.5,
                    button: 1
                },
                (800, 600)
            )
            .is_err()
        );
        assert!(
            arguments(
                Input::Text {
                    text: "x\ny".into()
                },
                (800, 600)
            )
            .is_err()
        );
        assert_eq!(
            arguments(
                Input::Click {
                    x: 1.0,
                    y: 1.0,
                    button: 1
                },
                (800, 600)
            )
            .unwrap(),
            ["mousemove", "799", "599", "click", "1"]
        );
        assert_eq!(
            arguments(
                Input::Text {
                    text: "--file=anything".into()
                },
                (800, 600)
            )
            .unwrap()
            .last()
            .unwrap(),
            "--file=anything"
        );
    }

    #[test]
    #[ignore = "requires NM_SMOKE_XVFB=1, an isolated Xvfb, ffmpeg and xdotool"]
    fn x11_capture_and_revocable_remote_input() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        assert_eq!(std::env::var("NM_SMOKE_XVFB").as_deref(), Ok("1"));
        let size = dimensions().unwrap();
        let permission = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        execute(
            Input::Click {
                x: 0.25,
                y: 0.25,
                button: 1,
            },
            permission.clone(),
            stop.clone(),
        )
        .unwrap();
        let location = || {
            Command::new("xdotool")
                .args(["getmouselocation", "--shell"])
                .output()
                .unwrap()
                .stdout
        };
        let before = location();
        assert!(
            String::from_utf8_lossy(&before)
                .contains(&format!("X={}", ((size.0 - 1) as f32 * 0.25).round()))
        );
        permission.store(false, Ordering::Relaxed);
        execute(
            Input::Click {
                x: 0.75,
                y: 0.75,
                button: 1,
            },
            permission,
            stop.clone(),
        )
        .unwrap();
        assert_eq!(location(), before);
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let capture_stop = stop.clone();
        let id = Uuid::new_v4();
        let worker = std::thread::spawn(move || capture(id, capture_stop, tx, |_| {}));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let frame = runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv()).await
        });
        stop.store(true, Ordering::Relaxed);
        worker.join().unwrap().unwrap();
        let ClientEvent::Media {
            activity_id,
            stream,
            data,
            ..
        } = frame.unwrap().unwrap()
        else {
            panic!("expected screen frame");
        };
        assert_eq!(activity_id, id);
        assert_eq!(stream, MediaStream::Screen);
        let bytes = STANDARD.decode(data).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), 640);
        assert!(decoded.height() > 0);
    }
}
