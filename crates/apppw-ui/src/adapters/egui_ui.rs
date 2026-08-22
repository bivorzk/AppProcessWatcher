use std::collections::BTreeSet;

use eframe::egui::{self, Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke};

use crate::{
    application::{AppState, DetailTab, validate_filter},
    domain::EventKind,
};

mod theme;
mod widgets;

use theme::*;
use widgets::*;

impl EventKind {
    fn colour(self) -> Color32 {
        match self {
            Self::Http | Self::Https => SUCCESS,
            Self::Tls => ACCENT_2,
            Self::Tcp => ACCENT,
            Self::Udp => WARNING,
            Self::Quic => Color32::from_rgb(147, 178, 255),
            Self::Unknown => DIM,
        }
    }
}

pub struct AppWatch {
    state: AppState,
}

impl std::ops::Deref for AppWatch {
    type Target = AppState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl std::ops::DerefMut for AppWatch {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl AppWatch {
    pub fn new(context: &eframe::CreationContext<'_>, state: AppState) -> Self {
        apply_theme(&context.egui_ctx);
        Self { state }
    }

    fn panel() -> Frame {
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(14))
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("AW")
                    .font(bold_font(19.0))
                    .strong()
                    .color(BG)
                    .background_color(ACCENT),
            );
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("APPWATCH")
                        .font(bold_font(17.0))
                        .strong()
                        .color(TEXT),
                );
                ui.label(RichText::new("NETWORK OBSERVER").size(9.0).color(DIM));
            });
        });
        ui.add_space(24.0);
        section_label(ui, "PROCESSES");
        ui.add_space(6.0);

        if process_button(
            ui,
            self.selected_pid.is_none(),
            true,
            "All processes",
            "6 apps",
        ) {
            self.selected_pid = None;
            self.selected_event = None;
        }

        let mut next_pid = None;
        for process in &self.processes {
            let selected = self.selected_pid == Some(process.pid);
            if process_button(
                ui,
                selected,
                process.active,
                process.name,
                &format!("PID {}", process.pid),
            ) {
                next_pid = Some(process.pid);
            }
        }
        if let Some(pid) = next_pid {
            self.selected_pid = Some(pid);
            self.selected_event = self
                .events
                .iter()
                .find(|event| event.pid == pid)
                .map(|event| event.id);
        }

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(8.0);
            Frame::new()
                .fill(PANEL_ALT)
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("●").color(SUCCESS).size(11.0));
                        ui.label(
                            RichText::new("CAPTURE ONLINE")
                                .font(bold_font(10.0))
                                .color(MUTED)
                                .strong(),
                        );
                    });
                    ui.label(
                        RichText::new("WinDivert · local session")
                            .color(DIM)
                            .size(10.0),
                    );
                });
        });
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("LIVE TRAFFIC")
                        .font(bold_font(10.0))
                        .color(ACCENT)
                        .strong(),
                );
                ui.label(
                    RichText::new(self.selected_process_name())
                        .font(bold_font(23.0))
                        .color(TEXT)
                        .strong(),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if secondary_button(ui, "Export JSON").clicked() {
                    self.toast = Some("Export becomes available when storage is connected.".into());
                }
                if secondary_button(ui, "Clear").clicked() {
                    self.events.clear();
                    self.selected_event = None;
                    self.toast = Some("Current screen data cleared.".into());
                }
                let label = if self.recording {
                    "Stop recording"
                } else {
                    "Start recording"
                };
                if primary_button(ui, label).clicked() {
                    self.recording = !self.recording;
                    self.toast = Some(if self.recording {
                        "Recording started.".into()
                    } else {
                        "Recording stopped.".into()
                    });
                }
                ui.label(
                    RichText::new(if self.recording {
                        format!("●  {}", format_time(self.session_seconds))
                    } else {
                        "■  PAUSED".into()
                    })
                    .font(bold_font(11.0))
                    .color(if self.recording { DANGER } else { DIM })
                    .strong(),
                );
            });
        });
    }

    fn stats(&self, ui: &mut egui::Ui) {
        let events = self.visible_events();
        let sent = if events.is_empty() { "0 B" } else { "4.8 MB" };
        let received = if events.is_empty() { "0 B" } else { "43.1 MB" };
        let http_count = events
            .iter()
            .filter(|event| matches!(event.kind, EventKind::Http | EventKind::Https))
            .count();
        let hosts = events
            .iter()
            .map(|event| event.host)
            .collect::<BTreeSet<_>>()
            .len();
        let completed: Vec<u64> = events
            .iter()
            .filter_map(|event| event.duration_ms)
            .collect();
        let average = if completed.is_empty() {
            "---".into()
        } else {
            format!(
                "{} ms",
                completed.iter().sum::<u64>() / completed.len() as u64
            )
        };
        let cards = [
            ("ACTIVE", events.len().to_string(), ACCENT),
            ("HTTP REQUESTS", http_count.to_string(), SUCCESS),
            (
                "PACKETS",
                if events.is_empty() {
                    "0".into()
                } else {
                    "12,418".into()
                },
                ACCENT_2,
            ),
            ("UPLOADED", sent.into(), WARNING),
            ("DOWNLOADED", received.into(), ACCENT),
            ("HOSTS", hosts.to_string(), SUCCESS),
            ("AVG RESPONSE", average, ACCENT_2),
        ];

        ui.columns(cards.len(), |columns| {
            for (column, (label, value, colour)) in columns.iter_mut().zip(cards) {
                Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(10, 9))
                    .show(column, |ui| {
                        ui.label(
                            RichText::new(label)
                                .font(bold_font(9.0))
                                .color(DIM)
                                .strong(),
                        );
                        ui.label(
                            RichText::new(value)
                                .font(bold_font(17.0))
                                .color(colour)
                                .strong(),
                        );
                    });
            }
        });
    }

    fn filter_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("⌕").size(19.0).color(ACCENT));
            let response = ui.add_sized(
                [ui.available_width() - 96.0, 30.0],
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("Filter: process:Discord.exe protocol:TCP port:443")
                    .text_color(TEXT),
            );
            if response.changed() {
                self.filter_error = validate_filter(&self.filter).err();
            }
            ui.label(
                RichText::new(format!("{} shown", self.visible_events().len()))
                    .size(10.0)
                    .color(DIM),
            );
        });
        if let Some(error) = &self.filter_error {
            ui.label(RichText::new(error).size(10.0).color(DANGER));
        }
    }

    fn event_table(&mut self, ui: &mut egui::Ui) {
        let visible_ids: Vec<u64> = self.visible_events().iter().map(|event| event.id).collect();
        let widths = [70.0, 155.0, 245.0, 62.0, 120.0, 72.0, 78.0, 58.0];
        let headings = [
            "METHOD", "HOST", "PATH", "STATUS", "PROTOCOL", "SENT", "RECEIVED", "TIME",
        ];

        Frame::new()
            .fill(PANEL_ALT)
            .inner_margin(Margin::symmetric(8, 7))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (heading, width) in headings.iter().zip(widths) {
                        ui.add_sized(
                            [width, 18.0],
                            egui::Label::new(
                                RichText::new(*heading)
                                    .font(bold_font(9.0))
                                    .color(DIM)
                                    .strong(),
                            ),
                        );
                    }
                });
            });

        egui::ScrollArea::vertical()
            .max_height(250.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for id in visible_ids.iter().copied() {
                    let Some(event) = self.events.iter().find(|event| event.id == id) else {
                        continue;
                    };
                    let selected = self.selected_event == Some(event.id);
                    let row_fill = if selected {
                        Color32::from_rgb(18, 41, 65)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let response = Frame::new()
                        .fill(row_fill)
                        .stroke(Stroke::new(
                            1.0,
                            if selected {
                                ACCENT
                            } else {
                                Color32::TRANSPARENT
                            },
                        ))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(Margin::symmetric(8, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                table_cell(ui, event.method, widths[0], event.kind.colour(), true);
                                table_cell(ui, event.host, widths[1], TEXT, false);
                                table_cell(
                                    ui,
                                    if event.path.is_empty() {
                                        "—"
                                    } else {
                                        event.path
                                    },
                                    widths[2],
                                    MUTED,
                                    false,
                                );
                                table_cell(
                                    ui,
                                    &event
                                        .status
                                        .map_or_else(|| "---".into(), |status| status.to_string()),
                                    widths[3],
                                    status_colour(event.status),
                                    true,
                                );
                                table_cell(
                                    ui,
                                    event.kind.label(),
                                    widths[4],
                                    event.kind.colour(),
                                    false,
                                );
                                table_cell(ui, event.sent, widths[5], MUTED, false);
                                table_cell(ui, event.received, widths[6], MUTED, false);
                                table_cell(
                                    ui,
                                    &event
                                        .duration_ms
                                        .map_or_else(|| "---".into(), |time| format!("{time} ms")),
                                    widths[7],
                                    MUTED,
                                    false,
                                );
                            });
                        })
                        .response
                        .interact(egui::Sense::click());
                    if response.clicked() {
                        self.selected_event = Some(event.id);
                    }
                }

                if visible_ids.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(32.0);
                        ui.label(
                            RichText::new("NO MATCHING TRAFFIC")
                                .font(bold_font(12.0))
                                .color(DIM)
                                .strong(),
                        );
                        ui.label(
                            RichText::new("Adjust the filter or select another process.")
                                .color(DIM),
                        );
                        ui.add_space(32.0);
                    });
                }
            });
    }

    fn details(&mut self, ui: &mut egui::Ui) {
        let selected = self.selected_event().cloned();
        Self::panel().show(ui, |ui| {
            let Some(event) = selected else {
                ui.vertical_centered(|ui| {
                    ui.add_space(34.0);
                    ui.label(
                        RichText::new("SELECT AN EVENT")
                            .font(bold_font(12.0))
                            .color(DIM)
                            .strong(),
                    );
                    ui.label(
                        RichText::new("Request and connection details appear here.").color(DIM),
                    );
                    ui.add_space(34.0);
                });
                return;
            };

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(event.method)
                        .font(bold_font(14.0))
                        .strong()
                        .color(event.kind.colour()),
                );
                ui.label(
                    RichText::new(format!("{}{}", event.host, event.path))
                        .size(14.0)
                        .color(TEXT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    badge(ui, event.kind.label(), event.kind.colour());
                    ui.label(
                        RichText::new(format!("#{}", event.id))
                            .size(10.0)
                            .color(DIM),
                    );
                });
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                for tab in DetailTab::ALL {
                    let selected = self.detail_tab == tab;
                    if ui
                        .selectable_label(
                            selected,
                            RichText::new(tab.label()).color(if selected { ACCENT } else { MUTED }),
                        )
                        .clicked()
                    {
                        self.detail_tab = tab;
                    }
                }
            });
            ui.separator();
            ui.add_space(5.0);

            match self.detail_tab {
                DetailTab::Headers => key_values(
                    ui,
                    &[
                        ("content-type", "application/json".into()),
                        ("accept", "application/json".into()),
                        ("authorisation", "[REDACTED]".into()),
                        ("user-agent", "AppWatch demo client".into()),
                    ],
                ),
                DetailTab::RequestBody => {
                    code_block(ui, "{\n  \"content\": \"Hello from AppWatch\"\n}")
                }
                DetailTab::Response => key_values(
                    ui,
                    &[
                        (
                            "Status",
                            event.status.map_or_else(
                                || "Encrypted / unavailable".into(),
                                |value| format!("{value} OK"),
                            ),
                        ),
                        ("Content type", "application/json".into()),
                        ("Transferred", event.received.into()),
                    ],
                ),
                DetailTab::Timing => key_values(
                    ui,
                    &[
                        ("Queueing", "4 ms".into()),
                        ("Connection", "18 ms".into()),
                        ("Waiting (TTFB)", "47 ms".into()),
                        ("Download", "5 ms".into()),
                        (
                            "Total",
                            event
                                .duration_ms
                                .map_or_else(|| "---".into(), |value| format!("{value} ms")),
                        ),
                    ],
                ),
                DetailTab::Connection => key_values(
                    ui,
                    &[
                        ("Process", event.process.into()),
                        ("PID", event.pid.to_string()),
                        ("Local", event.local.into()),
                        ("Remote", event.remote.into()),
                        (
                            "Transport",
                            if matches!(event.kind, EventKind::Udp | EventKind::Quic) {
                                "UDP".into()
                            } else {
                                "TCP".into()
                            },
                        ),
                        ("Captured using", "WinDivert".into()),
                    ],
                ),
            }
        });
    }
}

impl eframe::App for AppWatch {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick();
        if self.recording {
            context.request_repaint_after(std::time::Duration::from_secs(1));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("processes")
            .exact_size(220.0)
            .frame(Frame::new().fill(PANEL_ALT).inner_margin(Margin::same(14)))
            .show(ui, |ui| self.sidebar(ui));

        egui::Panel::top("top_bar")
            .frame(
                Frame::new()
                    .fill(BG)
                    .inner_margin(Margin::symmetric(18, 14)),
            )
            .show(ui, |ui| self.top_bar(ui));

        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(18, 8)))
            .show(ui, |ui| {
                self.stats(ui);
                ui.add_space(10.0);
                Self::panel().show(ui, |ui| {
                    self.filter_bar(ui);
                    ui.add_space(8.0);
                    self.event_table(ui);
                });
                ui.add_space(10.0);
                self.details(ui);

                if let Some(message) = self.toast.clone() {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("i").color(ACCENT).strong());
                        ui.label(RichText::new(message).size(10.0).color(MUTED));
                        if ui.small_button("Dismiss").clicked() {
                            self.toast = None;
                        }
                    });
                }
            });
    }
}
