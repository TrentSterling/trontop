//! System specifications page: Speccy-style section navigation and trees.
//! Renders worker snapshots only; live values resolve from the sampler snapshot
//! and sensor-bridge readings. No native calls here.
use super::*;
use crate::export::{Capture, Format, Options};
use crate::specs::{
    self, Group, Item, LiveKey, Row, SectionId, SectionState, SummaryLine, TempBand, Value,
};
use std::time::Instant;

const ROW_HEIGHT: f32 = 22.0;
const LIVE_WIDTH: f32 = 150.0;
/// Summary lines sit under their section title, past the icon.
const SUMMARY_INDENT: f32 = 28.0;
const HIDDEN_HINT: &str =
    "Private value hidden. Turn on \"Reveal private values\" at the top of this page to show it.";

/// Summary order (Speccy's), without the Sensor Sources section.
const SUMMARY_SECTIONS: [SectionId; 10] = [
    SectionId::OperatingSystem,
    SectionId::Cpu,
    SectionId::Memory,
    SectionId::Motherboard,
    SectionId::Graphics,
    SectionId::Storage,
    SectionId::OpticalDrives,
    SectionId::Audio,
    SectionId::Peripherals,
    SectionId::Network,
];

pub(super) fn section_icon(id: SectionId) -> Icon {
    match id {
        SectionId::Summary => Icon::Overview,
        SectionId::OperatingSystem => Icon::Window,
        SectionId::Cpu => Icon::System,
        SectionId::Memory => Icon::Memory,
        SectionId::Motherboard => Icon::Board,
        SectionId::Graphics => Icon::Gpu,
        SectionId::Storage => Icon::Drive,
        SectionId::OpticalDrives => Icon::Disc,
        SectionId::Audio => Icon::Speaker,
        SectionId::Peripherals => Icon::Keyboard,
        SectionId::Network => Icon::Network,
        SectionId::SensorBridge => Icon::Sensors,
    }
}

/// Stable collapse-state identity for a group, independent of scroll nesting.
fn group_id(section: SectionId, path: &[usize]) -> egui::Id {
    egui::Id::new(("spec_group", section.key(), path.to_vec()))
}

impl TrontopApp {
    /// Called from `logic`: start the workers the first time the System or
    /// Sensors page is shown and pull the latest published snapshot. Only
    /// publication reads here.
    pub(super) fn poll_specs(&mut self) {
        let visible = matches!(self.page, Page::System | Page::Sensors);
        if visible && self.specs.is_none() && self.specs_enabled {
            self.specs = Some(crate::specs::Monitor::spawn());
        }
        if let Some(monitor) = &mut self.specs {
            monitor.set_live_active(visible);
            if visible {
                self.specs_view = monitor.snapshot(Instant::now());
            }
        }
    }

    pub(super) fn system_page(&mut self, ui: &mut egui::Ui) {
        self.page_header(
            ui,
            "System",
            "Hardware and Windows specifications from background workers. Live values follow the sampler.",
            false,
        );
        ui.add_space(8.0);
        self.system_toolbar(ui);
        ui.add_space(8.0);
        let height = ui.available_height().max(120.0);
        let nav_width = (ui.available_width() * 0.2).clamp(150.0, 196.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(nav_width, height),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(nav_width);
                    egui::ScrollArea::vertical()
                        .id_salt("system_nav")
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.system_nav(ui));
                },
            );
            ui.add_space(10.0);
            let width = ui.available_width();
            ui.allocate_ui_with_layout(
                Vec2::new(width, height),
                Layout::top_down(Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(("system_body", self.system_section.key()))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if self.system_section == SectionId::Summary {
                                self.system_summary(ui);
                            } else {
                                self.system_section_body(ui, self.system_section);
                            }
                        });
                },
            );
        });
    }

    fn live_source(&self, now: Instant) -> specs::LiveSource<'_> {
        Some((&self.snapshot, &self.specs_view.bridge, now))
    }

    fn system_toolbar(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        ui.horizontal_wrapped(|ui| {
            let refresh = ui
                .add_enabled_ui(self.specs.is_some(), |ui| {
                    widgets::icon_button(ui, Icon::Restart, "Refresh", Vec2::ZERO, t.panel_raised, t)
                })
                .inner
                .on_hover_text("Read every section again. A read already in flight is not duplicated.")
                .on_disabled_hover_text("Specs workers start when this page is first shown.");
            if refresh.clicked()
                && let Some(monitor) = &self.specs
            {
                monitor.refresh();
                self.message = Some(("Reading every System section again.".into(), false));
            }
            if widgets::icon_button(ui, Icon::Copy, "Copy all", Vec2::ZERO, t.panel_raised, t)
                .on_hover_text("Copy every section as plain text. Private values follow the reveal setting.")
                .clicked()
            {
                ui.ctx().copy_text(specs::text(
                    &self.specs_view,
                    self.live_source(Instant::now()),
                    self.reveal_private,
                ));
                self.message = Some((
                    if self.reveal_private {
                        "System specs copied, including private values. Nothing uploaded."
                    } else {
                        "System specs copied with private values hidden. Nothing uploaded."
                    }
                    .into(),
                    false,
                ));
            }
            let can_save = self.exporter.available()
                && !self.exporter.busy()
                && self.specs_view.entries.iter().any(|e| e.section.is_some());
            for (format, label) in [(Format::SpecsText, "Save text"), (Format::SpecsJson, "Save JSON")] {
                let response = ui
                    .add_enabled_ui(can_save, |ui| {
                        widgets::icon_button(ui, Icon::Export, label, Vec2::ZERO, t.panel_raised, t)
                    })
                    .inner
                    .on_hover_text("Choose a file in Windows Save As. Written in the background; nothing is uploaded.")
                    .on_disabled_hover_text(if self.exporter.busy() {
                        "An export is already in progress."
                    } else if !self.exporter.available() {
                        "Saving files is unavailable in this session."
                    } else {
                        "Available after the first section is read."
                    });
                if response.clicked() {
                    self.save_specs(ui.ctx(), format);
                }
            }
            ui.checkbox(&mut self.specs_export_private, "Private values in saved files")
                .on_hover_text("Off by default and after every save. Serials, MAC and IP addresses, product IDs and user names stay out of files unless this is on.");
            ui.checkbox(&mut self.reveal_private, "Reveal private values")
                .on_hover_text("Serial numbers, MAC and IP addresses, product IDs, user names, SSIDs and GUIDs are hidden on screen by default.");
        });
    }

    fn save_specs(&mut self, ctx: &egui::Context, format: Format) {
        let options = Options {
            format,
            private_details: self.specs_export_private,
        };
        let capture = Capture::specs(self.snapshot.clone(), self.specs_view.clone(), options);
        match self.exporter.submit(capture, ctx.clone()) {
            Ok(()) => {
                self.specs_export_pending = true;
                self.message = Some((
                    "Choose where to save the System specs in Windows Save As.".into(),
                    false,
                ));
            }
            Err(error) => self.message = Some((error, true)),
        }
        // Private values are opt-in per save, like the snapshot export.
        self.specs_export_private = false;
    }

    /// Turns a finished specs save into a status message. Called with every
    /// export outcome; ignores outcomes of the snapshot export panel.
    pub(super) fn specs_export_finished(&mut self, outcome: &crate::export::Outcome) {
        if !std::mem::take(&mut self.specs_export_pending) {
            return;
        }
        self.message = Some(match outcome {
            crate::export::Outcome::Saved { path, bytes, .. } => (
                format!(
                    "System specs saved: {} ({}).",
                    path.display(),
                    format::bytes(*bytes)
                ),
                false,
            ),
            crate::export::Outcome::Cancelled => ("System specs save cancelled.".into(), false),
            crate::export::Outcome::Failed(error) => {
                (format!("System specs were not saved: {error}"), true)
            }
        });
    }

    fn system_nav(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        ui.spacing_mut().item_spacing.y = 3.0;
        for id in SectionId::ALL {
            let state = self.specs_view.get(id).map(|e| e.health.state);
            let color = state.map_or(t.accent, |s| state_color(s, t));
            let hover = match state {
                Some(state) => format!("{}: {}", id.title(), state.label()),
                None => "Headlines from every section".into(),
            };
            if section_button(ui, self.system_section == id, id, color, t)
                .on_hover_text(hover)
                .clicked()
            {
                self.system_section = id;
            }
        }
        ui.add_space(8.0);
        let read = self
            .specs_view
            .entries
            .iter()
            .filter(|e| e.section.is_some())
            .count();
        let busy = self
            .specs_view
            .entries
            .iter()
            .filter(|e| e.health.collecting_since.is_some())
            .count();
        widgets::hover_label(
            ui,
            RichText::new(format!(
                "{read} of {} sections read\n{busy} reading now",
                self.specs_view.entries.len()
            ))
            .size(10.0)
            .color(t.text_muted),
        );
    }

    fn system_summary(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let now = Instant::now();
        for id in SUMMARY_SECTIONS {
            let Some(entry) = self.specs_view.get(id).cloned() else {
                continue;
            };
            let heading = ui
                .horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                    section_icon(id).paint(ui.painter(), rect, t.ink(t.accent));
                    let heading = ui.add(
                        egui::Label::new(
                            RichText::new(id.title())
                                .size(13.0)
                                .strong()
                                .color(t.ink(t.accent)),
                        )
                        .sense(Sense::click()),
                    );
                    if !matches!(entry.health.state, SectionState::Complete) {
                        widgets::status_pill(
                            ui,
                            &entry.health.state.label().to_uppercase(),
                            state_color(entry.health.state, t),
                        );
                    }
                    heading
                })
                .inner;
            if heading
                .on_hover_text(format!("Open {}", id.title()))
                .clicked()
            {
                self.system_section = id;
            }
            ui.horizontal(|ui| {
                ui.add_space(SUMMARY_INDENT);
                ui.vertical(|ui| match &entry.section {
                    Some(section) if !section.summary.is_empty() => {
                        for (index, line) in section.summary.iter().enumerate() {
                            self.summary_row(ui, line, index % 2 == 1, now, t);
                        }
                    }
                    Some(_) => {
                        spec_text_row(ui, "", ("No summary", t.text_muted), None, false, t);
                    }
                    None => {
                        spec_text_row(
                            ui,
                            "",
                            (waiting_text(entry.health.state), t.text_muted),
                            None,
                            false,
                            t,
                        );
                    }
                });
            });
            ui.add_space(8.0);
        }
    }

    fn system_section_body(&mut self, ui: &mut egui::Ui, id: SectionId) {
        let t = self.colors();
        let now = Instant::now();
        let Some(entry) = self.specs_view.get(id).cloned() else {
            return;
        };
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
            section_icon(id).paint(ui.painter(), rect, t.ink(t.accent));
            widgets::hover_label(
                ui,
                RichText::new(id.title()).size(18.0).strong().color(t.text),
            );
            widgets::status_pill(
                ui,
                &entry.health.state.label().to_uppercase(),
                state_color(entry.health.state, t),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let Some(section) = &entry.section else {
                    return;
                };
                if widgets::icon_button(
                    ui,
                    Icon::Copy,
                    "Copy section",
                    Vec2::ZERO,
                    t.panel_raised,
                    t,
                )
                .on_hover_text(
                    "Copy this section as plain text. Private values follow the reveal setting.",
                )
                .clicked()
                {
                    let mut text = String::new();
                    specs::section_text(
                        &mut text,
                        section,
                        self.live_source(now),
                        self.reveal_private,
                    );
                    ui.ctx().copy_text(text);
                    self.message =
                        Some((format!("{} copied. Nothing uploaded.", id.title()), false));
                }
                if !section.groups.is_empty() {
                    if widgets::icon_button(
                        ui,
                        Icon::Expand,
                        "Collapse all",
                        Vec2::ZERO,
                        t.panel_raised,
                        t,
                    )
                    .on_hover_text("Collapse every group in this section")
                    .clicked()
                    {
                        set_groups_open(ui.ctx(), id, &section.groups, false);
                    }
                    if widgets::icon_button(
                        ui,
                        Icon::Collapse,
                        "Expand all",
                        Vec2::ZERO,
                        t.panel_raised,
                        t,
                    )
                    .on_hover_text("Expand every group in this section")
                    .clicked()
                    {
                        set_groups_open(ui.ctx(), id, &section.groups, true);
                    }
                }
            });
        });
        let freshness = match (entry.health.collected_at, entry.health.duration) {
            (Some(at), Some(duration)) => format!(
                "Read {} in {:.1} ms. Automatic refresh every 5 minutes. Right-click a row to copy it.",
                crate::diagnostics::age(Some(at), now),
                duration.as_secs_f64() * 1000.0
            ),
            _ => waiting_text(entry.health.state).into(),
        };
        widgets::hover_label(ui, RichText::new(freshness).size(10.0).color(t.text_muted));
        let issues = entry
            .health
            .issues
            .iter()
            .chain(entry.section.iter().flat_map(|s| s.issues.iter()))
            .collect::<Vec<_>>();
        if !issues.is_empty() {
            ui.add_space(4.0);
            for issue in issues {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    Icon::Info.paint(ui.painter(), rect, t.text_muted);
                    ui.add(
                        egui::Label::new(RichText::new(issue.as_str()).size(10.5).color(t.text))
                            .truncate(),
                    )
                    .on_hover_text(issue.as_str());
                });
            }
        }
        ui.add_space(8.0);
        let Some(section) = entry.section else {
            spec_text_row(
                ui,
                "",
                (waiting_text(entry.health.state), t.text_muted),
                None,
                false,
                t,
            );
            return;
        };
        if section.groups.is_empty() {
            for line in &section.summary {
                self.summary_row(ui, line, false, now, t);
            }
            return;
        }
        for (index, group) in section.groups.iter().enumerate() {
            self.spec_group(ui, id, group, &[index], now, t);
            ui.add_space(4.0);
        }
    }

    fn spec_group(
        &mut self,
        ui: &mut egui::Ui,
        section: SectionId,
        group: &Group,
        path: &[usize],
        now: Instant,
        t: Tokens,
    ) {
        let id = group_id(section, path);
        let (_, header, _) = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            group.expanded,
        )
        .show_header(ui, |ui| {
            let title = ui
                .add(
                    egui::Label::new(
                        RichText::new(&group.title)
                            .size(12.5)
                            .strong()
                            .color(t.text),
                    )
                    .truncate()
                    .sense(Sense::click()),
                )
                .on_hover_text(format!("{}\nRight-click to copy this group.", group.title));
            if let Some(key) = &group.live {
                let (text, color, hover) = self.headline_live(key, now, t);
                ui.label(RichText::new(text).size(12.5).strong().color(color))
                    .on_hover_text(hover);
            }
            title
        })
        .body(|ui| {
            let mut banded = false;
            for (index, item) in group.items.iter().enumerate() {
                match item {
                    Item::Row(row) => {
                        self.spec_row(ui, row, banded, now, t);
                        banded = !banded;
                    }
                    Item::Group(child) => {
                        let mut child_path = path.to_vec();
                        child_path.push(index);
                        self.spec_group(ui, section, child, &child_path, now, t);
                    }
                }
            }
        });
        header.inner.context_menu(|ui| {
            if ui.button("Copy group").clicked() {
                ui.ctx().copy_text(specs::group_text(
                    group,
                    self.live_source(now),
                    self.reveal_private,
                ));
                ui.close();
            }
        });
    }

    /// Display text, color and hover for one row, honoring the reveal setting.
    fn row_parts(&self, row: &Row, now: Instant, t: Tokens) -> (String, Color32, String) {
        let (text, color, hover) = if row.private && !self.reveal_private {
            ("Hidden".to_string(), t.text_muted, HIDDEN_HINT.to_string())
        } else if let Some(key) = &row.live {
            self.live_parts(key, now, t)
        } else {
            match &row.value {
                Value::Known(_) => {
                    let text = row.display_text().unwrap_or_default();
                    let hover = format!("{}: {text}", row.label);
                    (text, t.text, hover)
                }
                Value::Unavailable(reason) => ("Unavailable".into(), t.text_muted, reason.clone()),
            }
        };
        let hover = match &row.note {
            Some(note) => format!("{hover}\n{note}"),
            None => hover,
        };
        (text, color, hover)
    }

    fn spec_row(&mut self, ui: &mut egui::Ui, row: &Row, banded: bool, now: Instant, t: Tokens) {
        let (text, color, hover) = self.row_parts(row, now, t);
        let response = spec_text_row(ui, &row.label, (&text, color), Some(&hover), banded, t);
        let hidden = row.private && !self.reveal_private;
        // Unavailable rows copy their reason, never the placeholder word.
        let copied = if text == "Unavailable" {
            hover.lines().next().unwrap_or_default().to_string()
        } else {
            text.clone()
        };
        response.context_menu(|ui| {
            if hidden {
                ui.label(RichText::new("Private value hidden").color(t.text_muted));
                return;
            }
            if ui.button("Copy value").clicked() {
                ui.ctx().copy_text(copied.clone());
                ui.close();
            }
            if ui.button("Copy row").clicked() {
                ui.ctx().copy_text(format!("{}: {copied}", row.label));
                ui.close();
            }
        });
    }

    fn summary_row(
        &self,
        ui: &mut egui::Ui,
        line: &SummaryLine,
        banded: bool,
        now: Instant,
        t: Tokens,
    ) {
        let (text, color, hover) = if line.private && !self.reveal_private {
            ("Hidden".to_string(), t.text_muted, HIDDEN_HINT.to_string())
        } else {
            match &line.text {
                Value::Known(text) => (text.clone(), t.text, text.clone()),
                Value::Unavailable(reason) => ("Unavailable".into(), t.text_muted, reason.clone()),
            }
        };
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::hover());
        paint_band(ui, rect, &response, banded, t);
        let live = line
            .live
            .as_ref()
            .map(|key| self.headline_live(key, now, t));
        // Room for the whole live value (memory usage is long), within limits.
        let live_width = live.as_ref().map_or(0.0, |(text, _, _)| {
            let measured = ui
                .painter()
                .layout_no_wrap(text.clone(), FontId::monospace(12.0), Color32::PLACEHOLDER)
                .size()
                .x;
            (measured + 12.0).clamp(LIVE_WIDTH.min(width * 0.3), width * 0.45)
        });
        widgets::paint_text(
            ui,
            egui::Rect::from_min_max(
                rect.min + Vec2::new(8.0, 0.0),
                egui::pos2(rect.right() - live_width - 8.0, rect.bottom()),
            ),
            &text,
            FontId::proportional(12.0),
            color,
            Align::Min,
        );
        let mut tooltip = hover;
        if let Some((live_text, live_color, live_hover)) = live {
            widgets::paint_text(
                ui,
                egui::Rect::from_min_max(
                    egui::pos2(rect.right() - live_width, rect.top()),
                    rect.max - Vec2::new(8.0, 0.0),
                ),
                &live_text,
                FontId::monospace(12.0),
                live_color,
                Align::Max,
            );
            tooltip = format!("{tooltip}\n{live_hover}");
        }
        response.on_hover_text(tooltip);
    }

    /// Display text, color and hover text for a live key. Pure data lookup.
    pub(super) fn live_parts(
        &self,
        key: &LiveKey,
        now: Instant,
        t: Tokens,
    ) -> (String, Color32, String) {
        let value = specs::resolve(key, &self.snapshot, &self.specs_view.bridge, now);
        match value.value {
            Value::Known(text) => {
                let color = value.celsius.map_or(t.text, |celsius| {
                    t.ink(match specs::band(key, celsius) {
                        TempBand::Normal => t.good,
                        TempBand::Warm => theme::mix(t.good, t.danger, 0.5),
                        TempBand::Hot => t.danger,
                    })
                });
                let hover = match value.source {
                    Some(source) => format!("Live: {text}\nSource: {source}"),
                    None => format!("Live: {text}"),
                };
                (text, color, hover)
            }
            Value::Unavailable(reason) => ("Unavailable".into(), t.text_muted, reason),
        }
    }

    /// Live value beside a headline or group title: a muted "--" when absent,
    /// with the reason on hover, so titles do not repeat "Unavailable".
    fn headline_live(&self, key: &LiveKey, now: Instant, t: Tokens) -> (String, Color32, String) {
        let (text, color, hover) = self.live_parts(key, now, t);
        if color == t.text_muted && text == "Unavailable" {
            ("--".into(), t.text_muted, format!("Unavailable: {hover}"))
        } else {
            (text, color, hover)
        }
    }
}

fn set_groups_open(ctx: &egui::Context, section: SectionId, groups: &[Group], open: bool) {
    fn walk(
        ctx: &egui::Context,
        section: SectionId,
        group: &Group,
        path: &mut Vec<usize>,
        open: bool,
    ) {
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ctx,
            group_id(section, path),
            group.expanded,
        );
        state.set_open(open);
        state.store(ctx);
        for (index, item) in group.items.iter().enumerate() {
            if let Item::Group(child) = item {
                path.push(index);
                walk(ctx, section, child, path, open);
                path.pop();
            }
        }
    }
    for (index, group) in groups.iter().enumerate() {
        walk(ctx, section, group, &mut vec![index], open);
    }
}

fn waiting_text(state: SectionState) -> &'static str {
    match state {
        SectionState::Waiting => "Waiting for the first read.",
        SectionState::Collecting => "Reading now. Nothing is shown until the first read completes.",
        SectionState::Slow => "The first read is slow. Nothing is estimated meanwhile.",
        SectionState::Stopped => "The collection worker stopped before its first read.",
        _ => "No data.",
    }
}

fn state_color(state: SectionState, t: Tokens) -> Color32 {
    match state {
        SectionState::Complete => t.good,
        SectionState::Partial | SectionState::Slow => theme::mix(t.good, t.danger, 0.5),
        SectionState::Unavailable | SectionState::Stopped => t.danger,
        SectionState::Waiting | SectionState::Collecting => t.text_muted,
    }
}

fn paint_band(ui: &egui::Ui, rect: egui::Rect, response: &egui::Response, banded: bool, t: Tokens) {
    let fill = if response.hovered() {
        ui.visuals().widgets.hovered.weak_bg_fill
    } else if banded {
        ui.visuals().faint_bg_color
    } else {
        t.panel_raised
    };
    ui.painter().rect_filled(rect, 3.0, fill);
}

/// Aligned label and value columns on one fixed-height row.
pub(super) fn spec_text_row(
    ui: &mut egui::Ui,
    label: &str,
    (value, color): (&str, Color32),
    hover: Option<&str>,
    banded: bool,
    t: Tokens,
) -> egui::Response {
    let width = ui.available_width();
    let label_width = if label.is_empty() {
        0.0
    } else {
        (width * 0.34).clamp(96.0, 240.0)
    };
    // Long values (instruction sets, partition lists) wrap onto extra lines
    // instead of being cut; everything else keeps the fixed row height.
    let value_width = (width - label_width - 16.0).max(24.0);
    let galley = ui.painter().layout(
        value.to_string(),
        FontId::proportional(12.0),
        color,
        value_width,
    );
    let wrapped = galley.rows.len() > 1;
    let height = if wrapped {
        galley.size().y + 8.0
    } else {
        ROW_HEIGHT
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    paint_band(ui, rect, &response, banded, t);
    let first_line = egui::Rect::from_min_size(rect.min, Vec2::new(width, ROW_HEIGHT));
    if !label.is_empty() {
        widgets::paint_text(
            ui,
            egui::Rect::from_min_max(
                first_line.min + Vec2::new(8.0, 0.0),
                egui::pos2(first_line.left() + label_width, first_line.bottom()),
            ),
            label,
            FontId::proportional(12.0),
            t.text_muted,
            Align::Min,
        );
    }
    let value_left = rect.left() + label_width + 8.0;
    if wrapped {
        ui.painter_at(rect)
            .galley(egui::pos2(value_left, rect.top() + 4.0), galley, color);
    } else {
        widgets::paint_text(
            ui,
            egui::Rect::from_min_max(
                egui::pos2(value_left, rect.top()),
                rect.max - Vec2::new(8.0, 0.0),
            ),
            value,
            FontId::proportional(12.0),
            color,
            Align::Min,
        );
    }
    match hover {
        Some(hover) => response.on_hover_text(hover),
        None => response,
    }
}

/// Sub-navigation entry: icon, title, status dot, stable geometry.
fn section_button(
    ui: &mut egui::Ui,
    selected: bool,
    id: SectionId,
    dot: Color32,
    t: Tokens,
) -> egui::Response {
    let label = id.title();
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 28.0), Sense::click());
    let fill = if response.hovered() || response.has_focus() {
        ui.visuals().widgets.hovered.weak_bg_fill
    } else if selected {
        t.accent_dim
    } else {
        t.panel_raised
    };
    ui.painter().rect(
        response.rect,
        ui.visuals().widgets.inactive.corner_radius,
        fill,
        Stroke::new(
            1.0,
            if selected || response.has_focus() {
                theme::readable_text(t.accent, fill)
            } else {
                t.border
            },
        ),
        egui::StrokeKind::Inside,
    );
    if selected {
        let bar = egui::Rect::from_min_size(
            response.rect.left_top() + Vec2::new(2.0, 6.0),
            Vec2::new(2.0, 16.0),
        );
        ui.painter().rect_filled(bar, 1.0, t.accent);
    }
    let text = theme::readable_text(t.text, fill);
    section_icon(id).paint(
        ui.painter(),
        egui::Rect::from_center_size(
            response.rect.left_center() + Vec2::new(19.0, 0.0),
            Vec2::splat(16.0),
        ),
        text,
    );
    widgets::paint_text(
        ui,
        egui::Rect::from_min_max(
            response.rect.min + Vec2::new(34.0, 0.0),
            response.rect.max - Vec2::new(22.0, 0.0),
        ),
        label,
        FontId::proportional(12.0),
        text,
        Align::Min,
    );
    ui.painter().circle_filled(
        response.rect.right_center() - Vec2::new(11.0, 0.0),
        3.0,
        t.ink(dot),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, label)
    });
    response
}
