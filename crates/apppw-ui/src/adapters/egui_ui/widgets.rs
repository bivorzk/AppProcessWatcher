use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Stroke, TextWrapMode};

use super::theme::*;

pub(super) fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).font(bold_font(9.0)).color(DIM).strong());
}

pub(super) fn process_button(
    ui: &mut egui::Ui,
    selected: bool,
    active: bool,
    name: &str,
    detail: &str,
) -> bool {
    let fill = if selected {
        Color32::from_rgb(18, 48, 70)
    } else {
        Color32::TRANSPARENT
    };
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(
            1.0,
            if selected {
                ACCENT
            } else {
                Color32::TRANSPARENT
            },
        ))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(Margin::symmetric(9, 7))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("●")
                        .size(9.0)
                        .color(if active { SUCCESS } else { DIM }),
                );
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(name)
                            .font(bold_font(12.0))
                            .color(if selected { TEXT } else { MUTED })
                            .strong(),
                    );
                    ui.label(RichText::new(detail).size(9.0).color(DIM));
                });
            });
        })
        .response
        .interact(egui::Sense::click())
        .clicked()
}

pub(super) fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).font(bold_font(11.0)).color(BG).strong())
            .fill(ACCENT)
            .stroke(Stroke::NONE)
            .corner_radius(5),
    )
}

pub(super) fn secondary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(MUTED))
            .fill(PANEL)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(5),
    )
}

pub(super) fn badge(ui: &mut egui::Ui, text: &str, colour: Color32) {
    Frame::new()
        .fill(colour.gamma_multiply(0.12))
        .stroke(Stroke::new(1.0, colour.gamma_multiply(0.65)))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .font(bold_font(9.0))
                    .color(colour)
                    .strong(),
            );
        });
}

pub(super) fn table_cell(ui: &mut egui::Ui, text: &str, width: f32, colour: Color32, strong: bool) {
    let rich_text = RichText::new(text).size(10.5).color(colour);
    ui.add_sized(
        [width, 18.0],
        egui::Label::new(if strong {
            rich_text.font(bold_font(10.5)).strong()
        } else {
            rich_text
        })
        .truncate(),
    );
}

pub(super) fn status_colour(status: Option<u16>) -> Color32 {
    match status {
        Some(200..=299) => SUCCESS,
        Some(300..=399) => WARNING,
        Some(400..) => DANGER,
        _ => DIM,
    }
}

pub(super) fn key_values(ui: &mut egui::Ui, rows: &[(&str, String)]) {
    egui::ScrollArea::both()
        .id_salt("details_values")
        .max_height(220.0)
        .show(ui, |ui| {
            egui::Grid::new("details_grid")
                .num_columns(2)
                .spacing([28.0, 9.0])
                .show(ui, |ui| {
                    for (key, value) in rows {
                        ui.label(
                            RichText::new(*key)
                                .font(bold_font(10.0))
                                .color(DIM)
                                .strong(),
                        );
                        ui.add(
                            egui::Label::new(
                                RichText::new(value).monospace().size(11.0).color(TEXT),
                            )
                            .wrap_mode(TextWrapMode::Extend),
                        );
                        ui.end_row();
                    }
                });
        });
}

pub(super) fn code_block(ui: &mut egui::Ui, text: &str) {
    Frame::new()
        .fill(Color32::from_rgb(5, 10, 20))
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .id_salt("body_preview")
                .max_height(260.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(text).monospace().size(11.0).color(ACCENT_2),
                        )
                        .wrap_mode(TextWrapMode::Extend)
                        .selectable(true),
                    );
                });
        });
}

pub(super) fn format_time(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
