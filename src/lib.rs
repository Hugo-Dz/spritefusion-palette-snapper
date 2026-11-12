// Made by Sprite Fusion: https://www.spritefusion.com
// MIT License. Copyright (c) 2026 Hugo Duprez.
// Keep this notice in copies and adaptations of this file.

//! Apply any palette to your pixel art, or reduce it to a few clean colors.
//!
//! [`reduce_colors`] and [`snap_to_palette`] take an encoded image (PNG, GIF, WebP or JPEG) and return a PNG.
//! [`reduce_colors_rgba`] and [`snap_to_palette_rgba`] do the same on decoded pixels.

mod colors;
mod media;
mod oklab;
mod palette;
#[cfg(target_arch = "wasm32")]
mod wasm;

use thiserror::Error;

pub use colors::{
    DEFAULT_MAX_COLORS, DEFAULT_THRESHOLD, ReduceOptions, SnapMode, SnapOptions,
    reduce_colors_rgba, snap_to_palette_rgba,
};
pub use media::{MAX_INPUT_BYTES, MAX_PIXELS, decode_image, encode_png};
pub use oklab::Rgb;
pub use palette::{MAX_PALETTE_COLORS, hex, palette_from_rgba, parse_palette};

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("image decode failed: {0}")]
    Image(#[from] image::ImageError),
    #[error("PNG encoding failed: {0}")]
    Png(#[from] png::EncodingError),
    #[error("image exceeds the 64 MiB input or {MAX_PIXELS}-pixel limit")]
    Limit,
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Image(image::ImageError::IoError(error))
    }
}

/// Fewer colors, without a palette: similar colors merge into the one that appears most often, then the closest keep
/// merging until at most `max_colors` remain (32 by default, `None` for no cap). Returns a PNG.
///
/// # Errors
/// Fails for invalid images or options.
pub fn reduce_colors(image: &[u8], options: &ReduceOptions) -> Result<Vec<u8>, Error> {
    encode_png(&reduce_colors_rgba(&decode_image(image)?, options)?)
}

/// Every visible pixel becomes one of the palette's colors, by `mode`: `Nearest` (default) or `Lightness`. Returns a
/// PNG.
///
/// # Errors
/// Fails for invalid images, or a palette with no colors or more than 256.
pub fn snap_to_palette(
    image: &[u8],
    palette: &[Rgb],
    options: &SnapOptions,
) -> Result<Vec<u8>, Error> {
    encode_png(&snap_to_palette_rgba(
        &decode_image(image)?,
        palette,
        options,
    )?)
}

/// A palette from an image of its swatches (Lospec's PNGs, a screenshot): its opaque colors in reading order.
///
/// # Errors
/// Fails for invalid images, an image with no opaque pixels, or one with too many colors.
pub fn palette_from_image(image: &[u8]) -> Result<Vec<Rgb>, Error> {
    palette_from_rgba(&decode_image(image)?)
}
