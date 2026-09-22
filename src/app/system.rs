//! System specifications page: Speccy-style section navigation and trees.
//! Renders worker snapshots only; live values resolve from the sampler snapshot
//! and sensor-bridge readings. No native calls here.
use super::*;
use crate::specs::{
    self, Group, Item, LiveKey, Row, SectionId, SectionState, SummaryLine, TempBand, Value,
};
use std::time::Instant;

const NAV_WIDTH: f32 = 172.0;
const ROW_HEIGHT: f32 = 22.0;
const LIVE_WIDTH: f32 = 128.0;
const HIDDEN_HINT: &str =
    "Private value hidden. Turn on \"Reveal private values\" at the top of this page to show it.";

impl TrontopApp {
    /// Called from `logic`: start the workers the first time the page is shown
    /// and pull the latest published snapshot. Only publication reads here.
    pub(super) fn poll_specs(&mut self) {
        let visible = self.page == Page::System;
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
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(NAV_WIDTH, height),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(NAV_WIDTH);
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

    fn system_toolbar(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        ui.horizontal(|ui| {
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
            }
            if widgets::icon_button(ui, Icon::Export, "Copy as text", Vec2::ZERO, t.panel_raised, t)
                .on_hover_text("Copy every section as plain text. Private values follow the reveal setting.")
                .clicked()
            {
                ui.ctx().copy_text(specs::text(
                    &self.specs_view,
                    Some((&self.snapshot, &self.specs_view.bridge, Instant::now())),
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
            ui.checkbox(&mut self.reveal_private, "Reveal private values")
                .on_hover_text("Serial numbers, MAC and IP addresses, product IDs, user names, SSIDs and GUIDs are hidden by default.");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
                        "{read} of {} sections read / {busy} reading",
                        self.specs_view.entries.len()
                    ))
                    .size(10.0)
                    .color(t.text_muted),
                );
            });
        });
    }

    fn system_nav(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        ui.spacing_mut().item_spacing.y = 3.0;
        for id in SectionId::ALL {
            let color = match self.specs_view.get(id) {
                Some(entry) => state_color(entry.health.state, t),
                None => t.accent,
            };
            if section_button(ui, self.system_section == id, id.title(), color, t) {
                self.system_section = id;
            }
        }
    }

    fn system_summary(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let now = Instant::now();
        for id in SectionId::COLLECTED {
            if id == SectionId::SensorBridge {
                continue;
            }
            let Some(entry) = self.specs_view.get(id).cloned() else {
                continue;
            };
            let heading = ui.add(
                egui::Label::new(
                    RichText::new(id.title())
                        .size(13.0)
                        .strong()
                        .color(t.ink(t.accent)),
                )
                .sense(Sense::click()),
            );
            if heading
                .on_hover_text(format!("Open {}", id.title()))
                .clicked()
            {
                self.system_section = id;
            }
            match &entry.section {
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
            }
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
            widgets::hover_label(
                ui,
                RichText::new(id.title()).size(18.0).strong().color(t.text),
            );
            widgets::status_pill(
                ui,
                &entry.health.state.label().to_uppercase(),
                state_color(entry.health.state, t),
            );
        });
        let freshness = match (entry.health.collected_at, entry.health.duration) {
            (Some(at), Some(duration)) => format!(
                "Read {} in {:.1} ms. Automatic refresh every 5 minutes.",
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
            .chain(entry.section.iter().flat_map(|s| s.issues.iter()));
        for issue in issues {
            ui.add(
                egui::Label::new(
                    RichText::new(format!("Issue: {issue}"))
                        .size(10.0)
                        .color(t.text),
                )
                .truncate(),
            )
            .on_hover_text(issue);
        }
        ui.add_space(8.0);
        let Some(section) = entry.section else {
            return;
        };
        if section.groups.is_empty() {
            for line in &section.summary {
                self.summary_row(ui, line, false, now, t);
            }
            return;
        }
        for (index, group) in section.groups.iter().enumerate() {
            let group_id = ui.id().with(("spec_group", id.key(), index));
            self.spec_group(ui, group, group_id, now, t);
            ui.add_space(4.0);
        }
    }

    fn spec_group(&self, ui: &mut egui::Ui, group: &Group, id: egui::Id, now: Instant, t: Tokens) {
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            group.expanded,
        )
        .show_header(ui, |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(&group.title)
                        .size(12.5)
                        .strong()
                        .color(t.text),
                )
                .truncate(),
            )
            .on_hover_text(&group.title);
            if let Some(key) = &group.live {
                let (text, color, hover) = self.live_parts(key, now, t);
                ui.label(RichText::new(text).size(12.0).monospace().color(color))
                    .on_hover_text(hover);
            }
        })
        .body(|ui| {
            let mut banded = false;
            for (index, item) in group.items.iter().enumerate() {
                match item {
                    Item::Row(row) => {
                        self.spec_row(ui, row, banded, now, t);
                        banded = !banded;
                    }
                    Item::Group(child) => self.spec_group(ui, child, id.with(index), now, t),
                }
            }
        });
    }

    fn spec_row(&self, ui: &mut egui::Ui, row: &Row, banded: bool, now: Instant, t: Tokens) {
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
        spec_text_row(ui, &row.label, (&text, color), Some(&hover), banded, t);
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
        let live = line.live.as_ref().map(|key| self.live_parts(key, now, t));
        let live_width = if live.is_some() { LIVE_WIDTH } else { 0.0 };
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
    fn live_parts(&self, key: &LiveKey, now: Instant, t: Tokens) -> (String, Color32, String) {
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
                let hover = format!("Live: {text}");
                (text, color, hover)
            }
            Value::Unavailable(reason) => ("Unavailable".into(), t.text_muted, reason),
        }
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
        SectionState::Partial | SectionState::Slow => t.text,
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
fn spec_text_row(
    ui: &mut egui::Ui,
    label: &str,
    (value, color): (&str, Color32),
    hover: Option<&str>,
    banded: bool,
    t: Tokens,
) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::hover());
    paint_band(ui, rect, &response, banded, t);
    let label_width = if label.is_empty() {
        0.0
    } else {
        (width * 0.36).clamp(110.0, 240.0)
    };
    if !label.is_empty() {
        widgets::paint_text(
            ui,
            egui::Rect::from_min_max(
                rect.min + Vec2::new(8.0, 0.0),
                egui::pos2(rect.left() + label_width, rect.bottom()),
            ),
            label,
            FontId::proportional(12.0),
            t.text_muted,
            Align::Min,
        );
    }
    widgets::paint_text(
        ui,
        egui::Rect::from_min_max(
            egui::pos2(rect.left() + label_width + 8.0, rect.top()),
            rect.max - Vec2::new(8.0, 0.0),
        ),
        value,
        FontId::proportional(12.0),
        color,
        Align::Min,
    );
    if let Some(hover) = hover {
        response.on_hover_text(hover);
    }
}

/// Sub-navigation entry: stable geometry, status dot, accessible name.
fn section_button(ui: &mut egui::Ui, selected: bool, label: &str, dot: Color32, t: Tokens) -> bool {
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
    widgets::paint_text(
        ui,
        egui::Rect::from_min_max(
            response.rect.min + Vec2::new(12.0, 0.0),
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
    response.clicked()
}
