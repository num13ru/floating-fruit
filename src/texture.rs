use std::{fs, path::Path};

use anyhow::{Context, Result};
use eframe::egui::{self, ColorImage, TextureHandle};

pub fn load_from_file(ctx: &egui::Context, path: &Path) -> Result<TextureHandle> {
    let bytes = fs::read(path).with_context(|| format!("Failed reading {}", path.display()))?;
    let image = image::load_from_memory(&bytes)
        .with_context(|| format!("Failed decoding {}", path.display()))?
        .to_rgba8();

    let size = [image.width() as usize, image.height() as usize];
    let pixels = image.into_vec();
    let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);

    Ok(ctx.load_texture("album_art", color_image, egui::TextureOptions::LINEAR))
}
