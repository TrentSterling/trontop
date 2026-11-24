use super::*;
use std::ffi::OsString;
use std::os::windows::process::CommandExt;
use std::process::Command;
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

pub(super) struct Settings {
    time: String,
    groups: [bool; 7],
    threshold: u32,
    weekly: bool,
    pub review: Option<Review>,
}
#[derive(Clone, Copy)]
pub(super) enum Review {
    Save,
    Remove,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            time: "12:00".into(),
            groups: [true, false, false, false, false, false, false],
            threshold: 256,
            weekly: false,
            review: None,
        }
    }
}
fn mask(groups: &[bool; 7]) -> u8 {
    groups
        .iter()
        .enumerate()
        .fold(0, |v, (i, b)| v | (u8::from(*b) << i))
}
fn rule_mask(rule: cleaner::Rule) -> u8 {
    use cleaner::Rule::*;
    match rule {
        Temp | WindowsTemp => 1,
        Dump => 2,
        Chrome | Edge | Brave | Vivaldi | Firefox => 4,
        NuGet => 8,
        App(_) | Steam => 16,
        NpmLogs | ObsLogs => 32,
        PipWheels | ElectronDownloads => 64,
    }
}
fn valid_time(time: &str) -> bool {
    let Some((h, m)) = time.split_once(':') else {
        return false;
    };
    h.len() == 2
        && m.len() == 2
        && h.bytes().chain(m.bytes()).all(|c| c.is_ascii_digit())
        && h.parse::<u8>().is_ok_and(|n| n < 24)
        && m.parse::<u8>().is_ok_and(|n| n < 60)
}

fn task_name(system: &Path) -> io::Result<String> {
    let out = Command::new(system.join("whoami.exe"))
        .args(["/user", "/fo", "csv", "/nh"])
        .creation_flags(CREATE_NO_WINDOW.0)
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let sid = text
        .trim()
        .trim_end_matches('"')
        .rsplit('"')
        .next()
        .unwrap_or_default();
    if !out.status.success()
        || !sid.starts_with("S-1-")
        || !sid[2..].bytes().all(|c| c == b'-' || c.is_ascii_digit())
    {
        return Err(io::Error::other(
            "Could not identify the scheduling account.",
        ));
    }
    Ok(format!("Trontop Cleaner {sid}"))
}
enum Change {
    Query,
    Remove,
    Save {
        time: String,
        mask: u8,
        threshold: u32,
        weekly: bool,
    },
}
fn task(change: Change) -> io::Result<String> {
    maintenance::standard_user()?;
    let system = cleaner::system_directory()?;
    let name = task_name(&system)?;
    task_for(
        change,
        &system,
        &name,
        &std::env::current_exe()?,
        "--scheduled-clean",
    )
}
fn task_for(
    change: Change,
    system: &Path,
    name: &str,
    exe: &Path,
    mode: &str,
) -> io::Result<String> {
    let mut cmd = Command::new(system.join("schtasks.exe"));
    match change {
        Change::Query => {
            cmd.args(["/Query", "/TN", name, "/V", "/FO", "LIST"]);
        }
        Change::Remove => {
            cmd.args(["/Delete", "/TN", name, "/F"]);
        }
        Change::Save {
            time,
            mask,
            threshold,
            weekly,
        } => {
            if !valid_time(&time) || !(1..=127).contains(&mask) {
                return Err(io::Error::other(
                    "Choose categories and a valid 24-hour start time.",
                ));
            }
            let path = exe.to_str().ok_or_else(|| {
                io::Error::other("Task Scheduler requires a Unicode executable path.")
            })?;
            let action = format!("\"{path}\" {mode} {mask} {threshold}");
            cmd.args([
                "/Create",
                "/TN",
                name,
                "/TR",
                &action,
                "/SC",
                if weekly { "WEEKLY" } else { "DAILY" },
            ]);
            if weekly {
                cmd.args(["/D", "MON"]);
            }
            cmd.args(["/ST", &time, "/RL", "LIMITED", "/IT", "/F"]);
        }
    }
    let out = cmd.creation_flags(CREATE_NO_WINDOW.0).output()?;
    let msg = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .trim()
    .to_owned();
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "Task Scheduler ({}) {msg}",
            out.status
        )));
    }
    Ok(msg)
}
fn run_plan(preview: cleaner::Preview, mask: u8, threshold: u32, apply: bool) -> (String, bool) {
    let files: Vec<_> = preview
        .files
        .into_iter()
        .filter(|c| rule_mask(c.rule) & mask != 0)
        .collect();
    let bytes: u64 = files.iter().map(|c| c.bytes()).sum();
    let count = files.len();
    let reached = bytes >= u64::from(threshold) * 1024 * 1024;
    let receipt = if apply && reached {
        cleaner::apply(files)
    } else {
        cleaner::Receipt::default()
    };
    (serde_json::json!({"timeUnix":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        "mode":if apply{"scheduled clean"}else{"preview"},"selectedFiles":count,"selectedBytes":bytes,"thresholdMiB":threshold,
        "thresholdReached":reached,"previewSkipped":preview.skipped,"scanIssues":preview.issues.len(),
        "deletedFiles":receipt.deleted,"deletedBytes":receipt.bytes,"applySkipped":receipt.failures.len()}).to_string(),
        receipt.failures.is_empty())
}
pub(crate) fn scheduled_cleaner(args: &[OsString]) -> Option<i32> {
    let apply = match args.first()?.to_str()? {
        "--scheduled-clean" => true,
        "--scheduled-preview" => false,
        _ => return None,
    };
    let result = (|| -> io::Result<i32> {
        let [_, mask, threshold] = args else {
            return Err(io::Error::other(
                "Expected a category mask and threshold in MiB.",
            ));
        };
        let mask = mask
            .to_str()
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|n| (1..=127).contains(n))
            .ok_or_else(|| io::Error::other("Invalid category mask"))?;
        let threshold = threshold
            .to_str()
            .and_then(|v| v.parse::<u32>().ok())
            .ok_or_else(|| io::Error::other("Invalid threshold"))?;
        if apply {
            maintenance::standard_user()?;
        }
        let preview = cleaner::preview(&AtomicBool::default(), &AtomicU64::default())?;
        let (report, ok) = run_plan(preview, mask, threshold, apply);
        if apply {
            let dir = crate::trontop_state_directory()
                .ok_or_else(|| io::Error::other("Missing settings location."))?;
            std::fs::create_dir_all(&dir)?;
            std::fs::write(dir.join("last-scheduled-clean.json"), &report)?;
        }
        println!("{report}");
        Ok(if ok { 0 } else { 2 })
    })();
    Some(result.unwrap_or_else(|e| {
        eprintln!("{e}");
        1
    }))
}
impl Toolbox {
    pub(super) fn schedule_view(&mut self, ui: &mut egui::Ui) {
        let idle = !self.busy();
        ui.label("Run Trontop cleanup while signed in, even with the window closed. Keep the executable at its current path.");
        let settings = &mut self.schedule;
        let mut change = None;
        if let Some(review) = settings.review {
            match review {
                Review::Save => {
                    ui.strong(format!("Create or replace your cleanup schedule {} at {}? Permanently clear the selected categories at {} MiB or above.",if settings.weekly{"each Monday"}else{"daily"},settings.time,settings.threshold));
                    let labels = [
                        "temporary files",
                        "crash dumps",
                        "browser caches",
                        "NuGet downloads",
                        "Desktop app caches",
                        "Diagnostic logs",
                        "Developer downloads",
                    ];
                    ui.label(
                        labels
                            .iter()
                            .zip(settings.groups)
                            .filter_map(|(label, on)| on.then_some(*label))
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                }
                Review::Remove => {
                    ui.strong("Remove your Trontop scheduled task?");
                }
            }
            if ui
                .add_enabled(idle, egui::Button::new("Confirm schedule change"))
                .clicked()
            {
                change = Some(match review {
                    Review::Save => Change::Save {
                        time: settings.time.clone(),
                        mask: mask(&settings.groups),
                        threshold: settings.threshold,
                        weekly: settings.weekly,
                    },
                    Review::Remove => Change::Remove,
                });
            }
            if ui.button("Cancel").clicked() || change.is_some() {
                settings.review = None;
            }
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut settings.weekly, "Mondays only");
                ui.label("Start time (24h)");
                ui.add(egui::TextEdit::singleline(&mut settings.time).desired_width(58.0));
                ui.label("Minimum MiB");
                ui.add(egui::DragValue::new(&mut settings.threshold).range(0..=1048576));
            });
            ui.horizontal_wrapped(|ui| {
                for (label, on) in [
                    "Temporary files",
                    "Crash dumps",
                    "Browser caches",
                    "NuGet downloads",
                    "Desktop app caches",
                    "Diagnostic logs",
                    "Developer downloads",
                ]
                .into_iter()
                .zip(&mut settings.groups)
                {
                    ui.checkbox(on, label);
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        idle && mask(&settings.groups) != 0 && valid_time(&settings.time),
                        egui::Button::new("Review schedule"),
                    )
                    .clicked()
                {
                    settings.review = Some(Review::Save);
                }
                if ui
                    .add_enabled(idle, egui::Button::new("Read current schedule"))
                    .clicked()
                {
                    change = Some(Change::Query);
                }
                if ui
                    .add_enabled(idle, egui::Button::new("Remove schedule"))
                    .clicked()
                {
                    settings.review = Some(Review::Remove);
                }
            });
        }
        if let Some(change) = change {
            self.start(ui.ctx(), "Windows Task Scheduler", false, move |_, _| {
                task(change).map(|s| Output::Report(maintenance::Section::Schedule, s))
            });
        }
        if ui
            .add_enabled(idle, egui::Button::new("Last scheduled result"))
            .clicked()
        {
            self.start(ui.ctx(), "Reading last scheduled result", false, |_, _| {
                let path = crate::trontop_state_directory()
                    .ok_or_else(|| io::Error::other("Missing settings location."))?
                    .join("last-scheduled-clean.json");
                std::fs::read_to_string(path)
                    .map(|s| Output::Report(maintenance::Section::Schedule, s))
            });
        }
        if let Some((maintenance::Section::Schedule, s)) = &self.maintenance.report {
            ui.monospace(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Fixture;
    use super::*;
    #[test]
    #[ignore = "creates and removes a unique current-user task with a read-only action; needs release build"]
    fn scheduler_round_trip_with_preview_only_action() {
        let sys = cleaner::system_directory().unwrap();
        let exe = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/release/trontop.exe");
        assert!(exe.is_file());
        let name = format!(
            "Trontop Preview Fixture {} {}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let task = |change| task_for(change, &sys, &name, &exe, "--scheduled-preview");
        assert!(task(Change::Query).is_err());
        task(Change::Save {
            time: "05:01".into(),
            mask: 1,
            threshold: 256,
            weekly: true,
        })
        .unwrap();
        let observed = task(Change::Query);
        let xml = Command::new(sys.join("schtasks.exe"))
            .args(["/Query", "/TN", &name, "/XML"])
            .creation_flags(CREATE_NO_WINDOW.0)
            .output();
        let removed = task(Change::Remove);
        removed.unwrap();
        assert!(task(Change::Query).is_err());
        observed.unwrap();
        let xml = xml.unwrap();
        assert!(xml.status.success());
        let bytes: Vec<u8> = xml.stdout.into_iter().filter(|c| *c != 0).collect();
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.contains("--scheduled-preview 1 256")
                && text.contains("<Monday")
                && text.contains("InteractiveToken")
                && !text.contains("--scheduled-clean")
        );
        assert!(task_name(&sys).unwrap().starts_with("Trontop Cleaner S-1-"));
    }
    #[test]
    fn scheduled_threshold_and_selection_apply_only_to_eligible_fixture_files() {
        for (rule, bit) in [
            (cleaner::Rule::Steam, 16),
            (cleaner::Rule::ObsLogs, 32),
            (cleaner::Rule::PipWheels, 64),
        ] {
            assert_eq!(rule_mask(rule), bit);
            assert_eq!(
                rule_mask(rule) & 15,
                0,
                "alpha.56 schedules must keep their reviewed scope"
            );
        }
        for t in ["0:00", "24:00", "00:60", "１２:30"] {
            assert!(!valid_time(t));
        }
        for t in ["00:00", "23:59"] {
            assert!(valid_time(t));
        }
        assert_eq!(mask(&[true, false, true, false, false, false, false]), 5);
        let f = Fixture::new();
        let tmp = f.file("sample.tmp", 20);
        let dump = f.file("sample.dmp", 30);
        for path in [&tmp, &dump] {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(SystemTime::now() - Duration::from_secs(40 * 86400))
                .unwrap();
        }
        let preview = || {
            cleaner::inspect(&[
                (f.0.clone(), cleaner::Rule::Temp),
                (f.0.clone(), cleaner::Rule::Dump),
            ])
        };
        run_plan(preview(), 1, 0, false);
        run_plan(preview(), 1, 1, true);
        assert!(tmp.exists() && dump.exists());
        let (report, ok) = run_plan(preview(), 1, 0, true);
        assert!(ok && !tmp.exists() && dump.exists());
        let json: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(json["deletedFiles"], 1);
        assert_eq!(json["deletedBytes"], 20);
    }
}
