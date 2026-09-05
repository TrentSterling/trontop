use crate::format;
use crate::model::{
    PriorityClass, ProcessRow, ProcessTotals, ProcessTreeRow, SortColumn, SortDirection,
    SystemSnapshot, build_process_tree, sort_processes,
};
use crate::platform;
use crate::sampler::Sampler;
use crate::theme::{self, ThemeSettings, Tokens};
use crate::tray::{TrayAction, TrayController};
use crate::widgets;
use eframe::egui;
use egui::{Align, Color32, FontId, Layout, RichText, Sense, Stroke, Vec2};
use egui_extras::{Column, TableBuilder};
use std::collections::{HashMap, HashSet, VecDeque};

const HISTORY_LENGTH: usize = 120;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Page {
    Processes,
    Performance,
    History,
    Startup,
    Users,
    Details,
    Services,
}

impl Page {
    const ALL: [(Self, &'static str, &'static str); 7] = [
        (Self::Processes, "01", "Processes"),
        (Self::Performance, "02", "Performance"),
        (Self::History, "03", "History"),
        (Self::Startup, "04", "Startup"),
        (Self::Users, "05", "Users"),
        (Self::Details, "06", "Details"),
        (Self::Services, "07", "Services"),
    ];
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PerformanceDevice {
    Cpu,
    Memory,
    Disk(usize),
    Network(usize),
    Gpu,
}

#[derive(Clone, Copy)]
enum PendingControlAction {
    Priority {
        pid: u32,
        started_at_unix: u64,
        priority: PriorityClass,
    },
    Affinity {
        pid: u32,
        started_at_unix: u64,
        affinity_mask: usize,
    },
}

pub struct TrontopApp {
    sampler: Sampler,
    snapshot: SystemSnapshot,
    seen_generation: u64,
    page: Page,
    performance_device: PerformanceDevice,
    query: String,
    secondary_query: String,
    sort_column: SortColumn,
    sort_direction: SortDirection,
    visible_processes: Vec<ProcessRow>,
    visible_process_tree: Vec<ProcessTreeRow>,
    tree_mode: bool,
    tree_initialized: bool,
    expanded_pids: HashSet<u32>,
    selected_pid: Option<u32>,
    pending_end_pid: Option<u32>,
    pending_control_action: Option<PendingControlAction>,
    message: Option<(String, bool)>,
    cpu_history: VecDeque<f32>,
    memory_history: VecDeque<f32>,
    gpu_history: VecDeque<f32>,
    disk_history: HashMap<String, VecDeque<f32>>,
    network_history: HashMap<String, VecDeque<f32>>,
    theme: ThemeSettings,
    show_theme_editor: bool,
    show_run_task: bool,
    show_priority_editor: bool,
    show_affinity_editor: bool,
    affinity_draft: usize,
    run_command: String,
    tray: Option<TrayController>,
}

impl TrontopApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let saved_theme = cc
            .storage
            .and_then(|storage| storage.get_string(theme::STORAGE_KEY))
            .and_then(|value| ThemeSettings::decode(&value))
            .unwrap_or_default();
        theme::install(&cc.egui_ctx, saved_theme);
        let tray = TrayController::new(cc.egui_ctx.clone());
        let sampler = Sampler::spawn(cc.egui_ctx.clone(), tray.as_ref().map(TrayController::sink));
        Self {
            sampler,
            snapshot: SystemSnapshot::default(),
            seen_generation: 0,
            page: Page::Processes,
            performance_device: PerformanceDevice::Cpu,
            query: String::new(),
            secondary_query: String::new(),
            sort_column: SortColumn::Cpu,
            sort_direction: SortDirection::Descending,
            visible_processes: Vec::new(),
            visible_process_tree: Vec::new(),
            tree_mode: true,
            tree_initialized: false,
            expanded_pids: HashSet::new(),
            selected_pid: None,
            pending_end_pid: None,
            pending_control_action: None,
            message: None,
            cpu_history: VecDeque::with_capacity(HISTORY_LENGTH),
            memory_history: VecDeque::with_capacity(HISTORY_LENGTH),
            gpu_history: VecDeque::with_capacity(HISTORY_LENGTH),
            disk_history: HashMap::new(),
            network_history: HashMap::new(),
            theme: saved_theme,
            show_theme_editor: false,
            show_run_task: false,
            show_priority_editor: false,
            show_affinity_editor: false,
            affinity_draft: 0,
            run_command: String::new(),
            tray,
        }
    }

    fn colors(&self) -> Tokens {
        theme::tokens(self.theme)
    }

    fn accept_sample(&mut self, snapshot: SystemSnapshot) {
        self.seen_generation = snapshot.sequence;
        widgets::push_history(&mut self.cpu_history, snapshot.cpu_percent, HISTORY_LENGTH);
        widgets::push_history(
            &mut self.memory_history,
            memory_percent(&snapshot),
            HISTORY_LENGTH,
        );
        widgets::push_history(
            &mut self.gpu_history,
            snapshot.gpu.utilization_percent,
            HISTORY_LENGTH,
        );
        for disk in &snapshot.disks {
            let value = ((disk.read_bytes_per_sec + disk.write_bytes_per_sec) / 1_048_576.0) as f32;
            widgets::push_history(
                self.disk_history.entry(disk.mount.clone()).or_default(),
                value,
                HISTORY_LENGTH,
            );
        }
        for network in &snapshot.networks {
            let value = ((network.received_bytes_per_sec + network.transmitted_bytes_per_sec)
                / 1_048_576.0) as f32;
            widgets::push_history(
                self.network_history
                    .entry(network.name.clone())
                    .or_default(),
                value,
                HISTORY_LENGTH,
            );
        }
        self.snapshot = snapshot;
        if !self.tree_initialized && !self.snapshot.processes.is_empty() {
            let live_pids = self
                .snapshot
                .processes
                .iter()
                .map(|process| process.pid)
                .collect::<HashSet<_>>();
            self.expanded_pids.extend(
                self.snapshot
                    .processes
                    .iter()
                    .filter(|process| {
                        process
                            .parent_pid
                            .is_none_or(|parent| !live_pids.contains(&parent))
                    })
                    .map(|process| process.pid),
            );
            self.tree_initialized = true;
        }
        self.rebuild_visible_processes();
    }

    fn rebuild_visible_processes(&mut self) {
        let needle = self.query.trim().to_ascii_lowercase();
        let process_matches = |process: &&ProcessRow| {
            needle.is_empty()
                || process.name.to_ascii_lowercase().contains(&needle)
                || process.user.to_ascii_lowercase().contains(&needle)
                || process.pid.to_string().contains(&needle)
                || process.command.to_ascii_lowercase().contains(&needle)
                || process.executable.as_ref().is_some_and(|path| {
                    path.to_string_lossy()
                        .to_ascii_lowercase()
                        .contains(&needle)
                })
        };
        let matching_pids = self
            .snapshot
            .processes
            .iter()
            .filter(process_matches)
            .map(|process| process.pid)
            .collect::<HashSet<_>>();
        self.visible_processes = self
            .snapshot
            .processes
            .iter()
            .filter(|process| matching_pids.contains(&process.pid))
            .cloned()
            .collect();
        sort_processes(
            &mut self.visible_processes,
            self.sort_column,
            self.sort_direction,
        );
        self.visible_process_tree = build_process_tree(
            &self.snapshot.processes,
            &matching_pids,
            &self.expanded_pids,
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
                SortColumn::Name | SortColumn::Status | SortColumn::User | SortColumn::Pid => {
                    SortDirection::Ascending
                }
                _ => SortDirection::Descending,
            };
        }
        self.rebuild_visible_processes();
    }

    fn custom_chrome(&mut self, root: &mut egui::Ui) {
        let t = self.colors();
        egui::Panel::top("custom_chrome")
            .exact_size(42.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::panel_color(self.theme))
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .stroke(Stroke::new(1.0, t.border)),
            )
            .show(root, |ui| {
                let chrome_rect = ui.max_rect();
                ui.horizontal_centered(|ui| {
                    widgets::tront_mark(ui, t.accent, t.secondary, 25.0);
                    ui.label(RichText::new("TRONTOP").size(15.0).strong().color(t.text));
                    ui.label(
                        RichText::new("SYSTEM CONTROL DECK")
                            .size(9.0)
                            .strong()
                            .color(t.text_muted),
                    );
                    ui.add_space(8.0);
                    let live = self.seen_generation > 0;
                    widgets::status_pill(
                        ui,
                        if live { "LIVE" } else { "CONNECTING" },
                        if live { t.good } else { t.secondary },
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if chrome_button(ui, "X", t, true).clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if chrome_button(ui, "[]", t, false).clicked() {
                            let maximized = ui
                                .ctx()
                                .input(|input| input.viewport().maximized.unwrap_or(false));
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                        }
                        if chrome_button(ui, "_", t, false).clicked() {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        ui.label(
                            RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .monospace()
                                .size(10.0)
                                .color(t.text_muted),
                        );
                    });
                });
                let drag_rect = egui::Rect::from_min_max(
                    chrome_rect.min,
                    egui::pos2(chrome_rect.right() - 150.0, chrome_rect.bottom()),
                );
                let drag = ui.interact(
                    drag_rect,
                    ui.id().with("window_drag"),
                    Sense::click_and_drag(),
                );
                if drag.double_clicked() {
                    let maximized = ui
                        .ctx()
                        .input(|input| input.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                } else if drag.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
            });
    }

    fn navigation(&mut self, root: &mut egui::Ui) {
        let t = self.colors();
        egui::Panel::left("navigation")
            .exact_size(196.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::panel_color(self.theme))
                    .inner_margin(egui::Margin::symmetric(10, 14))
                    .stroke(Stroke::new(1.0, t.border)),
            )
            .show(root, |ui| {
                ui.label(
                    RichText::new("CONTROL")
                        .size(9.0)
                        .strong()
                        .color(t.text_muted),
                );
                ui.add_space(5.0);
                for (page, number, label) in Page::ALL {
                    if widgets::nav_button(ui, self.page == page, number, label, t) {
                        self.page = page;
                    }
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(9.0);
                ui.label(
                    RichText::new("MACHINE")
                        .size(9.0)
                        .strong()
                        .color(t.text_muted),
                );
                ui.add_space(6.0);
                widgets::mini_meter(ui, "CPU", self.snapshot.cpu_percent, t.accent, t);
                widgets::mini_meter(ui, "MEMORY", memory_percent(&self.snapshot), t.secondary, t);
                if self.snapshot.gpu.available {
                    widgets::mini_meter(
                        ui,
                        "GPU",
                        self.snapshot.gpu.utilization_percent,
                        theme::mix(t.accent, t.secondary, 0.5),
                        t,
                    );
                }

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    if ui
                        .add_sized(
                            [ui.available_width(), 31.0],
                            egui::Button::new("Theme Studio").fill(t.accent_dim),
                        )
                        .clicked()
                    {
                        self.show_theme_editor = true;
                    }
                    ui.add_space(5.0);
                    ui.label(
                        RichText::new(format!(
                            "{}\nNative telemetry | {:.2}s",
                            if self.snapshot.host_name.is_empty() {
                                "Windows PC"
                            } else {
                                &self.snapshot.host_name
                            },
                            self.snapshot.sample_seconds
                        ))
                        .size(9.0)
                        .color(t.text_muted),
                    );
                });
            });
    }

    fn command_bar(&mut self, root: &mut egui::Ui) {
        let t = self.colors();
        egui::Panel::top("command_bar")
            .exact_size(44.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::panel_color(self.theme))
                    .inner_margin(egui::Margin::symmetric(18, 7))
                    .stroke(Stroke::new(1.0, t.border)),
            )
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new(match self.page {
                            Page::Processes => "PROCESS MATRIX",
                            Page::Performance => "PERFORMANCE ARRAY",
                            Page::History => "RESOURCE HISTORY",
                            Page::Startup => "BOOT SEQUENCE",
                            Page::Users => "USER SESSIONS",
                            Page::Details => "PROCESS DETAILS",
                            Page::Services => "SERVICE CONTROL",
                        })
                        .size(10.0)
                        .strong()
                        .color(t.text_muted),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Run task").clicked() {
                            self.show_run_task = true;
                        }
                        if matches!(self.page, Page::Processes | Page::Details)
                            && ui
                                .add_enabled(
                                    self.selected_pid.is_some(),
                                    egui::Button::new("End task"),
                                )
                                .clicked()
                        {
                            self.request_end_selected();
                        }
                        if ui.button("Theme").clicked() {
                            self.show_theme_editor = true;
                        }
                    });
                });
            });
    }

    fn request_end_selected(&mut self) {
        let Some(pid) = self.selected_pid else { return };
        match platform::can_terminate(pid) {
            Ok(()) => self.pending_end_pid = Some(pid),
            Err(error) => self.message = Some((error, true)),
        }
    }

    fn keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let pages = [
            (egui::Key::Num1, Page::Processes),
            (egui::Key::Num2, Page::Performance),
            (egui::Key::Num3, Page::History),
            (egui::Key::Num4, Page::Startup),
            (egui::Key::Num5, Page::Users),
            (egui::Key::Num6, Page::Details),
            (egui::Key::Num7, Page::Services),
        ];
        for (key, page) in pages {
            if ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, key)) {
                self.page = page;
            }
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::T)) {
            self.show_theme_editor = true;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::R)) {
            self.show_run_task = true;
        }
    }

    fn inspector(&mut self, root: &mut egui::Ui) {
        if !matches!(self.page, Page::Processes | Page::Details) {
            return;
        }
        let t = self.colors();
        egui::Panel::right("inspector")
            .default_size(286.0)
            .min_size(245.0)
            .max_size(380.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(theme::panel_color(self.theme))
                    .inner_margin(egui::Margin::same(16))
                    .stroke(Stroke::new(1.0, t.border)),
            )
            .show(root, |ui| {
                ui.label(RichText::new("INSPECTOR").size(10.0).strong().color(t.text_muted));
                ui.add_space(10.0);
                egui::ScrollArea::vertical().id_salt("inspector_scroll").auto_shrink([false, false]).show(ui, |ui| {
                let selected = self.selected_process().cloned();
                if let Some(process) = selected {
                    ui.label(RichText::new(&process.name).size(19.0).strong().color(t.text));
                    ui.label(
                        RichText::new(format!("PID {} | {}", process.pid, process.user))
                            .monospace()
                            .color(t.accent),
                    );
                    ui.add_space(10.0);
                    widgets::detail_row(ui, "Status", &process.status, t);
                    widgets::detail_row(ui, "CPU", &format::percent(process.cpu_percent), t);
                    widgets::detail_row(ui, "GPU", &format::percent(process.gpu_percent), t);
                    widgets::detail_row(ui, "Working set", &format::bytes(process.memory_bytes), t);
                    widgets::detail_row(ui, "Virtual", &format::bytes(process.virtual_memory_bytes), t);
                    widgets::detail_row(ui, "Disk read", &format::rate(process.read_bytes_per_sec), t);
                    widgets::detail_row(ui, "Disk write", &format::rate(process.write_bytes_per_sec), t);
                    widgets::detail_row(
                        ui,
                        "CPU time",
                        &format::millis(process.accumulated_cpu_millis),
                        t,
                    );
                    widgets::detail_row(ui, "Running", &format::age_from_unix(process.started_at_unix), t);
                    if let Some(parent_pid) = process.parent_pid {
                        widgets::detail_row(ui, "Parent PID", &parent_pid.to_string(), t);
                    }
                    if let Some(cwd) = &process.cwd {
                        widgets::detail_row(ui, "Working dir", &cwd.display().to_string(), t);
                    }

                    ui.add_space(12.0);
                    widgets::section_label(ui, "PROCESS CONTROL", t);
                    widgets::detail_row(ui, "Priority", process.control.priority.label(), t);
                    let affinity = if process.control.system_affinity_mask == 0 {
                        "Unavailable".into()
                    } else {
                        format!(
                            "{} / {} logical processors",
                            process.control.affinity_mask.count_ones(),
                            process.control.system_affinity_mask.count_ones()
                        )
                    };
                    widgets::detail_row(ui, "Affinity", &affinity, t);
                    ui.horizontal(|ui| {
                        let controls_enabled = process.control.accessible
                            && platform::can_control(process.pid).is_ok();
                        if ui
                            .add_enabled(controls_enabled, egui::Button::new("Set priority"))
                            .on_disabled_hover_text(
                                "This process does not expose scheduling controls to Trontop.",
                            )
                            .clicked()
                        {
                            self.show_priority_editor = true;
                        }
                        if ui
                            .add_enabled(
                                controls_enabled && process.control.system_affinity_mask != 0,
                                egui::Button::new("CPU affinity"),
                            )
                            .on_disabled_hover_text(
                                "This process does not expose an editable CPU affinity mask.",
                            )
                            .clicked()
                        {
                            self.affinity_draft = process.control.affinity_mask;
                            self.show_affinity_editor = true;
                        }
                    });
                    ui.label(
                        RichText::new("Changes require confirmation and use your current Windows permissions.")
                            .size(9.0)
                            .color(t.text_muted),
                    );

                    if let Some(path) = &process.executable {
                        ui.add_space(9.0);
                        ui.label(RichText::new("EXECUTABLE").size(9.0).strong().color(t.text_muted));
                        ui.add(
                            egui::Label::new(
                                RichText::new(path.display().to_string()).size(10.0).monospace(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                        if ui.small_button("Reveal in Explorer").clicked() {
                            self.message = Some(match platform::reveal_in_explorer(path) {
                                Ok(()) => ("Opened Explorer".into(), false),
                                Err(error) => (error, true),
                            });
                        }
                    }
                    if !process.command.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new("COMMAND LINE").size(9.0).strong().color(t.text_muted));
                        ui.add(
                            egui::Label::new(RichText::new(&process.command).size(10.0).monospace())
                                .selectable(true)
                                .wrap(),
                        );
                    }
                    ui.add_space(12.0);
                    ui.scope(|ui| {
                        if ui
                            .add_sized(
                                [ui.available_width(), 34.0],
                                egui::Button::new(RichText::new("End task").color(Color32::WHITE))
                                    .fill(t.danger),
                            )
                            .clicked()
                        {
                            self.request_end_selected();
                        }
                    });
                } else {
                    ui.add_space(8.0);
                    egui::Frame::new()
                        .fill(theme::raised_color(self.theme))
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(self.theme.roundness)
                        .inner_margin(egui::Margin::same(16))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.vertical_centered(|ui| {
                                widgets::tront_mark(ui, t.accent, t.secondary, 44.0);
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("Select a process")
                                        .size(17.0)
                                        .strong()
                                        .color(t.text),
                                );
                                ui.label(
                                    RichText::new(
                                        "Lock the inspector to a PID for paths, live counters, priority, affinity, and guarded actions.",
                                    )
                                    .size(11.0)
                                    .color(t.text_muted),
                                );
                            });
                        });
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("TIP  |  Expand a parent row to trace its live process family.")
                            .size(10.0)
                            .monospace()
                            .color(t.text_muted),
                    );
                }
                });
            });
    }

    fn processes_page(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        self.page_header(
            ui,
            "Processes",
            "Live hierarchy and resource totals, sampled outside the render thread",
            true,
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("VIEW")
                    .size(10.0)
                    .strong()
                    .color(t.text_muted),
            );
            if ui
                .selectable_label(self.tree_mode, "Process tree")
                .clicked()
            {
                self.tree_mode = true;
            }
            if ui.selectable_label(!self.tree_mode, "Flat list").clicked() {
                self.tree_mode = false;
            }
            if self.tree_mode {
                ui.separator();
                ui.label(
                    RichText::new("Parent rows include descendants")
                        .size(10.0)
                        .color(t.text_muted),
                );
            }
        });
        ui.add_space(12.0);
        self.telemetry_strip(ui);
        ui.add_space(12.0);
        self.process_table(ui, false);
        let footer = if self.tree_mode {
            format!(
                "{} table rows | {} matching of {} processes",
                self.visible_process_tree.len(),
                self.visible_processes.len(),
                self.snapshot.process_count
            )
        } else {
            format!(
                "{} visible of {} processes",
                self.visible_processes.len(),
                self.snapshot.process_count
            )
        };
        ui.label(RichText::new(footer).size(10.0).color(t.text_muted));
    }

    fn page_header(&mut self, ui: &mut egui::Ui, title: &str, subtitle: &str, searchable: bool) {
        let t = self.colors();
        ui.horizontal(|ui| {
            ui.heading(RichText::new(title).color(t.text));
            if searchable {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let edit = egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Search processes...")
                        .margin(egui::Margin::symmetric(10, 7))
                        .desired_width(ui.available_width().clamp(140.0, 300.0));
                    if ui
                        .add(edit)
                        .on_hover_text("Search name, user, PID, executable path, or command line")
                        .changed()
                    {
                        self.rebuild_visible_processes();
                    }
                });
            }
        });
        ui.label(RichText::new(subtitle).size(11.0).color(t.text_muted));
    }

    fn telemetry_strip(&self, ui: &mut egui::Ui) {
        let t = self.colors();
        let gpu_value = if self.snapshot.gpu.available {
            format::percent(self.snapshot.gpu.utilization_percent)
        } else {
            "WARMING".into()
        };
        ui.columns(4, |columns| {
            widgets::stat_card(
                &mut columns[0],
                "CPU",
                &format::percent(self.snapshot.cpu_percent),
                if self.snapshot.cpu.brand.is_empty() {
                    "total machine load"
                } else {
                    &self.snapshot.cpu.brand
                },
                t.accent,
                self.theme,
                t,
            );
            widgets::stat_card(
                &mut columns[1],
                "MEMORY",
                &format::percent(memory_percent(&self.snapshot)),
                &format!(
                    "{} / {}",
                    format::bytes(self.snapshot.memory_used_bytes),
                    format::bytes(self.snapshot.memory_total_bytes)
                ),
                t.secondary,
                self.theme,
                t,
            );
            widgets::stat_card(
                &mut columns[2],
                "GPU",
                &gpu_value,
                if self.snapshot.gpu.available {
                    "Windows GPU Engine PDH"
                } else {
                    self.snapshot
                        .gpu
                        .error
                        .as_deref()
                        .unwrap_or("collecting exact counters")
                },
                theme::mix(t.accent, t.secondary, 0.5),
                self.theme,
                t,
            );
            widgets::stat_card(
                &mut columns[3],
                "UPTIME",
                &format::duration(self.snapshot.uptime_seconds),
                &format!("{} live entries", self.snapshot.process_count),
                t.good,
                self.theme,
                t,
            );
        });
    }

    fn process_table(&mut self, ui: &mut egui::Ui, detailed: bool) {
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            egui::ScrollArea::horizontal()
                .id_salt(("process_table_horizontal", detailed))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_min_width(if detailed { 900.0 } else { 660.0 });
                    self.process_table_inner(ui, detailed);
                });
        });
    }

    fn process_table_inner(&mut self, ui: &mut egui::Ui, detailed: bool) {
        let t = self.colors();
        let tree_mode = self.tree_mode && !detailed;
        let visible = if tree_mode {
            self.visible_process_tree.clone()
        } else {
            self.visible_processes
                .iter()
                .cloned()
                .map(|process| ProcessTreeRow {
                    totals: ProcessTotals {
                        process_count: 1,
                        cpu_percent: process.cpu_percent,
                        gpu_percent: process.gpu_percent,
                        memory_bytes: process.memory_bytes,
                        read_bytes_per_sec: process.read_bytes_per_sec,
                        write_bytes_per_sec: process.write_bytes_per_sec,
                    },
                    process,
                    depth: 0,
                    has_children: false,
                    descendant_count: 0,
                    expanded: false,
                })
                .collect()
        };
        let selected_pid = self.selected_pid;
        let mut clicked_pid = None;
        let mut toggled_pid = None;
        let mut requested_sort = None;
        // max_scroll_height is the BODY height, excluding the fixed header.
        // Reserve the footer and a possible horizontal scrollbar as well.
        let available_height = (ui.available_height() - 60.0).max(32.0);
        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .vscroll(true)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height)
            .column(
                Column::initial(if detailed { 210.0 } else { 240.0 })
                    .at_least(120.0)
                    .clip(true),
            )
            .column(Column::initial(72.0).at_least(64.0));
        if detailed {
            table = table
                .column(Column::initial(86.0).at_least(64.0).clip(true))
                .column(Column::initial(64.0).at_least(52.0));
        }
        table = table
            .column(Column::initial(76.0).at_least(68.0))
            .column(Column::initial(76.0).at_least(68.0))
            .column(Column::initial(98.0).at_least(82.0))
            .column(Column::initial(88.0).at_least(78.0))
            .column(Column::remainder().at_least(86.0));

        table
            .header(34.0, |mut header| {
                widgets::table_header(
                    &mut header,
                    "NAME",
                    SortColumn::Name,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                    t,
                );
                widgets::table_header(
                    &mut header,
                    "PID",
                    SortColumn::Pid,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                    t,
                );
                if detailed {
                    widgets::table_header(
                        &mut header,
                        "USER",
                        SortColumn::User,
                        self.sort_column,
                        self.sort_direction,
                        &mut requested_sort,
                        t,
                    );
                    widgets::table_header(
                        &mut header,
                        "STATE",
                        SortColumn::Status,
                        self.sort_column,
                        self.sort_direction,
                        &mut requested_sort,
                        t,
                    );
                }
                widgets::table_header(
                    &mut header,
                    "CPU",
                    SortColumn::Cpu,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                    t,
                );
                widgets::table_header(
                    &mut header,
                    "GPU",
                    SortColumn::Gpu,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                    t,
                );
                widgets::table_header(
                    &mut header,
                    "MEMORY",
                    SortColumn::Memory,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                    t,
                );
                widgets::table_header(
                    &mut header,
                    "READ",
                    SortColumn::ReadRate,
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                    t,
                );
                widgets::table_header(
                    &mut header,
                    if detailed { "CPU TIME" } else { "WRITE" },
                    if detailed {
                        SortColumn::CpuTime
                    } else {
                        SortColumn::WriteRate
                    },
                    self.sort_column,
                    self.sort_direction,
                    &mut requested_sort,
                    t,
                );
            })
            .body(|body| {
                body.rows(32.0, visible.len(), |mut row| {
                    let display = &visible[row.index()];
                    let process = &display.process;
                    row.set_selected(selected_pid == Some(process.pid));
                    widgets::table_column(&mut row, t, |ui| {
                        ui.scope(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.spacing_mut().button_padding = Vec2::ZERO;
                            ui.add_space(display.depth as f32 * 13.0);
                            if tree_mode && display.has_children {
                                let marker = if display.expanded { "-" } else { "+" };
                                if ui
                                    .add_sized(
                                        [20.0, 20.0],
                                        egui::Button::new(
                                            RichText::new(marker).monospace().color(t.text),
                                        )
                                        .fill(t.accent_dim)
                                        .stroke(Stroke::new(1.0, t.border)),
                                    )
                                    .on_hover_text(if display.expanded {
                                        "Collapse process subtree"
                                    } else {
                                        "Expand process subtree"
                                    })
                                    .clicked()
                                {
                                    toggled_pid = Some(process.pid);
                                }
                            } else if tree_mode {
                                ui.add_space(21.0);
                            }
                            let name = if tree_mode && display.has_children {
                                format!("{}  [{}]", process.name, display.descendant_count + 1)
                            } else {
                                process.name.clone()
                            };
                            if widgets::table_cell(ui, RichText::new(name).color(t.text).strong()) {
                                clicked_pid = Some(process.pid);
                            }
                        });
                    });
                    widgets::table_column(&mut row, t, |ui| {
                        if widgets::table_cell(
                            ui,
                            RichText::new(process.pid.to_string())
                                .monospace()
                                .color(t.text_muted),
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    if detailed {
                        widgets::table_column(&mut row, t, |ui| {
                            if widgets::table_cell(
                                ui,
                                RichText::new(&process.user).size(11.0).color(t.text_muted),
                            ) {
                                clicked_pid = Some(process.pid);
                            }
                        });
                        widgets::table_column(&mut row, t, |ui| {
                            if widgets::table_cell(
                                ui,
                                RichText::new(&process.status)
                                    .size(10.0)
                                    .color(t.text_muted),
                            ) {
                                clicked_pid = Some(process.pid);
                            }
                        });
                    }
                    widgets::table_column(&mut row, t, |ui| {
                        if widgets::heat_cell(
                            ui,
                            display.totals.cpu_percent,
                            format::percent(display.totals.cpu_percent),
                            t.accent,
                            t,
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    widgets::table_column(&mut row, t, |ui| {
                        if widgets::heat_cell(
                            ui,
                            display.totals.gpu_percent,
                            format::percent(display.totals.gpu_percent),
                            t.secondary,
                            t,
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    widgets::table_column(&mut row, t, |ui| {
                        let pressure = if self.snapshot.memory_total_bytes == 0 {
                            0.0
                        } else {
                            display.totals.memory_bytes as f32
                                / self.snapshot.memory_total_bytes as f32
                                * 100.0
                        };
                        if widgets::heat_cell(
                            ui,
                            pressure * 6.0,
                            format::bytes(display.totals.memory_bytes),
                            t.accent,
                            t,
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    widgets::table_column(&mut row, t, |ui| {
                        if widgets::table_cell(
                            ui,
                            RichText::new(format::rate(display.totals.read_bytes_per_sec))
                                .monospace()
                                .color(t.text_muted),
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    widgets::table_column(&mut row, t, |ui| {
                        let value = if detailed {
                            format::millis(process.accumulated_cpu_millis)
                        } else {
                            format::rate(display.totals.write_bytes_per_sec)
                        };
                        if widgets::table_cell(
                            ui,
                            RichText::new(value).monospace().color(t.text_muted),
                        ) {
                            clicked_pid = Some(process.pid);
                        }
                    });
                    if row.response().clicked() && toggled_pid != Some(process.pid) {
                        clicked_pid = Some(process.pid);
                    }
                });
            });
        if let Some(pid) = clicked_pid {
            self.selected_pid = Some(pid);
        }
        if let Some(pid) = toggled_pid {
            if !self.expanded_pids.insert(pid) {
                self.expanded_pids.remove(&pid);
            }
            self.rebuild_visible_processes();
        }
        if let Some(column) = requested_sort {
            self.sort_by(column);
        }
    }

    fn performance_page(&mut self, ui: &mut egui::Ui) {
        self.page_header(
            ui,
            "Performance",
            "Two minutes of native machine telemetry",
            false,
        );
        ui.add_space(10.0);
        let available = ui.available_size();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(210.0, available.y),
                Layout::top_down(Align::Min),
                |ui| self.performance_rail(ui),
            );
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width().max(320.0), available.y),
                Layout::top_down(Align::Min),
                |ui| match self.performance_device {
                    PerformanceDevice::Cpu => self.cpu_performance(ui),
                    PerformanceDevice::Memory => self.memory_performance(ui),
                    PerformanceDevice::Disk(index) => self.disk_performance(ui, index),
                    PerformanceDevice::Network(index) => self.network_performance(ui, index),
                    PerformanceDevice::Gpu => self.gpu_performance(ui),
                },
            );
        });
    }

    fn performance_rail(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let gpu_value = if self.snapshot.gpu.available {
            format::percent(self.snapshot.gpu.utilization_percent)
        } else {
            "UNAVAILABLE".into()
        };
        if widgets::device_button(
            ui,
            self.performance_device == PerformanceDevice::Cpu,
            "CPU",
            &format::percent(self.snapshot.cpu_percent),
            &self.cpu_history,
            t.accent,
            t,
        ) {
            self.performance_device = PerformanceDevice::Cpu;
        }
        if widgets::device_button(
            ui,
            self.performance_device == PerformanceDevice::Memory,
            "MEMORY",
            &format::percent(memory_percent(&self.snapshot)),
            &self.memory_history,
            t.secondary,
            t,
        ) {
            self.performance_device = PerformanceDevice::Memory;
        }
        for (index, disk) in self.snapshot.disks.iter().enumerate() {
            let history = self
                .disk_history
                .get(&disk.mount)
                .cloned()
                .unwrap_or_default();
            if widgets::device_button(
                ui,
                self.performance_device == PerformanceDevice::Disk(index),
                &format!("DISK {}", disk.mount),
                &format::rate(disk.read_bytes_per_sec + disk.write_bytes_per_sec),
                &history,
                t.good,
                t,
            ) {
                self.performance_device = PerformanceDevice::Disk(index);
            }
        }
        for (index, network) in self
            .snapshot
            .networks
            .iter()
            .enumerate()
            .filter(|(_, row)| row.total_received_bytes + row.total_transmitted_bytes > 0)
            .take(3)
        {
            let history = self
                .network_history
                .get(&network.name)
                .cloned()
                .unwrap_or_default();
            if widgets::device_button(
                ui,
                self.performance_device == PerformanceDevice::Network(index),
                "NETWORK",
                &format::rate(network.received_bytes_per_sec + network.transmitted_bytes_per_sec),
                &history,
                t.secondary,
                t,
            ) {
                self.performance_device = PerformanceDevice::Network(index);
            }
        }
        if widgets::device_button(
            ui,
            self.performance_device == PerformanceDevice::Gpu,
            "GPU ENGINES",
            &gpu_value,
            &self.gpu_history,
            theme::mix(t.accent, t.secondary, 0.5),
            t,
        ) {
            self.performance_device = PerformanceDevice::Gpu;
        }
    }

    fn cpu_performance(&self, ui: &mut egui::Ui) {
        let t = self.colors();
        widgets::performance_heading(
            ui,
            "CPU",
            if self.snapshot.cpu.brand.is_empty() {
                "Processor"
            } else {
                &self.snapshot.cpu.brand
            },
            &format::percent(self.snapshot.cpu_percent),
            t.accent,
            t,
        );
        widgets::history_graph(ui, &self.cpu_history, t.accent, 270.0, Some(100.0), t);
        ui.add_space(12.0);
        ui.columns(4, |columns| {
            widgets::metric(
                &mut columns[0],
                "UTILIZATION",
                &format::percent(self.snapshot.cpu_percent),
                t,
            );
            widgets::metric(
                &mut columns[1],
                "SPEED",
                &format!("{:.2} GHz", self.snapshot.cpu.frequency_mhz as f32 / 1000.0),
                t,
            );
            widgets::metric(
                &mut columns[2],
                "PROCESSES",
                &self.snapshot.process_count.to_string(),
                t,
            );
            widgets::metric(
                &mut columns[3],
                "UPTIME",
                &format::duration(self.snapshot.uptime_seconds),
                t,
            );
        });
        ui.add_space(10.0);
        widgets::detail_row(
            ui,
            "Physical cores",
            &self.snapshot.cpu.physical_cores.to_string(),
            t,
        );
        widgets::detail_row(
            ui,
            "Logical processors",
            &self.snapshot.cpu.logical_cores.to_string(),
            t,
        );
        widgets::detail_row(ui, "Operating system", &self.snapshot.os_name, t);
    }

    fn memory_performance(&self, ui: &mut egui::Ui) {
        let t = self.colors();
        let percent = memory_percent(&self.snapshot);
        widgets::performance_heading(
            ui,
            "MEMORY",
            &format::bytes(self.snapshot.memory_total_bytes),
            &format::percent(percent),
            t.secondary,
            t,
        );
        widgets::history_graph(ui, &self.memory_history, t.secondary, 270.0, Some(100.0), t);
        ui.add_space(12.0);
        ui.columns(4, |columns| {
            widgets::metric(
                &mut columns[0],
                "IN USE",
                &format::bytes(self.snapshot.memory_used_bytes),
                t,
            );
            widgets::metric(
                &mut columns[1],
                "AVAILABLE",
                &format::bytes(self.snapshot.memory_available_bytes),
                t,
            );
            widgets::metric(
                &mut columns[2],
                "COMMITTED",
                &format::bytes(self.snapshot.memory_used_bytes + self.snapshot.swap_used_bytes),
                t,
            );
            widgets::metric(
                &mut columns[3],
                "SWAP",
                &format!(
                    "{} / {}",
                    format::bytes(self.snapshot.swap_used_bytes),
                    format::bytes(self.snapshot.swap_total_bytes)
                ),
                t,
            );
        });
    }

    fn disk_performance(&self, ui: &mut egui::Ui, index: usize) {
        let t = self.colors();
        let Some(disk) = self.snapshot.disks.get(index) else {
            ui.label("Disk no longer present");
            return;
        };
        let history = self
            .disk_history
            .get(&disk.mount)
            .cloned()
            .unwrap_or_default();
        let total_rate = disk.read_bytes_per_sec + disk.write_bytes_per_sec;
        widgets::performance_heading(
            ui,
            &format!("DISK {}", disk.mount),
            &format!("{} | {}", disk.name, disk.kind),
            &format::rate(total_rate),
            t.good,
            t,
        );
        widgets::history_graph(ui, &history, t.good, 270.0, None, t);
        ui.add_space(12.0);
        ui.columns(4, |columns| {
            widgets::metric(
                &mut columns[0],
                "READ",
                &format::rate(disk.read_bytes_per_sec),
                t,
            );
            widgets::metric(
                &mut columns[1],
                "WRITE",
                &format::rate(disk.write_bytes_per_sec),
                t,
            );
            widgets::metric(
                &mut columns[2],
                "CAPACITY",
                &format::bytes(disk.total_bytes),
                t,
            );
            let used = disk.total_bytes.saturating_sub(disk.available_bytes);
            widgets::metric(
                &mut columns[3],
                "USED",
                &format::percent(if disk.total_bytes == 0 {
                    0.0
                } else {
                    used as f32 / disk.total_bytes as f32 * 100.0
                }),
                t,
            );
        });
        ui.add_space(10.0);
        widgets::detail_row(ui, "File system", &disk.file_system, t);
        widgets::detail_row(ui, "Mount", &disk.mount, t);
        widgets::detail_row(
            ui,
            "Removable",
            if disk.removable { "Yes" } else { "No" },
            t,
        );
    }

    fn network_performance(&self, ui: &mut egui::Ui, index: usize) {
        let t = self.colors();
        let Some(network) = self.snapshot.networks.get(index) else {
            ui.label("Network adapter no longer present");
            return;
        };
        let history = self
            .network_history
            .get(&network.name)
            .cloned()
            .unwrap_or_default();
        let rate = network.received_bytes_per_sec + network.transmitted_bytes_per_sec;
        widgets::performance_heading(
            ui,
            "NETWORK",
            &network.name,
            &format::rate(rate),
            t.secondary,
            t,
        );
        widgets::history_graph(ui, &history, t.secondary, 270.0, None, t);
        ui.add_space(12.0);
        ui.columns(4, |columns| {
            widgets::metric(
                &mut columns[0],
                "RECEIVE",
                &format::rate(network.received_bytes_per_sec),
                t,
            );
            widgets::metric(
                &mut columns[1],
                "SEND",
                &format::rate(network.transmitted_bytes_per_sec),
                t,
            );
            widgets::metric(
                &mut columns[2],
                "RECEIVED",
                &format::bytes(network.total_received_bytes),
                t,
            );
            widgets::metric(
                &mut columns[3],
                "SENT",
                &format::bytes(network.total_transmitted_bytes),
                t,
            );
        });
    }

    fn gpu_performance(&self, ui: &mut egui::Ui) {
        let t = self.colors();
        let color = theme::mix(t.accent, t.secondary, 0.5);
        widgets::performance_heading(
            ui,
            "GPU ENGINE ARRAY",
            "Windows GPU Engine counters",
            &format::percent(self.snapshot.gpu.utilization_percent),
            color,
            t,
        );
        if self.snapshot.gpu.available {
            widgets::history_graph(ui, &self.gpu_history, color, 250.0, Some(100.0), t);
            ui.add_space(10.0);
            let engines = self
                .snapshot
                .gpu
                .engine_utilization
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>();
            for (engine, usage) in engines {
                widgets::engine_meter(ui, &engine, usage, color, t);
            }
        } else {
            widgets::empty_state(
                ui,
                "GPU telemetry is warming up",
                self.snapshot.gpu.error.as_deref().unwrap_or("Trontop is enumerating the exact active GPU Engine PDH instances. Rate counters need two observations before they are valid."),
                t,
            );
        }
    }

    fn history_page(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        self.page_header(
            ui,
            "Resource history",
            "Lifetime CPU and I/O totals for the current process set",
            true,
        );
        ui.add_space(12.0);
        let mut rows = self.visible_processes.clone();
        rows.sort_by(|a, b| b.accumulated_cpu_millis.cmp(&a.accumulated_cpu_millis));
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                widgets::section_label(ui, "HEAVIEST LIFETIME CPU CONSUMERS", t);
                for (rank, process) in rows.iter().take(12).enumerate() {
                    egui::Frame::new()
                        .fill(theme::raised_color(self.theme))
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(self.theme.roundness)
                        .inner_margin(egui::Margin::symmetric(12, 9))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("{:02}", rank + 1))
                                        .monospace()
                                        .color(t.accent),
                                );
                                ui.label(RichText::new(&process.name).strong().color(t.text));
                                ui.label(
                                    RichText::new(format!("PID {}", process.pid))
                                        .monospace()
                                        .size(10.0)
                                        .color(t.text_muted),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} I/O",
                                            format::bytes(
                                                process.total_read_bytes
                                                    + process.total_write_bytes
                                            )
                                        ))
                                        .monospace()
                                        .color(t.text_muted),
                                    );
                                    ui.label(
                                        RichText::new(format::millis(
                                            process.accumulated_cpu_millis,
                                        ))
                                        .monospace()
                                        .color(t.secondary),
                                    );
                                });
                            });
                        });
                    ui.add_space(4.0);
                }
            });
    }

    fn startup_page(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        self.inventory_header(
            ui,
            "Startup",
            "Run keys and Startup folders, refreshed every 30 seconds",
            "Search startup inventory",
        );
        ui.add_space(12.0);
        let needle = self.secondary_query.trim().to_lowercase();
        let rows = self
            .snapshot
            .startup
            .iter()
            .filter(|row| {
                needle.is_empty()
                    || row.name.to_lowercase().contains(&needle)
                    || row.command.to_lowercase().contains(&needle)
            })
            .cloned()
            .collect::<Vec<_>>();
        widgets::inventory_table(
            ui,
            "startup_grid",
            ["NAME", "COMMAND", "SOURCE"],
            rows.into_iter()
                .map(|row| [row.name, row.command, row.source])
                .collect(),
            t,
        );
    }

    fn users_page(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        self.page_header(
            ui,
            "Users",
            "Live resource totals grouped by Windows account",
            false,
        );
        ui.add_space(12.0);
        let users = &self.snapshot.users;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let width = ui.available_width();
            let height = (ui.available_height() - 36.0).max(40.0);
            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::initial(width * 0.30).at_least(180.0).clip(true))
                .column(Column::initial(92.0).at_least(80.0))
                .column(Column::initial(92.0).at_least(76.0))
                .column(Column::initial(92.0).at_least(76.0))
                .column(Column::initial(116.0).at_least(94.0))
                .column(Column::remainder().at_least(100.0))
                .min_scrolled_height(0.0)
                .max_scroll_height(height)
                .header(34.0, |mut row| {
                    for label in ["ACCOUNT", "PROCESSES", "CPU", "GPU", "MEMORY", "DISK I/O"] {
                        widgets::table_column(&mut row, t, |ui| {
                            widgets::table_cell(
                                ui,
                                RichText::new(label).size(10.0).strong().color(t.text_muted),
                            );
                        });
                    }
                })
                .body(|body| {
                    body.rows(46.0, users.len(), |mut row| {
                        let user = &users[row.index()];
                        widgets::table_column(&mut row, t, |ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                            ui.painter().rect_filled(rect, 7.0, t.accent_dim);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                user.name.chars().next().unwrap_or('?').to_ascii_uppercase(),
                                FontId::proportional(15.0),
                                t.text,
                            );
                            widgets::table_cell(
                                ui,
                                RichText::new(&user.name).strong().color(t.text),
                            );
                        });
                        for (value, label, color) in [
                            (0.0, user.process_count.to_string(), t.accent),
                            (
                                user.cpu_percent,
                                format::percent(user.cpu_percent),
                                t.accent,
                            ),
                            (
                                user.gpu_percent,
                                format::percent(user.gpu_percent),
                                t.secondary,
                            ),
                            (0.0, format::bytes(user.memory_bytes), t.accent),
                            (0.0, format::rate(user.disk_bytes_per_sec), t.secondary),
                        ] {
                            widgets::table_column(&mut row, t, |ui| {
                                widgets::heat_cell(ui, value, label, color, t);
                            });
                        }
                    })
                });
        });
    }

    fn details_page(&mut self, ui: &mut egui::Ui) {
        self.page_header(
            ui,
            "Details",
            "Dense process telemetry with account, state, and lifetime CPU",
            true,
        );
        ui.add_space(12.0);
        self.process_table(ui, true);
        ui.label(
            RichText::new(
                "Drag dividers to resize columns. Scroll horizontally for more counters.",
            )
            .size(10.0)
            .color(self.colors().text_muted),
        );
    }

    fn services_page(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        self.inventory_header(
            ui,
            "Services",
            "Windows Service Control Manager inventory, refreshed every 30 seconds",
            "Search services",
        );
        ui.add_space(12.0);
        let needle = self.secondary_query.trim().to_lowercase();
        let rows = self
            .snapshot
            .services
            .iter()
            .filter(|row| {
                needle.is_empty()
                    || row.name.to_lowercase().contains(&needle)
                    || row.display_name.to_lowercase().contains(&needle)
            })
            .cloned()
            .collect::<Vec<_>>();
        widgets::inventory_table(
            ui,
            "services_grid",
            ["DISPLAY NAME", "SERVICE", "STATUS / PID"],
            rows.into_iter()
                .map(|row| {
                    let pid = if row.pid == 0 {
                        "-".into()
                    } else {
                        row.pid.to_string()
                    };
                    [
                        row.display_name,
                        row.name,
                        format!("{} | {pid}", row.status),
                    ]
                })
                .collect(),
            t,
        );
    }

    fn inventory_header(&mut self, ui: &mut egui::Ui, title: &str, subtitle: &str, hint: &str) {
        let t = self.colors();
        ui.horizontal(|ui| {
            ui.heading(RichText::new(title).color(t.text));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.secondary_query)
                        .hint_text(hint)
                        .margin(egui::Margin::symmetric(10, 7))
                        .desired_width(ui.available_width().clamp(140.0, 300.0)),
                );
            });
        });
        ui.label(RichText::new(subtitle).size(11.0).color(t.text_muted));
    }

    fn confirm_end_task(&mut self, ctx: &egui::Context) {
        let Some(pid) = self.pending_end_pid else {
            return;
        };
        let t = self.colors();
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
                ui.set_width(390.0);
                ui.label(RichText::new(format!("End {name}?")).size(18.0).strong().color(t.text));
                ui.label(RichText::new(format!("PID {pid} will be terminated immediately. Unsaved data in that process will be lost.")).color(t.text_muted));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.pending_end_pid = None;
                    }
                    if ui.add(egui::Button::new(RichText::new("End process").color(Color32::WHITE)).fill(t.danger)).clicked() {
                        self.message = Some(match platform::terminate_process(pid) {
                            Ok(()) => (format!("Ended {name} ({pid})"), false),
                            Err(error) => (error, true),
                        });
                        self.pending_end_pid = None;
                    }
                });
            });
    }

    fn priority_editor(&mut self, ctx: &egui::Context) {
        if !self.show_priority_editor {
            return;
        }
        let Some(process) = self.selected_process().cloned() else {
            self.show_priority_editor = false;
            return;
        };
        let t = self.colors();
        let mut open = true;
        let mut requested = None;
        egui::Window::new("Process priority")
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(410.0);
                ui.label(
                    RichText::new(format!("{} | PID {}", process.name, process.pid))
                        .size(17.0)
                        .strong()
                        .color(t.text),
                );
                ui.label(
                    RichText::new("Choose a Windows scheduler priority class. Realtime is deliberately unavailable as an action.")
                        .color(t.text_muted),
                );
                ui.add_space(12.0);
                for priority in PriorityClass::EDITABLE {
                    let current = process.control.priority == priority;
                    let text = if current {
                        format!("{}  |  CURRENT", priority.label())
                    } else {
                        priority.label().into()
                    };
                    let response = ui.add_sized(
                        [ui.available_width(), 31.0],
                        egui::Button::new(RichText::new(text).color(t.text))
                            .selected(current)
                            .fill(if current { t.accent_dim } else { t.panel_raised }),
                    );
                    if response.clicked() && !current {
                        requested = Some(priority);
                    }
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new("High priority can reduce responsiveness elsewhere on the machine and receives an additional explicit confirmation.")
                        .size(10.0)
                        .color(t.danger),
                );
            });
        if let Some(priority) = requested {
            self.pending_control_action = Some(PendingControlAction::Priority {
                pid: process.pid,
                started_at_unix: process.started_at_unix,
                priority,
            });
            self.show_priority_editor = false;
        } else {
            self.show_priority_editor = open;
        }
    }

    fn affinity_editor(&mut self, ctx: &egui::Context) {
        if !self.show_affinity_editor {
            return;
        }
        let Some(process) = self.selected_process().cloned() else {
            self.show_affinity_editor = false;
            return;
        };
        let system_mask = process.control.system_affinity_mask;
        if system_mask == 0 {
            self.show_affinity_editor = false;
            return;
        }
        let t = self.colors();
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("CPU affinity")
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(520.0);
                ui.label(
                    RichText::new(format!("{} | PID {}", process.name, process.pid))
                        .size(17.0)
                        .strong()
                        .color(t.text),
                );
                ui.label(
                    RichText::new(
                        "Logical processors available in the process's Windows processor group.",
                    )
                    .color(t.text_muted),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Select all").clicked() {
                        self.affinity_draft = system_mask;
                    }
                    if ui.button("Current mask").clicked() {
                        self.affinity_draft = process.control.affinity_mask;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} selected", self.affinity_draft.count_ones()))
                                .monospace()
                                .color(t.secondary),
                        );
                    });
                });
                ui.add_space(8.0);
                egui::Grid::new("affinity_processor_grid")
                    .num_columns(4)
                    .spacing([18.0, 7.0])
                    .show(ui, |ui| {
                        let mut shown = 0;
                        for processor in 0..usize::BITS {
                            let bit = 1_usize << processor;
                            if system_mask & bit == 0 {
                                continue;
                            }
                            let mut enabled = self.affinity_draft & bit != 0;
                            if ui
                                .checkbox(&mut enabled, format!("CPU {processor:02}"))
                                .changed()
                            {
                                if enabled {
                                    self.affinity_draft |= bit;
                                } else {
                                    self.affinity_draft &= !bit;
                                }
                            }
                            shown += 1;
                            if shown % 4 == 0 {
                                ui.end_row();
                            }
                        }
                    });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    if ui
                        .add_enabled(
                            self.affinity_draft != 0
                                && self.affinity_draft & !system_mask == 0
                                && self.affinity_draft != process.control.affinity_mask,
                            egui::Button::new("Review change"),
                        )
                        .clicked()
                    {
                        apply = true;
                    }
                    if self.affinity_draft == 0 {
                        ui.label(
                            RichText::new("Select at least one processor.")
                                .size(10.0)
                                .color(t.danger),
                        );
                    }
                });
            });
        if apply {
            self.pending_control_action = Some(PendingControlAction::Affinity {
                pid: process.pid,
                started_at_unix: process.started_at_unix,
                affinity_mask: self.affinity_draft,
            });
            self.show_affinity_editor = false;
        } else {
            self.show_affinity_editor = open && !cancel;
        }
    }

    fn confirm_control_action(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending_control_action else {
            return;
        };
        let (pid, started_at_unix) = match action {
            PendingControlAction::Priority {
                pid,
                started_at_unix,
                ..
            }
            | PendingControlAction::Affinity {
                pid,
                started_at_unix,
                ..
            } => (pid, started_at_unix),
        };
        let process = self
            .snapshot
            .processes
            .iter()
            .find(|process| process.pid == pid && process.started_at_unix == started_at_unix)
            .cloned();
        let name = process
            .as_ref()
            .map(|process| process.name.as_str())
            .unwrap_or("Unknown process");
        let (title, description, button, dangerous) = match action {
            PendingControlAction::Priority { priority, .. } => (
                format!("Set {name} to {} priority?", priority.label()),
                if priority == PriorityClass::High {
                    "High priority can starve other applications and reduce system responsiveness. Trontop will not offer Realtime priority."
                } else {
                    "This changes how Windows schedules the process until it exits or another tool changes it."
                },
                "Apply priority",
                priority == PriorityClass::High,
            ),
            PendingControlAction::Affinity { affinity_mask, .. } => (
                format!("Limit {name} CPU affinity?"),
                "This changes which logical processors may run the process until it exits or another tool changes it.",
                "Apply affinity",
                affinity_mask.count_ones()
                    < process
                        .as_ref()
                        .map_or(0, |row| row.control.system_affinity_mask.count_ones()),
            ),
        };
        let t = self.colors();
        egui::Window::new("Confirm process control")
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(420.0);
                ui.label(RichText::new(title).size(18.0).strong().color(t.text));
                ui.label(RichText::new(description).color(t.text_muted));
                ui.label(
                    RichText::new(format!("PID {pid} | identity checked against process start time"))
                        .size(10.0)
                        .monospace()
                        .color(t.secondary),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.pending_control_action = None;
                    }
                    let fill = if dangerous { t.danger } else { t.accent };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(button).color(Color32::WHITE))
                                .fill(fill),
                        )
                        .clicked()
                    {
                        let result = if process.is_none() {
                            Err("The selected PID no longer belongs to the same process. No change was made.".into())
                        } else {
                            match action {
                                PendingControlAction::Priority { priority, .. } => {
                                    platform::set_process_priority(pid, priority)
                                }
                                PendingControlAction::Affinity { affinity_mask, .. } => {
                                    platform::set_process_affinity(pid, affinity_mask)
                                }
                            }
                        };
                        self.message = Some(match result {
                            Ok(()) => (format!("Updated scheduling controls for {name} ({pid})"), false),
                            Err(error) => (error, true),
                        });
                        self.pending_control_action = None;
                    }
                });
            });
    }

    fn run_task_window(&mut self, ctx: &egui::Context) {
        if !self.show_run_task {
            return;
        }
        let t = self.colors();
        let mut open = true;
        egui::Window::new("Run new task")
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(440.0);
                ui.label(
                    RichText::new("Launch a program or command")
                        .size(17.0)
                        .strong()
                        .color(t.text),
                );
                ui.label(
                    RichText::new("The command is launched with your current Windows permissions.")
                        .size(11.0)
                        .color(t.text_muted),
                );
                ui.add_space(10.0);
                let response = ui.add_sized(
                    [ui.available_width(), 30.0],
                    egui::TextEdit::singleline(&mut self.run_command)
                        .hint_text("notepad.exe or C:\\path\\app.exe"),
                );
                if self.run_command.is_empty() {
                    response.request_focus();
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_run_task = false;
                    }
                    let run = ui.add_enabled(
                        !self.run_command.trim().is_empty(),
                        egui::Button::new("Run").fill(t.accent_dim),
                    );
                    if run.clicked()
                        || (response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                    {
                        self.message =
                            Some(match platform::launch_command(self.run_command.trim()) {
                                Ok(()) => (format!("Launched {}", self.run_command.trim()), false),
                                Err(error) => (error, true),
                            });
                        self.run_command.clear();
                        self.show_run_task = false;
                    }
                });
            });
        self.show_run_task &= open;
    }

    fn theme_editor(&mut self, ctx: &egui::Context) {
        if !self.show_theme_editor {
            return;
        }
        let before = self.theme;
        let t = self.colors();
        let mut open = true;
        egui::Window::new("Trontop Theme Studio")
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(500.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(RichText::new("LIVE VISUAL SYSTEM").size(10.0).strong().color(t.accent));
                ui.label(RichText::new("Build a control deck that belongs to your machine.").size(18.0).strong().color(t.text));
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("TrontStack").clicked() { self.theme = ThemeSettings::tront_stack(); }
                    if ui.button("Demigod").clicked() { self.theme = ThemeSettings::demigod(); }
                    if ui.button("Monke Portal").clicked() { self.theme = ThemeSettings::monke_portal(); }
                    if ui.button("Copper Legacy").clicked() { self.theme = ThemeSettings::copper_legacy(); }
                });
                ui.add_space(9.0);
                egui::Grid::new("theme_controls").num_columns(2).spacing([14.0, 9.0]).show(ui, |ui| {
                    ui.label("Mode");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.theme.dark, true, "Dark");
                        ui.selectable_value(&mut self.theme.dark, false, "Light");
                    });
                    ui.end_row();
                    ui.label("Primary");
                    ui.color_edit_button_srgb(&mut self.theme.accent);
                    ui.end_row();
                    ui.label("Secondary");
                    ui.color_edit_button_srgb(&mut self.theme.secondary);
                    ui.end_row();
                    ui.label("Gradient");
                    ui.checkbox(&mut self.theme.gradient_enabled, "Enabled");
                    ui.end_row();
                    ui.label("Angle");
                    ui.add(egui::Slider::new(&mut self.theme.gradient_angle, 0.0..=360.0).suffix(" deg"));
                    ui.end_row();
                    ui.label("Intensity");
                    ui.add(egui::Slider::new(&mut self.theme.gradient_strength, 0.0..=0.75));
                    ui.end_row();
                    ui.label("Panel frost");
                    ui.add(egui::Slider::new(&mut self.theme.frost, 0.45..=1.0));
                    ui.end_row();
                    ui.label("Roundness");
                    ui.add(egui::Slider::new(&mut self.theme.roundness, 0.0..=18.0).suffix(" px"));
                    ui.end_row();
                });
                ui.add_space(10.0);
                egui::Frame::new().fill(theme::raised_color(self.theme)).stroke(Stroke::new(1.0, t.border)).corner_radius(self.theme.roundness).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        widgets::tront_mark(ui, theme::tokens(self.theme).accent, theme::tokens(self.theme).secondary, 31.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new("TRONTOP // THEME PREVIEW").strong());
                            ui.label(RichText::new("High-performance telemetry by Tront").size(10.0).color(theme::tokens(self.theme).text_muted));
                        });
                    });
                });
                ui.add_space(7.0);
                ui.label(RichText::new("Inspired by the same gradient and harmony language used across Photochop and the TrontStack native tools.").size(10.0).color(t.text_muted));
            });
        self.show_theme_editor &= open;
        self.theme = self.theme.normalized();
        if self.theme != before {
            theme::install(ctx, self.theme);
            ctx.request_repaint();
        }
    }

    fn message_bar(&mut self, root: &mut egui::Ui) {
        let Some((message, is_error)) = self.message.clone() else {
            return;
        };
        let t = self.colors();
        egui::Panel::bottom("message_bar")
            .exact_size(34.0)
            .frame(egui::Frame::new().fill(if is_error {
                theme::mix(t.panel, t.danger, 0.35)
            } else {
                theme::mix(t.panel, t.good, 0.25)
            }))
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new(message).color(t.text));
                    if ui.small_button("Dismiss").clicked() {
                        self.message = None;
                    }
                });
            });
    }
}

impl eframe::App for TrontopApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(snapshot) = self.sampler.latest_after(self.seen_generation) {
            self.accept_sample(snapshot);
        }
        if let Some(action) = self.tray.as_ref().and_then(TrayController::poll) {
            match action {
                TrayAction::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                TrayAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.keyboard_shortcuts(&ctx);
        theme::paint_background(&ctx, self.theme);
        self.custom_chrome(ui);
        self.message_bar(ui);
        self.navigation(ui);
        self.command_bar(ui);
        self.inspector(ui);
        let t = self.colors();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::panel_color(self.theme))
                    .inner_margin(egui::Margin::same(18))
                    .stroke(Stroke::new(1.0, t.border)),
            )
            .show(ui, |ui| match self.page {
                Page::Processes => self.processes_page(ui),
                Page::Performance => self.performance_page(ui),
                Page::History => self.history_page(ui),
                Page::Startup => self.startup_page(ui),
                Page::Users => self.users_page(ui),
                Page::Details => self.details_page(ui),
                Page::Services => self.services_page(ui),
            });
        self.confirm_end_task(&ctx);
        self.priority_editor(&ctx);
        self.affinity_editor(&ctx);
        self.confirm_control_action(&ctx);
        self.run_task_window(&ctx);
        self.theme_editor(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(theme::STORAGE_KEY, self.theme.encode());
    }
}

fn chrome_button(ui: &mut egui::Ui, label: &str, t: Tokens, danger: bool) -> egui::Response {
    ui.add_sized(
        [34.0, 27.0],
        egui::Button::new(RichText::new(label).size(11.0).color(if danger {
            t.danger
        } else {
            t.text_muted
        }))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE),
    )
}

fn memory_percent(snapshot: &SystemSnapshot) -> f32 {
    if snapshot.memory_total_bytes == 0 {
        0.0
    } else {
        snapshot.memory_used_bytes as f32 / snapshot.memory_total_bytes as f32 * 100.0
    }
}
