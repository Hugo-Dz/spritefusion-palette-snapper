use std::io::Cursor;

use image::{ImageBuffer, ImageFormat, Luma, Rgba, RgbaImage};
use spritefusion_palette_snapper::{
    ReduceOptions, Rgb, SnapMode, SnapOptions, decode_image, encode_png, palette_from_image,
    reduce_colors, reduce_colors_rgba, snap_to_palette, snap_to_palette_rgba,
};

const GAME_BOY: [Rgb; 4] = [[8, 24, 32], [52, 104, 86], [136, 192, 112], [224, 248, 208]];

fn sprite() -> RgbaImage {
    RgbaImage::from_fn(16, 16, |x, y| {
        if x < 2 || y < 2 {
            Rgba([0, 0, 0, 0])
        } else {
            Rgba([
                (40 + x * 12 + (x * y) % 3) as u8,
                (30 + y * 10) as u8,
                (90 + (x + y) % 4) as u8,
                255,
            ])
        }
    })
}

fn png(image: &RgbaImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .unwrap();
    bytes
}

fn visible(image: &RgbaImage) -> Vec<u8> {
    let mut pixels = image.as_raw().clone();
    for pixel in pixels.chunks_exact_mut(4) {
        if pixel[3] == 0 {
            pixel.fill(0);
        }
    }
    pixels
}

fn color_type(png: &[u8]) -> png::ColorType {
    png::Decoder::new(Cursor::new(png))
        .read_info()
        .unwrap()
        .info()
        .color_type
}

#[test]
fn reduce_colors_and_snap_to_palette_give_the_same_pixels_as_their_rgba_versions_as_an_indexed_png()
{
    let input = png(&sprite());
    let options = ReduceOptions {
        max_colors: Some(8),
        ..ReduceOptions::default()
    };
    let reduced = reduce_colors(&input, &options).unwrap();
    assert_eq!(
        visible(&decode_image(&reduced).unwrap()),
        visible(&reduce_colors_rgba(&sprite(), &options).unwrap())
    );
    assert_eq!(color_type(&reduced), png::ColorType::Indexed);
    for mode in [SnapMode::Nearest, SnapMode::Lightness] {
        let options = SnapOptions { mode };
        let snapped = snap_to_palette(&input, &GAME_BOY, &options).unwrap();
        assert_eq!(
            visible(&decode_image(&snapped).unwrap()),
            visible(&snap_to_palette_rgba(&sprite(), &GAME_BOY, &options).unwrap())
        );
        assert_eq!(color_type(&snapped), png::ColorType::Indexed);
    }
}

#[test]
fn indexed_pngs_keep_semi_transparent_pixels_exactly() {
    let image = RgbaImage::from_fn(9, 3, |x, y| {
        Rgba([
            (x * 20) as u8,
            (y * 70) as u8,
            200,
            [0, 64, 255][y as usize],
        ])
    });
    let encoded = encode_png(&image).unwrap();
    assert_eq!(color_type(&encoded), png::ColorType::Indexed);
    assert_eq!(visible(&decode_image(&encoded).unwrap()), visible(&image));
}

#[test]
fn a_palette_can_be_an_image_of_its_swatches() {
    let swatches = RgbaImage::from_fn(64, 16, |x, _| {
        let [red, green, blue] = GAME_BOY[(x / 16) as usize];
        Rgba([red, green, blue, 255])
    });
    assert_eq!(palette_from_image(&png(&swatches)).unwrap(), GAME_BOY);
}

#[test]
fn more_than_256_colors_a_full_rgba_png_still_exact() {
    let image = RgbaImage::from_fn(20, 20, |x, y| {
        Rgba([(x * 12) as u8, (y * 12) as u8, 128, 255])
    });
    let encoded = encode_png(&image).unwrap();
    assert_eq!(color_type(&encoded), png::ColorType::Rgba);
    assert_eq!(decode_image(&encoded).unwrap(), image);
}

#[test]
fn greyscale_and_16_bit_inputs_decode_to_8_bit_rgba() {
    let grey: ImageBuffer<Luma<u8>, Vec<u8>> =
        ImageBuffer::from_raw(2, 2, vec![0, 128, 255, 64]).unwrap();
    let mut bytes = Vec::new();
    grey.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .unwrap();
    let decoded = decode_image(&bytes).unwrap();
    assert_eq!(decoded.get_pixel(1, 0).0, [128, 128, 128, 255]);
    let deep: ImageBuffer<Luma<u16>, Vec<u16>> =
        ImageBuffer::from_raw(3, 1, vec![0x00ff, 0x7f80, 0xff7f]).unwrap();
    let mut bytes = Vec::new();
    deep.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .unwrap();
    let decoded = decode_image(&bytes).unwrap();
    assert_eq!(
        decoded.pixels().map(|pixel| pixel[0]).collect::<Vec<_>>(),
        [0, 127, 255]
    );
}

#[test]
fn rejects_what_isnt_an_image() {
    assert!(decode_image(b"not an image").is_err());
}
