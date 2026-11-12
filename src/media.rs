// Made by Sprite Fusion: https://www.spritefusion.com
// MIT License. Copyright (c) 2026 Hugo Duprez.
// Keep this notice in copies and adaptations of this file.

use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
};

use image::{DynamicImage, Rgba, RgbaImage};

use crate::Error;

pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Maximum pixels per image.
pub const MAX_PIXELS: u64 = 4_194_304;

/// Decodes a PNG, GIF, WebP or JPEG (its first frame, for an animation) to 8-bit RGBA pixels.
///
/// # Errors
/// Fails for unsupported or malformed images, and images over the input or pixel limits.
pub fn decode_image(bytes: &[u8]) -> Result<RgbaImage, Error> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Error::Limit);
    }
    let (width, height) = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .into_dimensions()?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(Error::Limit);
    }
    Ok(to_rgba8(image::load_from_memory(bytes)?))
}

fn to_rgba8(image: DynamicImage) -> RgbaImage {
    let high = |value: u16| (value >> 8) as u8;
    match image {
        DynamicImage::ImageLuma16(luma) => {
            RgbaImage::from_fn(luma.width(), luma.height(), |x, y| {
                let value = high(luma.get_pixel(x, y)[0]);
                Rgba([value, value, value, 255])
            })
        }
        DynamicImage::ImageLumaA16(luma) => {
            RgbaImage::from_fn(luma.width(), luma.height(), |x, y| {
                let [value, alpha] = luma.get_pixel(x, y).0.map(high);
                Rgba([value, value, value, alpha])
            })
        }
        DynamicImage::ImageRgb16(rgb) => RgbaImage::from_fn(rgb.width(), rgb.height(), |x, y| {
            let [red, green, blue] = rgb.get_pixel(x, y).0.map(high);
            Rgba([red, green, blue, 255])
        }),
        DynamicImage::ImageRgba16(rgba) => {
            RgbaImage::from_fn(rgba.width(), rgba.height(), |x, y| {
                Rgba(rgba.get_pixel(x, y).0.map(high))
            })
        }
        other => other.to_rgba8(),
    }
}

/// Encodes RGBA pixels as a PNG: an indexed PNG (smaller) when there are at most 256 visible colors, transparency
/// counted as one, else a full RGBA PNG.
///
/// # Errors
/// Fails if the PNG encoder does.
pub fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, Error> {
    match indexed(image) {
        Some((entries, indices)) => {
            encode_indexed(image.width(), image.height(), &entries, &indices)
        }
        None => encode_rgba(image),
    }
}

fn indexed(image: &RgbaImage) -> Option<(Vec<[u8; 4]>, Vec<u8>)> {
    let entry = |pixel: &[u8]| {
        if pixel[3] == 0 {
            [0; 4]
        } else {
            [pixel[0], pixel[1], pixel[2], pixel[3]]
        }
    };
    let mut seen: HashSet<[u8; 4]> = HashSet::new();
    let mut entries = Vec::new();
    for pixel in image.as_raw().chunks_exact(4) {
        let entry = entry(pixel);
        if seen.insert(entry) {
            entries.push(entry);
            if entries.len() > 256 {
                return None;
            }
        }
    }
    entries.sort_by_key(|entry| entry[3] == 255);
    let index: HashMap<[u8; 4], u8> = entries
        .iter()
        .enumerate()
        .map(|(at, entry)| (*entry, at as u8))
        .collect();
    let indices = image
        .as_raw()
        .chunks_exact(4)
        .map(|pixel| index[&entry(pixel)])
        .collect();
    Some((entries, indices))
}

fn encode_indexed(
    width: u32,
    height: u32,
    entries: &[[u8; 4]],
    indices: &[u8],
) -> Result<Vec<u8>, Error> {
    let (depth, bits) = match entries.len() {
        0..=2 => (png::BitDepth::One, 1),
        3..=4 => (png::BitDepth::Two, 2),
        5..=16 => (png::BitDepth::Four, 4),
        _ => (png::BitDepth::Eight, 8),
    };
    let per_byte = 8 / bits;
    let row_bytes = (width as usize).div_ceil(per_byte);
    let mut data = vec![0u8; row_bytes * height as usize];
    for (y, row) in indices.chunks_exact(width as usize).enumerate() {
        for (x, &index) in row.iter().enumerate() {
            let shift = 8 - bits * (x % per_byte + 1);
            data[y * row_bytes + x / per_byte] |= index << shift;
        }
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(depth);
    encoder.set_palette(
        entries
            .iter()
            .flat_map(|entry| [entry[0], entry[1], entry[2]])
            .collect::<Vec<_>>(),
    );
    let alphas: Vec<u8> = entries
        .iter()
        .map(|entry| entry[3])
        .take_while(|&alpha| alpha < 255)
        .collect();
    if !alphas.is_empty() {
        encoder.set_trns(alphas);
    }
    encoder.set_compression(png::Compression::High);
    encoder.set_filter(png::Filter::NoFilter);
    encoder.write_header()?.write_image_data(&data)?;
    Ok(out)
}

fn encode_rgba(image: &RgbaImage) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width(), image.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    encoder.write_header()?.write_image_data(image.as_raw())?;
    Ok(out)
}
