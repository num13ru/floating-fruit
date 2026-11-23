//! Font setup for egui: Inter (proportional), IBM Plex Mono (monospace time),
//! Noto Sans CJK fallbacks.

use std::sync::Arc;

use eframe::egui;

pub const INTER_FAMILY: &str = "Inter Variable";
pub const INTER_BYTES: &[u8] = include_bytes!("../assets/fonts/InterVariable.ttf");

pub const IBMPLEX_MONO_FAMILY: &str = "IBM Plex Mono";
pub const IBM_PLEX_MONO_BYTES: &[u8] = include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf");

const CJK_FONTS: &[(&str, &[u8])] = &[
    (
        "Noto Sans CJK SC",
        include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf"),
    ),
    (
        "Noto Sans CJK TC",
        include_bytes!("../assets/fonts/NotoSansCJKtc-Regular.otf"),
    ),
    (
        "Noto Sans CJK JP",
        include_bytes!("../assets/fonts/NotoSansCJKjp-Regular.otf"),
    ),
    (
        "Noto Sans CJK KR",
        include_bytes!("../assets/fonts/NotoSansCJKkr-Regular.otf"),
    ),
];

pub fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        INTER_FAMILY.to_string(),
        Arc::new(egui::FontData::from_static(INTER_BYTES)),
    );
    for (family, bytes) in CJK_FONTS {
        fonts.font_data.insert(
            family.to_string(),
            Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    fonts.font_data.insert(
        IBMPLEX_MONO_FAMILY.to_string(),
        Arc::new(egui::FontData::from_static(IBM_PLEX_MONO_BYTES)),
    );
    // Inter first, then CJK fallbacks, then built-in fonts for icons.
    let list = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
    list.clear();
    list.push(INTER_FAMILY.to_string());
    for (family, _) in CJK_FONTS {
        list.push(family.to_string());
    }
    // Built-in fallbacks retained: Ubuntu-Light (proportional), NotoEmoji-Regular,
    // emoji-icon-font (icon glyphs).
    list.push("Ubuntu-Light".to_string());
    list.push("NotoEmoji-Regular".to_string());
    list.push("emoji-icon-font".to_string());

    // Monospace family: IBM Plex Mono first, built-ins retained.
    let monospace = fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default();
    monospace.clear();
    monospace.push(IBMPLEX_MONO_FAMILY.to_string());
    monospace.push("Hack".to_string());
    monospace.push("Ubuntu-Light".to_string());
    monospace.push("NotoEmoji-Regular".to_string());
    monospace.push("emoji-icon-font".to_string());

    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inter_precedes_cjk_then_builtin_fallback() {
        let fonts = font_definitions();
        let list = &fonts.families[&egui::FontFamily::Proportional];
        assert_eq!(list[0], INTER_FAMILY);
        assert_eq!(list.len(), 8); // Inter + 4 CJK + 3 built-ins
        assert!(list.contains(&"Noto Sans CJK SC".to_string()));
        assert!(list.contains(&"Noto Sans CJK TC".to_string()));
        assert!(list.contains(&"Noto Sans CJK JP".to_string()));
        assert!(list.contains(&"Noto Sans CJK KR".to_string()));
        assert_eq!(list.last().map(String::as_str), Some("emoji-icon-font"));
        assert_eq!(list[1], "Noto Sans CJK SC");

        for family in list {
            assert!(
                fonts.font_data.contains_key(family),
                "missing font_data for {family}"
            );
        }
    }
}

#[cfg(test)]
mod mono_tests {
    use super::*;

    #[test]
    fn monospace_family_starts_with_ibmplex() {
        let fonts = font_definitions();
        let list = &fonts.families[&egui::FontFamily::Monospace];
        assert_eq!(list[0], IBMPLEX_MONO_FAMILY);
        assert!(fonts.font_data.contains_key(IBMPLEX_MONO_FAMILY));
        assert!(list.contains(&"Hack".to_string()));
    }
}
