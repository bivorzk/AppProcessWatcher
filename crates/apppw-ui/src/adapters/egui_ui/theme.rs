use std::sync::Arc;

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, Vec2};

const CONSOLAS: &str = "consolas";
const CONSOLAS_BOLD: &str = "consolas-bold";
const BOLD_FAMILY: &str = "Consolas Bold";

pub(super) const BG: Color32 = Color32::from_rgb(7, 11, 22);
pub(super) const PANEL: Color32 = Color32::from_rgb(14, 20, 37);
pub(super) const PANEL_ALT: Color32 = Color32::from_rgb(10, 17, 32);
pub(super) const LINE: Color32 = Color32::from_rgb(38, 59, 104);
pub(super) const TEXT: Color32 = Color32::from_rgb(237, 247, 255);
pub(super) const MUTED: Color32 = Color32::from_rgb(169, 182, 203);
pub(super) const DIM: Color32 = Color32::from_rgb(114, 129, 159);
pub(super) const ACCENT: Color32 = Color32::from_rgb(101, 217, 255);
pub(super) const ACCENT_2: Color32 = Color32::from_rgb(200, 215, 255);
pub(super) const DANGER: Color32 = Color32::from_rgb(255, 107, 138);
pub(super) const SUCCESS: Color32 = Color32::from_rgb(100, 232, 183);
pub(super) const WARNING: Color32 = Color32::from_rgb(255, 207, 112);

pub(super) fn apply_theme(context: &egui::Context) {
    install_fonts(context);
    context.set_theme(egui::Theme::Dark);
    let mut style = (*context.style_of(egui::Theme::Dark)).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = PANEL_ALT;
    style.visuals.faint_bg_color = Color32::from_rgb(18, 29, 51);
    style.visuals.selection.bg_fill = Color32::from_rgb(20, 61, 86);
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, MUTED);
    style.visuals.widgets.inactive.bg_fill = PANEL;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(19, 44, 68);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(25, 65, 88);
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    style.spacing.item_spacing = Vec2::new(8.0, 7.0);
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::new(12.0, FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::new(11.0, FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        FontId::new(11.0, FontFamily::Monospace),
    );
    context.set_style_of(egui::Theme::Dark, style);
}

pub(super) fn bold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD_FAMILY.into()))
}

fn install_fonts(context: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fallback = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\consola.ttf") {
        fonts
            .font_data
            .insert(CONSOLAS.into(), Arc::new(FontData::from_owned(bytes)));
        for family in [FontFamily::Monospace, FontFamily::Proportional] {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, CONSOLAS.into());
        }
    }

    let mut bold_family = fallback;
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\consolab.ttf") {
        fonts
            .font_data
            .insert(CONSOLAS_BOLD.into(), Arc::new(FontData::from_owned(bytes)));
        bold_family.insert(0, CONSOLAS_BOLD.into());
    }
    fonts
        .families
        .insert(FontFamily::Name(BOLD_FAMILY.into()), bold_family);
    context.set_fonts(fonts);
}
