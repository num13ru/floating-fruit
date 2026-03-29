use std::{fs, path::Path};

use anyhow::{Context, Result};
use eframe::egui::{self, ColorImage, TextureHandle};

const MAX_TEXTURE_DIM: u32 = 400;

pub fn load_from_file(ctx: &egui::Context, path: &Path) -> Result<TextureHandle> {
    let bytes = fs::read(path).with_context(|| format!("Failed reading {}", path.display()))?;
    let mut image = image::load_from_memory(&bytes)
        .with_context(|| format!("Failed decoding {}", path.display()))?;

    if image.width() > MAX_TEXTURE_DIM || image.height() > MAX_TEXTURE_DIM {
        image = image.resize(
            MAX_TEXTURE_DIM,
            MAX_TEXTURE_DIM,
            image::imageops::FilterType::Lanczos3,
        );
    }

    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels = rgba.into_vec();
    let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);

    Ok(ctx.load_texture("album_art", color_image, egui::TextureOptions::LINEAR))
}
