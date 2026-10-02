use super::*;
use crate::model::ProcessRow;
use crate::process_actions::tree::Scope;

#[cfg(windows)]
struct OwnedTree {
    root: std::process::Child,
    rows: Vec<ProcessRow>,
}

#[cfg(windows)]
impl OwnedTree {
    fn spawn() -> Self {
        use std::io::{BufRead, BufReader};
        use std::os::windows::process::CommandExt;
        let mut root = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "platform::tree::tests::owned_hidden_tree_child",
                "--ignored",
                "--nocapture",
            ])
            .env("TRONTOP_OWNED_TREE_FIXTURE", "root")
            .creation_flags(0x0800_0000)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let stdout = root.stdout.take().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(ids) = line.strip_prefix("TRONTOP_OWNED_TREE ") {
                    let _ = sender.send(ids.to_owned());
                }
            }
        });
        // Construct owner before fallible verification so panic still cleans the root.
        let mut owned = Self {
            root,
            rows: Vec::new(),
        };
        let ids = receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        let mut parent = None;
        for (index, pid) in std::iter::once(owned.root.id())
            .chain(
                ids.split_whitespace()
                    .map(|pid| pid.parse::<u32>().unwrap()),
            )
            .enumerate()
        {
            let info = query_process_control(pid).unwrap();
            owned.rows.push(ProcessRow {
                pid,
                parent_pid: parent,
                name: "Owned tree fixture".into(),
                executable: Some(if index == 2 {
                    std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
                        .join("System32/ping.exe")
                } else {
                    std::env::current_exe().unwrap()
                }),
                control: info,
                ..Default::default()
            });
            parent = Some(pid);
        }
        owned
    }
    fn assert_live(&mut self) {
        assert!(self.root.try_wait().unwrap().is_none());
        for row in &self.rows {
            let handle = ProcessHandle::open(
                row.pid,
                windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
            )
            .unwrap();
            assert!(!handle.has_exited());
            assert_eq!(
                handle.created_at().unwrap(),
                row.identity().unwrap().created_at_100ns
            );
        }
    }
    fn assert_ended(&mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let live = self.rows.iter().any(|row| {
                ProcessHandle::open(
                    row.pid,
                    windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
                )
                .is_ok_and(|handle| {
                    !handle.has_exited()
                        && handle.created_at().ok()
                            == Some(row.identity().unwrap().created_at_100ns)
                })
            });
            if !live {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "owned tree did not exit"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = self.root.wait();
    }
}

#[cfg(windows)]
impl Drop for OwnedTree {
    fn drop(&mut self) {
        for row in self.rows.iter().rev() {
            if let Some(identity) = row.identity() {
                let _ = terminate_process(identity);
            }
        }
        let _ = self.root.kill();
        let _ = self.root.wait();
    }
}

#[cfg(windows)]
#[test]
#[ignore = "Owned hidden tree fixture only; requires parent environment guard"]
fn owned_hidden_tree_child() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::windows::process::CommandExt;
    let mode = std::env::var("TRONTOP_OWNED_TREE_FIXTURE").unwrap();
    assert!(mode == "root" || mode == "branch");
    let mut command = if mode == "root" {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "platform::tree::tests::owned_hidden_tree_child",
                "--ignored",
                "--nocapture",
            ])
            .env("TRONTOP_OWNED_TREE_FIXTURE", "branch");
        command
    } else {
        let mut command = std::process::Command::new("ping.exe");
        command.args(["-t", "127.0.0.1"]);
        command
    };
    let mut child = command
        .creation_flags(0x0800_0000)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let descendants = if mode == "root" {
        let stdout = child.stdout.take().unwrap();
        BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .find_map(|line| line.strip_prefix("TRONTOP_OWNED_TREE ").map(str::to_owned))
            .unwrap()
    } else {
        String::new()
    };
    println!("TRONTOP_OWNED_TREE {} {descendants}", child.id());
    std::io::stdout().flush().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(60));
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
#[test]
fn native_tree_ends_owned_root_child_grandchild_and_preserves_unlisted_instance() {
    let mut tree = OwnedTree::spawn();
    let mut unrelated = OwnedTree::spawn();
    let plan = Plan::build(
        &tree.rows,
        tree.rows[0].identity().unwrap(),
        Scope::SelectedTree,
    )
    .unwrap();
    assert_eq!(plan.targets().len(), 3);
    terminate_tree(&plan).unwrap();
    tree.assert_ended();
    unrelated.assert_live();
    println!("OWNED_END_TREE: root + child + grandchild exited; unlisted instance remains PASS");
}

#[cfg(windows)]
#[test]
fn batch_preflight_prevents_partial_termination_on_stale_identity_or_false_parent() {
    let mut tree = OwnedTree::spawn();
    let mut rows = tree.rows.clone();
    rows[2].control.created_at_100ns = Some(rows[2].identity().unwrap().created_at_100ns + 1);
    let plan = Plan::build(&rows, rows[0].identity().unwrap(), Scope::SelectedTree).unwrap();
    assert!(
        terminate_tree(&plan)
            .unwrap_err()
            .contains("No termination requests were sent")
    );
    tree.assert_live();
    rows = tree.rows.clone();
    rows[2].parent_pid = Some(rows[0].pid);
    let plan = Plan::build(&rows, rows[0].identity().unwrap(), Scope::SelectedTree).unwrap();
    assert!(
        terminate_tree(&plan)
            .unwrap_err()
            .contains("parent relationship")
    );
    tree.assert_live();
    rows = tree.rows.clone();
    for row in &mut rows {
        row.executable = Some("C:/Wrong/fixture.exe".into());
    }
    let plan = Plan::build(&rows, rows[0].identity().unwrap(), Scope::AllInstances).unwrap();
    assert!(
        terminate_tree(&plan)
            .unwrap_err()
            .contains("executable path")
    );
    tree.assert_live();
}

#[cfg(windows)]
#[test]
fn all_instances_from_selected_child_ends_both_reviewed_owned_trees() {
    let mut first = OwnedTree::spawn();
    let mut second = OwnedTree::spawn();
    let mut rows = first.rows.clone();
    rows.extend(second.rows.clone());
    let plan = Plan::build(
        &rows,
        first.rows[1].identity().unwrap(),
        Scope::AllInstances,
    )
    .unwrap();
    assert_eq!(plan.targets().len(), 6);
    terminate_tree(&plan).unwrap();
    first.assert_ended();
    second.assert_ended();
    println!("OWNED_END_ALL: selected child, both reviewed independent instances exited PASS");
}
