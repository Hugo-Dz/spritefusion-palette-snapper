// Made by Sprite Fusion: https://www.spritefusion.com
// MIT License. Copyright (c) 2026 Hugo Duprez.
// Keep this notice in copies and adaptations of this file.

use std::collections::HashMap;

use image::RgbaImage;

use crate::Error;
use crate::oklab::{Oklab, Rgb, distance, to_oklab};

/// A palette has 1 to 256 colors.
pub const MAX_PALETTE_COLORS: usize = 256;

/// A color as `#rrggbb`.
pub fn hex([red, green, blue]: Rgb) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

pub(crate) const fn key([red, green, blue]: Rgb) -> u32 {
    (red as u32) << 16 | (green as u32) << 8 | blue as u32
}

pub(crate) fn check_image(image: &RgbaImage) -> Result<(), Error> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err(Error::Invalid(format!(
            "An image is at least 1×1 pixels; this one is {width}×{height}."
        )));
    }
    Ok(())
}

fn is_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

fn normalize_hex(token: &str) -> Option<String> {
    let digits = token
        .strip_prefix('#')
        .or_else(|| token.strip_prefix("0x"))
        .or_else(|| token.strip_prefix("0X"))
        .unwrap_or(token);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let digits = digits.to_ascii_lowercase();
    let digits = match digits.len() {
        3 => digits.chars().flat_map(|digit| [digit, digit]).collect(),
        8 if digits.ends_with("ff") => digits[..6].to_string(),
        8 if digits.starts_with("ff") => digits[2..].to_string(),
        8 => String::new(),
        _ => digits,
    };
    (digits.len() == 6).then(|| format!("#{digits}"))
}

fn json_colors(text: &str) -> Option<Vec<String>> {
    if !text.trim_start_matches(is_space).starts_with(['[', '{']) {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let list = match &value {
        serde_json::Value::Object(object) => match object.get("colors") {
            Some(serde_json::Value::Null) | None => object.get("colours")?,
            Some(colors) => colors,
        },
        other => other,
    };
    list.as_array()?
        .iter()
        .map(|entry| entry.as_str().map(str::to_string))
        .collect()
}

fn gimp_color(line: &str) -> Option<Rgb> {
    let mut rest = line.trim_start_matches(is_space);
    let mut channels = [0u8; 3];
    for (index, channel) in channels.iter_mut().enumerate() {
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if !(1..=3).contains(&digits) {
            return None;
        }
        *channel = rest[..digits].parse::<u16>().ok()?.min(255) as u8;
        rest = &rest[digits..];
        let after = rest.trim_start_matches(is_space);
        if index < 2 && after.len() == rest.len() {
            return None;
        }
        if index == 2 && !(rest.is_empty() || rest.starts_with(is_space)) {
            return None;
        }
        if index < 2 {
            rest = after;
        }
    }
    Some(channels)
}

fn without_comment(line: &str) -> &str {
    let start = line
        .char_indices()
        .rfind(|&(_, c)| matches!(c, '\r' | '\u{2028}' | '\u{2029}'))
        .map_or(0, |(at, c)| at + c.len_utf8());
    match line[start..].find(';') {
        Some(at) => &line[..start + at],
        None => line,
    }
}

fn hex_tokens(text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let hash = bytes[at] == b'#' && bytes.get(at + 1).is_some_and(u8::is_ascii_alphanumeric);
        if hash || bytes[at].is_ascii_alphanumeric() {
            let start = at;
            at += usize::from(hash);
            while at < bytes.len() && bytes[at].is_ascii_alphanumeric() {
                at += 1;
            }
            tokens.push(&text[start..at]);
        } else {
            at += 1;
        }
    }
    tokens
}

fn rgb(hex: &str) -> Rgb {
    let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).unwrap_or(0);
    [channel(1), channel(3), channel(5)]
}

/// A palette from text, as colors in their order, repeats dropped:
/// - hex colors separated by anything (`#081820, 346856 88C070`), short forms (`#abc`) included;
/// - Lospec's .hex, Paint.NET's .txt (`FFRRGGBB` lines, `;` comments), GIMP's .gpl (`R G B name` lines);
/// - JSON: an array of hex colors, or an object with a `colors` array (Lospec's .json).
///
/// # Errors
/// Fails when a token isn't a color, or when there are none or more than 256.
pub fn parse_palette(text: &str) -> Result<Vec<Rgb>, Error> {
    let mut colors = Vec::new();
    if let Some(entries) = json_colors(text) {
        for entry in entries {
            let color = normalize_hex(entry.trim_matches(is_space))
                .ok_or_else(|| Error::Invalid(format!("\"{entry}\" is not a hex color.")))?;
            colors.push(rgb(&color));
        }
    } else if text
        .trim_start_matches(is_space)
        .get(..12)
        .is_some_and(|start| start.eq_ignore_ascii_case("GIMP Palette"))
    {
        colors.extend(lines(text).filter_map(gimp_color));
    } else {
        let text = lines(text)
            .map(without_comment)
            .collect::<Vec<_>>()
            .join("\n");
        for token in hex_tokens(&text) {
            let color = normalize_hex(token)
                .ok_or_else(|| Error::Invalid(format!("\"{token}\" is not a hex color.")))?;
            colors.push(rgb(&color));
        }
    }
    unique(colors)
}

pub(crate) fn unique(colors: impl IntoIterator<Item = Rgb>) -> Result<Vec<Rgb>, Error> {
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<Rgb> = colors
        .into_iter()
        .filter(|color| seen.insert(*color))
        .collect();
    if unique.is_empty() {
        return Err(Error::Invalid("The palette has no colors.".into()));
    }
    if unique.len() > MAX_PALETTE_COLORS {
        return Err(Error::Invalid(format!(
            "A palette has at most {MAX_PALETTE_COLORS} colors; this one has {}.",
            unique.len()
        )));
    }
    Ok(unique)
}

#[derive(Clone, Debug)]
pub(crate) struct ColorCount {
    pub color: Rgb,
    pub lab: Oklab,
    pub count: u64,
}

pub(crate) fn count_colors(image: &RgbaImage, min_alpha: u8) -> Vec<ColorCount> {
    let mut index: HashMap<u32, usize> = HashMap::new();
    let mut counts: Vec<ColorCount> = Vec::new();
    for pixel in image.as_raw().chunks_exact(4) {
        if pixel[3] < min_alpha {
            continue;
        }
        let color = [pixel[0], pixel[1], pixel[2]];
        if let Some(&at) = index.get(&key(color)) {
            counts[at].count += 1;
        } else {
            index.insert(key(color), counts.len());
            counts.push(ColorCount {
                color,
                lab: to_oklab(color),
                count: 1,
            });
        }
    }
    counts
}

pub(crate) fn merge_colors(
    colors: &[ColorCount],
    threshold: f64,
    max_colors: Option<usize>,
) -> HashMap<u32, Rgb> {
    struct Cluster {
        color: Rgb,
        lab: Oklab,
        count: u64,
        members: Vec<Rgb>,
        alive: bool,
    }
    let mut order: Vec<&ColorCount> = colors.iter().collect();
    order.sort_by(|first, second| second.count.cmp(&first.count));
    let mut clusters: Vec<Cluster> = Vec::new();
    for entry in order {
        let mut best = None;
        let mut best_distance = f64::INFINITY;
        for (at, cluster) in clusters.iter().enumerate() {
            let d = distance(entry.lab, cluster.lab);
            if d < best_distance {
                best = Some(at);
                best_distance = d;
            }
        }
        match best {
            Some(at) if best_distance <= threshold => {
                clusters[at].count += entry.count;
                clusters[at].members.push(entry.color);
            }
            _ => clusters.push(Cluster {
                color: entry.color,
                lab: entry.lab,
                count: entry.count,
                members: vec![entry.color],
                alive: true,
            }),
        }
    }
    let mut alive = clusters.len();
    while max_colors.is_some_and(|max| alive > max) {
        let mut pair = (0, 0);
        let mut pair_distance = f64::INFINITY;
        for i in 0..clusters.len() {
            if !clusters[i].alive {
                continue;
            }
            for j in i + 1..clusters.len() {
                if !clusters[j].alive {
                    continue;
                }
                let d = distance(clusters[i].lab, clusters[j].lab);
                if d < pair_distance {
                    pair = (i, j);
                    pair_distance = d;
                }
            }
        }
        let (keep, drop) = if clusters[pair.0].count >= clusters[pair.1].count {
            pair
        } else {
            (pair.1, pair.0)
        };
        let members = std::mem::take(&mut clusters[drop].members);
        clusters[keep].count += clusters[drop].count;
        clusters[keep].members.extend(members);
        clusters[drop].alive = false;
        alive -= 1;
    }
    let mut replacement = HashMap::new();
    for cluster in clusters.iter().filter(|cluster| cluster.alive) {
        for member in &cluster.members {
            replacement.insert(key(*member), cluster.color);
        }
    }
    replacement
}

/// A palette from an image of it (Lospec's PNGs, a screenshot of swatches): its opaque colors in reading order. An
/// image with more than 256 colors (a photo, a JPEG) has many near-identical ones: those within 0.04 merge, and colors
/// on under 0.1% of the pixels are dropped.
///
/// # Errors
/// Fails when the image has no opaque pixels, or still too many colors.
pub fn palette_from_rgba(image: &RgbaImage) -> Result<Vec<Rgb>, Error> {
    check_image(image)?;
    let colors = count_colors(image, 128);
    if colors.is_empty() {
        return Err(Error::Invalid("The image has no opaque pixels.".into()));
    }
    if colors.len() <= MAX_PALETTE_COLORS {
        return Ok(colors.iter().map(|entry| entry.color).collect());
    }
    let replacement = merge_colors(&colors, 0.04, None);
    let total: u64 = colors.iter().map(|entry| entry.count).sum();
    let mut counts: HashMap<u32, u64> = HashMap::new();
    for entry in &colors {
        *counts
            .entry(key(replacement[&key(entry.color)]))
            .or_default() += entry.count;
    }
    let kept: Vec<Rgb> = colors
        .iter()
        .filter(|entry| {
            counts.get(&key(entry.color)).copied().unwrap_or(0) as f64 >= total as f64 * 0.001
        })
        .map(|entry| entry.color)
        .collect();
    if kept.is_empty() || kept.len() > MAX_PALETTE_COLORS {
        return Err(Error::Invalid(format!(
            "The image has {} distinct colors; a palette has 1 to {MAX_PALETTE_COLORS}.",
            kept.len()
        )));
    }
    Ok(kept)
}
