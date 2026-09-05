use crate::format;
use crate::model::{ProcessRow, SortColumn, SortDirection, SystemSnapshot, sort_processes};
use crate::platform;
use crate::sampler::Sampler;
use crate::theme;
use eframe::egui;
use egui::{Align, Color32, FontId, Layout, RichText, Sense, Stroke, Vec2};
use egui_extras::{Column, TableBuilder};
use std::collections::VecDeque;

const HISTORY_LENGTH: usize = 120;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Page {
    Processes,
    Performance,
}

pub struct TrontopApp {
    sampler: Sampler,
    snapshot: SystemSnapshot,
    seen_generation: u64,
    page: Page,
    query: String,
    sort_column: SortColumn,
    sort_direction: SortDirection,
    visible_processes: Vec<ProcessRow>,
    selected_pid: Option<u32>,
    pending_end_pid: Option<u32>,
    message: Option<(String, bool)>,
    cpu_history: VecDeque<f32>,
    memory_history: VecDeque<f32>,
}

impl TrontopApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        let sampler = Sampler::spawn(cc.egui_ctx.clone());
        Self {
            sampler,
            snapshot: SystemSnapshot::default(),
            seen_generation: 0,
            page: Page::Processes,
            query: String::new(),
            sort_column: SortColumn::Cpu,
            sort_direction: SortDirection::Descending,
            visible_processes: Vec::new(),
            selected_pid: None,
            pending_end_pid: None,
            message: None,
            cpu_history: VecDeque::with_capacity(HISTORY_LENGTH),
            memory_history: VecDeque::with_capacity(HISTORY_LENGTH),
        }
    }

    fn accept_sample(&mut self, snapshot: SystemSnapshot) {
        self.seen_generation = snapshot.sequence;
        push_history(&mut self.cpu_history, snapshot.cpu_percent);
        let memory_percent = if snapshot.memory_total_bytes == 0 {
            0.0
        } else {
            snapshot.memory_used_bytes as f32 / snapshot.memory_total_bytes as f32 * 100.0
        };
        push_history(&mut self.memory_history, memory_percent);
        self.snapshot = snapshot;
        self.rebuild_visible_processes();
    }

    fn rebuild_visible_processes(&mut self) {
        let needle = self.query.trim().to_ascii_lowercase();
        self.visible_processes = self
            .snapshot
            .processes
            .iter()
            .filter(|process| {
                needle.is_empty()
                    || process.name.to_ascii_lowercase().contains(&needle)
                    || process.pid.to_string().contains(&needle)
                    || process.executable.as_ref().is_some_and(|path| {
                        path.to_string_lossy()
                            .to_ascii_lowercase()
                            .contains(&needle)
                    })
            })
            .cloned()
            .collect();
        sort_processes(
            &mut self.visible_processes,
            self.sort_column,
            self.sort_direction,
        );
    }

    fn selected_process(&self) -> Option<&ProcessRow> {
        let pid = self.selected_pid?;
        self.snapshot
            .processes
            .iter()
            .find(|process| process.pid == pid)
    }

    fn sort_by(&mut self, column: SortColumn) {
        if self.sort_column == column {
            self.sort_direction = self.sort_direction.toggled();
        } else {
            self.sort_column = column;
            self.sort_direction = match column {
                SortColumn::Name | SortColumn::Status | SortColumn::Pid => SortDirection::Ascending,
                _ => SortDirection::Descending,
            };
        }
        self.rebuild_visible_processes();
    }

    fn title_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("brand_bar")
            .exact_size(48.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(18, 16, 14))
                    .inner_margin(egui::Margin::symmetric(16, 8))
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                    ui.painter().rect_filled(rect, 5.0, theme::COPPER);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "T",
                        FontId::proportional(16.0),
                        Color32::WHITE,
                    );
                    ui.label(
                        RichText::new("TRONTOP")
                            .size(16.0)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.label(
                        RichText::new("WINDOWS PROCESS CONTROL")
                            .size(10.0)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("v0.1.0").monospace().color(theme::TEXT_MUTED));
                        let status = if self.seen_generation == 0 {
                            "STARTING"
                        } else {
                            "LIVE"
                        };
                        let color = if self.seen_generation == 0 {
                            theme::COPPER
                        } else {
                            theme::GOOD
                        };
                        ui.label(RichText::new(status).size(11.0).strong().color(color));
                    });
                });
            });
    }

    fn navigation(&mut self, root: &mut egui::Ui) {
        egui::Panel::left("navigation")
            .exact_size(188.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(10, 14))
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(root, |ui| {
                ui.label(
                    RichText::new("MONITOR")
                        .size(10.0)
                        .color(theme::TEXT_MUTED)
                        .strong(),
                );
                ui.add_space(5.0);
                if nav_button(ui, self.page == Page::Processes, "01", "Processes") {
                    self.page = Page::Processes;
                }
                if nav_button(ui, self.page == Page::Performance, "02", "Performance") {
                    self.page = Page::Performance;
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label(
                    RichText::new("MACHINE")
                        .size(10.0)
                        .color(theme::TEXT_MUTED)
                        .strong(),
                );
                ui.add_space(8.0);
                mini_meter(ui, "CPU", self.snapshot.cpu_percent);
                let memory_percent = memory_percent(&self.snapshot);
                mini_meter(ui, "MEMORY", memory_percent);

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(
                        RichText::new("Native telemetry\n1 second sample interval")
                            .size(10.0)
                            .color(theme::TEXT_MUTED),
                    );
                });
            });
    }

    fn inspector(&mut self, root: &mut egui::Ui) {
        egui::Panel::right("inspector")
            .default_size(290.0)
            .min_size(250.0)
            .max_size(360.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::same(16))
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(root, |ui| {
                ui.label(RichText::new("INSPECTOR").size(10.0).color(theme::TEXT_MUTED).strong());
                ui.add_space(12.0);

                let selected = self.selected_process().cloned();
                if let Some(process) = selected {
                    ui.label(RichText::new(&process.name).size(19.0).strong().color(theme::TEXT));
                    ui.label(RichText::new(format!("PID {}", process.pid)).monospace().color(theme::COPPER));
                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(10.0);
                    detail_row(ui, "Status", &process.status);
                    detail_row(ui, "CPU", &format::percent(process.cpu_percent));
                    detail_row(ui, "Working set", &format::bytes(process.memory_bytes));
                    detail_row(ui, "Virtual", &format::bytes(process.virtual_memory_bytes));
                    detail_row(ui, "Disk read", &format::rate(process.read_bytes_per_sec));
                    detail_row(ui, "Disk write", &format::rate(process.write_bytes_per_sec));
                    detail_row(
                        ui,
                        "Running",
                        &format::age_from_unix(process.started_at_unix),
                    );
                    if let Some(parent_pid) = process.parent_pid {
                        detail_row(ui, "Parent PID", &parent_pid.to_string());
                    }

                    if let Some(path) = &process.executable {
                        ui.add_space(10.0);
                        ui.label(RichText::new("EXECUTABLE").size(10.0).color(theme::TEXT_MUTED));
                        ui.label(RichText::new(path.display().to_string()).size(11.0).monospace());
                        ui.add_space(5.0);
                        if ui.button("Reveal in Explorer").clicked() {
                            self.message = Some(match platform::reveal_in_explorer(path) {
                                Ok(()) => ("Opened Explorer".into(), false),
                                Err(error) => (error, true),
                            });
                        }
                    }

                    ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                        let end = egui::Button::new(RichText::new("End task").color(Color32::WHITE))
                            .fill(theme::DANGER)
                            .min_size(Vec2::new(ui.available_width(), 34.0));
                        if ui.add(end).clicked() {
                            match platform::can_terminate(process.pid) {
                                Ok(()) => self.pending_end_pid = Some(process.pid),
                                Err(error) => self.message = Some((error, true)),
                            }
                        }
                    });
                } else {
                    ui.label(RichText::new("Select a process to inspect it.").color(theme::TEXT_MUTED));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Details and process controls stay here while the live table keeps updating.")
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                }
            });
    }

    fn processes_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading(RichText::new("Processes").color(theme::TEXT));
                ui.label(
                    RichText::new(format!(
                        "{} visible of {} running",
                        self.visible_processes.len(),
                        self.snapshot.process_count
                    ))
                    .size(11.0)
                    .color(theme::TEXT_MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let search = egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Search name, PID, or path")
                    .desired_width(280.0);
                if ui.add(search).changed() {
                    self.rebuild_visible_processes();
                }
            });
        });
        ui.add_space(12.0);
        self.telemetry_strip(ui);
        ui.add_space(12.0);
        self.process_table(ui);
    }

    fn telemetry_strip(&self, ui: &mut egui::Ui) {
        let memory_percent = memory_percent(&self.snapshot);
        ui.columns(4, |columns| {
            stat_card(
                &mut columns[0],
                "CPU",
                &format::percent(self.snapshot.cpu_percent),
                "total load",
            );
            stat_card(
                &mut columns[1],
                "MEMORY",
                &format::percent(memory_percent),
                &format!(
                    "{} / {}",
                    format::bytes(self.snapshot.memory_used_bytes),
                    format::bytes(self.snapshot.memory_total_bytes)
                ),
            );
            stat_card(
                &mut columns[2],
                "PROCESSES",
                &self.snapshot.process_count.to_string(),
                "live entries",
            );
            stat_card(
                &mut columns[3],
                "UPTIME",
                &format::duration(self.snapshot.uptime_seconds),
                &format!("sample {:.2}s", self.snapshot.sample_seconds),
            );
        });
    }

    fn process_table(&mut self, ui: &mut egui::Ui) {
        let available_height = ui.available_height();
        let visible = &self.visible_processes;
        let selected_pid = self.selected_pid;
        let mut clicked_pid = None;
        let mut requested_sort = None;

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .vscroll(true)
            .sense(Sense::click())
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height)
            .column(Column::initial(220.0).at_least(140.0).clip(true))
            .column(Column::initial(60.0).at_least(52.0))
            .column(Column::initial(75.0).at_least(62.0))
            .column(Column::initial(68.0).at_least(58.0))
            .column(Column::initial(90.0).at_least(74.0))
            .column(Column::initial(80.0).at_least(68.0))
            .column(Column::remainder().at_least(68.0))
            .header(30.0, |mut header| {
                table_header(
                    &mut header,
                    "NAME",
                    SortColumn::Name,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                );
                table_header(
                    &mut header,
                    "PID",
                    SortColumn::Pid,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                );
                table_header(
                    &mut header,
                    "STATUS",
                    SortColumn::Status,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                );
                table_header(
                    &mut header,
                    "CPU",
                    SortColumn::Cpu,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                );
                table_header(
                    &mut header,
                    "MEMORY",
                    SortColumn::Memory,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                );
                table_header(
                    &mut header,
                    "READ",
                    SortColumn::ReadRate,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                );
                table_header(
                    &mut header,
                    "WRITE",
                    SortColumn::WriteRate,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                );
            })
            .body(|body| {
                body.rows(29.0, visible.len(), |mut row| {
                    let process = &visible[row.index()];
                    row.set_selected(selected_pid == Some(process.pid));
                    let text_color = if process.status == "Run" {
                        theme::TEXT
                    } else {
                        Color32::from_rgb(204, 197, 190)
                    };
                    row.col(|ui| {
                        if table_cell(ui, RichText::new(&process.name).color(text_color).strong()) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    row.col(|ui| {
                        if table_cell(
                            ui,
                            RichText::new(process.pid.to_string())
                                .monospace()
                                .color(theme::TEXT_MUTED),
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    row.col(|ui| {
                        if table_cell(
                            ui,
                            RichText::new(&process.status)
                                .size(11.0)
                                .color(theme::TEXT_MUTED),
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    row.col(|ui| {
                        if table_cell(
                            ui,
                            RichText::new(format::percent(process.cpu_percent)).monospace(),
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    row.col(|ui| {
                        if table_cell(
                            ui,
                            RichText::new(format::bytes(process.memory_bytes)).monospace(),
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    row.col(|ui| {
                        if table_cell(
                            ui,
                            RichText::new(format::rate(process.read_bytes_per_sec))
                                .monospace()
                                .color(theme::TEXT_MUTED),
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    row.col(|ui| {
                        if table_cell(
                            ui,
                            RichText::new(format::rate(process.write_bytes_per_sec))
                                .monospace()
                                .color(theme::TEXT_MUTED),
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                });
            });

        if let Some(pid) = clicked_pid {
            self.selected_pid = Some(pid);
        }
        if let Some(column) = requested_sort {
            self.sort_by(column);
        }
    }

    fn performance_page(&self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("Performance").color(theme::TEXT));
        ui.label(
            RichText::new("Two minutes of machine load, sampled outside the render thread")
                .size(11.0)
                .color(theme::TEXT_MUTED),
        );
        ui.add_space(12.0);
        self.telemetry_strip(ui);
        ui.add_space(14.0);

        egui::Frame::new()
            .fill(theme::PANEL)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                graph_header(ui, "CPU LOAD", self.snapshot.cpu_percent, theme::COPPER);
                history_graph(ui, &self.cpu_history, theme::COPPER, 190.0);
            });
        ui.add_space(10.0);
        egui::Frame::new()
            .fill(theme::PANEL)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                graph_header(
                    ui,
                    "MEMORY PRESSURE",
                    memory_percent(&self.snapshot),
                    Color32::from_rgb(200, 146, 78),
                );
                history_graph(
                    ui,
                    &self.memory_history,
                    Color32::from_rgb(200, 146, 78),
                    190.0,
                );
            });
    }

    fn confirm_end_task(&mut self, ctx: &egui::Context) {
        let Some(pid) = self.pending_end_pid else {
            return;
        };
        let name = self
            .snapshot
            .processes
            .iter()
            .find(|process| process.pid == pid)
            .map(|process| process.name.clone())
            .unwrap_or_else(|| "Unknown process".into());

        egui::Window::new("Confirm end task")
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(RichText::new(format!("End {name}?")).size(18.0).strong());
                ui.label(
                    RichText::new(format!(
                        "PID {pid} will be terminated immediately. Unsaved data in that process will be lost."
                    ))
                    .color(theme::TEXT_MUTED),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.pending_end_pid = None;
                    }
                    if ui
                        .add(egui::Button::new("End process").fill(theme::DANGER))
                        .clicked()
                    {
                        self.message = Some(match platform::terminate_process(pid) {
                            Ok(()) => (format!("Ended {name} ({pid})"), false),
                            Err(error) => (error, true),
                        });
                        self.pending_end_pid = None;
                    }
                });
            });
    }

    fn message_bar(&mut self, root: &mut egui::Ui) {
        let Some((message, is_error)) = self.message.clone() else {
            return;
        };
        egui::Panel::bottom("message_bar")
            .exact_size(34.0)
            .frame(egui::Frame::new().fill(if is_error {
                Color32::from_rgb(76, 29, 27)
            } else {
                Color32::from_rgb(24, 55, 42)
            }))
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new(message).color(theme::TEXT));
                    if ui.small_button("Dismiss").clicked() {
                        self.message = None;
                    }
                });
            });
    }
}

impl eframe::App for TrontopApp {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(snapshot) = self.sampler.latest_after(self.seen_generation) {
            self.accept_sample(snapshot);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.title_bar(ui);
        self.message_bar(ui);
        self.navigation(ui);
        self.inspector(ui);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::same(18)),
            )
            .show(ui, |ui| match self.page {
                Page::Processes => self.processes_page(ui),
                Page::Performance => self.performance_page(ui),
            });
        let ctx = ui.ctx().clone();
        self.confirm_end_task(&ctx);
    }
}

fn nav_button(ui: &mut egui::Ui, selected: bool, icon: &str, label: &str) -> bool {
    let text = RichText::new(format!("{icon}   {label}"))
        .size(13.0)
        .color(if selected {
            Color32::WHITE
        } else {
            theme::TEXT_MUTED
        });
    ui.add(
        egui::Button::new(text)
            .fill(if selected {
                theme::COPPER_DIM
            } else {
                Color32::TRANSPARENT
            })
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(ui.available_width(), 34.0)),
    )
    .clicked()
}

fn mini_meter(ui: &mut egui::Ui, label: &str, value: f32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(10.0).color(theme::TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format::percent(value))
                    .monospace()
                    .color(theme::TEXT),
            );
        });
    });
    ui.add(
        egui::ProgressBar::new((value / 100.0).clamp(0.0, 1.0))
            .fill(theme::COPPER)
            .desired_width(ui.available_width())
            .desired_height(4.0),
    );
    ui.add_space(8.0);
}

fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, detail: &str) {
    egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_height(64.0);
            ui.label(
                RichText::new(label)
                    .size(9.0)
                    .strong()
                    .color(theme::TEXT_MUTED),
            );
            ui.label(
                RichText::new(value)
                    .size(19.0)
                    .monospace()
                    .color(theme::TEXT),
            );
            ui.label(RichText::new(detail).size(9.0).color(theme::TEXT_MUTED));
        });
}

fn detail_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(11.0).color(theme::TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .size(11.0)
                    .monospace()
                    .color(theme::TEXT),
            );
        });
    });
}

fn table_header(
    header: &mut egui_extras::TableRow<'_, '_>,
    label: &str,
    column: SortColumn,
    active: SortColumn,
    direction: SortDirection,
    requested: &mut Option<SortColumn>,
) {
    header.col(|ui| {
        let arrow = if active == column {
            match direction {
                SortDirection::Ascending => "  ^",
                SortDirection::Descending => "  v",
            }
        } else {
            ""
        };
        if ui
            .add(
                egui::Label::new(
                    RichText::new(format!("{label}{arrow}"))
                        .size(10.0)
                        .strong()
                        .color(theme::TEXT_MUTED),
                )
                .sense(Sense::click()),
            )
            .clicked()
        {
            *requested = Some(column);
        }
    });
}

fn table_cell(ui: &mut egui::Ui, text: RichText) -> bool {
    ui.add(egui::Label::new(text).sense(Sense::click()))
        .clicked()
}

fn graph_header(ui: &mut egui::Ui, label: &str, value: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .size(10.0)
                .strong()
                .color(theme::TEXT_MUTED),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format::percent(value))
                    .size(16.0)
                    .monospace()
                    .color(color),
            );
        });
    });
    ui.add_space(5.0);
}

fn history_graph(ui: &mut egui::Ui, history: &VecDeque<f32>, color: Color32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_rgb(12, 11, 10));
    for index in 1..4 {
        let y = egui::lerp(rect.top()..=rect.bottom(), index as f32 / 4.0);
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            Stroke::new(1.0, theme::BORDER),
        );
    }
    if history.len() > 1 {
        let denominator = (HISTORY_LENGTH - 1) as f32;
        let points = history
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let x = rect.right()
                    - ((history.len() - 1 - index) as f32 / denominator) * rect.width();
                let y = rect.bottom() - (value.clamp(0.0, 100.0) / 100.0) * rect.height();
                egui::pos2(x, y)
            })
            .collect::<Vec<_>>();
        painter.add(egui::Shape::line(points, Stroke::new(2.0, color)));
    }
}

fn memory_percent(snapshot: &SystemSnapshot) -> f32 {
    if snapshot.memory_total_bytes == 0 {
        0.0
    } else {
        snapshot.memory_used_bytes as f32 / snapshot.memory_total_bytes as f32 * 100.0
    }
}

fn push_history(history: &mut VecDeque<f32>, value: f32) {
    if history.len() == HISTORY_LENGTH {
        history.pop_front();
    }
    history.push_back(value);
}
