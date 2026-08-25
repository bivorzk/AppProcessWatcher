use std::collections::{BTreeSet, HashMap};

use eframe::egui::{self, Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke};

use crate::{
    application::{AppState, DetailTab, ProcessTab, validate_filter},
    domain::EventKind,
    runtime::{Runtime, RuntimeEvent},
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
    runtime: Runtime,
    process_icons: HashMap<u32, egui::TextureHandle>,
    revealed_response_bodies: BTreeSet<u64>,
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
    const MAX_RUNTIME_EVENTS_PER_FRAME: usize = 1_024;
    const MAX_BODY_PREVIEW_BYTES: usize = 64 * 1_024;

    pub fn new(context: &eframe::CreationContext<'_>, state: AppState, runtime: Runtime) -> Self {
        apply_theme(&context.egui_ctx);
        let process_icons = state
            .processes
            .iter()
            .filter_map(|process| {
                let pixels = process.icon_rgba.as_ref()?;
                Some((
                    process.pid,
                    context.egui_ctx.load_texture(
                        format!("process-icon-{}", process.pid),
                        egui::ColorImage::from_rgba_unmultiplied([32, 32], pixels),
                        egui::TextureOptions::LINEAR,
                    ),
                ))
            })
            .collect();
        Self {
            state,
            runtime,
            process_icons,
            revealed_response_bodies: BTreeSet::new(),
        }
    }

    fn receive_runtime_events(&mut self, context: &egui::Context) {
        for index in 0..Self::MAX_RUNTIME_EVENTS_PER_FRAME {
            let Ok(event) = self.runtime.try_recv() else {
                break;
            };
            match event {
                RuntimeEvent::Connection(mut event) => {
                    if self.recording {
                        self.classify_ja4(&mut event);
                        if let Some(existing) =
                            self.events.iter_mut().find(|item| item.id == event.id)
                        {
                            *existing = event;
                        } else {
                            self.events.push(event);
                        }
                    }
                }
                RuntimeEvent::Closed(id) => self.events.retain(|event| event.id != id),
                RuntimeEvent::Processes(processes) => self.replace_processes(context, processes),
                RuntimeEvent::Warning(warning) => self.set_toast(warning),
                RuntimeEvent::Error(error) => self.capture_error = Some(error),
            }
            if index + 1 == Self::MAX_RUNTIME_EVENTS_PER_FRAME {
                context.request_repaint();
            }
        }
    }

    fn replace_processes(
        &mut self,
        context: &egui::Context,
        processes: Vec<crate::domain::Process>,
    ) {
        let selected_name = (self.process_tab == ProcessTab::Applications)
            .then(|| {
                self.selected_pid.and_then(|pid| {
                    self.processes
                        .iter()
                        .find(|process| process.pid == pid)
                        .map(|process| process.name.clone())
                })
            })
            .flatten();
        if let Some(selected_pid) = self.selected_pid {
            self.selected_pid = processes
                .iter()
                .find(|process| process.pid == selected_pid)
                .or_else(|| {
                    selected_name.as_ref().and_then(|name| {
                        processes.iter().find(|process| {
                            process.application && process.name.eq_ignore_ascii_case(name)
                        })
                    })
                })
                .map(|process| process.pid);
            if self.selected_pid.is_none() {
                self.selected_event = None;
            }
        }

        self.process_icons
            .retain(|pid, _| processes.iter().any(|process| process.pid == *pid));
        for process in &processes {
            if self.process_icons.contains_key(&process.pid) {
                continue;
            }
            if let Some(pixels) = &process.icon_rgba {
                self.process_icons.insert(
                    process.pid,
                    context.load_texture(
                        format!("process-icon-{}", process.pid),
                        egui::ColorImage::from_rgba_unmultiplied([32, 32], pixels),
                        egui::TextureOptions::LINEAR,
                    ),
                );
            }
        }
        self.processes = processes;
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
        section_label(ui, "PROGRAMMES");
        let application_count = self
            .processes
            .iter()
            .filter(|process| process.application)
            .count();
        let processes_count = self.processes.len();
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.process_tab,
                ProcessTab::Applications,
                format!("Applications · {application_count}"),
            );
            ui.selectable_value(
                &mut self.process_tab,
                ProcessTab::Processes,
                format!("Processes · {processes_count}"),
            );
        });
        ui.add_space(6.0);

        let mut next_pid = None;
        egui::ScrollArea::vertical()
            .id_salt("programme-list")
            .max_height((ui.available_height() - 84.0).max(100.0))
            .show(ui, |ui| {
                if process_button(
                    ui,
                    self.selected_pid.is_none(),
                    true,
                    None,
                    "All traffic",
                    &match self.process_tab {
                        ProcessTab::Applications => format!("{application_count} open apps"),
                        ProcessTab::Processes => format!("{} processes", self.processes.len()),
                    },
                ) {
                    self.selected_pid = None;
                    self.selected_event = None;
                }

                for process in self.processes.iter().filter(|process| {
                    self.process_tab == ProcessTab::Processes || process.application
                }) {
                    let selected = self.selected_pid == Some(process.pid);
                    if process_button(
                        ui,
                        selected,
                        process.active,
                        self.process_icons.get(&process.pid),
                        &process.name,
                        &format!("PID {}", process.pid),
                    ) {
                        next_pid = Some(process.pid);
                    }
                }
            });
        if let Some(pid) = next_pid {
            self.selected_pid = Some(pid);
            self.selected_event = self
                .events
                .iter()
                .find(|event| event.pid == Some(pid))
                .map(|event| event.id);
        }

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(8.0);
            let widget_stroke = if self.capture_error.is_none() {
                Color32::from_rgb(30, 70, 110)
            } else {
                DANGER.gamma_multiply(0.55)
            };
            Frame::new()
                .fill(PANEL_ALT)
                .stroke(Stroke::new(1.0, widget_stroke))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("●")
                                .color(if self.capture_error.is_none() {
                                    SUCCESS
                                } else {
                                    DANGER
                                })
                                .size(11.0),
                        );
                        ui.label(
                            RichText::new(if self.capture_error.is_none() {
                                "CAPTURE ONLINE"
                            } else {
                                "CAPTURE OFFLINE"
                            })
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
                ui.menu_button("Proxy", |ui| {
                    if ui.button("Trust HTTPS certificate…").clicked() {
                        self.confirm_trust_certificate = true;
                        ui.close();
                    }
                    ui.separator();
                    ui.add_enabled_ui(self.selected_pid.is_some(), |ui| {
                        if ui.button("Relaunch selected app").clicked() {
                            self.runtime.relaunch_through_proxy(self.selected_pid);
                            self.set_toast("Relaunching the selected app through the proxy…");
                            ui.close();
                        }
                    });
                    if ui.button("Relaunch all open apps").clicked() {
                        self.confirm_relaunch_all = true;
                        ui.close();
                    }
                    ui.separator();
                    ui.label(RichText::new("Save your work first: relaunched apps are force-closed.").size(10.0).color(DANGER));
                    ui.label(
                        RichText::new("Warning: If a game is open that uses kernel-level anti-cheat, such as a Riot Games title, relaunching it through a proxy may flag it.")
                            .size(10.0)
                            .color(WARNING),
                    );
                });
                if secondary_button(ui, "Export JSON").clicked() {
                    self.set_toast("Export becomes available when storage is connected.");
                }
                if secondary_button(ui, "Clear").clicked() {
                    self.events.clear();
                    self.selected_event = None;
                    self.set_toast("Current screen data cleared.");
                }
                let label = if self.recording {
                    "Stop recording"
                } else {
                    "Start recording"
                };
                let record_clicked = if self.recording {
                    danger_button(ui, label).clicked()
                } else {
                    primary_button(ui, label).clicked()
                };
                if record_clicked {
                    self.recording = !self.recording;
                    let msg = if self.recording { "Recording started." } else { "Recording stopped." };
                    self.set_toast(msg);
                }
                let (timer_text, timer_color, pill_fill, pill_stroke) = if self.recording {
                    (
                        format!("●  {}", format_time(self.session_seconds)),
                        DANGER,
                        Color32::from_rgba_unmultiplied(255, 107, 138, 18),
                        DANGER.gamma_multiply(0.45),
                    )
                } else {
                    ("■  PAUSED".to_string(), DIM, Color32::TRANSPARENT, LINE)
                };
                Frame::new()
                    .fill(pill_fill)
                    .stroke(Stroke::new(1.0, pill_stroke))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(Margin::symmetric(8, 5))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(timer_text)
                                .font(bold_font(11.0))
                                .color(timer_color)
                                .strong(),
                        );
                    });
            });
        });
    }

    fn stats(&self, ui: &mut egui::Ui, visible: &[usize]) {
        let events = visible.iter().map(|&index| &self.events[index]);
        let sent = format_bytes(events.clone().map(|event| event.bytes_sent).sum());
        let received = format_bytes(events.clone().map(|event| event.bytes_received).sum());
        let packets: u64 = events.clone().map(|event| event.packet_count).sum();
        let http_count = events
            .clone()
            .filter(|event| matches!(event.kind, EventKind::Http | EventKind::Https))
            .count();
        let hosts = events
            .clone()
            .map(|event| &event.host)
            .collect::<BTreeSet<_>>()
            .len();
        let (duration_total, duration_count) = events
            .filter_map(|event| event.duration_ms)
            .fold((0_u64, 0_u64), |(total, count), duration| {
                (total.saturating_add(duration), count + 1)
            });
        let average = if duration_count == 0 {
            "---".into()
        } else {
            format!("{} ms", duration_total / duration_count)
        };
        let cards = [
            ("ACTIVE", visible.len().to_string(), ACCENT),
            ("HTTP REQUESTS", http_count.to_string(), SUCCESS),
            ("PACKETS", packets.to_string(), ACCENT_2),
            ("UPLOADED", sent, WARNING),
            ("DOWNLOADED", received, ACCENT),
            ("HOSTS", hosts.to_string(), SUCCESS),
            ("AVG RESPONSE", average, ACCENT_2),
        ];

        ui.columns(cards.len(), |columns| {
            for (column, (label, value, colour)) in columns.iter_mut().zip(cards) {
                let card = Frame::new()
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
                let rect = card.response.rect;
                let stripe = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x + 1.0, rect.min.y + 1.0),
                    egui::vec2(rect.width() - 2.0, 2.0),
                );
                column
                    .painter()
                    .rect_filled(stripe, CornerRadius::same(0), colour);
            }
        });
    }

    fn filter_bar(&mut self, ui: &mut egui::Ui, visible_count: usize) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("TRAFFIC VIEW")
                    .font(bold_font(9.0))
                    .color(DIM)
                    .strong(),
            );
            for (https_only, label) in [(false, "ALL TRAFFIC"), (true, "HTTPS REQUESTS")] {
                if ui
                    .selectable_label(
                        self.https_only == https_only,
                        RichText::new(label).font(bold_font(10.0)).color(
                            if self.https_only == https_only {
                                ACCENT
                            } else {
                                MUTED
                            },
                        ),
                    )
                    .clicked()
                {
                    self.https_only = https_only;
                    self.selected_event = None;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("FINGERPRINTS")
                    .font(bold_font(9.0))
                    .color(DIM)
                    .strong(),
            );
            for (show, label) in [(true, "SHOW ALL"), (false, "HIDE DIFFERENCES")] {
                if ui
                    .selectable_label(
                        self.show_ja4_differences == show,
                        RichText::new(label).font(bold_font(10.0)).color(
                            if self.show_ja4_differences == show { ACCENT } else { MUTED },
                        ),
                    )
                    .on_hover_text(if show {
                        "Show normal traffic and requests whose JA4 differs from the process baseline"
                    } else {
                        "Hide requests whose JA4 differs from the first fingerprint seen for that process"
                    })
                    .clicked()
                {
                    self.show_ja4_differences = show;
                    self.selected_event = None;
                }
            }
        });
        ui.add_space(4.0);
        let filter_id = egui::Id::new("traffic_filter");
        if ui.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::CTRL,
                egui::Key::F,
            ))
        }) {
            ui.memory_mut(|m| m.request_focus(filter_id));
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("⎕").size(19.0).color(ACCENT));
            let has_filter = !self.filter.is_empty();
            let side_reserve = if has_filter { 130.0 } else { 96.0 };
            let response = ui.add_sized(
                [(ui.available_width() - side_reserve).max(100.0), 30.0],
                egui::TextEdit::singleline(&mut self.filter)
                    .id(filter_id)
                    .hint_text("Filter: process:Discord.exe protocol:TCP port:443")
                    .text_color(TEXT),
            );
            if response.changed() {
                self.filter_error = validate_filter(&self.filter).err();
            }
            if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.filter.clear();
                self.filter_error = None;
                self.selected_event = None;
            }
            if has_filter
                && ui
                    .add(
                        egui::Button::new(RichText::new("✕").size(10.0).color(MUTED))
                            .fill(PANEL_ALT)
                            .stroke(Stroke::new(1.0, LINE))
                            .corner_radius(4),
                    )
                    .on_hover_text("Clear filter (Esc)")
                    .clicked()
            {
                self.filter.clear();
                self.filter_error = None;
                self.selected_event = None;
            }
            let count_label = if has_filter {
                format!("{visible_count} matched")
            } else {
                format!("{visible_count} events")
            };
            ui.label(RichText::new(count_label).size(10.0).color(DIM));
        });
        if let Some(error) = &self.filter_error {
            ui.label(RichText::new(error).size(10.0).color(DANGER));
        }
    }

    fn event_table(&mut self, ui: &mut egui::Ui, visible: &[usize]) {
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
            .show_rows(ui, 34.0, visible.len(), |ui, rows| {
                for row in rows {
                    let event = &self.events[visible[row]];
                    let selected = self.selected_event == Some(event.id);
                    let event_id = event.id;
                    let copy_host = event.host.clone();
                    let copy_url = if event.path.is_empty() {
                        event.host.clone()
                    } else {
                        format!("{}{}", event.host, event.path)
                    };
                    let row_fill = if selected {
                        Color32::from_rgb(18, 41, 65)
                    } else if event.ja4_outlier {
                        Color32::from_rgba_unmultiplied(255, 207, 112, 18)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let response = Frame::new()
                        .fill(row_fill)
                        .stroke(Stroke::new(
                            1.0,
                            if selected {
                                ACCENT
                            } else if event.ja4_outlier {
                                WARNING
                            } else {
                                Color32::TRANSPARENT
                            },
                        ))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(Margin::symmetric(8, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                table_cell(ui, &event.method, widths[0], event.kind.colour(), true);
                                table_cell(ui, &event.host, widths[1], TEXT, false);
                                table_cell(
                                    ui,
                                    if event.path.is_empty() {
                                        "—"
                                    } else {
                                        &event.path
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
                                table_cell(
                                    ui,
                                    &format_bytes(event.bytes_sent),
                                    widths[5],
                                    MUTED,
                                    false,
                                );
                                table_cell(
                                    ui,
                                    &format_bytes(event.bytes_received),
                                    widths[6],
                                    MUTED,
                                    false,
                                );
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
                    if response.hovered() && !selected {
                        ui.painter().rect_filled(
                            response.rect,
                            CornerRadius::same(4),
                            Color32::from_rgba_unmultiplied(101, 217, 255, 10),
                        );
                    }
                    if event.ja4_outlier {
                        response.clone().on_hover_text(
                            "JA4 differs from the first TLS fingerprint seen for this process. This is a clue, not proof of a modified client.",
                        );
                    }
                    if response.clicked() {
                        self.selected_event = Some(event_id);
                    }
                    let _ = response.context_menu(|ui| {
                        if ui.button("Copy host").clicked() {
                            ui.ctx().copy_text(copy_host);
                            ui.close();
                        }
                        if ui.button("Copy URL").clicked() {
                            ui.ctx().copy_text(copy_url);
                            ui.close();
                        }
                    });
                }
            });

        if visible.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                let (heading, body) = if !self.recording && self.events.is_empty() {
                    (
                        "PAUSED",
                        "Press Start recording to begin capturing traffic.",
                    )
                } else if self.events.is_empty() {
                    (
                        "WAITING FOR TRAFFIC",
                        "Network events will appear here automatically.",
                    )
                } else if !self.filter.is_empty() {
                    (
                        "NO MATCHES",
                        "Clear or adjust the filter to see more results.",
                    )
                } else if !self.show_ja4_differences {
                    (
                        "DIFFERENCES HIDDEN",
                        "Switch fingerprints to Show all to include JA4 differences.",
                    )
                } else {
                    (
                        "NO TRAFFIC",
                        "Select another process or switch the traffic view.",
                    )
                };
                ui.label(
                    RichText::new(heading)
                        .font(bold_font(12.0))
                        .color(DIM)
                        .strong(),
                );
                ui.label(RichText::new(body).color(DIM));
                ui.add_space(24.0);
            });
        }
    }

    fn details(&mut self, ui: &mut egui::Ui) {
        let selected = self
            .selected_event
            .and_then(|id| self.events.iter().position(|event| event.id == id));
        let detail_tab = self.detail_tab;
        let mut next_tab = None;
        let mut reveal_response = None;
        Self::panel().show(ui, |ui| {
            let Some(selected) = selected else {
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
            let event = &self.events[selected];

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&event.method)
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
                    if event.ja4_outlier {
                        badge(ui, "JA4 DIFFERENCE", WARNING);
                    }
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
                    let selected = detail_tab == tab;
                    if ui
                        .selectable_label(
                            selected,
                            RichText::new(tab.label()).color(if selected { ACCENT } else { MUTED }),
                        )
                        .clicked()
                    {
                        next_tab = Some(tab);
                    }
                }
            });
            ui.separator();
            ui.add_space(5.0);

            match detail_tab {
                DetailTab::Headers => {
                    let rows = header_rows(&event.request_headers);
                    if rows.is_empty() {
                        key_values(
                            ui,
                            &[("Headers", "Unavailable for encrypted/raw traffic".into())],
                        );
                    } else {
                        key_values(ui, &rows);
                    }
                }
                DetailTab::RequestBody => self.body_preview(ui, event.request_body.as_deref()),
                DetailTab::Response => {
                    let mut rows = vec![
                        (
                            "Status",
                            event.status.map_or_else(
                                || "Encrypted / unavailable".into(),
                                |value| format!("{value} OK"),
                            ),
                        ),
                        ("Transferred", format_bytes(event.bytes_received)),
                    ];
                    rows.extend(header_rows(&event.response_headers));
                    key_values(ui, &rows);
                    if let Some(body) = &event.response_body {
                        ui.add_space(8.0);
                        if is_probably_text(body)
                            || self.revealed_response_bodies.contains(&event.id)
                        {
                            self.body_preview(ui, Some(body));
                        } else if secondary_button(
                            ui,
                            "Encrypted or encoded response — click to view anyway",
                        )
                        .clicked()
                        {
                            reveal_response = Some(event.id);
                        }
                    }
                }
                DetailTab::Timing => key_values(
                    ui,
                    &[(
                        "Total",
                        event
                            .duration_ms
                            .map_or_else(|| "---".into(), |value| format!("{value} ms")),
                    )],
                ),
                DetailTab::Connection => key_values(
                    ui,
                    &[
                        ("Process", event.process.clone()),
                        (
                            "PID",
                            event
                                .pid
                                .map_or_else(|| "Unknown".into(), |pid| pid.to_string()),
                        ),
                        ("Local", event.local.clone()),
                        ("Remote", event.remote.clone()),
                        (
                            "Transport",
                            if matches!(event.kind, EventKind::Udp | EventKind::Quic) {
                                "UDP".into()
                            } else {
                                "TCP".into()
                            },
                        ),
                        ("Captured using", "WinDivert".into()),
                        (
                            "JA4",
                            event
                                .ja4
                                .clone()
                                .unwrap_or_else(|| "Unavailable (non-TLS or passthrough)".into()),
                        ),
                        (
                            "Fingerprint status",
                            if event.ja4_outlier {
                                "Differs from this process's session baseline".into()
                            } else if event.ja4.is_some() {
                                "Matches session baseline".into()
                            } else {
                                "Not available".into()
                            },
                        ),
                    ],
                ),
            }
        });
        if let Some(tab) = next_tab {
            self.detail_tab = tab;
        }
        if let Some(id) = reveal_response {
            self.revealed_response_bodies.insert(id);
        }
    }

    fn body_preview(&self, ui: &mut egui::Ui, body: Option<&[u8]>) {
        let Some(body) = body else {
            code_block(ui, "Unavailable or not captured");
            return;
        };
        let preview = &body[..body.len().min(Self::MAX_BODY_PREVIEW_BYTES)];
        code_block(ui, &String::from_utf8_lossy(preview));
        if preview.len() < body.len() {
            ui.label(
                RichText::new(format!(
                    "Preview limited to {} of {} for UI performance.",
                    format_bytes(preview.len() as u64),
                    format_bytes(body.len() as u64)
                ))
                .size(10.0)
                .color(DIM),
            );
        }
    }

    fn relaunch_confirmation(&mut self, context: &egui::Context) {
        if !self.confirm_relaunch_all {
            return;
        }
        let mut confirmed = false;
        let mut cancelled = false;
        egui::Window::new("Relaunch all open apps?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(context, |ui| {
                ui.label("This force-closes and restarts every visible desktop app with the AppWatch proxy settings. Save your work first.");
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Warning: If a game is open that uses kernel-level anti-cheat, such as a Riot Games title, relaunching it through a proxy may flag it.")
                        .color(WARNING),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Relaunch all").clicked() {
                        confirmed = true;
                    }
                    if secondary_button(ui, "Cancel").clicked() {
                        cancelled = true;
                    }
                });
            });
        if confirmed {
            self.runtime.relaunch_through_proxy(None);
            self.set_toast("Relaunching open apps through the proxy…");
            self.confirm_relaunch_all = false;
        } else if cancelled {
            self.confirm_relaunch_all = false;
        }
    }

    fn certificate_confirmation(&mut self, context: &egui::Context) {
        if !self.confirm_trust_certificate {
            return;
        }
        let mut confirmed = false;
        let mut cancelled = false;
        egui::Window::new("Trust AppWatch HTTPS certificate?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(context, |ui| {
                ui.label("This installs the AppWatch CA in your current Windows user's trusted root store.");
                ui.add_space(6.0);
                ui.label(
                    RichText::new("This allows AppWatch to decrypt HTTPS traffic. Only continue on your own device and remove the certificate when you no longer use HTTPS inspection.")
                        .color(WARNING),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Trust certificate").clicked() {
                        confirmed = true;
                    }
                    if secondary_button(ui, "Cancel").clicked() {
                        cancelled = true;
                    }
                });
            });
        if confirmed {
            self.runtime.trust_https_certificate();
            self.set_toast("Installing the HTTPS certificate for the current user…");
            self.confirm_trust_certificate = false;
        } else if cancelled {
            self.confirm_trust_certificate = false;
        }
    }
}

impl eframe::App for AppWatch {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive_runtime_events(context);
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
                let visible = self.visible_event_indices();
                self.stats(ui, &visible);
                ui.add_space(10.0);
                Self::panel().show(ui, |ui| {
                    self.filter_bar(ui, visible.len());
                    ui.add_space(8.0);
                    self.event_table(ui, &visible);
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
                            self.toast_at = None;
                        }
                    });
                }
                if let Some(error) = &self.capture_error {
                    ui.add_space(8.0);
                    ui.label(RichText::new(error).size(10.0).color(DANGER));
                }
            });
        self.relaunch_confirmation(ui.ctx());
        self.certificate_confirmation(ui.ctx());
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    if bytes >= MB as u64 {
        format!("{:.1} MB", bytes as f64 / MB)
    } else if bytes >= KB as u64 {
        format!("{:.1} KB", bytes as f64 / KB)
    } else {
        format!("{bytes} B")
    }
}

fn header_rows(headers: &[apppw_core::HttpHeader]) -> Vec<(&str, String)> {
    headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.clone()))
        .collect()
}

fn is_probably_text(body: &[u8]) -> bool {
    let preview = &body[..body.len().min(4096)];
    std::str::from_utf8(preview).is_ok_and(|text| {
        text.chars()
            .all(|character| !character.is_control() || matches!(character, '\n' | '\r' | '\t'))
    })
}

#[cfg(test)]
mod tests {
    use super::is_probably_text;

    #[test]
    fn binary_responses_require_explicit_reveal() {
        assert!(is_probably_text(br#"{"status":"ok"}"#));
        assert!(!is_probably_text(&[0x1f, 0x8b, 0x08, 0x00, 0xff, 0x00]));
    }
}
