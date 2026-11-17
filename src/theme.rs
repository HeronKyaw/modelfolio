//! Theme system: bundled Inter + JetBrains Mono, Phosphor icons, dark/light
//! palettes with an accent color. Defaults to following the OS theme.

use crate::store::config::ThemePref;
use egui::{Color32, Context, FontData, FontDefinitions, FontFamily, Style, TextStyle, Visuals};

pub fn init(ctx: &Context, pref: ThemePref, accent_hex: &str) {
    setup_fonts(ctx);
    apply(ctx, pref, accent_hex);
}

fn setup_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "inter".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Inter-Regular.otf"
        ))),
    );
    fonts.font_data.insert(
        "inter_semibold".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Inter-SemiBold.otf"
        ))),
    );
    fonts.font_data.insert(
        "jetbrains".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/JetBrainsMono-Regular.ttf"
        ))),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".into());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "jetbrains".into());
    fonts.families.insert(
        FontFamily::Name("semibold".into()),
        vec!["inter_semibold".into(), "inter".into()],
    );
    // Phosphor icon font becomes a fallback of proportional text, so icons
    // can be mixed inline with ordinary labels.
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

/// Resolve the effective dark flag from the preference.
pub fn is_dark(pref: ThemePref) -> bool {
    match pref {
        ThemePref::Dark => true,
        ThemePref::Light => false,
        ThemePref::System => system_dark(),
    }
}

fn system_dark() -> bool {
    match dark_light::detect() {
        Ok(dark_light::Mode::Dark) => true,
        Ok(dark_light::Mode::Light) => false,
        // Unspecified (or detection error): dark is the safer default.
        _ => true,
    }
}

pub fn apply(ctx: &Context, pref: ThemePref, accent_hex: &str) {
    let dark = is_dark(pref);
    let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
    ctx.set_theme(theme);
    ctx.set_style_of(theme, style(dark, accent_color(accent_hex)));
}

fn style(dark: bool, accent: Color32) -> Style {
    let mut style = Style {
        visuals: if dark { Visuals::dark() } else { Visuals::light() },
        ..Style::default()
    };
    style.visuals.panel_fill = if dark {
        Color32::from_rgb(18, 18, 22)
    } else {
        Color32::from_rgb(246, 247, 249)
    };
    style.visuals.window_fill = if dark {
        Color32::from_rgb(26, 26, 31)
    } else {
        Color32::from_rgb(252, 252, 253)
    };
    style.visuals.extreme_bg_color = if dark {
        Color32::from_rgb(12, 12, 15)
    } else {
        Color32::from_rgb(255, 255, 255)
    };
    style.visuals.selection.bg_fill = accent.gamma_multiply(0.35);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, accent);
    style.visuals.hyperlink_color = accent;

    let v = &mut style.visuals;
    v.window_corner_radius = egui::CornerRadius::same(12);
    v.menu_corner_radius = egui::CornerRadius::same(8);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open, &mut v.widgets.noninteractive] {
        w.corner_radius = egui::CornerRadius::same(6);
    }

    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.menu_margin = egui::Margin::same(6);

    // Text: headings use the semibold family.
    let sizes = [
        (TextStyle::Small, 12.0),
        (TextStyle::Body, 14.0),
        (TextStyle::Button, 14.0),
        (TextStyle::Heading, 17.0),
        (TextStyle::Monospace, 13.0),
    ];
    for (ts, size) in sizes {
        style
            .text_styles
            .entry(ts)
            .and_modify(|f| f.size = size)
            .or_insert_with(|| egui::FontId::new(size, FontFamily::Proportional));
    }
    if let Some(f) = style.text_styles.get_mut(&TextStyle::Heading) {
        f.family = FontFamily::Name("semibold".into());
    }
    style
}

/// Parse "#0a84ff" or "0a84ff"; fall back to a pleasant blue.
pub fn accent_color(hex: &str) -> Color32 {
    let s = hex.trim_start_matches('#');
    if s.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) {
            return Color32::from_rgb(r, g, b);
        }
    }
    Color32::from_rgb(0x0a, 0x84, 0xff)
}

pub const ACCENT_PRESETS: &[(&str, &str)] = &[
    ("Blue", "#0a84ff"),
    ("Purple", "#7c5cff"),
    ("Teal", "#2dd4bf"),
    ("Pink", "#f472b6"),
    ("Orange", "#f97316"),
    ("Green", "#34c759"),
];
