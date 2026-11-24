use super::*;
use crate::specs::native::registry::{self, Hive};
use std::os::windows::process::CommandExt;
use std::process::Command;
mod packages;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct App {
    hive: Hive,
    packaged: bool,
    key: String,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub size: Option<u64>,
    pub location: Option<PathBuf>,
    pub uninstall: Option<Launch>,
    pub maintenance: Option<Launch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Launch {
    pub label: &'static str,
    pub exe: PathBuf,
    pub arguments: String,
}

impl App {
    pub fn scope(&self) -> &'static str {
        match self.hive {
            Hive::CurrentUser => "Current user",
            Hive::LocalMachine => "All users",
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(query) || self.publisher.to_lowercase().contains(query)
    }
}

fn local_path(text: &str) -> Option<PathBuf> {
    let b = text.as_bytes();
    (b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && b[2] == b'\\'
        && !text.chars().any(|c| c.is_control() || c == '"'))
    .then(|| PathBuf::from(text))
}

// Keep vendor argument quoting intact. Never dispatch a registration through a shell or PATH.
fn parse_command(label: &'static str, text: &str) -> Option<Launch> {
    let text = text.trim();
    let (program, arguments) = if let Some(quoted) = text.strip_prefix('"') {
        quoted.split_once('"')?
    } else {
        let lower = text.to_ascii_lowercase();
        let end = lower
            .match_indices(".exe")
            .map(|(idx, _)| idx + 4)
            .find(|&end| text[end..].is_empty() || text[end..].starts_with(char::is_whitespace))?;
        text.split_at(end)
    };
    if !arguments.is_empty() && !arguments.starts_with(char::is_whitespace) {
        return None;
    }
    let exe = local_path(program)?;
    let filename = exe.file_name()?.to_str()?.to_ascii_lowercase();
    if !filename.ends_with(".exe")
        || matches!(
            filename.as_str(),
            "cmd.exe"
                | "powershell.exe"
                | "pwsh.exe"
                | "wscript.exe"
                | "cscript.exe"
                | "rundll32.exe"
                | "mshta.exe"
                | "regsvr32.exe"
                | "msiexec.exe"
        )
    {
        return None;
    }
    Some(Launch {
        label,
        exe,
        arguments: arguments.trim_start().to_owned(),
    })
}

fn product_code(key: &str) -> bool {
    let b = key.as_bytes();
    b.len() == 38
        && b[0] == b'{'
        && b[37] == b'}'
        && (1..37).all(|i| {
            if matches!(i, 9 | 14 | 19 | 24) {
                b[i] == b'-'
            } else {
                b[i].is_ascii_hexdigit()
            }
        })
}

fn registration(hive: Hive, key: &str, system: &Path) -> io::Result<Option<App>> {
    let value = |name| registry::read(hive, key, name).map_err(io::Error::other);
    let string = |name| {
        value(name).map(|v| {
            v.and_then(|v| v.text().map(str::to_owned))
                .unwrap_or_default()
        })
    };
    let number = |name| value(name).map(|v| v.and_then(|v| v.as_u64()));
    let name = string("DisplayName")?;
    if name.trim().is_empty()
        || number("SystemComponent")? == Some(1)
        || !string("ParentKeyName")?.is_empty()
    {
        return Ok(None);
    }
    let code = key.rsplit('\\').next().unwrap_or_default();
    let msi = number("WindowsInstaller")? == Some(1) && product_code(code);
    let installer = |label, flag| Launch {
        label,
        exe: system.join("msiexec.exe"),
        arguments: format!("{flag} {code}"),
    };
    let uninstall = if number("NoRemove")? == Some(1) {
        None
    } else if msi {
        Some(installer("Uninstall", "/x"))
    } else {
        parse_command("Uninstall", &string("UninstallString")?)
    };
    let maintenance = if msi && number("NoRepair")? != Some(1) {
        Some(installer("Repair", "/f"))
    } else if number("NoModify")? != Some(1) {
        parse_command("Modify", &string("ModifyPath")?)
    } else {
        None
    };
    Ok(Some(App {
        hive,
        packaged: false,
        key: key.to_owned(),
        name,
        publisher: string("Publisher")?,
        version: string("DisplayVersion")?,
        size: number("EstimatedSize")?.map(|n| n.saturating_mul(1024)),
        location: local_path(string("InstallLocation")?.trim_matches('"')),
        uninstall,
        maintenance,
    }))
}

#[derive(Default)]
pub(super) struct Inventory {
    packaged: bool,
    pub rows: Vec<App>,
    pub issues: usize,
}

pub(super) fn inventory() -> io::Result<Inventory> {
    let system = cleaner::system_directory()?;
    let mut found = Inventory::default();
    for hive in [Hive::CurrentUser, Hive::LocalMachine] {
        for parent in [
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ] {
            match registry::subkeys(hive, parent, usize::MAX) {
                Ok(keys) => {
                    for key in keys {
                        match registration(hive, &format!(r"{parent}\{key}"), &system) {
                            Ok(Some(app)) => found.rows.push(app),
                            Ok(None) => {}
                            Err(_) => found.issues += 1,
                        }
                    }
                }
                Err(_) => found.issues += 1,
            }
        }
    }
    found.rows.sort_by_cached_key(|app| app.name.to_lowercase());
    Ok(found)
}

pub(super) fn launch(app: &App, action: &Launch) -> io::Result<String> {
    super::maintenance::standard_user()?;
    if app.packaged {
        if app.uninstall.as_ref() != Some(action) {
            return Err(io::Error::other("Package action changed."));
        }
        return packages::remove(app);
    }
    let current = registration(app.hive, &app.key, &cleaner::system_directory()?)?;
    launch_desktop(app, action, current)
}

fn launch_desktop(app: &App, action: &Launch, current: Option<App>) -> io::Result<String> {
    if current.as_ref() != Some(app)
        || !(app.uninstall.as_ref() == Some(action) || app.maintenance.as_ref() == Some(action))
    {
        return Err(io::Error::other(
            "Registration changed. Refresh the app list and review it again.",
        ));
    }
    let mut cmd = Command::new(&action.exe);
    if let Some(parent) = action.exe.parent() {
        cmd.current_dir(parent);
    }
    #[cfg(test)]
    cmd.creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0);
    if !action.arguments.is_empty() {
        cmd.raw_arg(&action.arguments);
    }
    if let Err(error) = cmd.spawn() {
        if error.raw_os_error() != Some(740) {
            return Err(error);
        }
        // The reviewed executable requests UAC. Let Windows show its consent prompt.
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        use windows::core::PCWSTR;
        let exe: Vec<_> = action
            .exe
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let args: Vec<_> = action.arguments.encode_utf16().chain(Some(0)).collect();
        let result = unsafe {
            ShellExecuteW(
                None,
                windows::core::w!("open"),
                PCWSTR(exe.as_ptr()),
                PCWSTR(args.as_ptr()),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err(io::Error::other(
                "Windows did not start the installer. Try Windows Installed Apps.",
            ));
        }
    }
    Ok(format!(
        "Started {} for {}. Refresh after the installer closes.",
        action.label.to_lowercase(),
        app.name
    ))
}

#[derive(Default)]
pub(super) struct AppsView {
    pub data: Option<Inventory>,
    query: String,
    by_size: bool,
    chosen: Option<usize>,
    pub review: Option<(App, Launch)>,
}

impl Toolbox {
    pub(super) fn apps_view(&mut self, ui: &mut egui::Ui) {
        let idle = !self.busy();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(idle, egui::Button::new("Refresh app list"))
                .clicked()
            {
                self.apps.review = None;
                self.start(ui.ctx(), "Reading installed desktop apps", false, |_, _| {
                    inventory().map(Output::Apps)
                });
            }
            if ui
                .add_enabled(idle, egui::Button::new("List packaged apps"))
                .clicked()
            {
                self.apps.review = None;
                self.start(ui.ctx(), "Reading current-user packages", false, |_, _| {
                    packages::inventory().map(Output::Apps)
                });
            }
            if ui
                .add_enabled(idle, egui::Button::new("Windows Installed Apps"))
                .on_hover_text("Includes Store apps and system-managed registrations")
                .clicked()
            {
                self.open_destination(ui.ctx(), maintenance::Destination::Apps);
            }
        });
        if let Some((app, action)) = &self.apps.review {
            ui.heading(format!("{} {}?", action.label, app.name));
            ui.label(format!(
                "{}  ·  {}  ·  {}",
                app.publisher,
                app.version,
                app.scope()
            ));
            if app.packaged {
                ui.label("Remove this package and its app data for the current user.");
                ui.monospace(&app.key);
            } else {
                ui.monospace(format!("\"{}\" {}", action.exe.display(), action.arguments));
            }
            let (app, action) = (app.clone(), action.clone());
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    self.apps.review = None;
                }
                if ui
                    .add_enabled(
                        idle,
                        egui::Button::new(if app.packaged {
                            "Remove package"
                        } else {
                            "Launch installer"
                        }),
                    )
                    .clicked()
                {
                    self.apps.review = None;
                    self.start(ui.ctx(), "Starting installer", false, move |_, _| {
                        launch(&app, &action).map(Output::Notice)
                    });
                }
            });
            return;
        }
        let Some(data) = &self.apps.data else { return };
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.apps.query).hint_text("App or publisher"));
            ui.checkbox(&mut self.apps.by_size, "Largest first");
        });
        let query = self.apps.query.trim().to_lowercase();
        let mut rows: Vec<_> = data
            .rows
            .iter()
            .enumerate()
            .filter(|(_, a)| a.matches(&query))
            .collect();
        if self.apps.by_size {
            rows.sort_by_key(|(_, a)| std::cmp::Reverse(a.size));
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} of {} {} · {} unreadable",rows.len(),data.rows.len(),if data.packaged {"current-user packages"} else {"desktop registrations"},data.issues));
            if ui.button("Copy list").clicked() {
                let entries: Vec<_> = rows.iter().map(|(_,a)| serde_json::json!({"name":a.name,"version":a.version,"publisher":a.publisher,"reportedBytes":a.size,"scope":a.scope()})).collect();
                ui.ctx().copy_text(serde_json::Value::Array(entries).to_string());
            }
        });
        let mut map = None;
        if let Some((_, app)) = rows.iter().find(|(i, _)| Some(*i) == self.apps.chosen) {
            ui.strong(&app.name);
            ui.label(format!(
                "{}  ·  {}  ·  {}",
                app.publisher,
                app.version,
                app.scope()
            ));
            ui.horizontal_wrapped(|ui| {
                for action in [&app.uninstall, &app.maintenance].into_iter().flatten() {
                    if ui
                        .add_enabled(idle, egui::Button::new(action.label))
                        .clicked()
                    {
                        self.apps.review = Some(((*app).clone(), action.clone()));
                    }
                }
                if let Some(path) = &app.location
                    && ui
                        .add_enabled(idle, egui::Button::new("Map install folder"))
                        .on_hover_text(path.display().to_string())
                        .clicked()
                {
                    map = Some(path.clone());
                }
            });
            if app.uninstall.is_none() {
                ui.weak("Use Windows Installed Apps for available system actions.");
            }
        }
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("installed_apps")
            .show_rows(ui, 24.0, rows.len(), |ui, range| {
                for &(i, app) in &rows[range] {
                    ui.horizontal(|ui| {
                        ui.monospace(
                            app.size
                                .map(format::bytes)
                                .unwrap_or_else(|| "Unknown".into()),
                        );
                        if ui
                            .selectable_label(self.apps.chosen == Some(i), &app.name)
                            .on_hover_text(&app.publisher)
                            .clicked()
                        {
                            self.apps.chosen = Some(i);
                        }
                    });
                }
            });
        if let Some(path) = map {
            self.open_folder(ui.ctx(), path);
        }
    }
}

impl AppsView {
    pub(super) fn replace(&mut self, data: Inventory) {
        self.chosen = None;
        self.review = None;
        self.data = Some(data);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Fixture;
    use super::*;
    use crate::app::ui_smoke::{app as make_app, click_local_text, frame};
    fn fixture_app(plan: Launch) -> App {
        App {
            hive: Hive::CurrentUser,
            packaged: false,
            key: "fixture".into(),
            name: "Trontop fixture app".into(),
            publisher: "Fixture company".into(),
            version: "1.2".into(),
            size: Some(1024),
            location: None,
            uninstall: Some(plan),
            maintenance: None,
        }
    }
    #[test]
    fn installer_review_cancel_does_not_launch_and_stale_registration_is_rejected() {
        let fake = fixture_app(Launch {
            label: "Uninstall",
            exe: PathBuf::from(r"C:\Unknown\fixture.exe"),
            arguments: "/remove".into(),
        });
        let settings = ThemeSettings::default();
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = make_app(settings, false);
        app.page = Page::Toolbox;
        app.toolbox.tool = Tool::Apps;
        let mut data = Inventory::default();
        data.rows.push(fake.clone());
        app.toolbox.apps.replace(data);
        let size = Vec2::new(1120.0, 660.0);
        frame(&ctx, &mut app, size, vec![]);
        click_local_text(&ctx, &mut app, size, "Trontop fixture app");
        click_local_text(&ctx, &mut app, size, "Uninstall");
        assert!(app.toolbox.apps.review.is_some());
        click_local_text(&ctx, &mut app, size, "Cancel");
        assert!(app.toolbox.apps.review.is_none() && !app.toolbox.busy());
        let mut changed = fake.clone();
        changed.version = "2.0".into();
        assert!(
            launch_desktop(&fake, fake.uninstall.as_ref().unwrap(), Some(changed))
                .unwrap_err()
                .to_string()
                .contains("changed")
        );
    }
    #[test]
    fn disposable_vendor_uninstaller_receives_the_reviewed_argument() {
        let f = Fixture::new();
        let marker = f.file("disposable application.dat", 16);
        let exe = f.0.join("vendor uninstaller.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        let action = Launch {
            label: "Uninstall",
            exe,
            arguments: "--ignored --exact app::toolbox::apps::tests::disposable_vendor_process"
                .into(),
        };
        let app = fixture_app(action.clone());
        launch_desktop(&app, &action, Some(app.clone())).unwrap();
        let done = f.0.join("vendor-finished");
        let until = std::time::Instant::now() + Duration::from_secs(20);
        while !done.exists() {
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(Duration::from_millis(30));
        }
        assert!(!marker.exists());
        // Windows holds a mapped executable until its child exits.
        while std::fs::remove_file(&action.exe).is_err() {
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    #[test]
    #[ignore = "only launched from the disposable vendor fixture"]
    fn disposable_vendor_process() {
        let exe = std::env::current_exe().unwrap();
        if exe.file_name().unwrap() != "vendor uninstaller.exe" {
            return;
        }
        let base = exe.parent().unwrap();
        assert!(base.starts_with(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")));
        assert!(filename(base).starts_with("toolbox-fixture-"));
        std::fs::remove_file(base.join("disposable application.dat")).unwrap();
        std::fs::write(base.join("vendor-finished"), []).unwrap();
    }
    #[test]
    fn commands_preserve_raw_vendor_arguments_and_only_accept_explicit_executables() {
        for raw in [
            r#""C:\Program Files\App\setup.exe" /remove "some value""#,
            r#"C:\Program Files\App\setup.exe /remove "some value""#,
        ] {
            let plan = parse_command("Uninstall", raw).unwrap();
            assert_eq!(plan.exe, Path::new(r"C:\Program Files\App\setup.exe"));
            assert_eq!(plan.arguments, r#"/remove "some value""#);
        }
        for raw in [
            "msiexec /x {id}",
            r"C:relative.exe",
            r"\\host\share\a.exe",
            r"C:\Windows\system32\cmd.exe /c del",
            r#""C:\A.exe"/silent"#,
        ] {
            assert!(parse_command("Uninstall", raw).is_none());
        }
        assert!(product_code("{12345678-1234-1234-1234-123456789abc}"));
        assert!(!product_code("{12345678-1234-1234-1234-123456789ab!}"));
    }
    #[test]
    #[ignore = "reads the current machine's installed app registrations"]
    fn live_inventory_counts_only() {
        let data = inventory().unwrap();
        println!(
            "{} desktop registrations, {} direct uninstall, {} repair/modify, {} unreadable",
            data.rows.len(),
            data.rows.iter().filter(|a| a.uninstall.is_some()).count(),
            data.rows.iter().filter(|a| a.maintenance.is_some()).count(),
            data.issues
        );
        assert!(!data.rows.is_empty());
    }
}
