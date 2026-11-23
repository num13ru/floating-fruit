//! Inter font setup for egui.

use std::sync::Arc;

use eframe::egui;

pub const INTER_FAMILY: &str = "Inter Variable";
pub const INTER_BYTES: &[u8] = include_bytes!("../assets/fonts/InterVariable.ttf");

pub fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    let data = egui::FontData::from_static(INTER_BYTES);
    fonts
        .font_data
        .insert(INTER_FAMILY.to_string(), Arc::new(data));
    // Inter first, built-in fonts as fallback for glyphs Inter lacks (icons).
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, INTER_FAMILY.to_string());
    fonts
}
