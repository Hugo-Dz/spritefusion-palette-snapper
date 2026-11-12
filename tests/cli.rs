#![cfg(feature = "cli")]

use image::{Rgba, RgbaImage};
use spritefusion_palette_snapper::{
    ReduceOptions, Rgb, SnapMode, SnapOptions, decode_image, reduce_colors, snap_to_palette,
};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

const GAME_BOY: [Rgb; 4] = [[8, 24, 32], [52, 104, 86], [136, 192, 112], [224, 248, 208]];

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "spritefusion-palette-snapper-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        RgbaImage::from_fn(16, 16, |x, y| {
            Rgba([
                (40 + x * 12 + (x * y) % 3) as u8,
                (30 + y * 10) as u8,
                90,
                if x < 2 { 0 } else { 255 },
            ])
        })
        .save(dir.join("sprite.png"))
        .unwrap();
        fs::write(dir.join("gb.hex"), "081820\n346856\n88c070\ne0f8d0\n").unwrap();
        Self { dir }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_spritefusion-palette-snapper"))
            .current_dir(&self.dir)
            .args(args)
            .output()
            .unwrap()
    }

    fn input(&self) -> Vec<u8> {
        fs::read(self.dir.join("sprite.png")).unwrap()
    }

    fn output(&self, name: &str) -> RgbaImage {
        decode_image(&fs::read(self.dir.join(name)).unwrap()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn succeeds(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn reduces_without_a_palette_and_snaps_with_one() {
    let fixture = Fixture::new();
    succeeds(&fixture.run(&["sprite.png", "--max-colors", "8"]));
    let expected = reduce_colors(
        &fixture.input(),
        &ReduceOptions {
            max_colors: Some(8),
            ..ReduceOptions::default()
        },
    );
    assert_eq!(
        fixture.output("sprite_snapped.png"),
        decode_image(&expected.unwrap()).unwrap()
    );

    succeeds(&fixture.run(&[
        "sprite.png",
        "-o",
        "gb.png",
        "-p",
        "gb.hex",
        "-m",
        "lightness",
    ]));
    let options = SnapOptions {
        mode: SnapMode::Lightness,
    };
    let expected = snap_to_palette(&fixture.input(), &GAME_BOY, &options).unwrap();
    assert_eq!(fixture.output("gb.png"), decode_image(&expected).unwrap());

    succeeds(&fixture.run(&["sprite.png", "-o", "bw.png", "--palette", "#000000,#ffffff"]));
    let expected = snap_to_palette(
        &fixture.input(),
        &[[0, 0, 0], [255, 255, 255]],
        &SnapOptions::default(),
    );
    assert_eq!(
        fixture.output("bw.png"),
        decode_image(&expected.unwrap()).unwrap()
    );
}

#[test]
fn a_palette_can_be_an_image_of_its_swatches() {
    let fixture = Fixture::new();
    RgbaImage::from_fn(4, 1, |x, _| {
        let [red, green, blue] = GAME_BOY[x as usize];
        Rgba([red, green, blue, 255])
    })
    .save(fixture.dir.join("gb.png"))
    .unwrap();
    succeeds(&fixture.run(&["sprite.png", "-o", "a.png", "-p", "gb.png"]));
    succeeds(&fixture.run(&["sprite.png", "-o", "b.png", "-p", "gb.hex"]));
    assert_eq!(fixture.output("a.png"), fixture.output("b.png"));
}

#[test]
fn options_apply_with_or_without_a_palette_only() {
    let fixture = Fixture::new();
    for args in [
        ["sprite.png", "-p", "gb.hex", "-c", "8"].as_slice(),
        &["sprite.png", "-p", "gb.hex", "-t", "0.1"],
        &["sprite.png", "-m", "lightness"],
        &["sprite.png", "-c", "0"],
        &["sprite.png", "-t", "-1"],
        &["sprite.png", "-p", "#0818"],
    ] {
        assert!(!fixture.run(args).status.success(), "{args:?} should fail");
    }
    succeeds(&fixture.run(&["sprite.png", "-c", "none", "-t", "0"]));
    let visible = |image: RgbaImage| -> Vec<[u8; 4]> {
        image
            .pixels()
            .map(|pixel| if pixel[3] == 0 { [0; 4] } else { pixel.0 })
            .collect()
    };
    assert_eq!(
        visible(fixture.output("sprite_snapped.png")),
        visible(decode_image(&fixture.input()).unwrap())
    );
}

#[test]
fn requires_force_to_replace_and_never_overwrites_an_input() {
    let fixture = Fixture::new();
    succeeds(&fixture.run(&["sprite.png"]));
    assert!(!fixture.run(&["sprite.png"]).status.success());
    succeeds(&fixture.run(&["sprite.png", "--force"]));
    let before = fixture.input();
    for path in ["sprite.png", "missing/../sprite.png"] {
        assert!(
            !fixture
                .run(&["sprite.png", "-o", path, "--force"])
                .status
                .success()
        );
    }
    assert_eq!(fixture.input(), before);
}

#[test]
fn batches_go_to_an_output_directory() {
    let fixture = Fixture::new();
    fs::copy(
        fixture.dir.join("sprite.png"),
        fixture.dir.join("other.png"),
    )
    .unwrap();
    succeeds(&fixture.run(&[
        "sprite.png",
        "other.png",
        "--output-dir",
        "out",
        "-p",
        "gb.hex",
    ]));
    assert!(fixture.dir.join("out/sprite_snapped.png").exists());
    assert!(fixture.dir.join("out/other_snapped.png").exists());
    assert!(
        !fixture
            .run(&["sprite.png", "other.png", "-o", "one.png"])
            .status
            .success()
    );
}
