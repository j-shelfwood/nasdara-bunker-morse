use std::path::PathBuf;
use std::sync::Arc;

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle};

pub const BG: Color32 = Color32::from_rgb(0x0b, 0x0e, 0x12);
pub const SURFACE: Color32 = Color32::from_rgb(0x12, 0x16, 0x1c);
pub const SURFACE_HI: Color32 = Color32::from_rgb(0x19, 0x1e, 0x26);
pub const STROKE: Color32 = Color32::from_rgb(0x25, 0x2c, 0x37);
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xea, 0xf0);
pub const DIM: Color32 = Color32::from_rgb(0x8b, 0x95, 0xa4);
pub const FAINT: Color32 = Color32::from_rgb(0x4b, 0x54, 0x62);
pub const AMBER: Color32 = Color32::from_rgb(0xff, 0xb0, 0x20);
pub const TEAL: Color32 = Color32::from_rgb(0x3d, 0xdb, 0xd9);
pub const RED: Color32 = Color32::from_rgb(0xff, 0x6b, 0x57);
pub const BLUE: Color32 = Color32::from_rgb(0x5a, 0xa9, 0xff);

pub fn alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn digits(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("digits".into()))
}

pub fn text(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Windows ships Segoe UI and Bahnschrift, so the app looks native without bundling fonts. Elsewhere (development
/// previews) `NASDARA_FONT_DIR` can point at a folder with copies; without them egui's built-in fonts are used.
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from(r"C:\Windows\Fonts")];
    if let Some(d) = std::env::var_os("NASDARA_FONT_DIR") {
        dirs.insert(0, PathBuf::from(d));
    }
    dirs
}

fn load(file: &str) -> Option<Arc<FontData>> {
    font_dirs()
        .iter()
        .find_map(|d| std::fs::read(d.join(file)).ok())
        .map(|b| Arc::new(FontData::from_owned(b)))
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fallback_prop = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let fallback_mono = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    let mut family = |name: &str, files: &[&str], fallback: &[String]| -> Vec<String> {
        let mut list = Vec::new();
        for file in files {
            if let Some(data) = load(file) {
                fonts.font_data.insert(format!("{name}:{file}"), data);
                list.push(format!("{name}:{file}"));
            }
        }
        list.extend(fallback.iter().cloned());
        list
    };
    let regular = family("regular", &["segoeui.ttf"], &fallback_prop);
    let strong = family("bold", &["segoeuib.ttf", "segoeui.ttf"], &fallback_prop);
    let figures = family("digits", &["bahnschrift.ttf"], &fallback_mono);
    fonts.families.insert(FontFamily::Proportional, regular);
    fonts
        .families
        .insert(FontFamily::Name("bold".into()), strong);
    fonts
        .families
        .insert(FontFamily::Name("digits".into()), figures);
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, bold(20.0)),
            (TextStyle::Body, text(14.0)),
            (TextStyle::Button, text(14.0)),
            (TextStyle::Small, text(12.0)),
            (TextStyle::Monospace, digits(14.0)),
        ]
        .into();
        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = Some(TEXT);
        v.panel_fill = BG;
        v.window_fill = SURFACE;
        v.extreme_bg_color = SURFACE;
        v.faint_bg_color = SURFACE;
        v.window_stroke = Stroke::new(1.0, STROKE);
        v.window_corner_radius = 12.0.into();
        v.menu_corner_radius = 8.0.into();
        v.selection.bg_fill = alpha(AMBER, 0.35);
        v.selection.stroke = Stroke::new(1.0, AMBER);
        v.hyperlink_color = BLUE;
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = 8.0.into();
        }
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, STROKE);
        v.widgets.inactive.bg_fill = SURFACE_HI;
        v.widgets.inactive.weak_bg_fill = SURFACE_HI;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, STROKE);
        v.widgets.hovered.bg_fill = mix(SURFACE_HI, TEXT, 0.08);
        v.widgets.hovered.weak_bg_fill = mix(SURFACE_HI, TEXT, 0.08);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, FAINT);
        v.widgets.active.bg_fill = mix(SURFACE_HI, TEXT, 0.14);
        v.widgets.active.weak_bg_fill = mix(SURFACE_HI, TEXT, 0.14);
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(14.0, 7.0);
        style.interaction.tooltip_delay = 0.2;
    });
}
