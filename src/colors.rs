// Made by Sprite Fusion: https://www.spritefusion.com
// MIT License. Copyright (c) 2026 Hugo Duprez.
// Keep this notice in copies and adaptations of this file.

use std::collections::HashMap;

use image::RgbaImage;
use serde::Deserialize;

use crate::Error;
use crate::oklab::{Rgb, distance, to_oklab};
use crate::palette::{ColorCount, check_image, count_colors, key, merge_colors, unique};

/// Colors closer than this (OKLab distance) become one color: the default threshold.
pub const DEFAULT_THRESHOLD: f64 = 0.03;
/// The default cap without a palette.
pub const DEFAULT_MAX_COLORS: usize = 32;
const SNAP_MERGE: f64 = 0.03;

/// How a sprite's colors map onto a palette.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "lowercase")]
pub enum SnapMode {
    /// Each color becomes the palette color closest to it.
    #[default]
    Nearest,
    /// The sprite's range from dark to light is spread over the palette's, and each color takes the palette color
    /// closest in lightness.
    Lightness,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReduceOptions {
    /// At most this many colors: 32 by default, `None` for no cap.
    pub max_colors: Option<usize>,
    /// Colors closer than this (OKLab distance) become one color: 0.03 by default.
    pub threshold: f64,
}

impl Default for ReduceOptions {
    fn default() -> Self {
        Self {
            max_colors: Some(DEFAULT_MAX_COLORS),
            threshold: DEFAULT_THRESHOLD,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapOptions {
    pub mode: SnapMode,
}

fn recolor(image: &RgbaImage, mut replace: impl FnMut(u32) -> Rgb) -> RgbaImage {
    let mut result = image.clone();
    for pixel in result.chunks_exact_mut(4) {
        if pixel[3] < 1 {
            continue;
        }
        let next = replace(key([pixel[0], pixel[1], pixel[2]]));
        pixel[..3].copy_from_slice(&next);
    }
    result
}

/// Fewer colors, without a palette. Similar colors (within `threshold`) merge into the one that appears most often,
/// so every color left is one that was drawn: nothing is averaged or invented. Then the closest colors keep merging
/// until at most `max_colors` remain.
///
/// # Errors
/// Fails for an empty image, a `max_colors` of 0, or a negative or non-finite threshold.
pub fn reduce_colors_rgba(image: &RgbaImage, options: &ReduceOptions) -> Result<RgbaImage, Error> {
    check_image(image)?;
    if options.max_colors == Some(0) {
        return Err(Error::Invalid(
            "max_colors is a whole number from 1; got 0.".into(),
        ));
    }
    if !options.threshold.is_finite() || options.threshold < 0.0 {
        return Err(Error::Invalid(format!(
            "threshold is a distance from 0; got {}.",
            options.threshold
        )));
    }
    let replacement = merge_colors(
        &count_colors(image, 1),
        options.threshold,
        options.max_colors,
    );
    Ok(recolor(image, |key| replacement[&key]))
}

fn lightness_quantile(colors: &[ColorCount], quantile: f64) -> f64 {
    let mut sorted: Vec<&ColorCount> = colors.iter().collect();
    sorted.sort_by(|first, second| {
        first.lab[0]
            .partial_cmp(&second.lab[0])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let total: u64 = colors.iter().map(|entry| entry.count).sum();
    let mut seen = 0;
    for entry in &sorted {
        seen += entry.count;
        if seen as f64 >= quantile * total as f64 {
            return entry.lab[0];
        }
    }
    sorted.last().map_or(0.0, |entry| entry.lab[0])
}

/// Every opaque pixel becomes one of the palette's colors.
/// - `Nearest` (default): the palette color closest to it (OKLab).
/// - `Lightness`: the sprite's lightness range (2nd to 98th percentile of its pixels) is stretched over the palette's,
///   and each color takes the palette color closest in lightness.
///
/// # Errors
/// Fails for an empty image, or a palette with no colors or more than 256.
pub fn snap_to_palette_rgba(
    image: &RgbaImage,
    palette: &[Rgb],
    options: &SnapOptions,
) -> Result<RgbaImage, Error> {
    check_image(image)?;
    let swatches: Vec<_> = unique(palette.iter().copied())?
        .into_iter()
        .map(|color| (color, to_oklab(color)))
        .collect();
    let colors = count_colors(image, 1);
    if colors.is_empty() {
        return Ok(image.clone());
    }
    let low = lightness_quantile(&colors, 0.02);
    let high = lightness_quantile(&colors, 0.98);
    let palette_low = swatches
        .iter()
        .map(|(_, lab)| lab[0])
        .fold(f64::INFINITY, f64::min);
    let palette_high = swatches
        .iter()
        .map(|(_, lab)| lab[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let stretched = |lightness: f64| {
        if high > low {
            palette_low
                + ((lightness.max(low).min(high) - low) / (high - low))
                    * (palette_high - palette_low)
        } else {
            lightness
        }
    };
    let merged = merge_colors(&colors, SNAP_MERGE, None);
    let mut chosen: HashMap<u32, Rgb> = HashMap::new();
    let mut swatch_for = |color: Rgb| {
        *chosen.entry(key(color)).or_insert_with(|| {
            let lab = to_oklab(color);
            let mut best = 0;
            let mut best_distance = f64::INFINITY;
            for (at, (_, swatch)) in swatches.iter().enumerate() {
                let d = match options.mode {
                    SnapMode::Lightness => (stretched(lab[0]) - swatch[0]).abs(),
                    SnapMode::Nearest => distance(lab, *swatch),
                };
                if d < best_distance {
                    best = at;
                    best_distance = d;
                }
            }
            swatches[best].0
        })
    };
    Ok(recolor(image, |key| swatch_for(merged[&key])))
}
