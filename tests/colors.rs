use std::collections::HashSet;

use image::{Rgba, RgbaImage};
use spritefusion_palette_snapper::{
    ReduceOptions, Rgb, SnapMode, SnapOptions, palette_from_rgba, parse_palette,
    reduce_colors_rgba, snap_to_palette_rgba,
};

const GAME_BOY: [Rgb; 4] = [[8, 24, 32], [52, 104, 86], [136, 192, 112], [224, 248, 208]];

fn row(pixels: &[[u8; 4]]) -> RgbaImage {
    RgbaImage::from_fn(pixels.len() as u32, 1, |x, _| Rgba(pixels[x as usize]))
}

fn colors_of(image: &RgbaImage) -> HashSet<Rgb> {
    image
        .pixels()
        .filter(|pixel| pixel[3] > 0)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect()
}

fn ramp() -> RgbaImage {
    let pixels: Vec<[u8; 4]> = (0..60u32)
        .map(|i| {
            [
                (i * 4) as u8,
                (255 - i * 4) as u8,
                ((i * 37) % 256) as u8,
                255,
            ]
        })
        .collect();
    row(&pixels)
}

fn reduce(image: &RgbaImage, max_colors: Option<usize>, threshold: f64) -> RgbaImage {
    reduce_colors_rgba(
        image,
        &ReduceOptions {
            max_colors,
            threshold,
        },
    )
    .unwrap()
}

fn snap(image: &RgbaImage, palette: &[Rgb], mode: SnapMode) -> RgbaImage {
    snap_to_palette_rgba(image, palette, &SnapOptions { mode }).unwrap()
}

#[test]
fn hex_colors_in_any_separator_short_and_long_forms_repeats_dropped() {
    assert_eq!(
        parse_palette("#081820, 346856\n88C070 #abc #081820").unwrap(),
        [[8, 24, 32], [52, 104, 86], [136, 192, 112], [170, 187, 204]]
    );
    assert_eq!(
        parse_palette("e0f8d0\n#000\n0x346856").unwrap(),
        [[224, 248, 208], [0, 0, 0], [52, 104, 86]]
    );
}

#[test]
fn lospec_hex_paint_net_txt_gimp_gpl_json() {
    assert_eq!(
        parse_palette("081820\n346856\n88c070\ne0f8d0\n").unwrap(),
        GAME_BOY
    );
    let two = [[8, 24, 32], [52, 104, 86]];
    assert_eq!(
        parse_palette(";paint.net Palette File\n;Colors: 2\nFF081820\nFF346856\n").unwrap(),
        two
    );
    assert_eq!(
        parse_palette("GIMP Palette\nName: gb\nColumns: 4\n#\n  8  24  32\tdarkest\n 52 104  86\n")
            .unwrap(),
        two
    );
    assert_eq!(
        parse_palette(
            r#"{"name": "Decaf Bad Face", "author": "", "colors": ["081820", "346856"]}"#
        )
        .unwrap(),
        two
    );
    assert_eq!(parse_palette(r##"["#081820", "#346856"]"##).unwrap(), two);
}

#[test]
fn rejects_what_isnt_a_color_empty_palettes_and_more_than_256_colors() {
    let error = |text: &str| parse_palette(text).unwrap_err().to_string();
    assert!(error("#12345").contains("not a hex color"));
    assert!(error("#081820 green").contains("\"green\" is not a hex color"));
    assert!(error("").contains("no colors"));
    let many: Vec<String> = (0..257).map(|i| format!("#{i:06x}")).collect();
    assert!(error(&many.join("\n")).contains("at most 256"));
}

#[test]
fn noise_merges_into_the_most_frequent_color_never_into_an_average() {
    let noisy = row(&[
        [200, 40, 40, 255],
        [200, 40, 40, 255],
        [202, 41, 39, 255],
        [40, 40, 200, 255],
        [40, 200, 40, 255],
    ]);
    let reduced = reduce(&noisy, Some(32), 0.03);
    assert_eq!(reduced.get_pixel(2, 0).0, [200, 40, 40, 255]);
    assert_eq!(colors_of(&reduced).len(), 3);
}

#[test]
fn at_most_32_colors_by_default_any_number_with_max_colors_no_cap_with_none() {
    let ramp = ramp();
    let defaults = reduce_colors_rgba(&ramp, &ReduceOptions::default()).unwrap();
    assert_eq!(colors_of(&defaults).len(), 32);
    assert_eq!(colors_of(&reduce(&ramp, Some(8), 0.03)).len(), 8);
    assert_eq!(colors_of(&reduce(&ramp, None, 0.0)).len(), 60);
    assert!(colors_of(&reduce(&ramp, None, 0.03)).len() > 32);
    assert!(colors_of(&reduce(&ramp, Some(8), 0.03)).is_subset(&colors_of(&ramp)));
}

#[test]
fn threshold_sets_how_close_colors_must_be_to_merge() {
    let greys = row(&[
        [100, 100, 100, 255],
        [100, 100, 100, 255],
        [104, 104, 104, 255],
    ]);
    assert_eq!(colors_of(&reduce(&greys, Some(32), 0.03)).len(), 1);
    assert_eq!(colors_of(&reduce(&greys, Some(32), 0.0)).len(), 2);
}

#[test]
fn transparent_pixels_and_alpha_stay_as_they_are() {
    let image = row(&[[200, 40, 40, 255], [202, 41, 39, 128], [9, 9, 9, 0]]);
    assert_eq!(
        reduce(&image, Some(32), 0.03).into_raw(),
        [200, 40, 40, 255, 200, 40, 40, 128, 9, 9, 9, 0]
    );
}

#[test]
fn rejects_bad_options_and_images() {
    let ramp = ramp();
    let error =
        |options: ReduceOptions| reduce_colors_rgba(&ramp, &options).unwrap_err().to_string();
    assert!(
        error(ReduceOptions {
            max_colors: Some(0),
            threshold: 0.03
        })
        .contains("max_colors")
    );
    assert!(
        error(ReduceOptions {
            max_colors: Some(8),
            threshold: -1.0
        })
        .contains("threshold")
    );
    assert!(
        error(ReduceOptions {
            max_colors: Some(8),
            threshold: f64::NAN
        })
        .contains("threshold")
    );
    let empty = RgbaImage::new(0, 3);
    assert!(
        reduce_colors_rgba(&empty, &ReduceOptions::default())
            .unwrap_err()
            .to_string()
            .contains("1×1")
    );
}

#[test]
fn nearest_every_opaque_pixel_becomes_its_nearest_palette_color_alpha_and_transparent_pixels_stay()
{
    let snapped = snap(
        &row(&[[10, 20, 30, 255], [140, 190, 110, 200], [255, 0, 0, 0]]),
        &GAME_BOY,
        SnapMode::Nearest,
    );
    assert_eq!(
        snapped.into_raw(),
        [8, 24, 32, 255, 136, 192, 112, 200, 255, 0, 0, 0]
    );
}

#[test]
fn a_patch_drawn_in_one_color_stays_one_color_even_halfway_between_two_palette_colors() {
    let patch = row(&[
        [96, 96, 96, 255],
        [102, 102, 102, 255],
        [96, 96, 96, 255],
        [102, 102, 102, 255],
        [96, 96, 96, 255],
    ]);
    let snapped = snap(&patch, &[[0, 0, 0], [255, 255, 255]], SnapMode::Nearest);
    assert_eq!(colors_of(&snapped).len(), 1);
}

#[test]
fn lightness_keeps_a_sprites_light_and_dark_steps_on_another_palette() {
    let red = row(&[
        [150, 20, 20, 255],
        [180, 30, 30, 255],
        [210, 50, 50, 255],
        [235, 75, 75, 255],
    ]);
    let lightness = snap(&red, &GAME_BOY, SnapMode::Lightness);
    let steps: Vec<Rgb> = lightness
        .pixels()
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    assert_eq!(steps, GAME_BOY);
    assert!(colors_of(&snap(&red, &GAME_BOY, SnapMode::Nearest)).len() < 4);
}

#[test]
fn takes_a_palette_files_text_and_rejects_empty_palettes() {
    let ramp = ramp();
    let from_text = snap(
        &ramp,
        &parse_palette("081820\n346856\n88c070\ne0f8d0\n").unwrap(),
        SnapMode::Nearest,
    );
    assert_eq!(from_text, snap(&ramp, &GAME_BOY, SnapMode::Nearest));
    assert!(snap_to_palette_rgba(&ramp, &[], &SnapOptions::default()).is_err());
}

#[test]
fn an_image_with_no_opaque_pixels_comes_back_as_it_was() {
    let empty = row(&[[1, 2, 3, 0]]);
    assert_eq!(
        snap(&empty, &GAME_BOY, SnapMode::Lightness).into_raw(),
        [1, 2, 3, 0]
    );
}

#[test]
fn palette_from_rgba_reads_swatches_in_reading_order() {
    let swatches = RgbaImage::from_raw(
        4,
        2,
        vec![
            8, 24, 32, 255, 52, 104, 86, 255, 136, 192, 112, 255, 224, 248, 208, 255, 8, 24, 32,
            255, 52, 104, 86, 255, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    )
    .unwrap();
    assert_eq!(palette_from_rgba(&swatches).unwrap(), GAME_BOY);
}
