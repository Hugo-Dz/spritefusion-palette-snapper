// Made by Sprite Fusion: https://www.spritefusion.com
// MIT License. Copyright (c) 2026 Hugo Duprez.
// Keep this notice in copies and adaptations of this file.

use image::RgbaImage;
use serde::{Deserialize, Deserializer};
use wasm_bindgen::prelude::*;

use crate::{
    DEFAULT_MAX_COLORS, DEFAULT_THRESHOLD, ReduceOptions, Rgb, SnapMode, SnapOptions, decode_image,
    encode_png, hex, palette_from_rgba, parse_palette,
};

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ReduceJson {
    max_colors: Cap,
    threshold: Option<f64>,
}

#[derive(Default)]
enum Cap {
    #[default]
    Default,
    Count(usize),
    None,
}

impl<'de> Deserialize<'de> for Cap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Option::<usize>::deserialize(deserializer)?.map_or(Cap::None, Cap::Count))
    }
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SnapJson {
    mode: SnapMode,
}

fn reduce_options(json: &str) -> Result<ReduceOptions, String> {
    let options: ReduceJson =
        serde_json::from_str(json).map_err(|e| format!("Invalid options: {e}"))?;
    Ok(ReduceOptions {
        max_colors: match options.max_colors {
            Cap::Default => Some(DEFAULT_MAX_COLORS),
            Cap::Count(count) => Some(count),
            Cap::None => None,
        },
        threshold: options.threshold.unwrap_or(DEFAULT_THRESHOLD),
    })
}

fn snap_options(json: &str) -> Result<SnapOptions, String> {
    let options: SnapJson =
        serde_json::from_str(json).map_err(|e| format!("Invalid options: {e}"))?;
    Ok(SnapOptions { mode: options.mode })
}

fn pixels(data: &[u8], width: u32, height: u32) -> Result<RgbaImage, String> {
    let expected = u64::from(width) * u64::from(height) * 4;
    if data.len() as u64 != expected {
        return Err(format!(
            "A {width}×{height} image has {expected} bytes of RGBA pixels; this one has {}.",
            data.len()
        ));
    }
    RgbaImage::from_raw(width, height, data.to_vec())
        .ok_or_else(|| "Invalid image dimensions".into())
}

#[allow(clippy::needless_pass_by_value)]
fn throw(error: impl ToString) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn hexes(palette: &[Rgb]) -> Vec<String> {
    palette.iter().map(|color| hex(*color)).collect()
}

/// Fewer colors. Takes PNG, GIF, WebP or JPEG bytes and returns PNG bytes. Options: `max_colors` (32 by default,
/// `null` for no cap) and `threshold` (0.03). Pass "{}" for the defaults.
///
/// # Errors
/// Throws for invalid images, options, and resource limits.
#[wasm_bindgen]
pub fn reduce_colors(bytes: &[u8], options_json: &str) -> Result<Vec<u8>, JsValue> {
    let options = reduce_options(options_json).map_err(throw)?;
    let image = decode_image(bytes).map_err(throw)?;
    encode_png(&crate::reduce_colors_rgba(&image, &options).map_err(throw)?).map_err(throw)
}

/// Snaps every visible pixel to the palette: hex colors ("#081820,#346856") or a palette file's text. Takes PNG, GIF,
/// WebP or JPEG bytes and returns PNG bytes. Options: `mode`, "nearest" (default) or "lightness".
///
/// # Errors
/// Throws for invalid images, palettes, options, and resource limits.
#[wasm_bindgen]
pub fn snap_to_palette(
    bytes: &[u8],
    palette: &str,
    options_json: &str,
) -> Result<Vec<u8>, JsValue> {
    let options = snap_options(options_json).map_err(throw)?;
    let palette = parse_palette(palette).map_err(throw)?;
    let image = decode_image(bytes).map_err(throw)?;
    encode_png(&crate::snap_to_palette_rgba(&image, &palette, &options).map_err(throw)?)
        .map_err(throw)
}

/// `reduce_colors` on raw RGBA pixels, such as a canvas's `ImageData`. Returns RGBA pixels of the same size.
///
/// # Errors
/// Throws for invalid pixels or options.
#[wasm_bindgen]
pub fn reduce_colors_rgba(
    data: &[u8],
    width: u32,
    height: u32,
    options_json: &str,
) -> Result<Vec<u8>, JsValue> {
    let options = reduce_options(options_json).map_err(throw)?;
    let image = pixels(data, width, height).map_err(throw)?;
    Ok(crate::reduce_colors_rgba(&image, &options)
        .map_err(throw)?
        .into_raw())
}

/// `snap_to_palette` on raw RGBA pixels, such as a canvas's `ImageData`. Returns RGBA pixels of the same size.
///
/// # Errors
/// Throws for invalid pixels, palettes or options.
#[wasm_bindgen]
pub fn snap_to_palette_rgba(
    data: &[u8],
    width: u32,
    height: u32,
    palette: &str,
    options_json: &str,
) -> Result<Vec<u8>, JsValue> {
    let options = snap_options(options_json).map_err(throw)?;
    let palette = parse_palette(palette).map_err(throw)?;
    let image = pixels(data, width, height).map_err(throw)?;
    Ok(crate::snap_to_palette_rgba(&image, &palette, &options)
        .map_err(throw)?
        .into_raw())
}

/// A palette's colors as `#rrggbb`, from hex colors or a palette file's text (.hex, .txt, .gpl, .json).
///
/// # Errors
/// Throws when a token isn't a color, or when there are none or more than 256.
#[wasm_bindgen(js_name = parse_palette)]
pub fn parse_palette_colors(text: &str) -> Result<Vec<String>, JsValue> {
    parse_palette(text)
        .map(|palette| hexes(&palette))
        .map_err(throw)
}

/// A palette's colors as `#rrggbb`, from an image of its swatches (PNG, GIF, WebP or JPEG bytes).
///
/// # Errors
/// Throws for invalid images, an image with no opaque pixels, or one with too many colors.
#[wasm_bindgen]
pub fn palette_from_image(bytes: &[u8]) -> Result<Vec<String>, JsValue> {
    let image = decode_image(bytes).map_err(throw)?;
    palette_from_rgba(&image)
        .map(|palette| hexes(&palette))
        .map_err(throw)
}
