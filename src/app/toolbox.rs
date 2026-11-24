use super::*;
use crate::model::DiskRow;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
mod apps;
mod cleaner;
mod duplicates;
mod file_users;
#[cfg(windows)]
mod maintenance;
#[cfg(windows)]
mod schedule;
#[cfg(windows)]
pub(crate) use schedule::scheduled_cleaner;
mod scan;
#[cfg(test)]
mod tests;
mod treemap;
mod views;

#[derive(Default, PartialEq, Clone, Copy)]
enum Tool {
    #[default]
    SpaceView,
    Cleaner,
    Drives,
    FileUsers,
    #[cfg(windows)]
    Apps,
    #[cfg(windows)]
    Maintenance,
}

#[derive(Default, PartialEq, Clone, Copy)]
enum View {
    #[default]
    Map,
    Largest,
    Types,
    Duplicates,
    Projects,
    Changes,
}

enum Output {
    Scan(Arc<scan::Scan>, Option<Vec<(PathBuf, i128)>>, bool),
    Preview(cleaner::Preview),
    Cleaned(cleaner::Receipt),
    Users(PathBuf, Vec<file_users::FileUser>),
    Duplicates(duplicates::Report),
    #[cfg(windows)]
    Apps(apps::Inventory),
    #[cfg(windows)]
    Health(maintenance::Health),
    #[cfg(windows)]
    Report(maintenance::Section, String),
    #[cfg(windows)]
    Points(Vec<maintenance::RestorePoint>),
    #[cfg(windows)]
    Wipe(maintenance::Volume),
    #[cfg(windows)]
    Registry(Vec<maintenance::registry::Entry>),
    #[cfg(windows)]
    Notice(String),
    #[cfg(windows)]
    CopyReview(cleaner::CopyReview),
    #[cfg(windows)]
    CopyRemoved(PathBuf),
}

struct Job {
    result: mpsc::Receiver<io::Result<Output>>,
    label: &'static str,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU64>,
    scanning: bool,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

struct DriveEvent {
    time: SystemTime,
    added: bool,
    mount: String,
    name: String,
}

#[derive(Default)]
pub(super) struct Toolbox {
    tool: Tool,
    view: View,
    folder: String,
    file: String,
    job: Option<Job>,
    error: Option<String>,
    scan: Option<Arc<scan::Scan>>,
    deltas: Option<Vec<(PathBuf, i128)>>,
    partial_comparison: bool,
    duplicates: Option<duplicates::Report>,
    preview: Option<cleaner::Preview>,
    selected: HashSet<usize>,
    category: Option<cleaner::Rule>,
    review: Option<Vec<usize>>,
    receipt: Option<cleaner::Receipt>,
    events: VecDeque<DriveEvent>,
    users: Option<(PathBuf, Vec<file_users::FileUser>)>,
    #[cfg(windows)]
    apps: apps::AppsView,
    #[cfg(windows)]
    maintenance: maintenance::ViewState,
    #[cfg(windows)]
    notice: Option<String>,
    #[cfg(windows)]
    copy_review: Option<cleaner::CopyReview>,
    #[cfg(windows)]
    schedule: schedule::Settings,
    #[cfg(windows)]
    pub(super) sleep_rules: crate::platform::minimized::Rules,
}

impl Toolbox {
    fn start(
        &mut self,
        ctx: &egui::Context,
        label: &'static str,
        scanning: bool,
        work: impl FnOnce(&AtomicBool, &AtomicU64) -> io::Result<Output> + Send + 'static,
    ) {
        if self.job.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::<AtomicBool>::default();
        let progress = Arc::<AtomicU64>::default();
        let (stop, count, wake) = (cancel.clone(), progress.clone(), ctx.clone());
        match std::thread::Builder::new()
            .name("trontop-toolbox".into())
            .spawn(move || {
                let result = work(&stop, &count);
                let _ = tx.send(result);
                wake.request_repaint();
            }) {
            Ok(_) => {
                self.error = None;
                #[cfg(windows)]
                {
                    self.notice = None;
                }
                self.job = Some(Job {
                    result: rx,
                    label,
                    scanning,
                    cancel,
                    progress,
                });
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub(super) fn open_folder(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.tool = Tool::SpaceView;
        if self.job.is_some() {
            return;
        }
        self.folder = display(&path);
        let before = self.scan.clone();
        self.start(ctx, "Scanning folder", true, move |stop, count| {
            let scan = scan::run(&path, stop, count)?;
            let previous = before.filter(|s| s.root == scan.root);
            let partial = scan.skipped > 0 || previous.as_ref().is_some_and(|s| s.skipped > 0);
            let deltas = previous.map(|s| scan::changes(&s, &scan));
            Ok(Output::Scan(Arc::new(scan), deltas, partial))
        });
    }

    fn lookup(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.tool = Tool::FileUsers;
        self.file = display(&path);
        self.users = None;
        self.start(ctx, "Looking up file users", false, move |_, _| {
            let rows = file_users::query(&path)?;
            Ok(Output::Users(path, rows))
        });
    }

    pub(super) fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub(super) fn poll(&mut self) {
        #[cfg(windows)]
        self.sleep_rules.poll();
        let Some(job) = &self.job else { return };
        let result = match job.result.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(io::Error::other("Tool worker stopped.")),
        };
        let cancelled = job.cancel.load(Ordering::Relaxed);
        self.job = None;
        if cancelled {
            self.error = Some("Scan cancelled.".into());
            return;
        }
        match result {
            Ok(Output::Scan(scan, deltas, partial)) => {
                if self.scan.as_ref().is_none_or(|old| old.root != scan.root) {
                    self.view = View::Map;
                }
                self.duplicates = None;
                self.deltas = deltas;
                self.partial_comparison = partial;
                self.scan = Some(scan);
            }
            Ok(Output::Preview(preview)) => {
                self.preview = Some(preview);
                self.category = None;
                self.review = None;
                self.selected.clear();
            }
            Ok(Output::Cleaned(receipt)) => {
                self.receipt = Some(receipt);
            }
            Ok(Output::Users(path, rows)) => self.users = Some((path, rows)),
            Ok(Output::Duplicates(report)) => self.duplicates = Some(report),
            #[cfg(windows)]
            Ok(Output::Apps(data)) => self.apps.replace(data),
            #[cfg(windows)]
            Ok(Output::Health(data)) => self.maintenance.health = Some(data),
            #[cfg(windows)]
            Ok(Output::Report(section, data)) => self.maintenance.report = Some((section, data)),
            #[cfg(windows)]
            Ok(Output::Points(data)) => self.maintenance.points = Some(data),
            #[cfg(windows)]
            Ok(Output::Wipe(volume)) => self.maintenance.wipe = Some(volume),
            #[cfg(windows)]
            Ok(Output::Registry(rows)) => self.maintenance.registry = Some(rows),
            #[cfg(windows)]
            Ok(Output::Notice(text)) => self.notice = Some(text),
            #[cfg(windows)]
            Ok(Output::CopyReview(review)) => self.copy_review = Some(review),
            #[cfg(windows)]
            Ok(Output::CopyRemoved(path)) => {
                self.duplicates = None;
                self.scan = None;
                self.deltas = None;
                self.notice = Some(format!(
                    "Removed {}. Scan the folder again for current sizes.",
                    display(&path)
                ));
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    // Diff the two sampler snapshots. The first successful observation is the baseline.
    pub(super) fn observe_volumes(&mut self, before: &[DiskRow], after: &[DiskRow]) {
        for (rows, other, added) in [(before, after, false), (after, before, true)] {
            for row in rows {
                if !other.iter().any(|v| {
                    v.mount == row.mount && v.name == row.name && v.file_system == row.file_system
                }) {
                    self.events.push_front(DriveEvent {
                        time: SystemTime::now(),
                        added,
                        mount: row.mount.clone(),
                        name: row.name.clone(),
                    });
                    self.events.truncate(128);
                }
            }
        }
    }
}

impl TrontopApp {
    pub(super) fn toolbox_page(&mut self, ui: &mut egui::Ui) {
        let t = self.colors();
        let tools = &mut self.toolbox;
        ui.horizontal_wrapped(|ui| {
            for (tool, title) in [
                (Tool::SpaceView, "SpaceView"),
                (Tool::Cleaner, "Cleaner"),
                #[cfg(windows)]
                (Tool::Apps, "Installed Apps"),
                #[cfg(windows)]
                (Tool::Maintenance, "Maintenance"),
                (Tool::Drives, "Drive watch"),
                (Tool::FileUsers, "File users"),
            ] {
                if widgets::stable_tab(ui, tools.tool == tool, title).clicked() {
                    if tools.tool != tool {
                        tools.review = None;
                        #[cfg(windows)]
                        {
                            tools.apps.review = None;
                            tools.copy_review = None;
                            tools.maintenance.clear_review();
                            tools.schedule.review = None;
                        }
                    }
                    tools.tool = tool;
                }
            }
        });
        ui.separator();
        if let Some(job) = &tools.job {
            ui.horizontal_wrapped(|ui| {
                ui.strong(job.label);
                if job.scanning {
                    ui.label(format!(
                        "{} files checked",
                        job.progress.load(Ordering::Relaxed)
                    ));
                    if ui.button("Cancel scan").clicked() {
                        job.cancel.store(true, Ordering::Relaxed);
                    }
                    ui.ctx().request_repaint_after(Duration::from_millis(350));
                }
            });
        }
        if let Some(error) = &tools.error {
            ui.colored_label(t.text, error);
        }
        #[cfg(windows)]
        if let Some(notice) = &tools.notice {
            ui.label(notice);
        }
        match tools.tool {
            Tool::SpaceView => tools.space_view(ui, &self.snapshot.disks, t),
            Tool::Cleaner => tools.cleaner_view(ui, t),
            Tool::Drives => tools.drive_view(ui, &self.snapshot.disks, t),
            #[cfg(windows)]
            Tool::Apps => tools.apps_view(ui),
            #[cfg(windows)]
            Tool::Maintenance => {
                tools.maintenance_view(
                    ui,
                    &self.snapshot.disks,
                    &self.snapshot.processes,
                    &self.process_actions,
                );
                if let Some(id) = tools.maintenance.inspect.take() {
                    self.selected_pid = Some(id.pid);
                    self.inspector_visible = true;
                    self.query.clear();
                    self.rebuild_visible_processes();
                    self.page = Page::Details;
                }
            }
            Tool::FileUsers => {
                if let Some(identity) = tools.users_view(ui, &self.snapshot.processes, t) {
                    self.selected_pid = Some(identity.pid);
                    self.inspector_visible = true;
                    self.query.clear();
                    self.rebuild_visible_processes();
                    self.page = Page::Details;
                }
            }
        }
    }
}

fn display(path: &Path) -> String {
    let s = path.to_string_lossy();
    if let Some(unc) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        s.strip_prefix(r"\\?\").unwrap_or(&s).to_owned()
    }
}

fn filename(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| display(path), |name| name.to_string_lossy().into_owned())
}
