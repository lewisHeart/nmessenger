//! Frame-by-frame playback of bundled, high-resolution animated artwork.
use eframe::egui::{self, Color32, Rect};
use image::AnimationDecoder;
use std::{
    collections::VecDeque,
    io::Cursor,
    sync::{Arc, Mutex, mpsc},
};

pub struct Clip {
    pub kind: &'static str,
    pub name: &'static str,
    pub preview: &'static str,
    bytes: &'static [u8],
}

pub const CLIPS: &[Clip] = &[
    Clip {
        kind: "kiss",
        name: "Beijo",
        preview: "emojis/face-blowing-a-kiss.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/face-blowing-a-kiss.png"),
    },
    Clip {
        kind: "heart",
        name: "Coração",
        preview: "emojis/growing-heart.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/growing-heart.png"),
    },
    Clip {
        kind: "party",
        name: "Festa",
        preview: "emojis/party-popper.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/party-popper.png"),
    },
    Clip {
        kind: "rocket",
        name: "Foguete",
        preview: "emojis/rocket.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/rocket.png"),
    },
    Clip {
        kind: "applause",
        name: "Aplausos",
        preview: "emojis/clapping-hands.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/clapping-hands.png"),
    },
    Clip {
        kind: "laugh",
        name: "Risada",
        preview: "emojis/rolling-on-the-floor-laughing.png",
        bytes: include_bytes!(
            "../../assets/nmessenger/fluent_anim/rolling-on-the-floor-laughing.png"
        ),
    },
    Clip {
        kind: "mindblown",
        name: "Explodindo",
        preview: "emojis/exploding-head.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/exploding-head.png"),
    },
    Clip {
        kind: "impact",
        name: "Bomba",
        preview: "emojis/bomb.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/bomb.png"),
    },
    Clip {
        kind: "pig",
        name: "Porquinho",
        preview: "emojis/pig-face.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/pig-face.png"),
    },
    Clip {
        kind: "vomit",
        name: "Vômito",
        preview: "emojis/face-vomiting.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/face-vomiting.png"),
    },
    Clip {
        kind: "nauseated",
        name: "Enjoado",
        preview: "emojis/nauseated-face.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/nauseated-face.png"),
    },
    Clip {
        kind: "woozy",
        name: "Tonto",
        preview: "emojis/woozy-face.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/woozy-face.png"),
    },
    Clip {
        kind: "angry",
        name: "Furioso",
        preview: "emojis/angry-face.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/angry-face.png"),
    },
    Clip {
        kind: "skull",
        name: "Caveira",
        preview: "emojis/skull.png",
        bytes: include_bytes!("../../assets/nmessenger/fluent_anim/skull.png"),
    },
];

/// Only current choices are shown; retired IDs still play in existing messages.
pub const PICKER: &[&str] = &[
    "kiss", "impact", "applause", "vomit", "nauseated", "woozy", "angry", "skull", "laugh",
];

pub fn clip(kind: &str) -> Option<&'static Clip> {
    // Continue playing messages sent by older clients after retiring their static effects.
    let kind = match kind {
        "hammer" => "impact",
        other => other,
    };
    CLIPS.iter().find(|clip| clip.kind == kind)
}

pub struct Frame {
    pub image: egui::ColorImage,
    pub delay_ms: u64,
}

pub struct Movie {
    pub frames: Vec<Frame>,
    pub duration_ms: u64,
}

impl Movie {
    pub fn frame_at(&self, milliseconds: u64) -> usize {
        let mut end = 0;
        for (index, frame) in self.frames.iter().enumerate() {
            end += frame.delay_ms;
            if milliseconds < end {
                return index;
            }
        }
        self.frames.len() - 1
    }

    pub fn frame_end_ms(&self, index: usize) -> u64 {
        self.frames[..=index]
            .iter()
            .map(|frame| frame.delay_ms)
            .sum()
    }

    pub fn playback_duration_ms(&self) -> u64 {
        self.duration_ms * 3500_u64.div_ceil(self.duration_ms).clamp(1, 8)
    }
}

fn decode(bytes: &[u8]) -> Result<Movie, String> {
    let decoder = image::codecs::png::PngDecoder::new(Cursor::new(bytes))
        .map_err(|error| error.to_string())?
        .apng()
        .map_err(|error| error.to_string())?;
    let mut frames = Vec::new();
    let mut duration_ms = 0;
    for frame in decoder.into_frames() {
        let frame = frame.map_err(|error| error.to_string())?;
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let delay_ms = u64::from(numerator / denominator.max(1)).max(10);
        let buffer = frame.into_buffer();
        // Preserve all original Fluent frames; the thumbnail decoder has a different memory budget.
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [buffer.width() as usize, buffer.height() as usize],
            buffer.as_raw(),
        );
        frames.push(Frame { image, delay_ms });
        duration_ms += delay_ms;
    }
    if frames.len() < 2 || duration_ms == 0 {
        return Err("Piscadela sem animação.".into());
    }
    Ok(Movie {
        frames,
        duration_ms,
    })
}

enum State {
    Loading(mpsc::Receiver<Result<Arc<Movie>, String>>),
    Ready(Arc<Movie>),
    Failed,
}

struct Entry {
    kind: &'static str,
    state: State,
    textures: [Option<(egui::TextureHandle, usize)>; 2],
}

#[derive(Default)]
struct Cache {
    entries: VecDeque<Entry>,
}

fn cache(ctx: &egui::Context) -> Arc<Mutex<Cache>> {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Arc<Mutex<Cache>>>(egui::Id::unique("wink-movie-cache"))
            .clone()
    })
}

pub fn load(ctx: &egui::Context, clip: &'static Clip) -> Option<Arc<Movie>> {
    let cache = cache(ctx);
    let mut cache = cache.lock().ok()?;
    if let Some(index) = cache
        .entries
        .iter()
        .position(|entry| entry.kind == clip.kind)
    {
        let mut entry = cache.entries.remove(index)?;
        if let State::Loading(receiver) = &entry.state {
            match receiver.try_recv() {
                Ok(Ok(movie)) => entry.state = State::Ready(movie),
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => entry.state = State::Failed,
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(16));
                }
            }
        }
        let movie = match &entry.state {
            State::Ready(movie) => Some(movie.clone()),
            _ => None,
        };
        cache.entries.push_back(entry);
        return movie;
    }
    // Allow simultaneous conversations and a hovered preview without keeping the whole pack decoded.
    while cache.entries.len() >= 4 {
        cache.entries.pop_front();
    }
    let (sender, receiver) = mpsc::channel();
    cache.entries.push_back(Entry {
        kind: clip.kind,
        state: State::Loading(receiver),
        textures: [None, None],
    });
    let viewport = ctx.viewport_id();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = sender.send(decode(clip.bytes).map(Arc::new));
        // Detached conversations have their own viewport; repaint that window without user input.
        ctx.request_repaint_of(viewport);
    });
    None
}

#[allow(clippy::too_many_arguments)]
pub fn paint(
    ctx: &egui::Context,
    painter: &egui::Painter,
    clip: &Clip,
    movie: &Movie,
    index: usize,
    rect: Rect,
    alpha: f32,
    preview: bool,
) {
    let cache = cache(ctx);
    let Ok(mut cache) = cache.lock() else {
        return;
    };
    let Some(entry) = cache
        .entries
        .iter_mut()
        .find(|entry| entry.kind == clip.kind)
    else {
        return;
    };
    let image = &movie.frames[index].image;
    // A hovered thumbnail and the conversation may show different frames of the same clip.
    let texture = match &mut entry.textures[usize::from(preview)] {
        Some((texture, current)) => {
            if *current != index {
                texture.set(image.clone(), egui::TextureOptions::LINEAR);
                *current = index;
            }
            texture
        }
        slot @ None => {
            &slot
                .insert((
                    ctx.load_texture(
                        format!("wink-{}", clip.kind),
                        image.clone(),
                        egui::TextureOptions::LINEAR,
                    ),
                    index,
                ))
                .0
        }
    };
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::from_white_alpha((alpha.clamp(0.0, 1.0) * 255.0) as u8),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_movies_have_real_changing_frames_at_native_resolution() {
        for clip in CLIPS {
            let animation_header = clip
                .bytes
                .windows(4)
                .position(|bytes| bytes == b"acTL")
                .unwrap();
            let count = u32::from_be_bytes(
                clip.bytes[animation_header + 4..animation_header + 8]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let movie = decode(clip.bytes).unwrap();
            let duration = movie.duration_ms;
            assert_eq!(movie.frames.len(), count, "truncated movie: {}", clip.kind);
            assert!(count > 10, "not a full animation: {}", clip.kind);
            assert!(
                movie
                    .frames
                    .iter()
                    .all(|frame| frame.image.size == [256, 256])
            );
            let changed = movie
                .frames
                .windows(2)
                .filter(|pair| pair[0].image.pixels != pair[1].image.pixels)
                .count();
            assert!(changed > count / 2, "static artwork: {}", clip.kind);
            assert!(
                movie.frames.iter().any(|frame| frame
                    .image
                    .pixels
                    .iter()
                    .any(|pixel| pixel.a() == 0)),
                "missing transparency: {}",
                clip.kind
            );
            assert_eq!(movie.frame_at(0), 0);
            assert_eq!(movie.frame_at(movie.frames[0].delay_ms), 1);
            assert_eq!(movie.frame_at(duration - 1), count - 1);
            assert_eq!(movie.playback_duration_ms() % duration, 0);
            assert!(movie.playback_duration_ms() >= 3500 || duration * 8 < 3500);
        }
    }

    #[test]
    fn old_message_ids_still_resolve_and_unknown_ids_do_not_start_an_effect() {
        assert_eq!(clip("hammer").unwrap().kind, "impact");
        assert_eq!(clip("pig").unwrap().kind, "pig");
        assert!(clip("unknown").is_none());
        assert!(clip("rocket").is_some(), "old messages must still play");
        assert!(!PICKER.contains(&"rocket"));
        assert!(PICKER.contains(&"vomit"));
        assert!(PICKER.iter().all(|kind| clip(kind).is_some()));
    }

    #[test]
    fn player_updates_texture_pixels_and_stops_after_complete_cycles() {
        let ctx = egui::Context::default();
        let clip = clip("applause").unwrap();
        let movie = Arc::new(decode(clip.bytes).unwrap());
        let duration = movie.playback_duration_ms() as f64 / 1000.0;
        let next_frame = movie.frames[0].delay_ms as f64 / 1000.0 + 0.001;
        cache(&ctx).lock().unwrap().entries.push_back(Entry {
            kind: clip.kind,
            state: State::Ready(movie),
            textures: [None, None],
        });
        let mut first_frame = None;
        for seconds in [0.0, next_frame, duration] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(seconds),
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 480.0),
                    )),
                    ..Default::default()
                },
                |ui| super::super::effect(ui, ui.max_rect(), clip.kind, seconds as f32),
            );
            let uploaded = output
                .textures_delta
                .set
                .values()
                .flat_map(|deltas| deltas.iter())
                .find_map(|delta| {
                    let egui::ImageData::Color(image) = &delta.image;
                    (image.size == [256, 256]).then(|| image.pixels.clone())
                });
            output.textures_delta.clear();
            if seconds == 0.0 {
                first_frame = Some(uploaded.expect("first frame was not uploaded"));
            } else if seconds == next_frame {
                assert_ne!(
                    uploaded.as_ref(),
                    first_frame.as_ref(),
                    "player only moved the original image instead of changing its pixels"
                );
                assert!(uploaded.is_some(), "second frame was not uploaded");
                let largest_textured_width = ctx
                    .tessellate(output.shapes, output.pixels_per_point)
                    .into_iter()
                    .filter_map(|primitive| match primitive.primitive {
                        egui::epaint::Primitive::Mesh(mesh)
                            if mesh.texture_id != egui::TextureId::default() =>
                        {
                            let left = mesh
                                .vertices
                                .iter()
                                .map(|vertex| vertex.pos.x)
                                .fold(f32::INFINITY, f32::min);
                            let right = mesh
                                .vertices
                                .iter()
                                .map(|vertex| vertex.pos.x)
                                .fold(f32::NEG_INFINITY, f32::max);
                            Some(right - left)
                        }
                        _ => None,
                    })
                    .fold(0.0, f32::max);
                assert!(
                    (200.0..=320.0).contains(&largest_textured_width),
                    "wink should be visible without covering the conversation: {largest_textured_width}"
                );
            } else if seconds == duration {
                assert!(
                    output.shapes.is_empty(),
                    "finished effect still draws over the conversation"
                );
            }
        }
    }
}
