//! Vision: turning a captured image into something a local model can actually see.
//!
//! The desktop tool has been able to take screenshots for a while, but the file
//! path it returned was only ever pasted into the transcript as text — nothing
//! encoded the pixels and handed them to the model, so the assistant has never
//! actually *seen* anything. This module is that missing step.
//!
//! Ollama's `/api/chat` takes images as an `images` array of base64 payloads
//! hung off a message. Two things matter before you send one:
//!
//! - **Downscale first.** A 4K screenshot is ~8.3 megapixels. Vision models tile
//!   an image into patches, so cost and latency scale with pixel count, and past
//!   a point extra pixels stop adding legibility. Anthropic's guidance for Claude
//!   is to keep the long edge around 1568 px; local VLMs are smaller and slower,
//!   so [`MAX_EDGE`] is tighter still. Downscaling is also what keeps a
//!   screenshot from blowing the context window on its own.
//! - **Keep it lossless.** These are UI screenshots — the point is reading text
//!   and finding buttons. JPEG ringing around small glyphs costs more accuracy
//!   than the bytes it saves, so the re-encode stays PNG.

use base64::Engine;
use image::imageops::FilterType;

/// Longest edge, in pixels, of an image handed to a local vision model.
///
/// 1120 is 2× the 560-px tile size that several local VLMs (Llama 3.2 Vision,
/// Qwen-VL) use, so an image lands on a whole number of tiles instead of being
/// padded. It also keeps a downscaled 4K screenshot under ~1.4 MB of base64,
/// which is a sane share of a 32k context.
pub const MAX_EDGE: u32 = 1120;

/// An image ready to hand to the model.
#[derive(Debug, Clone)]
pub struct EncodedImage {
    /// Base64 payload, no data-URL prefix — this is what Ollama's `images`
    /// array wants.
    pub base64: String,
    pub width: u32,
    pub height: u32,
    /// Size of the encoded PNG in bytes, before base64 expands it ~4/3.
    pub bytes: usize,
    /// True when the source was larger than [`MAX_EDGE`] and got scaled down.
    pub downscaled: bool,
}

impl EncodedImage {
    /// One line for the transcript, so a developer can see what the model got.
    pub fn summary(&self) -> String {
        format!(
            "{}×{} px · {:.0} KB{}",
            self.width,
            self.height,
            self.bytes as f64 / 1024.0,
            if self.downscaled { " · downscaled" } else { "" }
        )
    }
}

/// Load an image from disk, downscale it to [`MAX_EDGE`], and base64-encode it
/// as PNG.
pub fn encode_file(path: &str) -> Result<EncodedImage, String> {
    let img = image::open(path).map_err(|e| format!("Couldn't read image {path}: {e}"))?;
    encode_image(img)
}

/// Same as [`encode_file`] but for an image already in memory.
pub fn encode_image(img: image::DynamicImage) -> Result<EncodedImage, String> {
    let (w, h) = (img.width(), img.height());
    let longest = w.max(h);

    // `resize` preserves aspect ratio and fits *within* the box, so passing
    // MAX_EDGE for both dimensions is correct for portrait and landscape alike.
    // Lanczos3 costs a little more than triangle but keeps small UI text legible,
    // which is the entire point of looking at a screenshot.
    let (img, downscaled) = if longest > MAX_EDGE {
        (img.resize(MAX_EDGE, MAX_EDGE, FilterType::Lanczos3), true)
    } else {
        (img, false)
    };

    let mut png: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("Couldn't encode PNG: {e}"))?;

    Ok(EncodedImage {
        base64: base64::engine::general_purpose::STANDARD.encode(&png),
        width: img.width(),
        height: img.height(),
        bytes: png.len(),
        downscaled,
    })
}

/// Encode a screenshot to **exactly** the virtual display size.
///
/// This is the one that matters for computer use. [`encode_file`] just caps the
/// long edge, which is fine for "look at this picture" but wrong for pointing:
/// if the image the model sees is 1120 px wide while its declared coordinate
/// space is 1024, every coordinate it returns is off by 9%. The picture and the
/// coordinate space have to be the same size, so this resizes to the display's
/// exact dimensions rather than to a cap.
pub fn encode_for_display(
    path: &str,
    display: &crate::computer_use::VirtualDisplay,
) -> Result<EncodedImage, String> {
    let img = image::open(path).map_err(|e| format!("Couldn't read image {path}: {e}"))?;
    let downscaled = img.width() != display.width || img.height() != display.height;

    // `resize_exact`, not `resize`: the display size was already computed from
    // the real aspect ratio, so forcing the exact dimensions cannot distort —
    // and it guarantees the model's coordinate space matches pixel for pixel.
    let img = if downscaled {
        img.resize_exact(display.width, display.height, FilterType::Lanczos3)
    } else {
        img
    };

    let mut png: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("Couldn't encode PNG: {e}"))?;

    Ok(EncodedImage {
        base64: base64::engine::general_purpose::STANDARD.encode(&png),
        width: img.width(),
        height: img.height(),
        bytes: png.len(),
        downscaled,
    })
}

/// Build the Ollama chat message that carries an image.
///
/// Ollama wants the base64 payloads on the message itself, as `images`, next to
/// the text — not as a separate content block the way the Anthropic and OpenAI
/// APIs do it.
pub fn user_message_with_images(text: &str, images: &[EncodedImage]) -> serde_json::Value {
    let payloads: Vec<&str> = images.iter().map(|i| i.base64.as_str()).collect();
    serde_json::json!({
        "role": "user",
        "content": text,
        "images": payloads,
    })
}

/// Does this model actually accept images? Sending an image to a text-only model
/// is silently useless — Ollama drops the `images` field and the reply reads as
/// if the model looked and saw nothing, which is worse than an honest error.
pub fn is_vision_model(tag: &str) -> bool {
    let t = tag.to_ascii_lowercase();
    const VISION: &[&str] = &[
        "llava", "bakllava", "moondream", "minicpm-v", "llama3.2-vision",
        "llama-3.2-vision", "qwen2-vl", "qwen2.5-vl", "qwen3-vl", "gemma3",
        "pixtral", "granite3.2-vision", "internvl", "cogvlm",
    ];
    VISION.iter().any(|v| t.contains(v)) || t.contains("-vl") || t.contains("vision")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32) -> image::DynamicImage {
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(w, h, image::Rgb([90, 120, 200])))
    }

    #[test]
    fn oversized_images_are_downscaled_within_the_box() {
        let enc = encode_image(solid(3840, 2160)).unwrap();
        assert!(enc.downscaled);
        assert_eq!(enc.width.max(enc.height), MAX_EDGE);
        // Aspect ratio survives: 3840/2160 is 16:9, so 1120 wide -> 630 high.
        assert_eq!((enc.width, enc.height), (1120, 630));
    }

    #[test]
    fn portrait_images_clamp_on_height() {
        let enc = encode_image(solid(1000, 2000)).unwrap();
        assert!(enc.downscaled);
        assert_eq!(enc.height, MAX_EDGE);
        assert_eq!(enc.width, 560);
    }

    #[test]
    fn small_images_are_left_alone() {
        let enc = encode_image(solid(640, 480)).unwrap();
        assert!(!enc.downscaled);
        assert_eq!((enc.width, enc.height), (640, 480));
    }

    #[test]
    fn encodes_to_decodable_base64_png() {
        let enc = encode_image(solid(64, 48)).unwrap();
        let raw = base64::engine::general_purpose::STANDARD.decode(&enc.base64).unwrap();
        assert_eq!(raw.len(), enc.bytes);
        // PNG magic — proves we sent an actual PNG, not a raw buffer.
        assert_eq!(&raw[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[test]
    fn message_carries_images_alongside_the_text() {
        let enc = encode_image(solid(32, 32)).unwrap();
        let msg = user_message_with_images("what is on screen?", std::slice::from_ref(&enc));
        assert_eq!(msg["role"], "user");
        assert_eq!(msg["content"], "what is on screen?");
        assert_eq!(msg["images"].as_array().unwrap().len(), 1);
        assert_eq!(msg["images"][0], enc.base64);
    }

    #[test]
    fn the_image_the_model_sees_matches_its_coordinate_space_exactly() {
        use crate::computer_use::VirtualDisplay;
        let dir = std::env::temp_dir().join("llamachat-vision-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("screen.png");
        solid(3840, 2160).save(&path).unwrap();

        let display = VirtualDisplay::fit(3840, 2160);
        let enc = encode_for_display(path.to_str().unwrap(), &display).unwrap();

        // The whole point: picture size == declared coordinate space. If these
        // ever drift, every click the model makes is proportionally wrong.
        assert_eq!(enc.width, display.width);
        assert_eq!(enc.height, display.height);
        assert!(enc.downscaled);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn vision_models_are_recognised_and_text_models_are_not() {
        for t in ["llava:7b", "llama3.2-vision:11b", "qwen2.5-vl:7b", "moondream", "gemma3:12b", "minicpm-v"] {
            assert!(is_vision_model(t), "{t} should be a vision model");
        }
        for t in ["llama3.2:3b", "qwen3:30b-a3b", "deepseek-r1:8b", "mistral:7b", "phi3:14b"] {
            assert!(!is_vision_model(t), "{t} should not be a vision model");
        }
    }
}
