use super::*;
use crate::process_actions::tree::{Plan, Scope};

#[derive(Clone)]
pub(super) struct PendingEndTree {
    root: ProcessIdentity,
    name: String,
    scope: Scope,
    review: Result<Plan, String>,
}

impl TrontopApp {
    pub(super) fn observed_process_state(&self, process: &ProcessRow) -> String {
        if process
            .identity()
            .is_some_and(|identity| self.process_actions.holds_suspension(identity))
        {
            "Suspended".into()
        } else {
            process_state_label(&process.status)
        }
    }

    pub(super) fn request_end_tree_selected(&mut self) {
        if !self.process_actions.ready() {
            return;
        }
        let Some(process) = self.selected_process() else {
            return;
        };
        let Some(root) = process.identity() else {
            self.message = Some((
                "Process identity is unavailable. Wait for a complete sample.".into(),
                true,
            ));
            return;
        };
        let scope = Scope::SelectedTree;
        self.pending_end_tree = Some(PendingEndTree {
            root,
            name: process.name.clone(),
            scope,
            review: Plan::build(&self.snapshot.processes, root, scope),
        });
        self.pending_end_task = None;
        self.pending_control_action = None;
    }

    pub(super) fn confirm_end_tree(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_end_tree.clone() else {
            return;
        };
        let t = self.colors();
        let mut cancel = false;
        let mut submitted = false;
        let closed = widgets::action_dialog(ctx, "Confirm end process tree", 520.0, t, |ui| {
            widgets::identity_card(
                ui,
                &pending.name,
                &format!("Selected PID {}", pending.root.pid),
                t,
            );
            let mut all = pending.scope == Scope::AllInstances;
            if ui
                .checkbox(&mut all, "End all instances of this executable")
                .changed()
            {
                pending.scope = if all {
                    Scope::AllInstances
                } else {
                    Scope::SelectedTree
                };
                pending.review = Plan::build(&self.snapshot.processes, pending.root, pending.scope);
            }
            widgets::hover_label(ui, RichText::new(if all {
                "Includes separate browser windows and profiles using the same executable, plus their children."
            } else {
                "Includes the selected process and its children. Separate instances and sibling processes are excluded."
            }).color(t.text_muted));
            widgets::hover_label(ui, RichText::new("Processes will be terminated immediately. Unsaved work in every listed process will be lost.").color(t.ink(t.danger)));
            ui.add_space(6.0);
            let current = pending.review.as_ref().is_ok_and(|plan| {
                plan.root() == pending.root
                    && plan.scope() == pending.scope
                    && plan.matches(&self.snapshot.processes)
            });
            match &pending.review {
                Ok(plan) => {
                    // Keep dialog geometry stable when the scope changes.
                    if let Some(path) = plan.executable() {
                        ui.add(
                            egui::Label::new(RichText::new(path).size(10.0).color(t.text_muted))
                                .truncate(),
                        )
                        .on_hover_text(path);
                    }
                    widgets::hover_label(
                        ui,
                        RichText::new(format!(
                            "{} processes in the reviewed list",
                            plan.targets().len()
                        ))
                        .strong()
                        .color(t.text),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("end_tree_targets")
                        .max_height((ctx.content_rect().height() - 400.0).clamp(68.0, 220.0))
                        .auto_shrink([false, true])
                        .show_rows(ui, 24.0, plan.targets().len(), |ui, range| {
                            for target in &plan.targets()[range] {
                                ui.horizontal(|ui| {
                                    ui.add_space((target.depth.min(8) * 10) as f32);
                                    ui.label(
                                        RichText::new(format!("{:>6}", target.identity.pid))
                                            .monospace()
                                            .color(t.text_muted),
                                    );
                                    ui.add(
                                        egui::Label::new(RichText::new(&target.name).color(t.text))
                                            .truncate(),
                                    )
                                    .on_hover_text(&target.name);
                                });
                            }
                        });
                    widgets::hover_label(ui, RichText::new(if current {
                        "Only listed processes are targeted. New processes require another confirmation."
                    } else {
                        "The target list changed. Refresh it and review before confirming."
                    }).size(11.0).color(t.text));
                }
                Err(error) => {
                    widgets::hover_label(ui, RichText::new(error).color(t.ink(t.danger)));
                }
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                cancel = ui.button("Cancel").clicked();
                if ui.button("Refresh targets").clicked() {
                    pending.review =
                        Plan::build(&self.snapshot.processes, pending.root, pending.scope);
                }
                // A refresh never submits in the same interaction. It must be reviewed.
                if widgets::action_button_enabled(
                    ui,
                    RichText::new("End listed processes").color(Color32::WHITE),
                    Vec2::ZERO,
                    t.danger,
                    t,
                    current && self.process_actions.ready(),
                )
                .on_disabled_hover_text(
                    "Review a current target list and wait for the action worker.",
                )
                .clicked()
                    && let Ok(plan) = &pending.review
                {
                    submitted = self.submit_process_action(
                        ctx,
                        ProcessAction::EndTree(plan.clone()),
                        format!(
                            "{} ({} reviewed processes)",
                            pending.name,
                            plan.targets().len()
                        ),
                    );
                }
            });
        });
        self.pending_end_tree = if closed || cancel || submitted {
            None
        } else {
            Some(pending)
        };
    }

    pub(super) fn suspension_controls(
        &mut self,
        ui: &mut egui::Ui,
        process: &ProcessRow,
        t: Tokens,
    ) {
        let Some(identity) = process.identity() else {
            return;
        };
        let held = self.process_actions.holds_suspension(identity);
        if held {
            widgets::hover_label(
                ui,
                RichText::new("Suspended by Trontop").color(t.ink(t.secondary)),
            );
        }
        let ready = self.process_actions.ready() && platform::can_control(identity.pid).is_ok();
        ui.horizontal(|ui| {
            if ui.add_enabled(ready && !held, egui::Button::new("Suspend"))
                .on_hover_text("Pause execution after confirmation. Trontop releases its hold when it closes.").clicked() {
                self.pending_control_action = Some(PendingControlAction::Suspend { identity });
            }
            if ui.add_enabled(ready, egui::Button::new("Resume"))
                .on_hover_text("Release Trontop's suspension, or request Windows to resume an externally suspended process.").clicked() {
                self.pending_control_action = Some(PendingControlAction::Resume { identity });
            }
        });
    }
}
