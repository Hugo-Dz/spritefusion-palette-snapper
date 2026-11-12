// Made by Sprite Fusion: https://www.spritefusion.com
// MIT License. Copyright (c) 2026 Hugo Duprez.
// Keep this notice in copies and adaptations of this file.

mod output;

use anyhow::{Context, Result, bail};
use clap::Parser;
use spritefusion_palette_snapper::{
    DEFAULT_MAX_COLORS, DEFAULT_THRESHOLD, MAX_INPUT_BYTES, ReduceOptions, Rgb, SnapMode,
    SnapOptions, palette_from_image, parse_palette, reduce_colors, snap_to_palette,
};
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

/// Apply any palette to your pixel art, or reduce it to a few clean colors.
///
/// Without --palette, reduces each image's colors. With one, snaps each image to the palette.
#[derive(Debug, Parser)]
#[command(name = "spritefusion-palette-snapper", version)]
struct Args {
    /// Input images: PNG, GIF, WebP or JPEG.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
    /// Output PNG (single input only).
    #[arg(short, long, conflicts_with = "output_dir")]
    output: Option<PathBuf>,
    /// Write <input-stem>_snapped.png here. Defaults to beside each input.
    #[arg(long)]
    output_dir: Option<PathBuf>,
    /// Replace existing outputs.
    #[arg(long)]
    force: bool,
    /// Snap to a palette: a palette file (.hex, .txt, .gpl, .json), an image of its swatches, or hex colors
    /// ("#081820,#346856,#88c070,#e0f8d0").
    #[arg(short, long)]
    palette: Option<String>,
    /// With a palette: how colors map onto it.
    #[arg(short, long, value_enum, requires = "palette")]
    mode: Option<SnapMode>,
    /// Without a palette: at most this many colors, or "none". Default: 32.
    #[arg(short = 'c', long, value_parser = parse_max_colors, conflicts_with = "palette")]
    max_colors: Option<MaxColors>,
    /// Without a palette: colors closer than this become one color (OKLab distance). Default: 0.03.
    #[arg(short, long, conflicts_with = "palette")]
    threshold: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
struct MaxColors(Option<usize>);

fn parse_max_colors(value: &str) -> Result<MaxColors, String> {
    if value.eq_ignore_ascii_case("none") {
        return Ok(MaxColors(None));
    }
    match value.parse::<usize>() {
        Ok(count) if count >= 1 => Ok(MaxColors(Some(count))),
        _ => Err("expected a whole number from 1, or none".into()),
    }
}

const IMAGE_EXTENSIONS: [&str; 9] = [
    "png", "gif", "webp", "jpg", "jpeg", "avif", "tif", "tiff", "bmp",
];

fn read_palette(value: &str) -> Result<Vec<Rgb>> {
    let path = Path::new(value);
    if !path.exists() {
        return Ok(parse_palette(value)?);
    }
    let is_image = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        });
    if is_image {
        palette_from_image(&read_input(path)?)
            .with_context(|| format!("reading the palette {value}"))
    } else {
        let text =
            fs::read_to_string(path).with_context(|| format!("reading the palette {value}"))?;
        parse_palette(&text).with_context(|| format!("reading the palette {value}"))
    }
}

struct Job {
    input: PathBuf,
    output: PathBuf,
}

fn prepare_jobs(args: &Args) -> Result<Vec<Job>> {
    if args.output.is_some() && args.inputs.len() != 1 {
        bail!("--output requires exactly one input; use --output-dir for a batch");
    }
    let sources = args
        .inputs
        .iter()
        .map(fs::canonicalize)
        .collect::<std::io::Result<HashSet<_>>>()
        .context("cannot find an input file")?;
    let mut outputs = HashSet::new();
    let mut jobs = Vec::new();
    for input in &args.inputs {
        let stem = input.file_stem().context("input filename has no stem")?;
        let output = args.output.clone().unwrap_or_else(|| {
            let mut filename = stem.to_os_string();
            filename.push("_snapped.png");
            args.output_dir
                .as_deref()
                .unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")))
                .join(filename)
        });
        if output
            .extension()
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("png"))
        {
            bail!("output must have a .png extension");
        }
        let absolute = normalized_output(&output)?;
        if sources.contains(&absolute) {
            bail!("refusing to overwrite input {}", output.display());
        }
        if !outputs.insert(absolute) {
            bail!("duplicate output name: {}", output.display());
        }
        if output.is_dir() {
            bail!("output is a directory: {}", output.display());
        }
        if output.exists() && !args.force {
            bail!(
                "output already exists: {}; use --force to replace outputs",
                output.display()
            );
        }
        jobs.push(Job {
            input: input.clone(),
            output,
        });
    }
    Ok(jobs)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let palette = args.palette.as_deref().map(read_palette).transpose()?;
    let reduce = ReduceOptions {
        max_colors: args
            .max_colors
            .map_or(Some(DEFAULT_MAX_COLORS), |cap| cap.0),
        threshold: args.threshold.unwrap_or(DEFAULT_THRESHOLD),
    };
    let snap = SnapOptions {
        mode: args.mode.unwrap_or_default(),
    };
    for job in prepare_jobs(&args)? {
        let image = read_input(&job.input)?;
        let png = match &palette {
            Some(palette) => snap_to_palette(&image, palette, &snap),
            None => reduce_colors(&image, &reduce),
        }
        .with_context(|| format!("processing {}", job.input.display()))?;
        output::save(&png, &job.output)?;
        eprintln!("{} -> {}", job.input.display(), job.output.display());
    }
    Ok(())
}

fn read_input(path: &Path) -> Result<Vec<u8>> {
    if fs::metadata(path)?.len() > MAX_INPUT_BYTES as u64 {
        bail!("{} exceeds the 64 MiB input limit", path.display());
    }
    fs::read(path).with_context(|| format!("reading {}", path.display()))
}

fn normalized_output(path: &Path) -> Result<PathBuf> {
    let mut resolved = PathBuf::new();
    for component in std::path::absolute(path)?.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            other => {
                resolved.push(other.as_os_str());
                if resolved.exists() {
                    resolved = fs::canonicalize(&resolved)?;
                }
            }
        }
    }
    Ok(resolved)
}
