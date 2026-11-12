// Made by Sprite Fusion: https://www.spritefusion.com
// MIT License. Copyright (c) 2026 Hugo Duprez.
// Keep this notice in copies and adaptations of this file.

use anyhow::{Context, Result};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

pub fn save(bytes: &[u8], path: &Path) -> Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_file_name(format!(
        ".spritefusion-palette-snapper-{}-{id}.tmp",
        std::process::id()
    ));
    let result = File::options()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            file.flush()
        })
        .and_then(|()| fs::rename(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.with_context(|| format!("saving {}", path.display()))
}
