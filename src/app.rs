use crate::format;
use crate::gpu_sensors::SensorHistory;
use crate::icons::Icon;
use crate::model::{
    PriorityClass, ProcessIdentity, ProcessRow, ProcessTotals, ProcessTreeRow, SortColumn,
    SortDirection, SystemSnapshot, build_process_tree, sort_process_indices,
};
use crate::platform;
use crate::process_actions::{Action as ProcessAction, Request as ProcessRequest};
use crate::sampler::Sampler;
use crate::theme::{self, ThemeSettings, Tokens};
use crate::tray::{TrayAction, TrayController};
use crate::widgets;
use eframe::egui;
use egui::{Align, Color32, FontId, Layout, RichText, Sense, Stroke, Vec2};
use egui_extras::{Column, TableBuilder};
use std::collections::{HashMap, HashSet, VecDeque};

const HISTORY_LENGTH: usize = 120;

mod diagnostics;
mod disks;
mod export;
mod inventory;
mod overview;
mod sensors;
mod service_controls;
mod storage;
mod tree_state;

#[cfg(test)]
mod ui_smoke;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Page {
    Overview,
    Sensors,
    Processes,
    Performance,
    History,
    Startup,
    Users,
    Details,
    Services,
}

impl Page {
    fn icon(self) -> Icon {
        match self {
            Self::Overview => Icon::Overview,
            Self::Sensors => Icon::Sensors,
            Self::Processes => Icon::Processes,
            Self::Performance => Icon::Performance,
            Self::History => Icon::History,
            Self::Startup => Icon::Startup,
            Self::Users => Icon::Users,
            Self::Details => Icon::Details,
            Self::Services => Icon::Services,
        }
    }

    const ALL: [(Self, &'static str, &'static str); 9] = [
        (Self::Overview, "00", "Overview"),
        (Self::Processes, "01", "Processes"),
        (Self::Performance, "02", "Performance"),
        (Self::History, "03", "History"),
        (Self::Startup, "04", "Startup"),
        (Self::Users, "05", "Users"),
        (Self::Details, "06", "Details"),
        (Self::Services, "07", "Services"),
        (Self::Sensors, "08", "Hardware sensors"),
    ];
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PerformanceDevice {
    Cpu,
    Memory,
    Disk(usize),
    Network(usize),
    Gpu,
    GpuSensors,
    PhysicalDisks,
}

#[derive(Clone, Copy)]
enum PendingControlAction {
    Priority {
        identity: ProcessIdentity,
        priority: PriorityClass,
    },
    Affinity {
        identity: ProcessIdentity,
        affinity_mask: usize,
    },
}

#[derive(Clone)]
struct PendingEndTask {
    identity: ProcessIdentity,
    name: String,
}

pub struct TrontopApp {
    graphics_recovering: std::sync::Arc<std::sync::atomic::AtomicBool>,
    sampler: Option<Sampler>,
    process_icons: crate::process_icons::Cache,
    snapshot: SystemSnapshot,
    seen_generation: u64,
    page: Page,
    performance_device: PerformanceDevice,
    query: String,
    secondary_query: String,
    sort_column: SortColumn,
    sort_direction: SortDirection,
    visible_processes: Vec<usize>,
    visible_process_tree: Vec<ProcessTreeRow>,
    history_processes: Vec<usize>,
    tree_mode: bool,
    tree_initialized: bool,
    tree_expansion: tree_state::Expansion,
    selected_pid: Option<u32>,
    inspector_visible: bool,
    selected_service: Option<String>,
    service_controller: crate::service_control::Controller,
    pending_service: Option<crate::service_control::Request>,
    service_event: Option<crate::service_control::Event>,
    service_observations: crate::service_control::Observations,
    pending_end_task: Option<PendingEndTask>,
    pending_control_action: Option<PendingControlAction>,
    process_actions: crate::process_actions::Controller,
    message: Option<(String, bool)>,
    cpu_history: VecDeque<f32>,
    memory_history: VecDeque<f32>,
    gpu_history: VecDeque<f32>,
    gpu_engine_names: Vec<String>,
    sensor_history: HashMap<String, SensorHistory>,
    disk_history: HashMap<String, VecDeque<f32>>,
    physical_disk_history: disks::Histories,
    selected_physical_disk: Option<String>,
    network_history: HashMap<String, VecDeque<f32>>,
    theme: ThemeSettings,
    show_theme_editor: bool,
    theme_studio: crate::theme_studio::Studio,
    show_diagnostics: bool,
    show_export: bool,
    export_options: crate::export::Options,
    exporter: crate::export::Exporter,
    export_result: Option<crate::export::Outcome>,
    show_run_task: bool,
    show_priority_editor: bool,
    show_affinity_editor: bool,
    affinity_draft: usize,
    run_command: String,
    tray: Option<TrayController>,
}

impl TrontopApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        graphics_recovering: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        let saved_theme = cc
            .storage
            .and_then(|storage| {
                storage
                    .get_string(theme::STORAGE_KEY)
                    .or_else(|| storage.get_string(theme::LEGACY_STORAGE_KEY))
            })
            .and_then(|value| ThemeSettings::decode(&value))
            .unwrap_or_default();
        theme::install(&cc.egui_ctx, saved_theme);
        let tray = TrayController::new(cc.egui_ctx.clone());
        let sampler = Sampler::spawn(cc.egui_ctx.clone(), tray.as_ref().map(TrayController::sink));
        let mut app = Self::with_services(saved_theme, Some(sampler), tray);
        app.graphics_recovering = graphics_recovering;
        app.process_icons = crate::process_icons::Cache::spawn(cc.egui_ctx.clone());
        app.service_controller = crate::service_control::Controller::spawn(cc.egui_ctx.clone());
        app.process_actions = crate::process_actions::Controller::spawn(cc.egui_ctx.clone());
        app.exporter = crate::export::Exporter::native();
        if let Some(value) = cc
            .storage
            .and_then(|s| s.get_string(crate::theme_studio::LIBRARY_KEY))
        {
            app.theme_studio.load_library(&value);
        }
        app
    }

    fn with_services(
        saved_theme: ThemeSettings,
        sampler: Option<Sampler>,
        tray: Option<TrayController>,
    ) -> Self {
        Self {
            graphics_recovering: Default::default(),
            sampler,
            process_icons: crate::process_icons::Cache::default(),
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
            history_processes: Vec::new(),
            tree_mode: true,
            tree_initialized: false,
            tree_expansion: tree_state::Expansion::default(),
            selected_pid: None,
            inspector_visible: true,
            selected_service: None,
            service_controller: crate::service_control::Controller::default(),
            pending_service: None,
            service_event: None,
            service_observations: crate::service_control::Observations::default(),
            pending_end_task: None,
            pending_control_action: None,
            process_actions: crate::process_actions::Controller::default(),
            message: None,
            cpu_history: VecDeque::with_capacity(HISTORY_LENGTH),
            memory_history: VecDeque::with_capacity(HISTORY_LENGTH),
            gpu_history: VecDeque::with_capacity(HISTORY_LENGTH),
            gpu_engine_names: vec![
                "3D".into(),
                "Copy".into(),
                "VideoEncode".into(),
                "VideoDecode".into(),
            ],
            sensor_history: HashMap::new(),
            disk_history: HashMap::new(),
            physical_disk_history: disks::Histories::default(),
            selected_physical_disk: None,
            network_history: HashMap::new(),
            theme: saved_theme,
            show_theme_editor: false,
            theme_studio: crate::theme_studio::Studio::default(),
            show_diagnostics: false,
            show_export: false,
            export_options: crate::export::Options::default(),
            exporter: crate::export::Exporter::default(),
            export_result: None,
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
        self.tree_expansion.retain_live(&snapshot.processes);
        if let Some(selected) = self.selected_process() {
            let same = snapshot
                .processes
                .iter()
                .find(|row| row.pid == selected.pid)
                .is_some_and(|row| {
                    row.started_at_unix == selected.started_at_unix
                        && match (row.identity(), selected.identity()) {
                            (Some(new), Some(old)) => new == old,
                            _ => true,
                        }
                });
            if !same {
                self.selected_pid = None;
                self.show_priority_editor = false;
                self.show_affinity_editor = false;
                // Pending confirmations keep their original target, never a reused PID.
            }
        }
        self.seen_generation = snapshot.sequence;
        self.physical_disk_history
            .push(&snapshot.physical_disks, std::time::Instant::now());
        if let Some(at) = snapshot.gpu_sensors.sampled_at {
            for adapter in &snapshot.gpu_sensors.adapters {
                if let Some(uuid) = &adapter.uuid {
                    self.sensor_history.entry(uuid.clone()).or_default();
                }
            }
            for (uuid, history) in &mut self.sensor_history {
                let adapter = snapshot
                    .gpu_sensors
                    .adapters
                    .iter()
                    .find(|a| a.uuid.as_ref() == Some(uuid))
                    .filter(|_| !snapshot.gpu_sensors.using_cached);
                history.push(at, adapter);
            }
            // Expire removed adapters after their entire history window is empty.
            self.sensor_history.retain(|uuid, history| {
                snapshot
                    .gpu_sensors
                    .adapters
                    .iter()
                    .any(|a| a.uuid.as_ref() == Some(uuid))
                    || history
                        .points
                        .iter()
                        .any(|p| p.temperature_c.is_some() || p.power_w.is_some())
            });
        }
        widgets::push_history(&mut self.cpu_history, snapshot.cpu_percent, HISTORY_LENGTH);
        widgets::push_history(
            &mut self.memory_history,
            memory_percent(&snapshot),
            HISTORY_LENGTH,
        );
        widgets::push_history(
            &mut self.gpu_history,
            snapshot.gpu.reading().exact().unwrap_or(f32::NAN),
            HISTORY_LENGTH,
        );
        for (name, _) in &snapshot.gpu.engine_utilization {
            if !self.gpu_engine_names.contains(name) && self.gpu_engine_names.len() < 32 {
                self.gpu_engine_names.push(name.clone());
            }
        }
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
            for process in self.snapshot.processes.iter().filter(|process| {
                process
                    .parent_pid
                    .is_none_or(|parent| !live_pids.contains(&parent))
            }) {
                self.tree_expansion.expand(process);
            }
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
            .enumerate()
            .filter(|(_, process)| matching_pids.contains(&process.pid))
            .map(|(index, _)| index)
            .collect();
        sort_process_indices(
            &mut self.visible_processes,
            &self.snapshot.processes,
            self.sort_column,
            self.sort_direction,
        );
        self.history_processes.clone_from(&self.visible_processes);
        self.history_processes.sort_by(|&a, &b| {
            self.snapshot.processes[b]
                .accumulated_cpu_millis
                .cmp(&self.snapshot.processes[a].accumulated_cpu_millis)
        });
        self.history_processes.truncate(12);
        self.visible_process_tree = build_process_tree(
            &self.snapshot.processes,
            &matching_pids,
            self.tree_expansion.pids(),
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
                    widgets::hover_label(
                        ui,
                        RichText::new("TRONTOP").size(15.0).strong().color(t.text),
                    );
                    widgets::hover_label(
                        ui,
                        RichText::new("SYSTEM CONTROL DECK")
                            .size(9.0)
                            .strong()
                            .color(t.text_muted),
                    );
                    ui.add_space(8.0);
                    let state = self
                        .snapshot
                        .diagnostics
                        .get(crate::diagnostics::Provider::System)
                        .state(
                            crate::diagnostics::Provider::System,
                            std::time::Instant::now(),
                        );
                    widgets::status_pill(
                        ui,
                        &state.label().to_uppercase(),
                        diagnostics::state_color(state, t),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if chrome_button(ui, Icon::Close, "Close", t, true).clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        let maximized = ui
                            .ctx()
                            .input(|input| input.viewport().maximized.unwrap_or(false));
                        if chrome_button(
                            ui,
                            if maximized {
                                Icon::Restore
                            } else {
                                Icon::Maximize
                            },
                            if maximized { "Restore" } else { "Maximize" },
                            t,
                            false,
                        )
                        .clicked()
                        {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                        }
                        if chrome_button(ui, Icon::Minimize, "Minimize", t, false).clicked() {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        widgets::hover_label(
                            ui,
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
                egui::Panel::bottom("navigation_footer")
                    .exact_size(230.0)
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        widgets::mini_meter(
                            ui,
                            "CPU",
                            (self.seen_generation > 0).then_some(self.snapshot.cpu_percent),
                            t.accent,
                            t,
                        );
                        widgets::mini_meter(
                            ui,
                            "MEMORY",
                            (self.snapshot.memory_total_bytes > 0)
                                .then(|| memory_percent(&self.snapshot)),
                            t.secondary,
                            t,
                        );
                        widgets::mini_meter(
                            ui,
                            "GPU",
                            self.snapshot.gpu.reading().exact(),
                            theme::mix(t.accent, t.secondary, 0.5),
                            t,
                        );
                        ui.add_space(8.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(if self.snapshot.host_name.is_empty() {
                                    "Windows PC"
                                } else {
                                    &self.snapshot.host_name
                                })
                                .size(9.0)
                                .color(t.text_muted),
                            )
                            .truncate(),
                        );
                        widgets::hover_label(
                            ui,
                            RichText::new(format!(
                                "Native telemetry | {:.2}s",
                                self.snapshot.sample_seconds
                            ))
                            .size(9.0)
                            .color(t.text_muted),
                        );
                        if widgets::icon_button(
                            ui,
                            Icon::Theme,
                            "Theme Studio",
                            Vec2::new(ui.available_width(), 31.0),
                            t.accent_dim,
                            t,
                        )
                        .clicked()
                        {
                            self.show_theme_editor = true;
                        }
                    });
                egui::ScrollArea::vertical()
                    .id_salt("navigation_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 5.0;
                        widgets::hover_label(
                            ui,
                            RichText::new("CONTROL")
                                .size(9.0)
                                .strong()
                                .color(t.text_muted),
                        );
                        ui.add_space(5.0);
                        for (page, _, label) in Page::ALL {
                            if widgets::nav_button(ui, self.page == page, page.icon(), label, t) {
                                self.page = page;
                            }
                        }
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
                    widgets::hover_label(
                        ui,
                        RichText::new(match self.page {
                            Page::Overview => "MACHINE OVERVIEW",
                            Page::Sensors => "HARDWARE SENSORS",
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
                        if widgets::icon_button(
                            ui,
                            Icon::Startup,
                            "Run task",
                            Vec2::ZERO,
                            t.accent_dim,
                            t,
                        )
                        .clicked()
                        {
                            self.show_run_task = true;
                        }
                        if matches!(self.page, Page::Processes | Page::Details)
                            && ui
                                .add_enabled_ui(self.selected_pid.is_some() && !self.process_actions.busy(), |ui| {
                                    widgets::icon_button(
                                        ui,
                                        Icon::Stop,
                                        "End task",
                                        Vec2::ZERO,
                                        t.panel_raised,
                                        t,
                                    )
                                })
                                .inner
                                .clicked()
                        {
                            self.request_end_selected();
                        }
                        if matches!(self.page, Page::Processes | Page::Details) {
                            let selected = self.selected_pid.is_some();
                            let fill = if selected && self.inspector_visible {
                                t.accent_dim
                            } else {
                                t.panel_raised
                            };
                            if ui.add_enabled_ui(selected, |ui| {
                                widgets::icon_button(ui, Icon::Details, "Inspector", Vec2::ZERO, fill, t)
                            }).inner
                                .on_hover_text(if self.inspector_visible {
                                    "Hide inspector and give the table more room. Selection is retained."
                                } else {
                                    "Show paths, live counters and controls for the selected process."
                                })
                                .on_disabled_hover_text("Select a process to inspect it.")
                                .clicked()
                            {
                                self.inspector_visible = !self.inspector_visible;
                            }
                        }
                        if widgets::icon_button(
                            ui,
                            Icon::Theme,
                            "Theme",
                            Vec2::ZERO,
                            t.panel_raised,
                            t,
                        )
                        .clicked()
                        {
                            self.show_theme_editor = true;
                        }
                        if widgets::icon_button(
                            ui,
                            Icon::Info,
                            "About",
                            Vec2::ZERO,
                            t.panel_raised,
                            t,
                        )
                        .clicked()
                        {
                            self.show_diagnostics = true;
                        }
                        if widgets::icon_button(
                            ui,
                            Icon::Export,
                            "Export",
                            Vec2::ZERO,
                            t.panel_raised,
                            t,
                        )
                        .clicked()
                        {
                            if !self.exporter.busy() {
                                self.export_options = crate::export::Options::default();
                                self.export_result = None;
                            }
                            self.show_export = true;
                        }
                    });
                });
            });
    }

    fn request_end_selected(&mut self) {
        if self.process_actions.busy() {
            return;
        }
        let Some(process) = self.selected_process() else {
            return;
        };
        let result = platform::can_terminate(process.pid).and_then(|()| {
            process
                .identity()
                .map(|identity| PendingEndTask {
                    identity,
                    name: process.name.clone(),
                })
                .ok_or_else(|| {
                    "Process identity is unavailable or still being sampled. No action was taken."
                        .into()
                })
        });
        match result {
            Ok(target) => self.pending_end_task = Some(target),
            Err(error) => self.message = Some((error, true)),
        }
    }

    fn keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let pages = [
            (egui::Key::Num0, Page::Overview),
            (egui::Key::Num8, Page::Sensors),
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
        if !matches!(self.page, Page::Processes | Page::Details)
            || !self.inspector_visible
            || self.selected_process().is_none()
        {
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
                widgets::hover_label(ui, RichText::new("INSPECTOR").size(10.0).strong().color(t.text_muted));
                ui.add_space(10.0);
                egui::ScrollArea::vertical().id_salt("inspector_scroll").auto_shrink([false, false]).show(ui, |ui| {
                let selected = self.selected_process().cloned();
                if let Some(process) = selected {
                    ui.horizontal(|ui| {
                        self.process_icons.paint(ui, process.executable.as_deref(), 32.0, t.text_muted, egui::Sense::hover()).on_hover_text("Executable icon; not a verified publisher identity");
                        widgets::status_pill(ui, &process.status, if process.status == "Running" { t.good } else { t.text_muted });
                    });
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(RichText::new(&process.name).size(19.0).strong().color(t.text)).truncate())
                        .on_hover_text(&process.name);
                    ui.add_space(10.0);
                    widgets::detail_row(ui, "PID", &process.pid.to_string(), t);
                    widgets::detail_row(ui, "Account", &process.user, t);
                    widgets::detail_row(ui, "Status", &process.status, t);
                    widgets::detail_row(ui, "CPU", &format::percent(process.cpu_percent), t);
                    widgets::detail_row(ui, "GPU", &process.gpu_percent.label(), t);
                    widgets::hover_label(ui, RichText::new(process.gpu_percent.status()).size(10.0).color(t.text_muted))
                        .on_hover_text(process.gpu_percent.explanation());
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
                        let controls_enabled = !self.process_actions.busy() && process.control.accessible && process.identity().is_some()
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
                    widgets::hover_label(ui,
                        RichText::new("Changes require confirmation and use your current Windows permissions.")
                            .size(9.0)
                            .color(t.text_muted),
                    );

                    if let Some(path) = &process.executable {
                        ui.add_space(9.0);
                        widgets::hover_label(ui, RichText::new("EXECUTABLE").size(9.0).strong().color(t.text_muted));
                        ui.add(
                            egui::Label::new(
                                RichText::new(path.display().to_string()).size(10.0).monospace(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                        if ui.add_enabled(self.process_actions.ready(), egui::Button::new("Reveal in Explorer").small())
                            .on_disabled_hover_text("Wait for the pending action, or check that the action worker is available.")
                            .clicked() {
                            self.submit_process_action(ui.ctx(), ProcessAction::Reveal(path.clone()), path.display().to_string());
                        }
                    }
                    if !process.command.is_empty() {
                        ui.add_space(8.0);
                        widgets::hover_label(ui, RichText::new("COMMAND LINE").size(9.0).strong().color(t.text_muted));
                        ui.add(
                            egui::Label::new(RichText::new(&process.command).size(10.0).monospace())
                                .selectable(true)
                                .wrap(),
                        );
                    }
                    ui.add_space(12.0);
                    ui.scope(|ui| {
                        if widgets::action_button_enabled(ui, RichText::new("End task").color(Color32::WHITE), Vec2::new(ui.available_width(), 34.0), t.danger, t, !self.process_actions.busy())
                            .on_disabled_hover_text("Wait for the pending process action to finish.")
                            .clicked()
                        {
                            self.request_end_selected();
                        }
                    });
                } else {
                    ui.add_space(8.0);
                    let frame = egui::Frame::new()
                        .fill(theme::raised_color(self.theme))
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(self.theme.roundness)
                        .inner_margin(egui::Margin::same(16));
                    widgets::hover_frame(ui, frame, |ui| {
                            ui.set_width(ui.available_width());
                            ui.vertical_centered(|ui| {
                                widgets::tront_mark(ui, t.accent, t.secondary, 44.0);
                                ui.add_space(8.0);
                                widgets::hover_label(ui,
                                    RichText::new("Select a process")
                                        .size(17.0)
                                        .strong()
                                        .color(t.text),
                                );
                                widgets::hover_label(ui,
                                    RichText::new(
                                        "Lock the inspector to a PID for paths, live counters, priority, affinity, and guarded actions.",
                                    )
                                    .size(11.0)
                                    .color(t.text_muted),
                                );
                            });
                        });
                    ui.add_space(10.0);
                    widgets::hover_label(ui,
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
            widgets::hover_label(
                ui,
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
                widgets::hover_label(
                    ui,
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
        widgets::hover_label(ui, RichText::new(footer).size(10.0).color(t.text_muted));
    }

    fn page_header(&mut self, ui: &mut egui::Ui, title: &str, subtitle: &str, searchable: bool) {
        let t = self.colors();
        ui.horizontal(|ui| {
            ui.heading(RichText::new(title).size(24.0).strong().color(t.text));
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
        widgets::hover_label(ui, RichText::new(subtitle).size(11.0).color(t.text_muted));
    }

    fn telemetry_strip(&self, ui: &mut egui::Ui) {
        let t = self.colors();
        let gpu_value = self.snapshot.gpu.reading().label();
        let cpu_value = format::percent(self.snapshot.cpu_percent);
        let memory_value = format::percent(memory_percent(&self.snapshot));
        let memory_detail = format!(
            "{} / {}",
            format::bytes(self.snapshot.memory_used_bytes),
            format::bytes(self.snapshot.memory_total_bytes)
        );
        let uptime_value = format::duration(self.snapshot.uptime_seconds);
        let uptime_detail = format!("{} live entries", self.snapshot.process_count);
        let cards = [
            (
                "CPU",
                cpu_value.as_str(),
                if self.snapshot.cpu.brand.is_empty() {
                    "total machine load"
                } else {
                    &self.snapshot.cpu.brand
                },
                t.accent,
            ),
            (
                "MEMORY",
                memory_value.as_str(),
                memory_detail.as_str(),
                t.secondary,
            ),
            (
                "GPU",
                gpu_value.as_str(),
                if self.snapshot.gpu.available {
                    "Busiest Windows GPU engine"
                } else {
                    self.snapshot
                        .gpu
                        .error
                        .as_deref()
                        .unwrap_or("collecting exact counters")
                },
                theme::mix(t.accent, t.secondary, 0.5),
            ),
            (
                "UPTIME",
                uptime_value.as_str(),
                uptime_detail.as_str(),
                t.good,
            ),
        ];
        // Keep complete metric values above the table, even with a wide inspector.
        // Breakpoints use logical egui points, not physical screen pixels.
        let count = if ui.available_width() >= 736.0 { 4 } else { 2 };
        for row in cards.chunks(count) {
            ui.columns(count, |columns| {
                for (ui, &(label, value, detail, color)) in columns.iter_mut().zip(row) {
                    widgets::stat_card(ui, label, value, detail, color, self.theme, t);
                }
            });
        }
    }

    fn process_table(&mut self, ui: &mut egui::Ui, detailed: bool) {
        let hidden_sort = if detailed {
            self.sort_column == SortColumn::WriteRate
        } else {
            matches!(
                self.sort_column,
                SortColumn::User | SortColumn::Status | SortColumn::CpuTime
            )
        };
        if hidden_sort {
            // Page switches must not leave an invisible sort key with no marked
            // header. Keep shared visible sorts; otherwise return to CPU descending.
            self.sort_column = SortColumn::Cpu;
            self.sort_direction = SortDirection::Descending;
            self.rebuild_visible_processes();
        }
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
        let visible_count = if tree_mode {
            self.visible_process_tree.len()
        } else {
            self.visible_processes.len()
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
                body.rows(32.0, visible_count, |mut row| {
                    let display = if tree_mode {
                        self.visible_process_tree[row.index()]
                    } else {
                        let process_index = self.visible_processes[row.index()];
                        ProcessTreeRow {
                            process_index,
                            totals: ProcessTotals::from_process(
                                &self.snapshot.processes[process_index],
                            ),
                            depth: 0,
                            has_children: false,
                            descendant_count: 0,
                            expanded: false,
                        }
                    };
                    let process = &self.snapshot.processes[display.process_index];
                    row.set_selected(selected_pid == Some(process.pid));
                    widgets::table_column(&mut row, t, |ui| {
                        ui.scope(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.spacing_mut().button_padding = Vec2::ZERO;
                            // Keep room for the controls, icon and process name at
                            // any hierarchy depth or user-resized column width.
                            let desired_indent = display.depth as f32 * 13.0;
                            let indent =
                                desired_indent.min((ui.available_width() - 160.0).clamp(0.0, 78.0));
                            ui.add_space(indent);
                            if tree_mode && display.has_children {
                                let label = if display.expanded {
                                    "Collapse process subtree"
                                } else {
                                    "Expand process subtree"
                                };
                                let response = widgets::icon_button(
                                    ui,
                                    if display.expanded {
                                        Icon::Collapse
                                    } else {
                                        Icon::Expand
                                    },
                                    "",
                                    Vec2::splat(20.0),
                                    t.accent_dim,
                                    t,
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        ui.is_enabled(),
                                        label,
                                    )
                                });
                                if response.on_hover_text(label).clicked() {
                                    toggled_pid = Some(process.pid);
                                }
                            } else if tree_mode {
                                ui.add_space(21.0);
                            }
                            if self
                                .process_icons
                                .paint(
                                    ui,
                                    process.executable.as_deref(),
                                    18.0,
                                    t.text_muted,
                                    egui::Sense::click(),
                                )
                                .clicked()
                            {
                                clicked_pid = Some(process.pid);
                            }
                            let name = if tree_mode && display.has_children {
                                format!("{}  [{}]", process.name, display.descendant_count + 1)
                            } else {
                                process.name.clone()
                            };
                            let response = widgets::table_label(
                                ui,
                                RichText::new(name).color(t.text).strong(),
                            )
                            .on_hover_ui(|ui| {
                                ui.label(&process.name);
                                if tree_mode {
                                    ui.label(format!(
                                        "Hierarchy depth: {}{}",
                                        display.depth,
                                        if indent < desired_indent {
                                            " (indent compressed)"
                                        } else {
                                            ""
                                        }
                                    ));
                                    if let Some(parent) = process.parent_pid {
                                        ui.label(format!("Reported parent PID: {parent}"));
                                    }
                                }
                            });
                            if response.clicked() {
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
                        if widgets::gpu_cell(
                            ui,
                            display.totals.gpu_percent,
                            display.totals.process_count > 1,
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
            if let Some(process) = self.snapshot.processes.iter().find(|row| row.pid == pid) {
                self.tree_expansion.toggle(process);
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
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("performance_rail")
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.performance_rail(ui));
                },
            );
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width().max(320.0), available.y),
                Layout::top_down(Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("performance_content")
                        .auto_shrink([false, false])
                        .show(ui, |ui| match self.performance_device {
                            PerformanceDevice::Cpu => self.cpu_performance(ui),
                            PerformanceDevice::Memory => self.memory_performance(ui),
                            PerformanceDevice::Disk(index) => self.disk_performance(ui, index),
                            PerformanceDevice::Network(index) => {
                                self.network_performance(ui, index)
                            }
                            PerformanceDevice::Gpu => self.gpu_performance(ui),
                            PerformanceDevice::GpuSensors => self.gpu_sensor_performance(ui),
                            PerformanceDevice::PhysicalDisks => self.physical_disks_performance(ui),
                        });
                },
            );
        });
    }

    fn performance_rail(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let gpu_value = self.snapshot.gpu.reading().label();
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
        // Keep hardware temperatures near the top instead of below long disk lists.
        let hottest = self
            .snapshot
            .gpu_sensors
            .adapters
            .iter()
            .filter_map(|a| a.temperature_c)
            .max();
        if widgets::device_button(
            ui,
            self.performance_device == PerformanceDevice::GpuSensors,
            "GPU SENSORS",
            &hottest.map_or_else(|| "Unavailable".into(), |v| format!("{v} °C")),
            &VecDeque::new(),
            t.secondary,
            t,
        ) {
            self.performance_device = PerformanceDevice::GpuSensors;
        }
        if widgets::device_button(
            ui,
            self.performance_device == PerformanceDevice::PhysicalDisks,
            "PHYSICAL DISKS",
            self.snapshot
                .physical_disks
                .state(std::time::Instant::now())
                .label(),
            &VecDeque::new(),
            t.good,
            t,
        ) {
            self.performance_device = PerformanceDevice::PhysicalDisks;
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
                &format!("VOLUME {}", disk.mount),
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
            widgets::hover_label(ui, "Disk no longer present");
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
            &format!("VOLUME {}", disk.mount),
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
            widgets::hover_label(ui, "Network adapter no longer present");
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
            "Busiest engine; >= means partial coverage",
            &self.snapshot.gpu.reading().label(),
            color,
            t,
        );
        widgets::history_graph(ui, &self.gpu_history, color, 250.0, Some(100.0), t);
        ui.add_space(10.0);
        for engine in self.gpu_engine_names.iter().take(8) {
            let usage = self
                .snapshot
                .gpu
                .engine_utilization
                .iter()
                .find(|(name, _)| name == engine)
                .and_then(|(_, value)| value.exact())
                .filter(|_| self.snapshot.gpu.error.is_none());
            widgets::engine_meter(ui, engine, usage, color, t);
        }
        self.provider_notice(ui, crate::diagnostics::Provider::GpuActivity);
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
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                widgets::section_label(ui, "HEAVIEST LIFETIME CPU CONSUMERS", t);
                for (rank, &index) in self.history_processes.iter().enumerate() {
                    let process = &self.snapshot.processes[index];
                    widgets::hover_frame(ui, widgets::surface(ui, t, rank % 2 == 1), |ui| {
                        ui.horizontal(|ui| {
                            widgets::hover_label(
                                ui,
                                RichText::new(format!("{:02}", rank + 1))
                                    .monospace()
                                    .color(t.ink(t.accent)),
                            );
                            widgets::hover_label(
                                ui,
                                RichText::new(&process.name).strong().color(t.text),
                            );
                            widgets::hover_label(
                                ui,
                                RichText::new(format!("PID {}", process.pid))
                                    .monospace()
                                    .size(10.0)
                                    .color(t.text_muted),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                widgets::hover_label(
                                    ui,
                                    RichText::new(format!(
                                        "{} I/O",
                                        format::bytes(
                                            process.total_read_bytes + process.total_write_bytes
                                        )
                                    ))
                                    .monospace()
                                    .color(t.text_muted),
                                );
                                widgets::hover_label(
                                    ui,
                                    RichText::new(format::millis(process.accumulated_cpu_millis))
                                        .monospace()
                                        .color(t.ink(t.secondary)),
                                );
                            });
                        });
                    });
                    ui.add_space(4.0);
                }
            });
    }

    fn startup_page(&mut self, ui: &mut egui::Ui) {
        self.inventory_header(
            ui,
            "Startup",
            "Read-only Run keys and Startup folders, with independent source freshness",
            "Search startup inventory",
        );
        self.startup_inventory(ui);
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
                        ] {
                            widgets::table_column(&mut row, t, |ui| {
                                widgets::heat_cell(ui, value, label, color, t);
                            });
                        }
                        widgets::table_column(&mut row, t, |ui| {
                            widgets::gpu_cell(ui, user.gpu_percent, true, t);
                        });
                        for (label, color) in [
                            (format::bytes(user.memory_bytes), t.accent),
                            (format::rate(user.disk_bytes_per_sec), t.secondary),
                        ] {
                            widgets::table_column(&mut row, t, |ui| {
                                widgets::heat_cell(ui, 0.0, label, color, t);
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
        widgets::hover_label(
            ui,
            RichText::new(
                "Drag dividers to resize columns. Scroll horizontally for more counters.",
            )
            .size(10.0)
            .color(self.colors().text_muted),
        );
    }

    fn services_page(&mut self, ui: &mut egui::Ui) {
        self.inventory_header(
            ui,
            "Services",
            "Windows services with confirmed, background Start / Stop / Restart",
            "Search services",
        );
        self.service_inventory(ui);
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
        widgets::hover_label(ui, RichText::new(subtitle).size(11.0).color(t.text_muted));
    }

    fn confirm_end_task(&mut self, ctx: &egui::Context) {
        let Some(target) = self.pending_end_task.clone() else {
            return;
        };
        let t = self.colors();
        let pid = target.identity.pid;
        let name = target.name;
        let still_listed = self
            .snapshot
            .processes
            .iter()
            .any(|row| row.identity() == Some(target.identity));
        egui::Window::new("Confirm end task")
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(390.0);
                widgets::hover_label(ui, RichText::new(format!("End {name}?")).size(18.0).strong().color(t.text));
                widgets::hover_label(ui, RichText::new(format!("PID {pid} will be terminated immediately. Unsaved data in that process will be lost.")).color(t.text_muted));
                widgets::hover_label(ui, RichText::new(if still_listed {
                    "The native process creation time is rechecked when you confirm."
                } else { "The original process exited or changed. Cancel and select again." }).size(11.0).color(t.text));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.pending_end_task = None;
                    }
                    if widgets::action_button_enabled(ui, RichText::new("End process").color(Color32::WHITE), Vec2::ZERO, t.danger, t, still_listed && self.process_actions.ready())
                        .on_disabled_hover_text("The process must still match and the action worker must be ready.").clicked()
                        && self.submit_process_action(ctx, ProcessAction::End(target.identity), format!("{name} ({pid})")) {
                            self.pending_end_task = None;
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
                widgets::hover_label(ui,
                    RichText::new(format!("{} | PID {}", process.name, process.pid))
                        .size(17.0)
                        .strong()
                        .color(t.text),
                );
                widgets::hover_label(ui,
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
                    let response = widgets::action_button(ui, RichText::new(text).color(t.text), Vec2::new(ui.available_width(), 31.0),
                        if current { t.accent_dim } else { t.panel_raised }, t);
                    if response.clicked() && !current {
                        requested = Some(priority);
                    }
                }
                ui.add_space(8.0);
                widgets::hover_label(ui,
                    RichText::new("High priority can reduce responsiveness elsewhere on the machine and receives an additional explicit confirmation.")
                        .size(10.0)
                        .color(t.ink(t.danger)),
                );
            });
        if let Some(priority) = requested {
            let Some(identity) = process.identity() else {
                self.message = Some((
                    "Process identity is unavailable. No action was taken.".into(),
                    true,
                ));
                self.show_priority_editor = false;
                return;
            };
            self.pending_control_action =
                Some(PendingControlAction::Priority { identity, priority });
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
                widgets::hover_label(
                    ui,
                    RichText::new(format!("{} | PID {}", process.name, process.pid))
                        .size(17.0)
                        .strong()
                        .color(t.text),
                );
                widgets::hover_label(
                    ui,
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
                        widgets::hover_label(
                            ui,
                            RichText::new(format!("{} selected", self.affinity_draft.count_ones()))
                                .monospace()
                                .color(t.ink(t.secondary)),
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
                        widgets::hover_label(
                            ui,
                            RichText::new("Select at least one processor.")
                                .size(10.0)
                                .color(t.ink(t.danger)),
                        );
                    }
                });
            });
        if apply {
            let Some(identity) = process.identity() else {
                self.message = Some((
                    "Process identity is unavailable. No action was taken.".into(),
                    true,
                ));
                self.show_affinity_editor = false;
                return;
            };
            self.pending_control_action = Some(PendingControlAction::Affinity {
                identity,
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
        let identity = match action {
            PendingControlAction::Priority { identity, .. }
            | PendingControlAction::Affinity { identity, .. } => identity,
        };
        let pid = identity.pid;
        let process = self
            .snapshot
            .processes
            .iter()
            .find(|process| process.identity() == Some(identity))
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
                widgets::hover_label(ui, RichText::new(title).size(18.0).strong().color(t.text));
                widgets::hover_label(ui, RichText::new(description).color(t.text_muted));
                widgets::hover_label(
                    ui,
                    RichText::new(format!(
                        "PID {pid} | native creation time rechecked on confirmation"
                    ))
                    .size(10.0)
                    .monospace()
                    .color(t.ink(t.secondary)),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.pending_control_action = None;
                    }
                    let fill = if dangerous { t.danger } else { t.accent };
                    if widgets::action_button_enabled(
                        ui,
                        RichText::new(button).color(Color32::WHITE),
                        Vec2::ZERO,
                        fill,
                        t,
                        process.is_some() && self.process_actions.ready(),
                    )
                    .on_disabled_hover_text(
                        "The process must still match and the action worker must be ready.",
                    )
                    .clicked()
                    {
                        let action = match action {
                            PendingControlAction::Priority { priority, .. } => {
                                ProcessAction::Priority(identity, priority)
                            }
                            PendingControlAction::Affinity { affinity_mask, .. } => {
                                ProcessAction::Affinity(identity, affinity_mask)
                            }
                        };
                        if self.submit_process_action(ctx, action, format!("{name} ({pid})")) {
                            self.pending_control_action = None;
                        }
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
                widgets::hover_label(
                    ui,
                    RichText::new("Launch a program or command")
                        .size(17.0)
                        .strong()
                        .color(t.text),
                );
                widgets::hover_label(
                    ui,
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
                    let run = ui
                        .add_enabled_ui(
                            !self.run_command.trim().is_empty() && self.process_actions.ready(),
                            |ui| widgets::action_button(ui, "Run", Vec2::ZERO, t.accent_dim, t),
                        )
                        .inner
                        .on_disabled_hover_text("Enter a command and wait for the pending action to finish. An unavailable worker cannot send commands.");
                    if self.process_actions.ready()
                        && (run.clicked()
                            || (response.lost_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter))))
                    {
                        let command = self.run_command.trim().to_owned();
                        if self
                            .submit_process_action(ctx, ProcessAction::Launch(command.clone()), command)
                        {
                            self.run_command.clear();
                            self.show_run_task = false;
                        }
                    }
                });
            });
        self.show_run_task &= open;
    }

    fn theme_editor(&mut self, ctx: &egui::Context) {
        self.theme_studio
            .show(ctx, &mut self.theme, &mut self.show_theme_editor);
    }

    fn submit_process_action(
        &mut self,
        ctx: &egui::Context,
        action: ProcessAction,
        target: String,
    ) -> bool {
        // Defense in depth: local callbacks/tests must not bypass the recovery UI.
        if self
            .graphics_recovering
            .load(std::sync::atomic::Ordering::Acquire)
        {
            self.message = Some((
                "Graphics are reconnecting. No action was sent.".into(),
                true,
            ));
            return false;
        }
        match self
            .process_actions
            .submit(ProcessRequest::new(action, target))
        {
            Ok(()) => {
                self.message = None;
                ctx.request_repaint(); // Paint pending state without waiting for the native result.
                true
            }
            Err(error) => {
                self.message = Some((error, true));
                false
            }
        }
    }

    fn message_bar(&mut self, root: &mut egui::Ui) {
        let busy = self.process_actions.busy();
        let pending = self.process_actions.active().map(|request| {
            let elapsed = request.confirmed_at.elapsed();
            if elapsed < crate::process_actions::SLOW_AFTER {
                root.ctx().request_repaint_after(crate::process_actions::SLOW_AFTER - elapsed);
                (format!("Working: {}", request.description()), false)
            } else {
                (format!("Still waiting for Windows: {}. Outcome pending; no duplicate request sent.", request.description()), false)
            }
        });
        let Some((message, is_error)) = pending.or_else(|| self.message.clone()) else {
            return;
        };
        let t = self.colors();
        egui::Panel::bottom("message_bar")
            .exact_size(34.0)
            .frame(
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(10, 0))
                    .fill(if is_error {
                        theme::mix(t.panel, t.danger, 0.35)
                    } else if busy {
                        theme::mix(t.panel, t.secondary, 0.12)
                    } else {
                        theme::mix(t.panel, t.good, 0.25)
                    }),
            )
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !busy && ui.small_button("Dismiss").clicked() {
                            self.message = None;
                        }
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                            widgets::hover_label(ui, RichText::new(&message).color(t.text))
                                .on_hover_text(message);
                        });
                    });
                });
            });
    }
}

impl eframe::App for TrontopApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(snapshot) = self
            .sampler
            .as_ref()
            .and_then(|sampler| sampler.latest_after(self.seen_generation))
        {
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
        if let Some(outcome) = self.process_actions.poll() {
            self.message = Some(outcome.message());
        }
        if self
            .graphics_recovering
            .load(std::sync::atomic::Ordering::Acquire)
        {
            // Keep native window controls available, but never execute process or
            // service actions against an old frozen picture while GPU work recovers.
            // App state and telemetry remain owned by this same app/context.
            self.custom_chrome(ui);
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading("Reconnecting graphics");
                ui.label("Your view and theme are retained. Process controls resume when drawing recovers.");
            });
            return;
        }
        self.process_icons.begin_frame(&ctx);
        self.poll_service_command();
        if let Some(result) = self.exporter.poll() {
            self.export_result = Some(result);
        }
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
                Page::Overview => self.overview_page(ui),
                Page::Sensors => self.sensors_page(ui),
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
        self.confirm_service_command(&ctx);
        self.run_task_window(&ctx);
        self.theme_editor(&ctx);
        self.diagnostics_window(&ctx);
        self.export_window(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(theme::STORAGE_KEY, self.theme.encode());
        storage.set_string(
            crate::theme_studio::LIBRARY_KEY,
            self.theme_studio.encode_library(),
        );
    }
}

fn chrome_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    t: Tokens,
    danger: bool,
) -> egui::Response {
    let mut colors = t;
    if danger {
        colors.text = t.danger;
    }
    let response =
        widgets::icon_button(ui, icon, "", Vec2::new(34.0, 27.0), t.panel_raised, colors);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response.on_hover_text(label)
}

fn memory_percent(snapshot: &SystemSnapshot) -> f32 {
    if snapshot.memory_total_bytes == 0 {
        0.0
    } else {
        snapshot.memory_used_bytes as f32 / snapshot.memory_total_bytes as f32 * 100.0
    }
}
