//! Texture images (Basic Materials part 2, brief 28). An image is kept
//! once in the document, named by its content (the start of its SHA-256),
//! as the PNG or JPEG bytes it was loaded from; the file stores those bytes
//! as they are. Decoding is pure: the same bytes give the same pixels.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// An image's name: the first 16 bytes of its SHA-256, in hex.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ImageId(pub String);

/// The formats Prism reads (brief 28: PNG and JPEG).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageKind {
    Png,
    Jpeg,
}

impl ImageKind {
    /// The file extension for it.
    pub fn extension(self) -> &'static str {
        match self {
            ImageKind::Png => "png",
            ImageKind::Jpeg => "jpg",
        }
    }
}

/// Said when bytes are not a PNG or JPEG Prism can read.
pub const NOT_AN_IMAGE: &str = "That is not a PNG or JPEG image Prism can read.";

/// The largest side kept for the viewport (WebGL2's limit); the file keeps
/// the original.
pub const VIEWPORT_MAX: u32 = 2048;

/// An image as loaded: its bytes, format and size in pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Image {
    pub kind: ImageKind,
    pub width: u32,
    pub height: u32,
    pub bytes: Arc<Vec<u8>>,
}

impl Image {
    /// Read PNG or JPEG bytes (the size is checked by decoding them).
    pub fn read(bytes: Vec<u8>) -> Result<Image, &'static str> {
        let kind = match image::guess_format(&bytes) {
            Ok(image::ImageFormat::Png) => ImageKind::Png,
            Ok(image::ImageFormat::Jpeg) => ImageKind::Jpeg,
            _ => return Err(NOT_AN_IMAGE),
        };
        let img = decode(&bytes, kind).ok_or(NOT_AN_IMAGE)?;
        if img.width() == 0 || img.height() == 0 {
            return Err(NOT_AN_IMAGE);
        }
        Ok(Image {
            kind,
            width: img.width(),
            height: img.height(),
            bytes: Arc::new(bytes),
        })
    }

    /// Its name: the same bytes always give the same id.
    pub fn id(&self) -> ImageId {
        id_of(&self.bytes)
    }

    /// Its pixels as RGBA, at most `max` pixels on a side (downscaled,
    /// keeping its shape). None if the bytes no longer decode.
    pub fn rgba(&self, max: u32) -> Option<(u32, u32, Vec<u8>)> {
        let mut img = decode(&self.bytes, self.kind)?;
        if img.width() > max || img.height() > max {
            img = img.resize(max, max, image::imageops::FilterType::Triangle);
        }
        let rgba = img.to_rgba8();
        Some((rgba.width(), rgba.height(), rgba.into_raw()))
    }

    /// Its average colour, sRGB with alpha (what the Shaded style without
    /// textures draws).
    pub fn average(&self) -> Option<[u8; 4]> {
        let (_, _, px) = self.rgba(64)?;
        let n = (px.len() / 4).max(1) as u64;
        let mut sum = [0u64; 4];
        for p in px.as_chunks::<4>().0 {
            for (s, c) in sum.iter_mut().zip(p) {
                *s += u64::from(*c);
            }
        }
        Some(sum.map(|s| u8::try_from(s / n).unwrap_or(255)))
    }
}

/// The id bytes would have.
pub fn id_of(bytes: &[u8]) -> ImageId {
    let hash = Sha256::digest(bytes);
    ImageId(hash[..16].iter().map(|b| format!("{b:02x}")).collect())
}

fn decode(bytes: &[u8], kind: ImageKind) -> Option<image::DynamicImage> {
    let format = match kind {
        ImageKind::Png => image::ImageFormat::Png,
        ImageKind::Jpeg => image::ImageFormat::Jpeg,
    };
    image::load_from_memory_with_format(bytes, format).ok()
}

/// A material's base-colour texture: the image, and how big one copy of it
/// is in the model (metres, width along u and height along v). It repeats
/// to fill each face.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Texture {
    pub image: ImageId,
    pub size: [f64; 2],
}

impl Texture {
    /// A texture one metre wide, its height keeping the image's shape.
    pub fn of(image: &Image) -> Texture {
        Texture {
            image: image.id(),
            size: [1.0, f64::from(image.height) / f64::from(image.width)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny PNG made in memory.
    pub(crate) fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba(rgba));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png)
            .expect("png");
        out.into_inner()
    }

    #[test]
    fn an_image_reads_its_size_and_is_named_by_its_bytes() {
        let bytes = png(4, 2, [200, 100, 50, 255]);
        let a = Image::read(bytes.clone()).expect("a png");
        assert_eq!((a.kind, a.width, a.height), (ImageKind::Png, 4, 2));
        assert_eq!(a.id(), Image::read(bytes).unwrap().id());
        assert_ne!(a.id(), Image::read(png(4, 2, [0, 0, 0, 255])).unwrap().id());
        assert_eq!(a.id().0.len(), 32);
        assert_eq!(a.average(), Some([200, 100, 50, 255]));
        assert_eq!(Texture::of(&a).size, [1.0, 0.5]);
        assert_eq!(Image::read(b"not an image".to_vec()), Err(NOT_AN_IMAGE));
    }

    #[test]
    fn a_big_image_is_downscaled_for_the_viewport() {
        let a = Image::read(png(3000, 1500, [1, 2, 3, 255])).unwrap();
        let (w, h, px) = a.rgba(VIEWPORT_MAX).unwrap();
        assert_eq!((w, h), (2048, 1024));
        assert_eq!(px.len(), (w * h * 4) as usize);
        assert_eq!((a.width, a.height), (3000, 1500), "the original is kept");
    }
}
