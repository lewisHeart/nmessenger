use eframe::egui::{self, ColorImage, TextureHandle};
use image::{AnimationDecoder, ImageReader};
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    io::Cursor,
    path::PathBuf,
    time::Instant,
};

#[derive(Clone, Hash, PartialEq, Eq)]
pub enum ImageSource {
    File(uuid::Uuid),
    Local(PathBuf),
    Bundled(&'static str),
}

pub struct DecodedFrame {
    pub image: ColorImage,
    pub delay_ms: u64,
}

#[allow(dead_code)]
pub struct LocalAsset {
    pub path: PathBuf,
    pub name: String,
    pub avatar: bool,
}

struct Animation {
    frames: Vec<(TextureHandle, u64)>,
    started: Instant,
    last_used: Cell<Instant>,
    bytes: usize,
}

#[derive(Default)]
pub struct ImageCache {
    images: HashMap<ImageSource, Animation>,
    pub pending: HashSet<ImageSource>,
    pub failed: HashSet<ImageSource>,
}

impl ImageCache {
    pub fn insert(&mut self, ctx: &egui::Context, key: ImageSource, frames: Vec<DecodedFrame>) {
        self.pending.remove(&key);
        self.failed.remove(&key);
        self.images.remove(&key);
        let bytes = frames
            .iter()
            .map(|f| f.image.pixels.len() * 4)
            .sum::<usize>();
        while self.images.len() >= 256
            || self.images.values().map(|a| a.bytes).sum::<usize>() + bytes > 96 * 1024 * 1024
        {
            let oldest = self
                .images
                .iter()
                .min_by_key(|(_, a)| a.last_used.get())
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                self.images.remove(&oldest);
            } else {
                break;
            }
        }
        let frames = frames
            .into_iter()
            .enumerate()
            .map(|(i, frame)| {
                (
                    ctx.load_texture(
                        format!("nm-image-{i}"),
                        frame.image,
                        egui::TextureOptions::LINEAR,
                    ),
                    frame.delay_ms.max(20),
                )
            })
            .collect();
        self.images.insert(
            key,
            Animation {
                frames,
                started: Instant::now(),
                last_used: Cell::new(Instant::now()),
                bytes,
            },
        );
    }

    pub fn texture(
        &self,
        ctx: &egui::Context,
        key: &ImageSource,
    ) -> Option<(egui::TextureId, egui::Vec2)> {
        let animation = self.images.get(key)?;
        animation.last_used.set(Instant::now());
        let total: u64 = animation.frames.iter().map(|(_, d)| *d).sum();
        if total == 0 {
            return None;
        }
        let mut position = (animation.started.elapsed().as_millis() % total as u128) as u64;
        for (texture, duration) in &animation.frames {
            if position < *duration {
                if animation.frames.len() > 1 {
                    ctx.request_repaint_after(std::time::Duration::from_millis(
                        duration - position,
                    ));
                }
                return Some((texture.id(), texture.size_vec2()));
            }
            position -= *duration;
        }
        None
    }
}

pub fn decode(bytes: &[u8]) -> Result<Vec<DecodedFrame>, String> {
    decode_sized(bytes, None)
}

pub fn decode_sized(bytes: &[u8], maximum: Option<u32>) -> Result<Vec<DecodedFrame>, String> {
    if bytes.len() > nm_protocol::MAX_UPLOAD_BYTES {
        return Err("Imagem excede 25 MiB.".into());
    }
    let maximum = maximum.map(|side| side.max(1));
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let dimensions = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_dimensions()
        .map_err(|e| e.to_string())?;
    if dimensions.0 as u64 * dimensions.1 as u64 > 16_000_000 {
        return Err("Imagem excede 16 megapixels.".into());
    }
    let animated = match reader.format() {
        Some(image::ImageFormat::Gif) => Some(
            image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
                .map_err(|e| e.to_string())?
                .into_frames(),
        ),
        Some(image::ImageFormat::Png) => {
            let decoder = image::codecs::png::PngDecoder::new(Cursor::new(bytes))
                .map_err(|e| e.to_string())?;
            if decoder.is_apng().map_err(|e| e.to_string())? {
                Some(decoder.apng().map_err(|e| e.to_string())?.into_frames())
            } else {
                None
            }
        }
        _ => None,
    };
    if let Some(animated) = animated {
        let mut frames = Vec::new();
        let mut pixels = 0usize;
        for frame in animated.take(360) {
            let frame = frame.map_err(|e| e.to_string())?;
            let (numerator, denominator) = frame.delay().numer_denom_ms();
            let mut buffer = frame.into_buffer();
            if let Some(side) = maximum {
                buffer = image::DynamicImage::ImageRgba8(buffer)
                    .thumbnail(side, side)
                    .to_rgba8();
            }
            pixels += buffer.width() as usize * buffer.height() as usize;
            if pixels > 8_000_000 {
                break;
            }
            frames.push(DecodedFrame {
                image: ColorImage::from_rgba_unmultiplied(
                    [buffer.width() as usize, buffer.height() as usize],
                    buffer.as_raw(),
                ),
                delay_ms: u64::from(numerator / denominator.max(1)).max(20),
            });
        }
        if frames.is_empty() {
            return Err("Animação sem quadros decodificáveis.".into());
        }
        Ok(frames)
    } else {
        let image = reader.decode().map_err(|e| e.to_string())?;
        let image = match maximum {
            Some(side) => image.thumbnail(side, side),
            None => image,
        }.to_rgba8();
        Ok(vec![DecodedFrame {
            image: ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                image.as_raw(),
            ),
            delay_ms: 100,
        }])
    }
}

pub fn discover(directory: Option<PathBuf>) -> Result<Vec<LocalAsset>, String> {
    let root = directory
        .or_else(|| std::env::var_os("NM_CONTENT_DIR").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("escargot/analysis/resources/mct"));
    if !root.exists() {
        return Ok(Vec::new());
    }
    fn visit(
        path: &std::path::Path,
        depth: usize,
        out: &mut Vec<LocalAsset>,
    ) -> Result<(), String> {
        if depth > 5 || out.len() >= 180 {
            return Ok(());
        }
        for entry in std::fs::read_dir(path).map_err(|e| format!("Conteúdo local: {e}"))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            let path = entry.path();
            if kind.is_dir() {
                visit(&path, depth + 1, out)?;
            } else if kind.is_file() {
                let lower = path.to_string_lossy().to_lowercase();
                let ext = path
                    .extension()
                    .and_then(|x| x.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ["png", "jpg", "jpeg", "gif"].contains(&ext.as_str())
                    && !lower.contains("wink")
                    && !lower.contains(".mct.contents")
                    && out.len() < 180
                {
                    out.push(LocalAsset {
                        name: path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .replace('_', " "),
                        avatar: lower.contains("usertile"),
                        path,
                    });
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(&root, 0, &mut out)?;
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_images_respect_requested_texture_size() {
        let image = image::DynamicImage::new_rgba8(400, 200);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let frames = decode_sized(bytes.get_ref(), Some(40)).unwrap();
        assert_eq!(frames[0].image.size, [40, 20]);
        assert_eq!(decode(bytes.get_ref()).unwrap()[0].image.size, [400, 200]);
    }

    #[test]
    fn fluent_apng_decodes_as_a_full_animation_for_inline_emoticons() {
        let bytes = crate::resources::bytes("fluent_anim/clapping-hands.png").unwrap();
        let decoded = decode_sized(bytes, Some(48)).unwrap();
        assert_eq!(decoded.len(), 28);
        assert!(decoded.iter().all(|frame| frame.image.size == [48, 48]));
        assert!(
            decoded
                .windows(2)
                .any(|pair| pair[0].image.pixels != pair[1].image.pixels)
        );
        assert!(decoded.iter().all(|frame| frame.delay_ms >= 20));
    }

    #[test]
    fn gif_decoder_preserves_multiple_frames_and_frame_delays() {
        let red = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        let green = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 255, 0, 255]));
        let frames = vec![
            image::Frame::from_parts(red, 0, 0, image::Delay::from_numer_denom_ms(100, 1)),
            image::Frame::from_parts(green, 0, 0, image::Delay::from_numer_denom_ms(200, 1)),
        ];
        let mut bytes = Vec::new();
        image::codecs::gif::GifEncoder::new(&mut bytes)
            .encode_frames(frames)
            .unwrap();
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].delay_ms, 100);
        assert_eq!(decoded[1].delay_ms, 200);
        assert_eq!(decoded[0].image.pixels[0], egui::Color32::RED);
        assert_eq!(decoded[1].image.pixels[0], egui::Color32::GREEN);
        assert!(decode(b"not an image").is_err());
    }
}
