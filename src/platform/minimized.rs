//! Session-only rules for explicitly selected process lifetimes. The worker owns every native handle.
use super::{ProcessHandle, ProcessIdentity, can_control, suspension::Hold};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::BOOL;

const GRACE: Duration = Duration::from_secs(30);
const INTERVAL: Duration = Duration::from_millis(500);
const MAX_RULES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Starting,
    Awake,
    Waiting,
    Sleeping,
    Stopping,
}
impl Status {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Starting => "Checking windows",
            Self::Awake => "Awake",
            Self::Waiting => "Minimized · waiting 30 s",
            Self::Sleeping => "Sleeping",
            Self::Stopping => "Releasing rule",
        }
    }
}
#[derive(Clone)]
pub(crate) struct Entry {
    pub id: ProcessIdentity,
    pub name: String,
    pub status: Status,
}
enum Command {
    Arm(ProcessIdentity),
    Remove(ProcessIdentity),
}
enum Event {
    Status(ProcessIdentity, Status),
    Removed(ProcessIdentity, Option<String>),
}

#[derive(Default)]
pub(crate) struct Rules {
    worker: Option<(Sender<Command>, Receiver<Event>)>,
    pub entries: Vec<Entry>,
    pub error: Option<String>,
}
impl Rules {
    pub(crate) fn contains(&self, id: ProcessIdentity) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }
    pub(crate) fn sleeping(&self, id: ProcessIdentity) -> bool {
        self.entries
            .iter()
            .any(|e| e.id == id && e.status == Status::Sleeping)
    }
    pub(crate) fn arm(
        &mut self,
        ctx: &egui::Context,
        id: ProcessIdentity,
        name: String,
    ) -> Result<(), String> {
        can_control(id.pid)?;
        if self.contains(id) || self.entries.len() >= MAX_RULES {
            return Err("Rule already active or the 16-rule limit is reached.".into());
        }
        if self.worker.is_none() {
            let (commands, requests) = mpsc::channel();
            let (events, results) = mpsc::channel();
            let ctx = ctx.clone();
            std::thread::Builder::new()
                .name("trontop-minimized-apps".into())
                .spawn(move || {
                    worker(requests, |event| {
                        let _ = events.send(event);
                        ctx.request_repaint();
                    })
                })
                .map_err(|e| e.to_string())?;
            self.worker = Some((commands, results));
        }
        self.worker
            .as_ref()
            .unwrap()
            .0
            .send(Command::Arm(id))
            .map_err(|_| "Sleep worker stopped.")?;
        self.entries.push(Entry {
            id,
            name,
            status: Status::Starting,
        });
        self.error = None;
        Ok(())
    }
    pub(crate) fn remove(&mut self, id: ProcessIdentity) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id)
            && entry.status != Status::Stopping
            && let Some((commands, _)) = &self.worker
            && commands.send(Command::Remove(id)).is_ok()
        {
            entry.status = Status::Stopping;
        }
    }
    pub(crate) fn poll(&mut self) {
        let Some((_, results)) = &self.worker else {
            return;
        };
        loop {
            match results.try_recv() {
                Ok(Event::Status(id, status)) => {
                    if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id)
                        && entry.status != Status::Stopping
                    {
                        entry.status = status;
                    }
                }
                Ok(Event::Removed(id, error)) => {
                    if let Some(error) = error {
                        self.error = Some(format!("PID {}: {error}", id.pid));
                    }
                    self.entries.retain(|e| e.id != id);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.entries.clear();
                    self.worker = None;
                    self.error =
                        Some("Sleep worker stopped. Its suspension handles were released.".into());
                    break;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Windows {
    #[default]
    None,
    Minimized,
    Visible,
}
impl Windows {
    fn observe(&mut self, iconic: bool) {
        if !iconic {
            *self = Self::Visible;
        } else if *self == Self::None {
            *self = Self::Minimized;
        }
    }
}
fn pid(window: HWND) -> u32 {
    let mut id = 0;
    if !window.is_invalid() {
        unsafe {
            GetWindowThreadProcessId(window, Some(&mut id));
        }
    }
    id
}
fn windows(pids: impl Iterator<Item = u32>) -> Result<HashMap<u32, Windows>, String> {
    unsafe extern "system" fn collect(window: HWND, param: LPARAM) -> BOOL {
        // EnumWindows invokes the callback synchronously on this worker.
        let states = unsafe { &mut *(param.0 as *mut HashMap<u32, Windows>) };
        if let Some(state) = states.get_mut(&pid(window))
            && unsafe { IsWindowVisible(window).as_bool() }
        {
            state.observe(unsafe { IsIconic(window).as_bool() });
        }
        true.into()
    }
    let mut states: HashMap<_, _> = pids.map(|pid| (pid, Windows::None)).collect();
    unsafe { EnumWindows(Some(collect), LPARAM((&raw mut states) as isize)) }
        .map_err(|e| e.to_string())?;
    let foreground = pid(unsafe { GetForegroundWindow() });
    if foreground == 0 {
        states.values_mut().for_each(|s| *s = Windows::None);
    } else if let Some(s) = states.get_mut(&foreground) {
        *s = Windows::Visible;
    }
    Ok(states)
}

struct Rule {
    id: ProcessIdentity,
    process: ProcessHandle,
    since: Option<Instant>,
    hold: Option<Hold>,
    status: Status,
}
impl Rule {
    fn new(id: ProcessIdentity) -> Result<Self, String> {
        can_control(id.pid)?;
        if id.pid == pid(unsafe { GetShellWindow() }) {
            return Err("Windows shell is excluded.".into());
        }
        let process = ProcessHandle::verified(id, PROCESS_QUERY_LIMITED_INFORMATION)?;
        Ok(Self {
            id,
            process,
            since: None,
            hold: None,
            status: Status::Starting,
        })
    }
    fn update(&mut self, state: Windows, now: Instant) -> Result<Status, String> {
        if state != Windows::Minimized {
            self.hold = None;
            self.since = None;
            return Ok(Status::Awake);
        }
        if now.duration_since(*self.since.get_or_insert(now)) < GRACE {
            return Ok(Status::Waiting);
        }
        if self.hold.is_none() {
            self.hold = Some(Hold::create(self.id)?);
        }
        Ok(Status::Sleeping)
    }
}
fn worker(requests: Receiver<Command>, mut report: impl FnMut(Event)) {
    let mut rules: Vec<Rule> = Vec::new();
    loop {
        let command = if rules.is_empty() {
            match requests.recv() {
                Ok(cmd) => Some(cmd),
                Err(_) => return,
            }
        } else {
            match requests.recv_timeout(INTERVAL) {
                Ok(cmd) => Some(cmd),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        };
        match command {
            Some(Command::Arm(id)) => {
                let rule = Rule::new(id).and_then(|rule| {
                    let states = windows(std::iter::once(id.pid))?;
                    if states[&id.pid] == Windows::None {
                        Err("Choose an app with a visible or minimized desktop window.".into())
                    } else {
                        Ok(rule)
                    }
                });
                match rule {
                    Ok(rule) => rules.push(rule),
                    Err(e) => report(Event::Removed(id, Some(e))),
                }
            }
            Some(Command::Remove(id)) => {
                if let Some(index) = rules.iter().position(|r| r.id == id) {
                    rules.swap_remove(index);
                    report(Event::Removed(id, None));
                }
            }
            None => {}
        }
        if rules.is_empty() {
            continue;
        }
        let states = match windows(rules.iter().map(|r| r.id.pid)) {
            Ok(states) => states,
            Err(error) => {
                for rule in rules.drain(..) {
                    let id = rule.id;
                    drop(rule);
                    report(Event::Removed(id, Some(error.clone())));
                }
                continue;
            }
        };
        let now = Instant::now();
        // Drop the native hold before reporting that the rule was removed.
        let mut i = 0;
        while i < rules.len() {
            let r = &mut rules[i];
            let id = r.id;
            let result = if r.process.has_exited() {
                Ok(None)
            } else {
                r.update(states[&id.pid], now).map(Some)
            };
            match result {
                Ok(Some(state)) => {
                    if state != r.status {
                        r.status = state;
                        report(Event::Status(id, state));
                    }
                    i += 1;
                }
                other => {
                    rules.swap_remove(i);
                    report(Event::Removed(id, other.err()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::HeartbeatChild;
    #[test]
    fn minimized_only_after_grace_and_real_owned_process_wakes_on_restore_or_teardown() {
        let child = HeartbeatChild::spawn();
        let mut rule = Rule::new(child.identity).unwrap();
        let now = Instant::now();
        assert_eq!(
            rule.update(Windows::Minimized, now).unwrap(),
            Status::Waiting
        );
        child.assert_running();
        assert_eq!(
            rule.update(Windows::Visible, now + GRACE).unwrap(),
            Status::Awake
        );
        assert_eq!(
            rule.update(Windows::Minimized, now + GRACE).unwrap(),
            Status::Waiting
        );
        assert_eq!(
            rule.update(Windows::Minimized, now + GRACE * 2).unwrap(),
            Status::Sleeping
        );
        child.assert_paused();
        assert_eq!(
            rule.update(Windows::Visible, now + GRACE * 2).unwrap(),
            Status::Awake
        );
        child.assert_running();
        rule.update(Windows::Minimized, now + GRACE * 3).unwrap();
        rule.update(Windows::Minimized, now + GRACE * 4).unwrap();
        child.assert_paused();
        drop(rule);
        child.assert_running();
    }
    #[test]
    fn visible_window_overrides_any_number_of_minimized_windows() {
        let mut state = Windows::None;
        state.observe(true);
        assert!(state == Windows::Minimized);
        state.observe(false);
        state.observe(true);
        assert!(state == Windows::Visible);
    }

    #[test]
    fn cancelling_a_rejected_rule_does_not_acknowledge_removal_twice() {
        let (tx, rx) = mpsc::channel();
        let id = ProcessIdentity {
            pid: std::process::id(),
            created_at_100ns: 1,
        };
        tx.send(Command::Arm(id)).unwrap();
        tx.send(Command::Remove(id)).unwrap();
        drop(tx);
        let mut events = Vec::new();
        worker(rx, |e| events.push(e));
        assert!(matches!(events.as_slice(),[Event::Removed(target,Some(_))] if *target==id));
    }

    #[test]
    fn actual_worker_refuses_a_windowless_child_and_a_reused_pid() {
        let child = HeartbeatChild::spawn();
        let ctx = egui::Context::default();
        let mut rules = Rules::default();
        for (id, expected) in [
            (child.identity, "desktop window"),
            (
                ProcessIdentity {
                    created_at_100ns: child.identity.created_at_100ns + 1,
                    ..child.identity
                },
                "different process",
            ),
        ] {
            rules.arm(&ctx, id, "fixture".into()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while rules.contains(id) {
                rules.poll();
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                rules.error.as_ref().is_some_and(|e| e.contains(expected)),
                "{:?}",
                rules.error
            );
            child.assert_running();
        }
    }
}
