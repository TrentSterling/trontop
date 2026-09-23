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

const HISTORY_LENGTH: usize = widgets::HISTORY_WINDOW;
/// Sidebar footer: three 20 px meters, host line and a 28 px Theme Studio button.
const NAV_FOOTER_HEIGHT: f32 = 122.0;

mod diagnostics;
mod disks;
mod export;
mod gpus;
mod graphs;
mod inventory;
mod overview;
mod preferences;
mod sensors;
mod service_controls;
mod storage;
mod system;
mod tree_state;

#[cfg(test)]
mod ui_smoke;

#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(test, derive(Debug))]
enum Page {
    Overview,
    Graphs,
    Sensors,
    Processes,
    Performance,
    History,
    Startup,
    Users,
    Details,
    Services,
    System,
}

impl Page {
    fn icon(self) -> Icon {
        match self {
            Self::Overview => Icon::Overview,
            Self::Graphs => Icon::Graphs,
            Self::Sensors => Icon::Sensors,
            Self::Processes => Icon::Processes,
            Self::Performance => Icon::Performance,
            Self::History => Icon::History,
            Self::Startup => Icon::Startup,
            Self::Users => Icon::Users,
            Self::Details => Icon::Details,
            Self::Services => Icon::Services,
            Self::System => Icon::System,
        }
    }

    const ALL: [(Self, &'static str, &'static str); 11] = [
        (Self::Overview, "00", "Overview"),
        (Self::Graphs, "09", "Graphs"),
        (Self::Processes, "01", "Processes"),
        (Self::Performance, "02", "Performance"),
        (Self::History, "03", "History"),
        (Self::Startup, "04", "Startup"),
        (Self::Users, "05", "Users"),
        (Self::Details, "06", "Details"),
        (Self::Services, "07", "Services"),
        (Self::Sensors, "08", "Hardware sensors"),
        (Self::System, "10", "System"),
    ];

    /// Sidebar groups, top to bottom. Every page appears exactly once.
    const NAV_GROUPS: [(&'static str, &'static [Self]); 3] = [
        (
            "Monitor",
            &[
                Self::Overview,
                Self::Graphs,
                Self::Performance,
                Self::Sensors,
            ],
        ),
        (
            "Processes",
            &[Self::Processes, Self::Details, Self::Users, Self::History],
        ),
        ("System", &[Self::Startup, Self::Services, Self::System]),
    ];

    /// The page name shown in the navigation and the command bar.
    fn title(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(page, _, _)| *page == self)
            .map_or("", |&(_, _, name)| name)
    }

    /// One plain-English sentence under the command bar, or nothing.
    fn intro(self) -> &'static str {
        match self {
            Self::Overview => "",
            Self::Graphs => "Two minutes of every signal. Hover any graph for exact readings.",
            Self::Processes => "Everything running now. Parent rows include their children.",
            Self::Performance => "Pick a device on the left for its detail view.",
            Self::History => "Which processes used the most CPU time and disk since they started.",
            Self::Startup => "Programs Windows starts when you sign in. Read-only.",
            Self::Users => "Resource use per Windows account.",
            Self::Details => "Every process with all counters. Drag column edges to resize.",
            Self::Services => "Windows services. Start, stop and restart ask first.",
            Self::Sensors => "Temperatures, power, clocks and fans.",
            Self::System => "What is inside this PC.",
        }
    }

    /// Pages whose search box lives in the command bar, with its hint.
    fn search_hint(self) -> Option<&'static str> {
        match self {
            Self::Processes | Self::Details | Self::History => Some("Search processes"),
            Self::Startup => Some("Search startup"),
            Self::Services => Some("Search services"),
            _ => None,
        }
    }
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
    preferences: crate::preferences::Controller,
    preferences_theme: ThemeSettings,
    preferences_library_revision: u64,
    next_memory_save: std::time::Instant,
    closing_at: Option<std::time::Instant>,
    close_authorized: bool,
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
    /// Overview Disk tile: sum of live physical-disk read + write bytes/s.
    /// NaN marks a sample with no live disk reading, so the gap stays visible.
    overview_disk_total: VecDeque<f32>,
    /// Overview Network tile: receive + send bytes/s over deduplicated adapters.
    overview_net_total: VecDeque<f32>,
    gpu_engine_names: Vec<String>,
    sensor_history: HashMap<String, SensorHistory>,
    disk_history: HashMap<String, VecDeque<f32>>,
    physical_disk_history: disks::Histories,
    graphs: graphs::Dashboard,
    selected_physical_disk: Option<String>,
    /// The Performance selection the rail last scrolled into view.
    rail_followed: Option<(PerformanceDevice, Option<String>)>,
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
    /// Started lazily on the first System or Sensors page view; never in
    /// headless tests.
    specs: Option<crate::specs::Monitor>,
    specs_enabled: bool,
    specs_view: crate::specs::Snapshot,
    system_section: crate::specs::SectionId,
    reveal_private: bool,
    /// Opt-in per save; resets after every System specs save.
    specs_export_private: bool,
    /// The running export job was started from the System page.
    specs_export_pending: bool,
    /// The page and window height the navigation last scrolled into view.
    nav_revealed: Option<(Page, u32)>,
}

impl TrontopApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        graphics_recovering: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        let preferences = crate::preferences::Controller::native(
            crate::trontop_state_directory(),
            cc.egui_ctx.clone(),
        );
        let saved_theme = ThemeSettings::default();
        theme::install(&cc.egui_ctx, saved_theme);
        let tray = TrayController::new(cc.egui_ctx.clone());
        let sampler = Sampler::spawn(cc.egui_ctx.clone(), tray.as_ref().map(TrayController::sink));
        let mut app = Self::with_services(saved_theme, Some(sampler), tray);
        app.preferences = preferences;
        app.graphics_recovering = graphics_recovering;
        app.process_icons = crate::process_icons::Cache::spawn(cc.egui_ctx.clone());
        app.service_controller = crate::service_control::Controller::spawn(cc.egui_ctx.clone());
        app.process_actions = crate::process_actions::Controller::spawn(cc.egui_ctx.clone());
        app.exporter = crate::export::Exporter::native();
        app.specs_enabled = true;
        app
    }

    fn with_services(
        saved_theme: ThemeSettings,
        sampler: Option<Sampler>,
        tray: Option<TrayController>,
    ) -> Self {
        Self {
            preferences: Default::default(),
            preferences_theme: saved_theme,
            preferences_library_revision: 0,
            next_memory_save: std::time::Instant::now() + std::time::Duration::from_secs(30),
            closing_at: None,
            close_authorized: false,
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
            overview_disk_total: VecDeque::with_capacity(HISTORY_LENGTH),
            overview_net_total: VecDeque::with_capacity(HISTORY_LENGTH),
            gpu_engine_names: vec![
                "3D".into(),
                "Copy".into(),
                "VideoEncode".into(),
                "VideoDecode".into(),
            ],
            sensor_history: HashMap::new(),
            disk_history: HashMap::new(),
            physical_disk_history: disks::Histories::default(),
            graphs: graphs::Dashboard::default(),
            selected_physical_disk: None,
            rail_followed: None,
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
            specs: None,
            specs_enabled: false,
            specs_view: crate::specs::Snapshot::default(),
            system_section: crate::specs::SectionId::Summary,
            reveal_private: false,
            specs_export_private: false,
            specs_export_pending: false,
            nav_revealed: None,
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
        self.graphs.sample(&snapshot, std::time::Instant::now());
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
            // This plain history cannot mark lower bounds, so partial samples
            // stay gaps here; the Graphs page records them as marked points.
            snapshot.gpu.reading().exact().unwrap_or(f32::NAN),
            HISTORY_LENGTH,
        );
        widgets::push_history(
            &mut self.overview_disk_total,
            overview::disk_throughput(&snapshot.physical_disks, std::time::Instant::now())
                .map_or(f32::NAN, |(total, _)| total as f32),
            HISTORY_LENGTH,
        );
        widgets::push_history(
            &mut self.overview_net_total,
            overview::network_throughput(&snapshot)
                .map_or(f32::NAN, |(receive, send)| (receive + send) as f32),
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
                    ui.add_space(theme::space::XS);
                    // Live is the normal state and needs no badge. Anything else
                    // (Starting, Partial, Stale, Unavailable) earns a pill.
                    let state = self
                        .snapshot
                        .diagnostics
                        .get(crate::diagnostics::Provider::System)
                        .state(
                            crate::diagnostics::Provider::System,
                            std::time::Instant::now(),
                        );
                    if state != crate::diagnostics::State::Live {
                        widgets::status_pill(
                            ui,
                            &state.label().to_uppercase(),
                            diagnostics::state_color(state, t),
                        );
                    }
                    // An explicit id: the status pill before this group comes and
                    // goes with the System state, and an auto id would shift the
                    // window buttons' ids with it (egui's red debug outlines).
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .id(egui::Id::new("chrome_window_buttons"))
                            .layout(Layout::right_to_left(Align::Center)),
                        |ui| {
                            if chrome_button(ui, Icon::Close, "Close", t, true).clicked() {
                                self.request_close(ui.ctx());
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
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(
                                    !maximized,
                                ));
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
                        },
                    );
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
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 10,
                        top: 8,
                        bottom: 10,
                    })
                    .stroke(Stroke::new(1.0, t.border)),
            )
            .show(root, |ui| {
                let footer = egui::Panel::bottom("navigation_footer")
                    .exact_size(NAV_FOOTER_HEIGHT)
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| self.navigation_footer(ui, t))
                    .response
                    .rect;
                // Hairline between the page list and the meters, in the gap
                // above the footer (no extra height).
                ui.painter().hline(
                    footer.x_range(),
                    footer.top() - 4.0,
                    Stroke::new(1.0, t.border),
                );
                egui::ScrollArea::vertical()
                    .id_salt("navigation_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let reveal = (self.page, ui.ctx().content_rect().height() as u32);
                        for (group, (heading, pages)) in Page::NAV_GROUPS.into_iter().enumerate() {
                            if group > 0 {
                                ui.add_space(theme::space::XS);
                            }
                            // Section headers: 11 px, muted, sentence case.
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 14.0),
                                Sense::hover(),
                            );
                            ui.painter().text(
                                rect.left_bottom() + Vec2::new(4.0, -1.0),
                                egui::Align2::LEFT_BOTTOM,
                                heading,
                                FontId::proportional(11.0),
                                t.text_muted,
                            );
                            for &page in pages {
                                if widgets::nav_button(
                                    ui,
                                    self.page == page,
                                    page.icon(),
                                    page.title(),
                                    t,
                                ) {
                                    self.page = page;
                                }
                                // Very short windows cannot fit every entry above
                                // the footer: keep the active one in view, once per
                                // page or height change (no per-frame scrolling).
                                if page == reveal.0 && self.nav_revealed != Some(reveal) {
                                    ui.scroll_to_cursor_animation(
                                        None,
                                        egui::style::ScrollAnimation::none(),
                                    );
                                    self.nav_revealed = Some(reveal);
                                }
                            }
                        }
                    });
            });
    }

    /// Frameless meters, host line and Theme Studio. Fixed height so the page
    /// list above it never has to scroll at 1000x580.
    fn navigation_footer(&mut self, ui: &mut egui::Ui, t: Tokens) {
        ui.spacing_mut().item_spacing.y = 0.0;
        let has_sample = self.seen_generation > 0;
        let cpu = has_sample.then_some(self.snapshot.cpu_percent);
        widgets::mini_meter_text(
            ui,
            "CPU",
            &cpu.map_or_else(|| "--".into(), format::percent),
            cpu,
            &self.cpu_history,
            t.accent,
            t,
        )
        .on_hover_text(if has_sample {
            "Whole-machine CPU load. The line shows the last two minutes."
        } else {
            "Waiting for the first system sample."
        });
        ui.add_space(theme::space::XS);
        let memory = (self.snapshot.memory_total_bytes > 0).then(|| memory_percent(&self.snapshot));
        widgets::mini_meter_text(
            ui,
            "Memory",
            &memory.map_or_else(|| "--".into(), format::percent),
            memory,
            &self.memory_history,
            t.secondary,
            t,
        )
        .on_hover_text(if memory.is_some() {
            format!(
                "{} of {} physical memory in use.",
                format::bytes(self.snapshot.memory_used_bytes),
                format::bytes(self.snapshot.memory_total_bytes)
            )
        } else {
            "Waiting for the first system sample.".into()
        });
        ui.add_space(theme::space::XS);
        // Same reading as every other GPU surface: a partial sample shows its
        // lower bound as "3.1%+", and only a missing reading shows "--".
        let gpu = self.snapshot.gpu.reading();
        widgets::mini_meter_text(
            ui,
            "GPU",
            &gpu.value().map_or_else(|| "--".into(), |_| gpu.label()),
            gpu.value(),
            &self.gpu_history,
            theme::mix(t.accent, t.secondary, 0.5),
            t,
        )
        .on_hover_text(gpu.explanation());
        ui.add_space(theme::space::M);
        let host = if self.snapshot.host_name.is_empty() {
            "Windows PC"
        } else {
            &self.snapshot.host_name
        };
        let line = if self.snapshot.sample_seconds > 0.0 {
            format!("{host} \u{b7} {:.1} s", self.snapshot.sample_seconds)
        } else {
            host.to_owned()
        };
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 12.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add(
                    egui::Label::new(RichText::new(line).size(10.0).color(t.text_muted))
                        .truncate()
                        .selectable(false),
                )
                .on_hover_text(
                    "Computer name and sample interval. Telemetry runs in the background.",
                );
            },
        );
        ui.add_space(theme::space::S);
        if widgets::icon_button(
            ui,
            Icon::Theme,
            "Theme Studio",
            Vec2::new(ui.available_width(), 28.0),
            t.accent_dim,
            t,
        )
        .on_hover_text("Colors, presets and appearance (Ctrl+T)")
        .clicked()
        {
            self.show_theme_editor = true;
        }
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
                #[cfg(test)]
                ui.ctx().data_mut(|data| {
                    data.remove_temp::<Vec<(String, egui::Rect)>>(command_rects_id());
                });
                // Labels need about 1100 px of content width; below that the
                // buttons keep their icons and say what they do on hover.
                let compact = ui.max_rect().width() + 36.0 < 1100.0;
                ui.horizontal_centered(|ui| {
                    widgets::hover_label(
                        ui,
                        RichText::new(self.page.title())
                            .size(17.0)
                            .strong()
                            .color(t.text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if command_button(ui, Icon::Startup, "Run task", compact, t.accent_dim, t)
                            .on_hover_text("Start a program or command (Ctrl+R)")
                            .clicked()
                        {
                            self.show_run_task = true;
                        }
                        if command_button(ui, Icon::Info, "About", compact, t.panel_raised, t)
                            .on_hover_text("Version, telemetry providers and support report")
                            .clicked()
                        {
                            self.show_diagnostics = true;
                        }
                        if command_button(ui, Icon::Export, "Export", compact, t.panel_raised, t)
                            .on_hover_text("Save a snapshot as CSV or JSON")
                            .clicked()
                        {
                            if !self.exporter.busy() {
                                self.export_options = crate::export::Options::default();
                                self.export_result = None;
                            }
                            self.show_export = true;
                        }
                        let selected = self.selected_pid.is_some();
                        // Below the 1100 px label breakpoint, an unselected row
                        // leaves End task and Inspector nothing to act on, so
                        // they disappear entirely instead of sitting there
                        // disabled and icon-only. At full width they stay put,
                        // just disabled, since there is room to spare.
                        let show_process_actions = selected || !compact;
                        if matches!(self.page, Page::Processes | Page::Details)
                            && show_process_actions
                        {
                            ui.add_space(theme::space::M);
                            let can_end = selected && !self.process_actions.busy();
                            if ui
                                .add_enabled_ui(can_end, |ui| {
                                    command_button(
                                        ui,
                                        Icon::Stop,
                                        "End task",
                                        compact,
                                        t.panel_raised,
                                        t,
                                    )
                                })
                                .inner
                                .on_disabled_hover_text("Select a process to end it.")
                                .clicked()
                            {
                                self.request_end_selected();
                            }
                            let fill = if selected && self.inspector_visible {
                                t.accent_dim
                            } else {
                                t.panel_raised
                            };
                            if ui
                                .add_enabled_ui(selected, |ui| {
                                    command_button(ui, Icon::Details, "Inspector", compact, fill, t)
                                })
                                .inner
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
                        if let Some(hint) = self.page.search_hint() {
                            ui.add_space(theme::space::M);
                            let width = (ui.available_width() - theme::space::M).min(220.0);
                            if width >= 96.0 {
                                let processes = matches!(
                                    self.page,
                                    Page::Processes | Page::Details | Page::History
                                );
                                let query = if processes {
                                    &mut self.query
                                } else {
                                    &mut self.secondary_query
                                };
                                let edit = egui::TextEdit::singleline(query)
                                    .hint_text(hint)
                                    .margin(egui::Margin::symmetric(10, 6))
                                    .desired_width(width);
                                let response = ui.add(edit).on_hover_text(match self.page {
                                    Page::Startup => "Filter by name, command or source",
                                    Page::Services => "Filter by service or display name",
                                    _ => "Search name, user, PID, executable path, or command line",
                                });
                                if processes && response.changed() {
                                    self.rebuild_visible_processes();
                                }
                            }
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
            (egui::Key::Num9, Page::Graphs),
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
                    let state_label = process_state_label(&process.status);
                    ui.horizontal(|ui| {
                        self.process_icons.paint(ui, process.executable.as_deref(), 32.0, t.text_muted, egui::Sense::hover()).on_hover_text("Executable icon; not a verified publisher identity");
                        widgets::status_pill(ui, &state_label, if state_label == "Running" { t.good } else { t.text_muted });
                    });
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(RichText::new(&process.name).size(19.0).strong().color(t.text)).truncate())
                        .on_hover_text(&process.name);
                    ui.add_space(10.0);
                    widgets::detail_row(ui, "PID", &process.pid.to_string(), t);
                    widgets::detail_row(ui, "Account", &process.user, t);
                    widgets::detail_row(ui, "Status", &state_label, t);
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
        self.page_intro(ui);
        ui.add_space(theme::space::M);
        self.process_toolbar(ui);
        ui.add_space(theme::space::M);
        self.process_table(ui, false);
        let footer = if self.tree_mode {
            format!(
                "{} processes \u{b7} {} rows",
                self.snapshot.process_count,
                self.visible_process_tree.len()
            )
        } else {
            format!(
                "{} of {} shown",
                self.visible_processes.len(),
                self.snapshot.process_count
            )
        };
        widgets::hover_label(ui, RichText::new(footer).size(10.0).color(t.text_muted));
    }

    /// The command bar owns the page title and search box, so a page header is
    /// only the page's one-line intro ([`Page::intro`]), or nothing. The
    /// arguments are kept so page modules need no signature churn; the copy
    /// lives in one table so every page reads the same way.
    fn page_header(&mut self, ui: &mut egui::Ui, _title: &str, _subtitle: &str, _searchable: bool) {
        self.page_intro(ui);
    }

    fn page_intro(&self, ui: &mut egui::Ui) {
        let intro = self.page.intro();
        if intro.is_empty() {
            return;
        }
        let t = self.colors();
        // A truncated label shows its full text on hover.
        ui.add(
            egui::Label::new(RichText::new(intro).size(11.0).color(t.text_muted))
                .truncate()
                .selectable(false),
        );
    }

    /// One 30 px row: a Tree/Flat segmented toggle and the total process
    /// count. This replaces the old CPU/MEMORY/GPU/UPTIME badge row, which
    /// duplicated the sidebar meters and cost the table about 100 px of body
    /// height. Tree mode's "parent rows include children" hint lives only in
    /// the page intro now, not duplicated here too.
    fn process_toolbar(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let count_text = format!("{} processes", self.snapshot.process_count);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 30.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = theme::space::M;
                if ui.selectable_label(self.tree_mode, "Tree").clicked() {
                    self.tree_mode = true;
                }
                if ui.selectable_label(!self.tree_mode, "Flat").clicked() {
                    self.tree_mode = false;
                }
                widgets::hover_label(ui, RichText::new(count_text).size(10.0).color(t.text_muted));
            },
        );
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
        // Reserve a possible horizontal scrollbar and (on Processes) the row
        // count footer below the table; Details has none since P5 folded its
        // footer into the page intro.
        let available_height = (ui.available_height() - 40.0).max(32.0);
        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .vscroll(true)
            // Cells own keyboard activation. The full-row hit area only fills
            // mouse gaps; making it focusable adds an invisible duplicate stop.
            .sense(Sense::CLICK)
            .cell_layout(Layout::left_to_right(Align::Center))
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height)
            // NAME is the flexible remainder column, not whichever numeric
            // column happens to sit last: it grows with the window instead of
            // leaving a truncated name next to a wide, mostly-empty tail
            // column. It is not user-resizable (same as any last-column
            // remainder), so PID and the rest keep their own drag handles.
            .column(
                Column::remainder()
                    .at_least(120.0)
                    .clip(true)
                    .resizable(false),
            )
            .column(Column::initial(56.0).at_least(50.0));
        if detailed {
            table = table
                .column(Column::initial(48.0).at_least(42.0).clip(true))
                .column(Column::initial(56.0).at_least(48.0));
        }
        table = table
            .column(Column::initial(58.0).at_least(52.0))
            .column(Column::initial(58.0).at_least(52.0))
            .column(Column::initial(74.0).at_least(64.0))
            .column(Column::initial(88.0).at_least(78.0))
            // WRITE / CPU TIME: a fixed numeric column now that NAME owns the
            // flexible remainder, not a wide mostly-empty tail. CPU TIME is
            // always exactly "HH:MM:SS", which needs less room than READ's
            // and WRITE's variable-length rate strings.
            .column(
                Column::initial(if detailed { 74.0 } else { 88.0 })
                    .at_least(if detailed { 64.0 } else { 78.0 })
                    .clip(true),
            );

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
                                    // The adjacent name is the keyboard target.
                                    egui::Sense::CLICK,
                                )
                                .clicked()
                            {
                                clicked_pid = Some(process.pid);
                            }
                            let name = if tree_mode && display.has_children {
                                format!("{}  [{}]", process.name, display.descendant_count + 1)
                            } else {
                                widgets::truncate_keep_extension(
                                    ui,
                                    &process.name,
                                    ui.available_width(),
                                )
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
                            // Same display mapping as the Users page: an
                            // unreadable owner reads "Not readable", never
                            // the raw "Unknown account" model value.
                            let (display_name, unreadable) = account_display(&process.user);
                            let mut response = widgets::table_label(
                                ui,
                                RichText::new(display_name).size(11.0).color(t.text_muted),
                            );
                            if unreadable {
                                response = response.on_hover_text(
                                    "Windows did not let Trontop read the owner of this \
                                     process (usually a system service; needs admin).",
                                );
                            }
                            if response.clicked() {
                                clicked_pid = Some(process.pid);
                            }
                        });
                        widgets::table_column(&mut row, t, |ui| {
                            let state_label = process_state_label(&process.status);
                            // Running is the overwhelming common case, so it
                            // recedes in muted text; any other state is the
                            // noteworthy one and reads in full-contrast text.
                            let color = if state_label == "Running" {
                                t.text_muted
                            } else {
                                t.text
                            };
                            if widgets::table_cell(
                                ui,
                                RichText::new(state_label).size(11.0).color(color),
                            ) {
                                clicked_pid = Some(process.pid);
                            }
                        });
                    }
                    widgets::table_column(&mut row, t, |ui| {
                        if widgets::cpu_cell(ui, display.totals.cpu_percent, t) {
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
        let rail_width = if available.x < 900.0 { 180.0 } else { 210.0 };
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(rail_width, available.y),
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
        let empty = VecDeque::new();
        // Scroll the selected device into view once per selection change
        // (another page, a shortcut or a removed device can select one below
        // the fold), never every frame, so the rail still scrolls freely.
        let current = (
            self.performance_device,
            (self.performance_device == PerformanceDevice::PhysicalDisks)
                .then(|| self.selected_physical_disk.clone())
                .flatten(),
        );
        let follow = self.rail_followed.as_ref() != Some(&current);
        self.rail_followed = Some(current);
        if widgets::rail_button(
            ui,
            self.performance_device == PerformanceDevice::Cpu,
            follow,
            "CPU",
            &format::percent(self.snapshot.cpu_percent),
            &self.cpu_history,
            Some(100.0),
            t.accent,
            t,
        ) {
            self.performance_device = PerformanceDevice::Cpu;
        }
        if widgets::rail_button(
            ui,
            self.performance_device == PerformanceDevice::Memory,
            follow,
            "Memory",
            &format::percent(memory_percent(&self.snapshot)),
            &self.memory_history,
            Some(100.0),
            t.secondary,
            t,
        ) {
            self.performance_device = PerformanceDevice::Memory;
        }
        let gpu_value = self.snapshot.gpu.reading().label();
        if widgets::rail_button(
            ui,
            self.performance_device == PerformanceDevice::Gpu,
            follow,
            "GPU",
            &gpu_value,
            &self.gpu_history,
            Some(100.0),
            theme::mix(t.accent, t.secondary, 0.5),
            t,
        ) {
            self.performance_device = PerformanceDevice::Gpu;
        }
        // The hottest reporting adapter drives both the value and the
        // sparkline; most machines have exactly one NVML-visible GPU.
        let hottest = self
            .snapshot
            .gpu_sensors
            .adapters
            .iter()
            .filter(|a| a.temperature_c.is_some())
            .max_by_key(|a| a.temperature_c);
        // No inner spaces: the rail column is too narrow for "47 °C · 43 W"
        // without clipping the unit off the end.
        let thermal_value = hottest.map_or_else(
            || "Unavailable".into(),
            |a| {
                format!(
                    "{}·{}",
                    a.temperature_c
                        .map_or_else(|| "--°C".into(), |v| format!("{v}°C")),
                    a.power_w
                        .map_or_else(|| "--W".into(), |v| format!("{v:.0}W")),
                )
            },
        );
        let thermal_history = hottest
            .and_then(|a| a.uuid.as_deref())
            .and_then(|uuid| self.sensor_history.get(uuid))
            .map_or_else(VecDeque::new, |history| {
                history
                    .points
                    .iter()
                    .map(|p| p.temperature_c.unwrap_or(f32::NAN))
                    .collect()
            });
        if widgets::rail_button(
            ui,
            self.performance_device == PerformanceDevice::GpuSensors,
            follow,
            "GPU thermals",
            &thermal_value,
            &thermal_history,
            None,
            t.secondary,
            t,
        ) {
            self.performance_device = PerformanceDevice::GpuSensors;
        }
        for disk in &self.snapshot.physical_disks.devices {
            let value = crate::disk_activity::Metric::Active
                .format(disk.readings[crate::disk_activity::Metric::Active as usize].value);
            let history = self
                .physical_disk_history
                .values
                .get(&disk.instance)
                .map_or_else(VecDeque::new, |h| {
                    h[crate::disk_activity::Metric::Active as usize].clone()
                });
            let selected = self.performance_device == PerformanceDevice::PhysicalDisks
                && self.selected_physical_disk.as_deref() == Some(disk.instance.as_str());
            if widgets::rail_button(
                ui,
                selected,
                follow,
                &format!("Disk {}", disk.number),
                &value,
                &history,
                Some(100.0),
                t.good,
                t,
            ) {
                self.performance_device = PerformanceDevice::PhysicalDisks;
                self.selected_physical_disk = Some(disk.instance.clone());
            }
        }
        for (index, disk) in self.snapshot.disks.iter().enumerate() {
            let history = self.disk_history.get(&disk.mount).unwrap_or(&empty);
            if widgets::rail_button(
                ui,
                self.performance_device == PerformanceDevice::Disk(index),
                follow,
                &format!("Volume {}", disk.mount),
                &format::rate(disk.read_bytes_per_sec + disk.write_bytes_per_sec),
                history,
                None,
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
            let history = self.network_history.get(&network.name).unwrap_or(&empty);
            if widgets::rail_button(
                ui,
                self.performance_device == PerformanceDevice::Network(index),
                follow,
                &network.name,
                &format::rate(network.received_bytes_per_sec + network.transmitted_bytes_per_sec),
                history,
                None,
                t.secondary,
                t,
            ) {
                self.performance_device = PerformanceDevice::Network(index);
            }
        }
    }

    fn cpu_performance(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let clock_health = self
            .snapshot
            .diagnostics
            .get(crate::diagnostics::Provider::CpuClock);
        let clock_state = clock_health.state(
            crate::diagnostics::Provider::CpuClock,
            std::time::Instant::now(),
        );
        let live = clock_state == crate::diagnostics::State::Live;
        let clocks = self.snapshot.cpu.clocks.as_ref();
        let ghz = |value: Option<f64>| {
            value.map_or_else(
                || "-- GHz".into(),
                |v| format!("{}{:.2} GHz", if live { "" } else { "~" }, v / 1000.0),
            )
        };
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
        )
        .on_hover_text(format!(
            "{} physical cores, {} logical processors\n{}",
            self.snapshot.cpu.physical_cores,
            self.snapshot.cpu.logical_cores,
            self.snapshot.os_name
        ));
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.graphs.cpu_all_cores, false, "Total CPU");
            ui.selectable_value(&mut self.graphs.cpu_all_cores, true, "All cores");
        });
        ui.add_space(theme::space::S);
        if self.graphs.cpu_all_cores {
            self.graphs.ensure_sample(&self.snapshot);
            self.graphs.cpu_grid(ui, self.snapshot.cpu.logical_cores, t);
        } else {
            // Room below: the clock tiles and the per-processor header.
            let height = widgets::fit_height(ui, 124.0, 160.0, 270.0);
            widgets::history_graph_with_window(
                ui,
                &self.cpu_history,
                t.accent,
                height,
                Some(100.0),
                t,
                ("120 s", HISTORY_LENGTH),
                Some(&|v: f32| format!("{v:.0}%")),
            );
        }
        ui.add_space(theme::space::L);
        let contributors = clocks.map_or_else(
            || "No clock interval measured yet.".to_owned(),
            |v| {
                format!(
                    "{} of {} logical processors reporting, {:.2} s interval.",
                    v.contributing(),
                    v.processors.len(),
                    v.interval_seconds
                )
            },
        );
        let provenance = format!(
            "{}\n{}\n{contributors}{}",
            crate::cpu_clock::SOURCE,
            crate::cpu_clock::SEMANTICS,
            if live {
                ""
            } else {
                "\n~ marks a retained reading, not a fresh measurement."
            }
        );
        let state = (!live).then(|| clock_state.label());
        ui.columns(2, |columns| {
            widgets::value_tile(
                &mut columns[0],
                "Fastest processor",
                &ghz(clocks.map(|v| v.fastest_mhz)),
                &provenance,
                state,
                false,
                t,
            );
            widgets::value_tile(
                &mut columns[1],
                "Slowest reporting",
                &ghz(clocks.map(|v| v.slowest_mhz)),
                &provenance,
                state,
                true,
                t,
            );
        });
        ui.add_space(theme::space::M);
        egui::CollapsingHeader::new("Per-processor clocks").show(ui, |ui| {
            widgets::hover_label(
                ui,
                RichText::new(crate::cpu_clock::SOURCE)
                    .size(11.0)
                    .color(t.text_muted),
            )
            .on_hover_text(crate::cpu_clock::SEMANTICS);
            widgets::detail_row(
                ui,
                "Legacy CurrentMhz / CPU 0",
                &if self.snapshot.cpu.frequency_mhz > 0 {
                    format!("{:.2} GHz", self.snapshot.cpu.frequency_mhz as f64 / 1000.0)
                } else {
                    "-- GHz".into()
                },
                t,
            )
            ;
            widgets::hover_label(
                ui,
                RichText::new(
                    "The legacy power clock is kept separate and never substituted for a missing interval.",
                )
                .size(11.0)
                .color(t.text_muted),
            );
            if let Some(clocks) = clocks {
                let row_height = 18.0 + widgets::surface(ui, t, false).total_margin().sum().y;
                egui::ScrollArea::vertical()
                    .id_salt("cpu-clock-processors")
                    .max_height(240.0)
                    .show_rows(ui, row_height, clocks.processors.len(), |ui, rows| {
                        for p in &clocks.processors[rows] {
                            widgets::detail_row(
                                ui,
                                &format!(
                                    "Group {} / CPU {} \u{b7} nominal {:.2} GHz",
                                    p.group,
                                    p.number,
                                    p.nominal_mhz as f64 / 1000.0
                                ),
                                &ghz(p.mhz),
                                t,
                            );
                        }
                    });
            }
        });
    }

    fn memory_performance(&self, ui: &mut egui::Ui) {
        let t = self.colors();
        let percent = memory_percent(&self.snapshot);
        let (state, chip, provenance) = self.memory_counter_state();
        widgets::performance_heading_with_state(
            ui,
            "Memory",
            &if self.snapshot.memory_total_bytes > 0 {
                format!(
                    "{} installed",
                    format::bytes(self.snapshot.memory_total_bytes)
                )
            } else {
                "Capacity not reported".into()
            },
            &if self.snapshot.memory_total_bytes > 0 {
                format::percent(percent)
            } else {
                "--".into()
            },
            chip,
            t.secondary,
            t,
        )
        .on_hover_text(provenance);
        // Room below: two rows of four tiles on a wide pane.
        let height = widgets::fit_height(ui, 136.0, 160.0, 270.0);
        widgets::history_graph_with_window(
            ui,
            &self.memory_history,
            t.secondary,
            height,
            Some(100.0),
            t,
            ("120 s", HISTORY_LENGTH),
            Some(&|v: f32| format!("{v:.0}%")),
        );
        ui.add_space(theme::space::L);
        let memory = self.snapshot.memory_details;
        let bytes = |value: Option<u64>| value.map_or_else(|| "--".into(), format::bytes);
        let physical = (self.snapshot.memory_total_bytes > 0).then_some(());
        let fields = [
            (
                "In use",
                bytes(physical.map(|_| self.snapshot.memory_used_bytes)),
                "Physical memory in use.",
                false,
            ),
            (
                "Available",
                bytes(physical.map(|_| self.snapshot.memory_available_bytes)),
                "Physical memory available to new allocations.",
                false,
            ),
            (
                "Committed",
                bytes(memory.map(|m| m.commit_bytes)),
                "Commit is allocated virtual memory, not RAM plus page-file use.",
                true,
            ),
            (
                "Commit limit",
                bytes(memory.map(|m| m.commit_limit_bytes)),
                "Physical memory plus page files.",
                true,
            ),
            (
                "Commit peak",
                bytes(memory.map(|m| m.commit_peak_bytes)),
                "Highest commit since boot.",
                true,
            ),
            (
                "System cache",
                bytes(memory.map(|m| m.system_cache_bytes)),
                "Includes standby pages and the system working set.",
                true,
            ),
            (
                "Paged pool",
                bytes(memory.map(|m| m.kernel_paged_bytes)),
                "Kernel memory that can be paged out.",
                true,
            ),
            (
                "Nonpaged pool",
                bytes(memory.map(|m| m.kernel_nonpaged_bytes)),
                "Kernel memory that always stays in RAM.",
                true,
            ),
        ];
        let count = if ui.available_width() >= 760.0 { 4 } else { 2 };
        for (row_index, row) in fields.chunks(count).enumerate() {
            ui.columns(count, |cols| {
                for (column_index, (column, (label, value, hover, counter))) in
                    cols.iter_mut().zip(row).enumerate()
                {
                    let hover = match state {
                        Some(state) if *counter => format!(
                            "{hover}
Counters: {state}."
                        ),
                        _ => (*hover).to_owned(),
                    };
                    widgets::value_tile(
                        column,
                        label,
                        value,
                        &hover,
                        // The hero carries the one counter state chip; a
                        // retained counter value is marked on its hover.
                        None,
                        (row_index + column_index) % 2 == 1,
                        t,
                    );
                }
            });
            ui.add_space(theme::space::M);
        }
    }

    /// Memory counter state: a chip label only when the counters are not Live,
    /// the hero chip, and the provenance text for the hero's hover.
    fn memory_counter_state(
        &self,
    ) -> (
        Option<&'static str>,
        Option<(&'static str, Color32)>,
        String,
    ) {
        let provider = crate::diagnostics::Provider::MemoryCounters;
        let health = self.snapshot.diagnostics.get(provider);
        let now = std::time::Instant::now();
        let state = health.state(provider, now);
        let provenance = format!(
            "Windows K32GetPerformanceInfo, sampled in the background. Last usable: {}.              Cached values keep their original timestamp; missing readings never become zero.",
            crate::diagnostics::age(health.last_success, now)
        );
        if state == crate::diagnostics::State::Live {
            return (None, None, provenance);
        }
        let (short, chip) = if self.snapshot.memory_details.is_some() {
            ("Cached", "Counters: Cached")
        } else {
            ("Unavailable", "Counters: Unavailable")
        };
        (
            Some(short),
            Some((chip, diagnostics::state_color(state, self.colors()))),
            provenance,
        )
    }

    fn disk_performance(&mut self, ui: &mut egui::Ui, index: usize) {
        let t = self.colors();
        self.graphs.ensure_sample(&self.snapshot);
        let Some(disk) = self.snapshot.disks.get(index) else {
            widgets::gap_row(
                ui,
                "Volume",
                "No longer present",
                "This volume is no longer reported. Choose another device on the left.",
                t,
            );
            return;
        };
        let total_rate = disk.read_bytes_per_sec + disk.write_bytes_per_sec;
        let kind = volume_kind(&disk.kind);
        let subline = [kind.as_str(), disk.file_system.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" \u{b7} ");
        widgets::performance_heading(
            ui,
            &format!("Volume {}", disk.mount),
            &subline,
            &format::rate(total_rate),
            t.good,
            t,
        )
        .on_hover_text(format!(
            "{}{}\nRemovable: {}\nRead + write throughput for this mounted volume.",
            if disk.name.is_empty() {
                "Unnamed volume"
            } else {
                disk.name.as_str()
            },
            if disk.mount.is_empty() {
                String::new()
            } else {
                format!(" ({})", disk.mount)
            },
            if disk.removable { "yes" } else { "no" }
        ));
        let height = widgets::fit_height(ui, 84.0, 160.0, 360.0);
        self.graphs.volume_graph(ui, &disk.mount, height, t);
        ui.add_space(theme::space::L);
        let used = (disk.total_bytes > 0).then(|| {
            let used = disk.total_bytes.saturating_sub(disk.available_bytes);
            format::percent(used as f32 / disk.total_bytes as f32 * 100.0)
        });
        ui.columns(4, |columns| {
            widgets::value_tile(
                &mut columns[0],
                "Read",
                &format::rate(disk.read_bytes_per_sec),
                "Bytes read per second from this volume.",
                None,
                false,
                t,
            );
            widgets::value_tile(
                &mut columns[1],
                "Write",
                &format::rate(disk.write_bytes_per_sec),
                "Bytes written per second to this volume.",
                None,
                true,
                t,
            );
            widgets::value_tile(
                &mut columns[2],
                "Capacity",
                &if disk.total_bytes > 0 {
                    format::bytes(disk.total_bytes)
                } else {
                    "--".into()
                },
                &format!("{} free", format::bytes(disk.available_bytes)),
                None,
                false,
                t,
            );
            widgets::value_tile(
                &mut columns[3],
                "Used",
                used.as_deref().unwrap_or("--"),
                if used.is_some() {
                    "Share of capacity in use."
                } else {
                    "Capacity not reported, so no share is shown."
                },
                None,
                true,
                t,
            );
        });
    }

    fn network_performance(&mut self, ui: &mut egui::Ui, index: usize) {
        let t = self.colors();
        self.graphs.ensure_sample(&self.snapshot);
        let Some(network) = self.snapshot.networks.get(index) else {
            widgets::gap_row(
                ui,
                "Network adapter",
                "No longer present",
                "This adapter is no longer reported. Choose another device on the left.",
                t,
            );
            return;
        };
        let rate = network.received_bytes_per_sec + network.transmitted_bytes_per_sec;
        widgets::performance_heading(
            ui,
            &network.name,
            "Network adapter",
            &format::rate(rate),
            t.secondary,
            t,
        )
        .on_hover_text("Receive plus send throughput for this adapter.");
        let height = widgets::fit_height(ui, 84.0, 160.0, 360.0);
        self.graphs.network_graph(ui, &network.name, height, t);
        ui.add_space(theme::space::L);
        ui.columns(4, |columns| {
            for (index, (column, (label, value, hover))) in columns
                .iter_mut()
                .zip([
                    (
                        "Receive",
                        format::rate(network.received_bytes_per_sec),
                        "Bytes received per second.",
                    ),
                    (
                        "Send",
                        format::rate(network.transmitted_bytes_per_sec),
                        "Bytes sent per second.",
                    ),
                    (
                        "Received",
                        format::bytes(network.total_received_bytes),
                        "Total received, as reported by Windows.",
                    ),
                    (
                        "Sent",
                        format::bytes(network.total_transmitted_bytes),
                        "Total sent, as reported by Windows.",
                    ),
                ])
                .enumerate()
            {
                widgets::value_tile(column, label, &value, hover, None, index % 2 == 1, t);
            }
        });
    }

    fn gpu_performance(&mut self, ui: &mut egui::Ui) {
        if !self.snapshot.gpu.adapters.is_empty() {
            self.gpu_adapters_page(ui);
            return;
        }
        let t = self.colors();
        let color = theme::mix(t.accent, t.secondary, 0.5);
        widgets::performance_heading(
            ui,
            "GPU",
            "",
            &self.snapshot.gpu.reading().label(),
            color,
            t,
        )
        .on_hover_text("Busiest Windows GPU engine. A trailing + marks partial counter coverage.");
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
                widgets::section_header(ui, "Most CPU time since start", None, t);
                widgets::history_header(ui, t);
                // Flat, zebra-striped rows read as one continuous table; a
                // gap between them would break the banding back into the
                // old boxed-row look.
                ui.spacing_mut().item_spacing.y = 0.0;
                for (rank, &index) in self.history_processes.iter().enumerate() {
                    let process = &self.snapshot.processes[index];
                    widgets::history_row(ui, rank, process, t);
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
        let total_processes = self.snapshot.process_count.max(1) as f32;
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
                    body.rows(34.0, users.len(), |mut row| {
                        let user = &users[row.index()];
                        // Windows sometimes will not disclose the owner of a system
                        // service's processes without admin rights; that gap reads
                        // as "Not readable", never a guessed account name. Details
                        // uses the same mapping (account_display) for its USER column.
                        let (display_name, unreadable) = account_display(&user.name);
                        let share = user.process_count as f32 / total_processes * 100.0;
                        widgets::table_column(&mut row, t, |ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                            ui.painter().rect_filled(rect, 7.0, t.accent_dim);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                display_name
                                    .chars()
                                    .next()
                                    .unwrap_or('?')
                                    .to_ascii_uppercase(),
                                FontId::proportional(15.0),
                                t.text,
                            );
                            let response = widgets::table_label(
                                ui,
                                RichText::new(display_name).strong().color(t.text),
                            );
                            if unreadable {
                                response.on_hover_text(
                                    "Windows did not let Trontop read the owner of these \
                                     processes (usually system services; needs admin).",
                                );
                            }
                        });
                        widgets::table_column(&mut row, t, |ui| {
                            widgets::heat_cell(
                                ui,
                                share,
                                user.process_count.to_string(),
                                t.accent,
                                t,
                            );
                        });
                        widgets::table_column(&mut row, t, |ui| {
                            widgets::cpu_cell(ui, user.cpu_percent, t);
                        });
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

    /// Startup and Services: the search box is in the command bar now.
    fn inventory_header(&mut self, ui: &mut egui::Ui, _title: &str, _subtitle: &str, _hint: &str) {
        self.page_intro(ui);
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
                widgets::hover_label(ui, RichText::new("End this process?").size(18.0).strong().color(t.text));
                widgets::identity_card(ui, &name, &format!("PID {pid}"), t);
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
                widgets::identity_card(ui, &process.name, &format!("PID {}", process.pid), t);
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
                widgets::identity_card(ui, &process.name, &format!("PID {}", process.pid), t);
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
                // Keep review/cancel outside the scrolling processor grid.
                egui::ScrollArea::vertical()
                    .id_salt("affinity_processors")
                    .max_height((ctx.content_rect().height() - 310.0).clamp(100.0, 340.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let columns =
                            (ui.available_width() / 76.0).floor().clamp(1.0, 8.0) as usize;
                        let cell_width =
                            (ui.available_width() - (columns - 1) as f32 * 6.0) / columns as f32;
                        egui::Grid::new("affinity_processor_grid")
                            .num_columns(columns)
                            .spacing([6.0, 6.0])
                            .show(ui, |ui| {
                                let mut shown = 0;
                                for processor in 0..usize::BITS {
                                    let bit = 1_usize << processor;
                                    if system_mask & bit == 0 {
                                        continue;
                                    }
                                    let enabled = self.affinity_draft & bit != 0;
                                    if ui
                                        .add_sized(
                                            [cell_width, 28.0],
                                            egui::Button::new(format!("CPU {processor:02}"))
                                                .selected(enabled),
                                        )
                                        .clicked()
                                    {
                                        if enabled {
                                            self.affinity_draft &= !bit;
                                        } else {
                                            self.affinity_draft |= bit;
                                        }
                                    }
                                    shown += 1;
                                    if shown % columns == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
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
                format!("Set {} priority?", priority.label()),
                if priority == PriorityClass::High {
                    "High priority can starve other applications and reduce system responsiveness. Trontop will not offer Realtime priority."
                } else {
                    "This changes how Windows schedules the process until it exits or another tool changes it."
                },
                "Apply priority",
                priority == PriorityClass::High,
            ),
            PendingControlAction::Affinity { affinity_mask, .. } => (
                "Change CPU affinity?".to_owned(),
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
                widgets::identity_card(ui, name, &format!("PID {pid}"), t);
                if let PendingControlAction::Affinity { affinity_mask, .. } = action {
                    widgets::detail_row(ui, "Requested CPUs", &format::cpu_set(affinity_mask), t);
                    widgets::detail_row(
                        ui,
                        "Processor count",
                        &format!("{} selected in this group", affinity_mask.count_ones()),
                        t,
                    );
                }
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
        self.theme_studio.show(
            ctx,
            &mut self.theme,
            &mut self.show_theme_editor,
            self.preferences.can_edit(),
        );
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
    fn raw_input_hook(&mut self, ctx: &egui::Context, _raw_input: &mut egui::RawInput) {
        // Restore once before begin_pass, never replace Memory halfway through a
        // layout. Theme editing stays disabled until the saved library is loaded.
        self.poll_preferences(ctx);
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.preferences_logic(ctx);
        if let Some(snapshot) = self
            .sampler
            .as_ref()
            .and_then(|sampler| sampler.latest_after(self.seen_generation))
        {
            self.accept_sample(snapshot);
        }
        self.poll_specs();
        if let Some(action) = self.tray.as_ref().and_then(TrayController::poll) {
            match action {
                TrayAction::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                TrayAction::Quit => self.request_close(ctx),
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
            self.preferences_bar(ui);
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading("Reconnecting graphics");
                ui.label("Your view and theme are retained. Process controls resume when drawing recovers.");
            });
            self.preferences_close_dialog(&ctx);
            return;
        }
        self.process_icons.begin_frame(&ctx);
        self.poll_service_command();
        if let Some(result) = self.exporter.poll() {
            self.specs_export_finished(&result);
            self.export_result = Some(result);
        }
        self.keyboard_shortcuts(&ctx);
        theme::paint_background(&ctx, self.theme);
        self.custom_chrome(ui);
        self.preferences_bar(ui);
        self.message_bar(ui);
        self.navigation(ui);
        self.command_bar(ui);
        self.inspector(ui);
        let t = self.colors();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::panel_color(self.theme))
                    // A short top edge: the page title already sits in the
                    // command bar right above.
                    .inner_margin(egui::Margin {
                        left: 18,
                        right: 18,
                        top: 10,
                        bottom: 18,
                    })
                    .stroke(Stroke::new(1.0, t.border)),
            )
            .show(ui, |ui| match self.page {
                Page::Overview => self.overview_page(ui),
                Page::Graphs => self.graphs_page(ui),
                Page::Sensors => self.sensors_page(ui),
                Page::Processes => self.processes_page(ui),
                Page::Performance => self.performance_page(ui),
                Page::History => self.history_page(ui),
                Page::Startup => self.startup_page(ui),
                Page::Users => self.users_page(ui),
                Page::Details => self.details_page(ui),
                Page::Services => self.services_page(ui),
                Page::System => self.system_page(ui),
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
        if self.preferences.can_edit()
            && (self.theme != self.preferences_theme
                || self.theme_studio.revision() != self.preferences_library_revision)
        {
            self.capture_preferences(&ctx);
        }
        self.preferences.dispatch(self.closing_at.is_some());
        self.preferences.schedule(&ctx);
        self.preferences_close_dialog(&ctx);
    }
}

/// A command-bar action. Compact bars draw the icon only; the label stays the
/// accessible name and the hover text.
fn command_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    compact: bool,
    base: Color32,
    t: Tokens,
) -> egui::Response {
    let response = if compact {
        let response = widgets::icon_button(ui, icon, "", Vec2::new(32.0, 28.0), base, t);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
        });
        response.on_hover_text(label)
    } else {
        widgets::icon_button(ui, icon, label, Vec2::new(0.0, 28.0), base, t)
    };
    #[cfg(test)]
    ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<Vec<(String, egui::Rect)>>(command_rects_id())
            .push((label.to_owned(), response.rect));
    });
    response
}

/// Test hook: command buttons drawn this frame, by label. Icon-only buttons
/// have no text shape for tests to find.
#[cfg(test)]
fn command_rects_id() -> egui::Id {
    egui::Id::new("trontop_command_rects")
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

/// Sysinfo's abbreviated `ProcessStatus` debug names translated to full words for
/// display (Windows currently only ever reports `Run` through this crate, but the
/// mapping stays exhaustive so unexpected values still read as words, not code).
fn process_state_label(status: &str) -> String {
    match status {
        "Run" => "Running",
        "Sleep" => "Sleeping",
        "Idle" => "Idle",
        "Stop" => "Stopped",
        "Zombie" => "Zombie",
        "Tracing" => "Tracing",
        "Dead" => "Dead",
        "Wakekill" => "Wakekill",
        "Waking" => "Waking",
        "Parked" => "Parked",
        "LockBlocked" => "Lock blocked",
        "UninterruptibleDiskSleep" => "Uninterruptible disk sleep",
        "Suspended" => "Suspended",
        "Unknown" => "Unknown",
        other => return other.to_string(),
    }
    .to_string()
}

/// Windows sometimes will not disclose the owner of a system service's
/// processes without admin rights; that gap displays as "Not readable",
/// never a guessed account name. Shared by the Users and Details/Processes
/// USER columns. The raw model value ("Unknown account") is untouched; this
/// is display-only. Returns `(display_name, unreadable)`.
fn account_display(name: &str) -> (&str, bool) {
    if name == "Unknown account" {
        ("Not readable", true)
    } else {
        (name, false)
    }
}

fn memory_percent(snapshot: &SystemSnapshot) -> f32 {
    if snapshot.memory_total_bytes == 0 {
        0.0
    } else {
        snapshot.memory_used_bytes as f32 / snapshot.memory_total_bytes as f32 * 100.0
    }
}

/// A volume's media kind for the Performance hero: "SSD" or "HDD" as sysinfo
/// reports them, and nothing for an unknown kind (never a guess).
fn volume_kind(kind: &str) -> String {
    match kind {
        "SSD" | "HDD" => kind.to_owned(),
        other if other.starts_with("Unknown") || other.is_empty() => String::new(),
        other => other.to_owned(),
    }
}
