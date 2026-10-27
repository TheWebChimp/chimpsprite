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
    /// Imagen llena de píxeles transparentes. Un tamaño imposible de representar (más bytes de los
    /// que admite un `Vec`) no entra en pánico: da una imagen vacía de 0×0 y un aviso en el log.
    /// Para distinguir ese caso, `try_new`.
    pub fn new(width: u32, height: u32, mode: ColorMode) -> Self {
        Self::try_new(width, height, mode).unwrap_or_else(|| {
            log::warn!("image of {width}×{height} pixels is too large; using an empty image");
            Self::try_new(0, 0, mode).expect("an empty image always fits")
        })
    }

    /// Como `new`, pero `None` si la cantidad de bytes no cabe en memoria direccionable.
    pub fn try_new(width: u32, height: u32, mode: ColorMode) -> Option<Self> {
        let n = (width as usize).checked_mul(height as usize)?;
        let pixel_bytes = match mode {
            ColorMode::Rgba | ColorMode::Grayscale => std::mem::size_of::<Rgba>(),
            ColorMode::Indexed { .. } => 1,
        };
        if n.checked_mul(pixel_bytes)? > isize::MAX as usize {
            return None;
        }
        let pixels = match mode.transparent() {
            PixelValue::Rgba(c) => PixelBuffer::Rgba(vec![c; n]),
            PixelValue::Index(i) => PixelBuffer::Indexed(vec![i; n]),
        };
        Some(Self {
            width,
            height,
            pixels,
        })
    }

    /// Imagen a partir de píxeles ya decodificados; `None` si el largo no coincide.
    pub fn from_buffer(width: u32, height: u32, pixels: PixelBuffer) -> Option<Self> {
        let len = match &pixels {
            PixelBuffer::Rgba(p) => p.len(),
            PixelBuffer::Indexed(p) => p.len(),
        };
        (len == (width as usize) * (height as usize)).then_some(Self {
            width,
            height,
            pixels,
        })
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
        (x < self.width && y < self.height).then(|| y as usize * self.width as usize + x as usize)
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

    /// Copia del rectángulo `(x, y, width, height)`, que debe caber dentro de la imagen.
    pub fn crop(&self, x: u32, y: u32, width: u32, height: u32) -> Image {
        assert!(
            x as u64 + width as u64 <= self.width as u64
                && y as u64 + height as u64 <= self.height as u64,
            "crop out of bounds"
        );
        let rows = (y..y + height).map(|row| {
            let start = row as usize * self.width as usize + x as usize;
            start..start + width as usize
        });
        let pixels = match &self.pixels {
            PixelBuffer::Rgba(p) => {
                PixelBuffer::Rgba(rows.flat_map(|r| p[r].iter().copied()).collect())
            }
            PixelBuffer::Indexed(p) => {
                PixelBuffer::Indexed(rows.flat_map(|r| p[r].iter().copied()).collect())
            }
        };
        Image {
            width,
            height,
            pixels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_copies_sub_rectangle() {
        let mut img = Image::new(
            4,
            3,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        img.set(2, 1, PixelValue::Index(7)).unwrap();
        let c = img.crop(1, 1, 2, 2);
        assert_eq!((c.width(), c.height()), (2, 2));
        assert_eq!(c.get(1, 0), Some(PixelValue::Index(7)));
        assert_eq!(c.get(0, 0), Some(PixelValue::Index(0)));
    }

    #[test]
    fn impossible_sizes_do_not_panic() {
        assert!(Image::try_new(u32::MAX, u32::MAX, ColorMode::Rgba).is_none());
        let img = Image::new(u32::MAX, u32::MAX, ColorMode::Rgba);
        assert_eq!((img.width(), img.height()), (0, 0));
        assert_eq!(img.get(0, 0), None);
        let ok = Image::try_new(3, 2, ColorMode::Rgba).unwrap();
        assert_eq!(ok, Image::new(3, 2, ColorMode::Rgba));
    }

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
