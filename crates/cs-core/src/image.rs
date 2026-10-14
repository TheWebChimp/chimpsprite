use serde::{Deserialize, Serialize};

use crate::{ColorMode, PixelValue, Rgba};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelBuffer {
    Rgba(Vec<Rgba>),
    Indexed(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: PixelBuffer,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    #[error("pixel ({x}, {y}) out of bounds")]
    OutOfBounds { x: u32, y: u32 },
    #[error("pixel value does not match the image color mode")]
    ModeMismatch,
}

impl Image {
    /// Imagen llena de píxeles transparentes.
    pub fn new(width: u32, height: u32, mode: ColorMode) -> Self {
        let n = (width * height) as usize;
        let pixels = match mode.transparent() {
            PixelValue::Rgba(c) => PixelBuffer::Rgba(vec![c; n]),
            PixelValue::Index(i) => PixelBuffer::Indexed(vec![i; n]),
        };
        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &PixelBuffer {
        &self.pixels
    }

    fn offset(&self, x: u32, y: u32) -> Option<usize> {
        (x < self.width && y < self.height).then(|| (y * self.width + x) as usize)
    }

    pub fn get(&self, x: u32, y: u32) -> Option<PixelValue> {
        let i = self.offset(x, y)?;
        Some(match &self.pixels {
            PixelBuffer::Rgba(p) => PixelValue::Rgba(p[i]),
            PixelBuffer::Indexed(p) => PixelValue::Index(p[i]),
        })
    }

    pub fn set(&mut self, x: u32, y: u32, value: PixelValue) -> Result<(), ImageError> {
        let i = self.offset(x, y).ok_or(ImageError::OutOfBounds { x, y })?;
        match (&mut self.pixels, value) {
            (PixelBuffer::Rgba(p), PixelValue::Rgba(c)) => p[i] = c,
            (PixelBuffer::Indexed(p), PixelValue::Index(c)) => p[i] = c,
            _ => return Err(ImageError::ModeMismatch),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rgba_image_is_transparent() {
        let img = Image::new(2, 2, ColorMode::Rgba);
        assert_eq!(img.get(1, 1), Some(PixelValue::Rgba(Rgba::TRANSPARENT)));
    }

    #[test]
    fn new_indexed_image_uses_transparent_index() {
        let img = Image::new(
            2,
            2,
            ColorMode::Indexed {
                transparent_index: 3,
            },
        );
        assert_eq!(img.get(0, 0), Some(PixelValue::Index(3)));
    }

    #[test]
    fn set_get_roundtrip_and_errors() {
        let mut img = Image::new(2, 2, ColorMode::Rgba);
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        img.set(1, 0, red).unwrap();
        assert_eq!(img.get(1, 0), Some(red));
        assert_eq!(
            img.set(2, 0, red),
            Err(ImageError::OutOfBounds { x: 2, y: 0 })
        );
        assert_eq!(
            img.set(0, 0, PixelValue::Index(1)),
            Err(ImageError::ModeMismatch)
        );
        assert_eq!(img.get(5, 5), None);
    }
}
