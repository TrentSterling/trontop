use super::*;

impl Toolbox {
    pub(in crate::app::toolbox) fn open_destination(
        &mut self,
        ctx: &egui::Context,
        d: Destination,
    ) {
        self.start(ctx, "Opening Windows tool", false, move |_, _| {
            d.open().map(Output::Notice)
        });
    }
    fn destinations(&mut self, ui: &mut egui::Ui, actions: &[(Destination, &str)]) {
        ui.horizontal_wrapped(|ui| {
            for &(d, label) in actions {
                if ui
                    .add_enabled(!self.busy(), egui::Button::new(label))
                    .clicked()
                {
                    self.open_destination(ui.ctx(), d);
                }
            }
        });
    }
    pub(in crate::app::toolbox) fn maintenance_view(
        &mut self,
        ui: &mut egui::Ui,
        disks: &[DiskRow],
        processes: &[crate::model::ProcessRow],
        controls: &crate::process_actions::Controller,
    ) {
        ui.horizontal_wrapped(|ui| {
            for (section, title) in [
                (Section::Health, "Health check"),
                (Section::Updates, "Updates"),
                (Section::Browsers, "Browsers"),
                (Section::Registry, "Registry"),
                (Section::Recovery, "Restore points"),
                (Section::Storage, "Drive tools"),
                (Section::Schedule, "Schedule"),
                (Section::Performance, "Performance"),
                (Section::Drivers, "Drivers"),
                (Section::Cloud, "Cloud storage"),
            ] {
                if ui
                    .selectable_label(self.maintenance.section == section, title)
                    .clicked()
                    && self.maintenance.section != section
                {
                    self.maintenance.clear_review();
                    self.schedule.review = None;
                    self.maintenance.section = section;
                }
            }
        });
        ui.separator();
        let section = self.maintenance.section;
        egui::ScrollArea::vertical()
            .id_salt(("maintenance", section as u8))
            .show(ui, |ui| match section {
                Section::Health => self.health_view(ui, disks),
                Section::Browsers => self.browsers_view(ui),
                Section::Updates => self.updates_view(ui),
                Section::Registry => self.registry_view(ui),
                Section::Recovery => self.recovery_view(ui),
                Section::Storage => self.storage_view(ui, disks),
                Section::Schedule => self.schedule_view(ui),
                Section::Performance => self.performance_view(ui, processes, controls),
                Section::Drivers => self.drivers_view(ui),
                Section::Cloud => self.cloud_view(ui),
            });
    }
    fn performance_view(
        &mut self,
        ui: &mut egui::Ui,
        processes: &[crate::model::ProcessRow],
        controls: &crate::process_actions::Controller,
    ) {
        let mut stop = None;
        for entry in &self.sleep_rules.entries {
            ui.push_id(("sleep", entry.id.pid), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&entry.name);
                    ui.label(format!("PID {} · {}", entry.id.pid, entry.status.label()));
                    if ui
                        .add_enabled(
                            entry.status != crate::platform::minimized::Status::Stopping,
                            egui::Button::new("Wake and remove rule"),
                        )
                        .clicked()
                    {
                        stop = Some(entry.id);
                    }
                })
            });
        }
        if let Some(id) = stop {
            self.sleep_rules.remove(id);
        }
        if let Some(error) = &self.sleep_rules.error {
            ui.label(error);
        }
        if let Some((id, name)) = self.maintenance.sleep_review.clone() {
            ui.strong(format!("Sleep {name} (PID {}) when minimized?", id.pid));
            ui.label("After all its windows stay minimized for 30 seconds, this process pauses, including background work. Restoring a window, removing the rule or exiting Trontop releases the pause. The rule expires when this process exits.");
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    self.maintenance.sleep_review = None;
                }
                let current = processes.iter().any(|p| p.identity() == Some(id));
                if ui
                    .add_enabled(
                        current && controls.ready() && !controls.holds_suspension(id),
                        egui::Button::new("Enable sleep rule"),
                    )
                    .clicked()
                {
                    self.maintenance.sleep_review = None;
                    if let Err(e) = self.sleep_rules.arm(ui.ctx(), id, name) {
                        self.sleep_rules.error = Some(e);
                    }
                }
            });
            return;
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.maintenance.process_filter)
                .hint_text("Filter processes by name or PID"),
        );
        ui.checkbox(&mut self.maintenance.memory_first, "Rank by memory");
        let filter = self.maintenance.process_filter.trim().to_lowercase();
        let mut rows: Vec<_> = processes
            .iter()
            .filter(|p| p.identity().is_some())
            .filter(|p| {
                filter.is_empty()
                    || p.name.to_lowercase().contains(&filter)
                    || p.pid.to_string() == filter
            })
            .collect();
        if self.maintenance.memory_first {
            rows.sort_by_key(|p| std::cmp::Reverse(p.memory_bytes));
        } else {
            rows.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
        }
        for p in rows.into_iter().take(20) {
            ui.push_id(p.pid, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button(&p.name).clicked() {
                        self.maintenance.inspect = p.identity();
                    }
                    ui.monospace(format!(
                        "PID {}    {:.1}% CPU    {}",
                        p.pid,
                        p.cpu_percent,
                        format::bytes(p.memory_bytes)
                    ));
                    let id = p.identity().unwrap();
                    let enabled = controls.ready()
                        && !controls.holds_suspension(id)
                        && crate::platform::can_control(id.pid).is_ok()
                        && !self.sleep_rules.contains(id);
                    if ui
                        .add_enabled(enabled, egui::Button::new("Sleep when minimized"))
                        .clicked()
                    {
                        self.maintenance.sleep_review = Some((id, p.name.clone()));
                    }
                })
            });
        }
    }
    fn drivers_view(&mut self, ui: &mut egui::Ui) {
        self.destinations(
            ui,
            &[(Destination::Drivers, "Review and install optional drivers")],
        );
        ui.horizontal_wrapped(|ui|{
            if ui.add_enabled(!self.busy(),egui::Button::new("List installed drivers")).clicked(){
                self.start(ui.ctx(),"Reading signed driver inventory",false,|_,_|{
                    let rows=wmi::query_once(r"ROOT\CIMV2","SELECT DeviceName, DriverVersion, DriverProviderName, IsSigned FROM Win32_PnPSignedDriver",Duration::from_secs(20)).map_err(io::Error::other)?;
                    let mut lines:Vec<_>=rows.iter().filter_map(|r|Some(format!("{}   {}   {}   {}",r.text("DeviceName")?,
                        r.text("DriverVersion").unwrap_or_default(),r.text("DriverProviderName").unwrap_or_default(),
                        match r.bool("IsSigned"){Some(true)=>"signed",Some(false)=>"unsigned",None=>"signature unknown"}))).collect();
                    lines.sort();
                    Ok(Output::Report(Section::Drivers,format!("{} device driver records\n{}",lines.len(),lines.join("\n"))))
                });
            }
            if ui.add_enabled(!self.busy(),egui::Button::new("Check Windows Update for drivers")).clicked(){
                self.start(ui.ctx(),"Querying Windows Update for drivers",false,|_,_|{
                    let result=powershell(r#"$s=New-Object -ComObject Microsoft.Update.Session
$r=$s.CreateUpdateSearcher().Search("IsInstalled=0 and Type='Driver' and IsHidden=0")
if ($r.ResultCode -ne 2) {throw "Windows Update returned result $($r.ResultCode)"}
"$($r.Updates.Count) driver updates offered by Windows Update"
foreach ($u in $r.Updates) { $u.Title }"#,None)?;
                    Ok(Output::Report(Section::Drivers,result))
                });
            }
        });
        if let Some((Section::Drivers, s)) = &self.maintenance.report {
            if ui.button("Copy driver report").clicked() {
                ui.ctx().copy_text(s.clone());
            }
            ui.monospace(s);
        }
    }
    fn cloud_view(&mut self, ui: &mut egui::Ui) {
        ui.label("Review online storage in your provider account, or map a synced folder locally in SpaceView.");
        self.destinations(
            ui,
            &[
                (
                    Destination::GoogleStorage,
                    "Google Drive, Gmail and Photos storage",
                ),
                (Destination::OneDriveStorage, "OneDrive storage"),
            ],
        );
        let mut seen = HashSet::new();
        for name in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
            if let Some(path) = std::env::var_os(name)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                && seen.insert(path.clone())
                && ui
                    .add_enabled(
                        !self.busy(),
                        egui::Button::new(format!("Map {}", path.display())),
                    )
                    .clicked()
            {
                self.open_folder(ui.ctx(), path);
            }
        }
        if ui
            .add_enabled(
                !self.busy(),
                egui::Button::new("Choose a local folder in SpaceView"),
            )
            .clicked()
        {
            self.tool = Tool::SpaceView;
        }
    }
    fn health_view(&mut self, ui: &mut egui::Ui, disks: &[DiskRow]) {
        let idle = !self.busy();
        if ui
            .add_enabled(idle, egui::Button::new("Run health check"))
            .clicked()
        {
            self.start(
                ui.ctx(),
                "Checking storage and installed apps",
                true,
                |stop, count| health(stop, count).map(Output::Health),
            );
        }
        if let Some(h) = &self.maintenance.health {
            ui.strong(format!(
                "{} in {} eligible cleaner files",
                format::bytes(h.bytes),
                h.files
            ));
            ui.label(format!(
                "{} skipped files or inaccessible locations",
                h.excluded
            ));
            ui.label(format!(
                "{} desktop registrations · {} unreadable",
                h.apps, h.unreadable
            ));
            match h.bin {
                Some((n, b)) => ui.label(format!("Recycle Bin · {n} items · {}", format::bytes(b))),
                None => ui.weak("Recycle Bin totals unavailable"),
            };
        }
        for disk in disks {
            ui.label(format!(
                "{}   {} free of {}",
                disk.mount,
                format::bytes(disk.available_bytes),
                format::bytes(disk.total_bytes)
            ));
        }
        if ui
            .add_enabled(idle, egui::Button::new("Review eligible files"))
            .clicked()
        {
            self.tool = Tool::Cleaner;
            self.start(ui.ctx(), "Previewing cleanup", true, |s, c| {
                cleaner::preview(s, c).map(Output::Preview)
            });
        }
        self.destinations(
            ui,
            &[
                (Destination::Bin, "Open Recycle Bin"),
                (Destination::Schedule, "Schedule with Storage Sense"),
            ],
        );
    }

    fn browsers_view(&mut self, ui: &mut egui::Ui) {
        let idle = !self.busy();
        if ui
            .add_enabled(idle, egui::Button::new("Scan installed extensions"))
            .clicked()
        {
            self.start(
                ui.ctx(),
                "Reading browser extension manifests",
                false,
                |_, _| extensions::inventory().map(|s| Output::Report(Section::Browsers, s)),
            );
        }
        ui.label("Choose a browser to manage history, cookies, saved data or extensions in its own profile settings.");
        for browser in [
            Browser::Chrome,
            Browser::Edge,
            Browser::Brave,
            Browser::Firefox,
            Browser::Vivaldi,
        ] {
            ui.push_id(browser.name(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(browser.name());
                    for (title, ext) in [("Privacy settings", false), ("Manage extensions", true)] {
                        if ui.add_enabled(idle, egui::Button::new(title)).clicked() {
                            self.start(ui.ctx(), "Opening browser settings", false, move |_, _| {
                                browser.open(ext).map(Output::Notice)
                            });
                        }
                    }
                })
            });
        }
        if let Some((Section::Browsers, s)) = &self.maintenance.report {
            if ui.button("Copy extension list").clicked() {
                ui.ctx().copy_text(s.clone());
            }
            ui.monospace(s);
        }
    }

    fn updates_view(&mut self, ui: &mut egui::Ui) {
        let idle = !self.busy();
        self.destinations(
            ui,
            &[
                (Destination::Drivers, "Optional driver updates"),
                (Destination::Store, "Store app updates"),
            ],
        );
        if ui
            .add_enabled(idle, egui::Button::new("Check WinGet updates"))
            .on_hover_text("Queries the Windows Package Manager community source")
            .clicked()
        {
            self.start(ui.ctx(), "Querying WinGet", false, |_, _| {
                available_updates().map(|s| Output::Report(Section::Updates, s))
            });
        }
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.maintenance.package_id)
                    .hint_text("Exact WinGet package ID"),
            );
            let id = self.maintenance.package_id.trim();
            if ui
                .add_enabled(idle && valid_id(id), egui::Button::new("Review update"))
                .clicked()
            {
                self.maintenance.update_review = Some(id.into());
            }
        });
        if let Some(id) = self.maintenance.update_review.clone() {
            ui.label(format!(
                "Run the interactive upgrade for {id} from the winget source?"
            ));
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    self.maintenance.update_review = None;
                }
                if ui
                    .add_enabled(idle, egui::Button::new("Update this package"))
                    .clicked()
                {
                    self.maintenance.update_review = None;
                    self.start(ui.ctx(), "Waiting for WinGet", false, move |_, _| {
                        upgrade(&id).map(Output::Notice)
                    });
                }
            });
        }
        if let Some((Section::Updates, s)) = &self.maintenance.report {
            if ui.button("Copy WinGet report").clicked() {
                ui.ctx().copy_text(s.clone());
            }
            egui::ScrollArea::horizontal().show(ui, |ui| ui.monospace(s));
        }
    }

    fn registry_view(&mut self, ui: &mut egui::Ui) {
        let idle = !self.busy();
        ui.label("Check current-user App Paths for missing executables on mounted fixed drives. Archiving renames the registration and preserves its values for restore.");
        if ui
            .add_enabled(idle, egui::Button::new("Scan app paths"))
            .clicked()
        {
            self.maintenance.registry_review = None;
            self.start(ui.ctx(), "Inspecting App Paths", false, |_, _| {
                registry::scan().map(Output::Registry)
            });
        }
        if let Some(e) = self.maintenance.registry_review.clone() {
            ui.strong(format!(
                "{} {}?",
                if e.archived { "Restore" } else { "Archive" },
                e.key
            ));
            ui.monospace(&e.target);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    self.maintenance.registry_review = None;
                }
                if ui
                    .add_enabled(idle, egui::Button::new("Confirm registry change"))
                    .clicked()
                {
                    self.maintenance.registry_review = None;
                    self.start(ui.ctx(), "Updating App Paths", false, move |_, _| {
                        registry::toggle(&e).map(Output::Registry)
                    });
                }
            });
        } else if let Some(rows) = &self.maintenance.registry {
            ui.label(format!("{} stale or archived registrations", rows.len()));
            for row in rows {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            idle,
                            egui::Button::new(if row.archived { "Restore" } else { "Archive" }),
                        )
                        .clicked()
                    {
                        self.maintenance.registry_review = Some(row.clone());
                    }
                    ui.label(&row.key).on_hover_text(&row.target);
                });
            }
        }
    }

    fn recovery_view(&mut self, ui: &mut egui::Ui) {
        let idle = !self.busy();
        self.destinations(
            ui,
            &[
                (
                    Destination::Protection,
                    "Create or configure restore points",
                ),
                (Destination::Restore, "Open recovery wizard"),
            ],
        );
        if ui
            .add_enabled(idle, egui::Button::new("List restore points"))
            .clicked()
        {
            self.maintenance.point_review = None;
            self.start(ui.ctx(), "Reading restore points", false, |_, _| {
                restore_points().map(Output::Points)
            });
        }
        if let Some(p) = self.maintenance.point_review.clone() {
            ui.strong(format!(
                "Permanently remove restore point {} ({})?",
                p.number, p.description
            ));
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    self.maintenance.point_review = None;
                }
                if ui
                    .add_enabled(idle, egui::Button::new("Remove restore point"))
                    .clicked()
                {
                    self.maintenance.point_review = None;
                    self.start(
                        ui.ctx(),
                        "Removing selected restore point",
                        false,
                        move |_, _| remove_point(&p).map(Output::Points),
                    );
                }
            });
        } else if let Some(points) = &self.maintenance.points {
            ui.label(format!("{} restore points", points.len()));
            for (i, p) in points.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(idle && i > 0, egui::Button::new("Remove"))
                        .clicked()
                    {
                        self.maintenance.point_review = Some(p.clone());
                    }
                    ui.label(format!(
                        "#{}   {}   {}",
                        p.number,
                        wmi::dmtf_date(&p.time).unwrap_or_else(|| p.time.clone()),
                        p.description
                    ));
                });
            }
        }
    }

    fn storage_view(&mut self, ui: &mut egui::Ui, disks: &[DiskRow]) {
        let idle = !self.busy();
        self.destinations(
            ui,
            &[
                (Destination::Optimize, "Optimize drives"),
                (Destination::Disks, "Disk Management"),
                (Destination::Bin, "Open Recycle Bin"),
            ],
        );
        ui.separator();
        ui.strong("Wipe free space");
        ui.label("Windows Cipher fills unallocated space on the selected NTFS volume. This can temporarily use all free space. SSD remapping limits physical erasure.");
        if let Some(v) = &self.maintenance.wipe {
            let phrase = format!(
                "{} {} {:08X}",
                if v.entire { "ERASE" } else { "WIPE" },
                v.root,
                v.serial
            );
            let confirmation = if v.entire {
                "Open full-volume format"
            } else {
                "Run free-space wipe"
            };
            if v.entire {
                ui.strong("All files on this volume will be permanently lost. Windows Format will ask you to confirm.");
            }
            ui.label(format!(
                "{} · {} · serial {:08X}. Type {phrase} to proceed.",
                v.root, v.label, v.serial
            ));
            ui.text_edit_singleline(&mut self.maintenance.typed);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    self.maintenance.clear_review();
                }
                if ui
                    .add_enabled(
                        idle && self.maintenance.typed == phrase,
                        egui::Button::new(confirmation),
                    )
                    .clicked()
                {
                    let volume = self.maintenance.wipe.take().unwrap();
                    self.maintenance.typed.clear();
                    self.start(
                        ui.ctx(),
                        "Waiting for Windows drive tool",
                        false,
                        move |_, _| wipe(&volume).map(Output::Notice),
                    );
                }
            });
        } else {
            for disk in disks.iter().filter(|d| {
                d.file_system.eq_ignore_ascii_case("NTFS") && drive_root(&d.mount).is_some()
            }) {
                if ui
                    .add_enabled(
                        idle,
                        egui::Button::new(format!(
                            "Review {} · {} free",
                            disk.mount,
                            format::bytes(disk.available_bytes)
                        )),
                    )
                    .clicked()
                {
                    let mount = disk.mount.clone();
                    self.start(ui.ctx(), "Identifying volume", false, move |_, _| {
                        volume(&mount, false).map(Output::Wipe)
                    });
                }
            }
            ui.separator();
            ui.collapsing("Erase an entire data volume",|ui|{
                ui.label("Windows Format overwrites the selected logical volume with one zero pass. Boot, system, paging, profile and Trontop volumes are excluded.");
                for disk in disks.iter().filter(|d|drive_root(&d.mount).is_some()&&matches!(d.file_system.to_ascii_uppercase().as_str(),"NTFS"|"EXFAT"|"FAT32")){
                    if ui.add_enabled(idle,egui::Button::new(format!("Review full erase of {} · {}",disk.mount,format::bytes(disk.total_bytes)))).clicked(){
                        let mount=disk.mount.clone();
                        self.start(ui.ctx(),"Checking data volume identity and Windows roles",false,move|_,_|volume(&mount,true).map(Output::Wipe));
                    }
                }
            });
        }
    }
}
