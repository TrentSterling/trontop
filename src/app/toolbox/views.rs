use super::*;

enum Destination {
    Folder(PathBuf),
    File(PathBuf),
    CheckDuplicates(Arc<scan::Scan>),
    #[cfg(windows)]
    ReviewCopy(PathBuf, PathBuf, u64),
}

impl Toolbox {
    pub(super) fn space_view(&mut self, ui: &mut egui::Ui, disks: &[DiskRow], t: Tokens) {
        let idle = self.job.is_none();
        #[cfg(windows)]
        if let Some(review) = self.copy_review.clone() {
            ui.strong("Permanently remove this duplicate?");
            ui.label(format!("Remove  {}", display(&review.remove)));
            ui.label(format!("Keep  {}", display(&review.keep)));
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    self.copy_review = None;
                }
                if ui
                    .add_enabled(idle, egui::Button::new("Remove verified copy"))
                    .clicked()
                {
                    self.copy_review = None;
                    self.start(ui.ctx(), "Rechecking both files", false, move |_, _| {
                        review.apply()?;
                        Ok(Output::CopyRemoved(review.remove))
                    });
                }
            });
            return;
        }
        let mut destination = None;
        ui.add_enabled_ui(idle, |ui| {
            ui.horizontal_wrapped(|ui| {
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.folder)
                        .desired_width((ui.available_width() - 150.0).clamp(90.0, 520.0))
                        .hint_text("Drive or folder path"),
                );
                if ui.button("Scan folder").clicked()
                    || (edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    destination = Some(Destination::Folder(
                        self.folder.trim().trim_matches('"').into(),
                    ));
                }
                if let Some(parent) = self.scan.as_ref().and_then(|scan| scan.root.parent())
                    && ui.button("Up").clicked()
                {
                    destination = Some(Destination::Folder(parent.into()));
                }
            });
            ui.horizontal_wrapped(|ui| {
                for disk in disks {
                    if ui
                        .button(&disk.mount)
                        .on_hover_text(format!(
                            "{} free of {}",
                            format::bytes(disk.available_bytes),
                            format::bytes(disk.total_bytes)
                        ))
                        .clicked()
                    {
                        destination = Some(Destination::Folder(disk.mount.clone().into()));
                    }
                }
            });
        });
        if let Some(scan) = &self.scan {
            widgets::section_header(ui, &display(&scan.root), None, t);
            ui.label(format!(
                "{} logical  ·  {} files  ·  {} unreadable  ·  {} links skipped",
                format::bytes(scan.bytes),
                scan.files,
                scan.skipped,
                scan.links
            ));
            ui.horizontal_wrapped(|ui| {
                for (v, label) in [
                    (View::Map, "Size map"),
                    (View::Largest, "Largest files"),
                    (View::Types, "File types"),
                    (View::Duplicates, "Duplicates"),
                    (View::Projects, "Build caches"),
                    (View::Changes, "Changes"),
                ] {
                    if widgets::stable_tab(ui, self.view == v, label).clicked() {
                        self.view = v;
                    }
                }
            });
            match self.view {
                View::Map => {
                    if let Some(item) = draw_map(ui, scan, t, idle) {
                        destination = Some(item);
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("folder_entries")
                        .show_rows(ui, 22.0, scan.entries.len(), |ui, range| {
                            for e in &scan.entries[range] {
                                if item_row(ui, &e.path, e.bytes, e.directory, idle) {
                                    destination = Some(if e.directory {
                                        Destination::Folder(e.path.clone())
                                    } else {
                                        Destination::File(e.path.clone())
                                    });
                                }
                            }
                        });
                }
                View::Largest => {
                    ui.weak("Up to 50 files by logical size. Select a file to look up its users.");
                    egui::ScrollArea::vertical()
                        .id_salt("largest_files")
                        .show(ui, |ui| {
                            for (bytes, path) in &scan.largest {
                                if item_row(ui, path, *bytes, false, idle) {
                                    destination = Some(Destination::File(path.clone()));
                                }
                            }
                        });
                }
                View::Projects => {
                    ui.weak("Generated folders in recognized Unity, Rust and Node projects. Select one to explore.");
                    if scan.projects.is_empty() {
                        ui.label("No matching project and generated folder pairs.");
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("project_caches")
                        .show_rows(ui, 24.0, scan.projects.len(), |ui, range| {
                            for p in &scan.projects[range] {
                                ui.horizontal(|ui| {
                                    ui.monospace(format::bytes(p.bytes));
                                    ui.strong(p.kind);
                                    let label = format!(
                                        "{} / {}",
                                        filename(&p.root),
                                        filename(&p.generated)
                                    );
                                    if ui
                                        .add_enabled(idle, egui::Button::new(label))
                                        .on_hover_text(display(&p.generated))
                                        .clicked()
                                    {
                                        destination =
                                            Some(Destination::Folder(p.generated.clone()));
                                    }
                                });
                            }
                        });
                }
                View::Types => {
                    egui::ScrollArea::vertical()
                        .id_salt("file_types")
                        .show_rows(ui, 22.0, scan.types.len(), |ui, range| {
                            for (ext, (bytes, files)) in &scan.types[range] {
                                ui.horizontal(|ui| {
                                    ui.monospace(format::bytes(*bytes));
                                    ui.strong(if ext.is_empty() {
                                        "(no extension)".to_owned()
                                    } else {
                                        format!(".{ext}")
                                    });
                                    ui.label(format!("{files} files"));
                                });
                            }
                        });
                }
                View::Duplicates => {
                    ui.label("Compare files of at least 1 MiB by content. Hard-linked files are excluded on Windows.");
                    if ui
                        .add_enabled(idle, egui::Button::new("Check for duplicates"))
                        .clicked()
                    {
                        destination = Some(Destination::CheckDuplicates(scan.clone()));
                    }
                    if let Some(result) = &self.duplicates {
                        ui.strong(format!(
                            "{} matching groups   ·   {} files skipped",
                            result.groups.len(),
                            result.skipped
                        ));
                        egui::ScrollArea::vertical()
                            .id_salt("duplicate_groups")
                            .show(ui, |ui| {
                                for (bytes, paths) in &result.groups {
                                    ui.collapsing(
                                        format!(
                                            "{}   ·   {} copies   ·   {}",
                                            format::bytes(*bytes),
                                            paths.len(),
                                            display(&paths[0])
                                        ),
                                        |ui| {
                                            for path in paths {
                                                ui.push_id(path, |ui| {
                                                    ui.horizontal_wrapped(|ui| {
                                                        if ui
                                                            .add_enabled(
                                                                idle,
                                                                egui::Button::selectable(
                                                                    false,
                                                                    display(path),
                                                                ),
                                                            )
                                                            .clicked()
                                                        {
                                                            destination = Some(Destination::File(
                                                                path.clone(),
                                                            ));
                                                        }
                                                        #[cfg(windows)]
                                                        if ui
                                                            .add_enabled(
                                                                idle,
                                                                egui::Button::new("Remove copy…"),
                                                            )
                                                            .clicked()
                                                            && let Some(keep) = paths
                                                                .iter()
                                                                .find(|other| *other != path)
                                                        {
                                                            destination =
                                                                Some(Destination::ReviewCopy(
                                                                    keep.clone(),
                                                                    path.clone(),
                                                                    *bytes,
                                                                ));
                                                        }
                                                    })
                                                });
                                            }
                                        },
                                    );
                                }
                            });
                    }
                }
                View::Changes => match &self.deltas {
                    None => {
                        ui.label("Scan this folder again to compare it with this result.");
                    }
                    Some(rows) => {
                        ui.label("Change since the previous scan of this folder.");
                        if self.partial_comparison {
                            ui.weak("Either scan had unreadable entries. Differences may reflect access changes.");
                        }
                        if rows.is_empty() {
                            ui.strong("No size changes.");
                        }
                        egui::ScrollArea::vertical()
                            .id_salt("disk_deltas")
                            .show_rows(ui, 21.0, rows.len(), |ui, range| {
                                for (path, n) in &rows[range] {
                                    ui.horizontal(|ui| {
                                        ui.monospace(format!(
                                            "{}{}",
                                            if *n > 0 { "+" } else { "-" },
                                            format::bytes(n.unsigned_abs() as u64)
                                        ));
                                        ui.label(filename(path)).on_hover_text(display(path));
                                    });
                                }
                            });
                    }
                },
            }
        } else if idle {
            ui.add_space(theme::space::M);
            ui.label("Select a drive or enter a folder to map its contents.");
        }
        match destination {
            #[cfg(windows)]
            Some(Destination::ReviewCopy(keep, remove, bytes)) => self.start(
                ui.ctx(),
                "Preparing duplicate review",
                false,
                move |_, _| {
                    cleaner::CopyReview::capture(keep, remove, bytes).map(Output::CopyReview)
                },
            ),
            Some(Destination::Folder(path)) => self.open_folder(ui.ctx(), path),
            Some(Destination::File(path)) => self.lookup(ui.ctx(), path),
            Some(Destination::CheckDuplicates(scan)) => {
                self.duplicates = None;
                self.start(
                    ui.ctx(),
                    "Comparing file contents",
                    true,
                    move |stop, count| duplicates::find(&scan, stop, count).map(Output::Duplicates),
                );
            }
            None => {}
        }
    }

    pub(super) fn cleaner_view(&mut self, ui: &mut egui::Ui, t: Tokens) {
        widgets::section_header(ui, "Custom cleanup", None, t);
        ui.label("Temporary files, browser and desktop app caches, and NuGet downloads qualify after 7 days. Crash dumps, diagnostic logs and supported developer downloads qualify after 30 days.");
        ui.horizontal_wrapped(|ui| {
            let idle = self.job.is_none() && self.review.is_none();
            if ui.add_enabled(idle, egui::Button::new("Preview cleanup")).clicked() {
                self.preview = None;
                self.receipt = None;
                self.start(ui.ctx(), "Checking cleanup candidates", true, |cancel, checked| {
                    cleaner::preview(cancel, checked).map(Output::Preview)
                });
            }
            #[cfg(windows)]
            if self.review.is_none() && ui.button("Windows Disk Cleanup…")
                .on_hover_text("Open the Windows drive cleanup dialog for system files and the Recycle Bin.")
                .clicked()
            {
                let launch = cleaner::disk_cleanup_program().and_then(|program| {
                    std::process::Command::new(program).spawn()
                });
                if let Err(e) = launch {
                    self.error = Some(format!("Could not open Windows Disk Cleanup: {e}"));
                }
            }
        });
        if let Some(receipt) = &self.receipt {
            ui.strong(format!(
                "Deleted {} files ({})  ·  {} skipped",
                receipt.deleted,
                format::bytes(receipt.bytes),
                receipt.failures.len()
            ));
            egui::ScrollArea::vertical()
                .id_salt("cleanup_receipt")
                .show_rows(ui, 23.0, receipt.failures.len(), |ui, rows| {
                    for message in &receipt.failures[rows] {
                        ui.add(egui::Label::new(message).truncate())
                            .on_hover_text(message);
                    }
                });
        }
        let Some(preview) = &self.preview else {
            return;
        };
        if !preview.issues.is_empty() {
            ui.collapsing(
                format!("{} folder access issues", preview.issues.len()),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("cleanup_issues")
                        .max_height(82.0)
                        .show_rows(ui, 23.0, preview.issues.len(), |ui, rows| {
                            for issue in &preview.issues[rows] {
                                ui.add(egui::Label::new(issue).truncate())
                                    .on_hover_text(issue);
                            }
                        });
                },
            );
        }
        ui.label(format!(
            "{} in {} eligible files  ·  {} inaccessible or protected",
            format::bytes(preview.bytes),
            preview.files.len(),
            preview.skipped
        ));
        if preview.files.is_empty() {
            return;
        }
        let selected_bytes = self
            .selected
            .iter()
            .map(|i| preview.files[*i].bytes())
            .sum();
        let summary = format!(
            "{} {} ({})",
            self.selected.len(),
            if self.selected.len() == 1 {
                "file"
            } else {
                "files"
            },
            format::bytes(selected_bytes)
        );
        if self.review.is_none() {
            egui::ComboBox::from_id_salt("cleanup_category")
                .selected_text(self.category.map_or("All categories", cleaner::Rule::label))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.category, None, "All categories");
                    for group in &preview.groups {
                        ui.selectable_value(
                            &mut self.category,
                            Some(group.rule),
                            format!(
                                "{}   {} {}   {}",
                                group.rule.label(),
                                group.range.len(),
                                if group.range.len() == 1 {
                                    "file"
                                } else {
                                    "files"
                                },
                                format::bytes(group.bytes)
                            ),
                        );
                    }
                });
        }
        let visible = preview
            .groups
            .iter()
            .find(|g| Some(g.rule) == self.category)
            .map_or(0..preview.files.len(), |g| g.range.clone());
        let mut confirmed = false;
        if self.review.is_some() {
            ui.strong(format!("Permanently delete {summary}?"));
            ui.label("This bypasses the Recycle Bin. Changed and busy files are skipped.");
            ui.horizontal(|ui| {
                if ui.button("Keep files").clicked() {
                    self.review = None;
                }
                confirmed = ui.button("Permanently delete selected").clicked();
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                let label = if self.category.is_some() {
                    "Select category"
                } else {
                    "Select all"
                };
                if ui.button(label).clicked() {
                    self.selected.extend(visible.clone());
                }
                if ui
                    .button("Clear")
                    .on_hover_text("Clear the visible selection")
                    .clicked()
                {
                    self.selected.retain(|i| !visible.contains(i));
                }
                if ui
                    .add_enabled(
                        !self.selected.is_empty() && self.job.is_none(),
                        egui::Button::new(format!("Review {summary}")),
                    )
                    .clicked()
                {
                    let mut frozen: Vec<_> = self.selected.iter().copied().collect();
                    frozen.sort_unstable();
                    self.review = Some(frozen);
                }
            });
        }
        if let Some(review) = &self.review {
            if review
                .iter()
                .any(|&i| preview.files[i].rule.owner_exe().is_some())
            {
                ui.label("Close the selected apps before cleaning their caches.");
            }
            egui::ScrollArea::vertical()
                .id_salt("review_files")
                .show_rows(ui, 23.0, review.len(), |ui, range| {
                    for &i in &review[range] {
                        let f = &preview.files[i];
                        ui.add(
                            egui::Label::new(format!(
                                "{}   {}",
                                format::bytes(f.bytes()),
                                display(&f.path)
                            ))
                            .truncate(),
                        )
                        .on_hover_text(display(&f.path));
                    }
                });
        } else {
            egui::ScrollArea::vertical()
                .id_salt(("cleanup_files", visible.start, visible.end))
                .show_rows(ui, 23.0, visible.len(), |ui, range| {
                    for offset in range {
                        let i = visible.start + offset;
                        let file = &preview.files[i];
                        let mut checked = self.selected.contains(&i);
                        let label = format!(
                            "{}   {}   {}",
                            format::bytes(file.bytes()),
                            file.rule.label(),
                            filename(&file.path)
                        );
                        if ui
                            .push_id(i, |ui| {
                                ui.checkbox(&mut checked, label)
                                    .on_hover_text(display(&file.path))
                                    .changed()
                            })
                            .inner
                        {
                            if checked {
                                self.selected.insert(i);
                            } else {
                                self.selected.remove(&i);
                            }
                        }
                    }
                });
        }
        if confirmed {
            let files = self
                .review
                .take()
                .unwrap_or_default()
                .into_iter()
                .map(|i| preview.files[i].clone())
                .collect();
            self.selected.clear();
            self.preview = None;
            self.review = None;
            self.start(ui.ctx(), "Cleaning selected files", false, move |_, _| {
                Ok(Output::Cleaned(cleaner::apply(files)))
            });
        }
    }

    pub(super) fn drive_view(&mut self, ui: &mut egui::Ui, disks: &[DiskRow], t: Tokens) {
        widgets::section_header(ui, "Mounted volumes", None, t);
        let mut target = None;
        egui::ScrollArea::vertical()
            .id_salt("drive_watch")
            .show(ui, |ui| {
                for disk in disks {
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(self.job.is_none(), egui::Button::new(&disk.mount))
                            .clicked()
                        {
                            target = Some(PathBuf::from(&disk.mount));
                        }
                        ui.strong(&disk.name);
                        ui.label(format!(
                            "{} free of {}   {}",
                            format::bytes(disk.available_bytes),
                            format::bytes(disk.total_bytes),
                            disk.file_system
                        ));
                    });
                }
                if disks.is_empty() {
                    ui.label("Waiting for a mounted volume.");
                }
                ui.add_space(theme::space::M);
                widgets::section_header(ui, "Mount history", None, t);
                if self.events.is_empty() {
                    ui.label("No mount changes observed this session.");
                }
                for event in &self.events {
                    let seconds = event
                        .time
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        % 86400;
                    ui.horizontal(|ui| {
                        ui.monospace(format!(
                            "{:02}:{:02}:{:02} UTC",
                            seconds / 3600,
                            seconds / 60 % 60,
                            seconds % 60
                        ));
                        ui.strong(if event.added { "Mounted" } else { "Removed" });
                        ui.label(format!("{}   {}", event.mount, event.name));
                    });
                }
            });
        if let Some(path) = target {
            self.open_folder(ui.ctx(), path);
        }
    }

    pub(super) fn users_view(
        &mut self,
        ui: &mut egui::Ui,
        processes: &[ProcessRow],
        t: Tokens,
    ) -> Option<ProcessIdentity> {
        widgets::section_header(ui, "Who is using this file?", None, t);
        let mut inspect = None;
        ui.add_enabled_ui(self.job.is_none(), |ui| {
            ui.horizontal_wrapped(|ui| {
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.file)
                        .desired_width((ui.available_width() - 150.0).clamp(90.0, 520.0))
                        .hint_text("File path"),
                );
                if ui.button("Find processes").clicked()
                    || (edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    self.lookup(ui.ctx(), self.file.trim().trim_matches('"').into());
                }
            });
        });
        if let Some((path, users)) = &self.users {
            ui.label(display(path));
            if users.is_empty() {
                ui.label("Windows Restart Manager reported no applications using this file.");
            }
            egui::ScrollArea::vertical()
                .id_salt("file_users")
                .show(ui, |ui| {
                    for user in users {
                        ui.horizontal(|ui| {
                            ui.monospace(user.identity.pid.to_string());
                            ui.strong(&user.name);
                            let live = processes
                                .iter()
                                .any(|p| p.identity() == Some(user.identity));
                            if ui
                                .add_enabled(live, egui::Button::new("Inspect process"))
                                .clicked()
                            {
                                inspect = Some(user.identity);
                            }
                        });
                    }
                });
        }
        inspect
    }
}

fn item_row(ui: &mut egui::Ui, path: &Path, bytes: u64, dir: bool, enabled: bool) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized(
            [85.0, 20.0],
            egui::Label::new(RichText::new(format::bytes(bytes)).monospace()),
        );
        let label = if dir {
            format!("{}/", filename(path))
        } else {
            filename(path)
        };
        ui.add_enabled(enabled, egui::Button::selectable(false, label))
            .on_hover_text(display(path))
            .clicked()
    })
    .inner
}

fn draw_map(ui: &mut egui::Ui, scan: &scan::Scan, t: Tokens, enabled: bool) -> Option<Destination> {
    let width = ui.available_width().max(1.0);
    let height = (ui.available_height() * 0.52).clamp(70.0, 270.0);
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    if scan.bytes == 0 {
        ui.painter().text(
            area.center(),
            egui::Align2::CENTER_CENTER,
            "No file bytes to map",
            FontId::proportional(14.0),
            t.text_muted,
        );
        return None;
    }
    let mut tiles: Vec<_> = scan
        .entries
        .iter()
        .take(36)
        .take_while(|e| e.bytes > 0)
        .map(|e| (e.bytes, Some(e)))
        .collect();
    let rest = scan.bytes.saturating_sub(tiles.iter().map(|x| x.0).sum());
    if rest > 0 {
        tiles.push((rest, None));
        tiles.sort_by_key(|v| std::cmp::Reverse(v.0));
    }
    let sizes: Vec<_> = tiles.iter().map(|(n, _)| *n as f64).collect();
    let mut destination = None;
    for r in treemap::layout_norm(height / width, &sizes) {
        let (bytes, entry) = tiles[r.index];
        let rect = egui::Rect::from_min_size(
            area.min + egui::vec2(r.x, r.y) * width,
            egui::vec2(r.w, r.h) * width,
        )
        .intersect(area)
        .shrink(1.0);
        if !rect.is_positive() {
            continue;
        }
        let label = entry.map_or_else(|| "Other entries".into(), |e| filename(&e.path));
        let color = theme::mix(t.accent, t.secondary, ((r.index * 7) % 13) as f32 / 12.0);
        let fill = theme::mix(t.panel_raised, color, if t.dark { 0.54 } else { 0.28 });
        let response = ui.interact(
            rect,
            ui.id().with(("map_tile", r.index)),
            if enabled && entry.is_some() {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        ui.painter().rect_filled(rect, 2.0, fill);
        if response.hovered() {
            ui.painter().rect_stroke(
                rect,
                2.0,
                Stroke::new(1.5, t.text),
                egui::StrokeKind::Inside,
            );
        }
        if rect.width() > 54.0 && rect.height() > 21.0 {
            let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
            painter.text(
                rect.min + egui::vec2(6.0, 4.0),
                egui::Align2::LEFT_TOP,
                &label,
                FontId::proportional(14.0),
                theme::readable_text(t.text, fill),
            );
            if rect.height() > 42.0 {
                painter.text(
                    rect.min + egui::vec2(6.0, 24.0),
                    egui::Align2::LEFT_TOP,
                    format::bytes(bytes),
                    FontId::monospace(11.0),
                    theme::readable_text(t.text, fill),
                );
            }
        }
        if response
            .on_hover_text(format!("{label}   {}", format::bytes(bytes)))
            .clicked()
            && let Some(e) = entry
        {
            destination = Some(if e.directory {
                Destination::Folder(e.path.clone())
            } else {
                Destination::File(e.path.clone())
            });
        }
    }
    destination
}
