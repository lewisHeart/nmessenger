use base64::{Engine, engine::general_purpose::STANDARD};
use nm_protocol::{ClientEvent, MAX_MEDIA_FRAME_BYTES, MediaStream};
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

pub struct CaptureSpec {
    pub activity: Uuid,
    pub stream: MediaStream,
    pub input: Vec<String>,
    pub width: u32,
    pub fps: u32,
}

pub fn camera_input(device: &str) -> Vec<String> {
    if super::test_media() && device == "test:pattern" {
        return vec![
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            "testsrc=size=640x360:rate=12".into(),
        ];
    }
    #[cfg(target_os = "windows")]
    {
        let selected = if device.is_empty() || device.starts_with("/dev/video") {
            windows_cameras().into_iter().next().unwrap_or_default()
        } else {
            device.to_owned()
        };
        return vec!["-f".into(), "dshow".into(), "-i".into(), format!("video={selected}")];
    }
    #[cfg(not(target_os = "windows"))]
    vec![
        "-f".into(),
        "v4l2".into(),
        "-framerate".into(),
        "12".into(),
        "-i".into(),
        device.into(),
    ]
}

#[cfg(target_os = "windows")]
pub fn windows_cameras() -> Vec<String> {
    let output = Command::new("ffmpeg").args(["-hide_banner", "-list_devices", "true", "-f", "dshow", "-i", "dummy"])
        .stdin(Stdio::null()).stdout(Stdio::null()).output();
    let Ok(output) = output else { return Vec::new(); };
    parse_windows_cameras(&String::from_utf8_lossy(&output.stderr))
}

#[cfg(any(target_os = "windows", test))]
fn parse_windows_cameras(output: &str) -> Vec<String> {
    output.lines().filter(|line| line.contains("(video)"))
        .filter_map(|line| line.split('"').nth(1)).map(str::to_owned).collect()
}

/// Owns the child and its pipes; ending a call always terminates capture.
pub fn capture(
    spec: CaptureSpec,
    stop: Arc<AtomicBool>,
    outgoing: Sender<ClientEvent>,
    observe: impl Fn(&[u8]) + Send + 'static,
) -> Result<(), String> {
    if stop.load(Ordering::Relaxed) {
        return Ok(());
    }
    #[cfg(target_os = "android")]
    if spec.stream == MediaStream::Camera {
        return crate::android::capture_camera(stop, move |jpeg| {
            observe(jpeg);
            let _ = outgoing.try_send(ClientEvent::Media {
                activity_id: spec.activity, stream: spec.stream,
                sequence: super::next_sequence(), data: STANDARD.encode(jpeg),
            });
        });
    }
    #[cfg(target_os = "linux")]
    if spec.stream == MediaStream::Camera && !super::test_media()
        && let Some(device) = spec.input.last()
        && !std::path::Path::new(device).exists()
    {
        return Err(format!("Câmera não encontrada: {device}. Escolha um dispositivo disponível nas configurações."));
    }
    let mut child = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-threads",
            "1",
            "-filter_threads",
            "1",
        ])
        .args(&spec.input)
        .args([
            "-an",
            "-vf",
            &format!("scale={}:-2", spec.width),
            "-r",
            &spec.fps.to_string(),
            "-q:v",
            "6",
            "-f",
            "image2pipe",
            "-vcodec",
            "mjpeg",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Não foi possível iniciar a captura (ffmpeg): {error}"))?;
    let mut stdout = child.stdout.take().ok_or("Captura sem saída de vídeo")?;
    let stderr = child.stderr.take().ok_or("Captura sem diagnóstico")?;
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.take(16_384).read_to_string(&mut text);
        text
    });
    let first_frame = Arc::new(AtomicBool::new(false));
    let seen_frame = first_frame.clone();
    let reader_stop = stop.clone();
    let reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 8192];
        while !reader_stop.load(Ordering::Relaxed) {
            let Ok(count) = stdout.read(&mut chunk) else {
                break;
            };
            if count == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..count]);
            while let Some(end) = buffer.windows(2).position(|bytes| bytes == [0xff, 0xd9]) {
                let jpeg: Vec<u8> = buffer.drain(..end + 2).collect();
                if jpeg.starts_with(&[0xff, 0xd8]) && jpeg.len() <= MAX_MEDIA_FRAME_BYTES {
                    if reader_stop.load(Ordering::Relaxed) { break; }
                    seen_frame.store(true, Ordering::Relaxed);
                    observe(&jpeg);
                    let _ = outgoing.try_send(ClientEvent::Media {
                        activity_id: spec.activity,
                        stream: spec.stream,
                        sequence: super::next_sequence(),
                        data: STANDARD.encode(jpeg),
                    });
                }
            }
            if buffer.len() > MAX_MEDIA_FRAME_BYTES * 2 {
                buffer.clear();
            }
        }
    });
    let started = Instant::now();
    let result = loop {
        if stop.load(Ordering::Relaxed) {
            break Ok(());
        }
        if started.elapsed() > Duration::from_secs(12) && !first_frame.load(Ordering::Relaxed) {
            break Err("A câmera não produziu imagens em 12 segundos. Verifique o dispositivo selecionado.".to_owned());
        }
        match child.try_wait() {
            Ok(Some(status)) => break Err(format!("A captura de vídeo foi encerrada ({status}).")),
            Ok(None) => std::thread::sleep(Duration::from_millis(30)),
            Err(error) => break Err(error.to_string()),
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    let detail = errors.join().unwrap_or_default();
    result.map_err(|error| {
        if detail.trim().is_empty() {
            error
        } else {
            format!(
                "{error} {}",
                detail.lines().take(3).collect::<Vec<_>>().join(" ")
            )
        }
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn missing_camera_is_reported_before_starting_ffmpeg() {
        if super::super::test_media() { return; }
        let path = format!("/dev/nmessenger-test-camera-{}", Uuid::new_v4());
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        let result = capture(CaptureSpec {
            activity: Uuid::new_v4(), stream: MediaStream::Camera,
            input: camera_input(&path), width: 640, fps: 12,
        }, Arc::new(AtomicBool::new(false)), sender, |_| {});
        assert!(result.unwrap_err().contains(&path));
    }

    #[test]
    fn windows_device_listing_selects_video_names_only() {
        let output = "[dshow] \"Webcam HD\" (video)\n[dshow] Alternative name \"@device_123\"\n[dshow] \"Microphone\" (audio)";
        assert_eq!(parse_windows_cameras(output), vec!["Webcam HD"]);
    }
}
