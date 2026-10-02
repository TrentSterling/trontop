use super::*;
use crate::model::ProcessControlInfo;

fn row(pid: u32, parent: Option<u32>, time: u64, path: Option<&str>) -> ProcessRow {
    ProcessRow {
        pid,
        parent_pid: parent,
        name: "chrome.exe".into(),
        executable: path.map(Into::into),
        control: ProcessControlInfo {
            created_at_100ns: Some(time),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn fixture() -> Vec<ProcessRow> {
    vec![
        row(900001, None, 100, Some("C:/Browser/chrome.exe")),
        row(900002, Some(900001), 101, Some("c:\\browser\\CHROME.EXE")),
        row(900003, Some(900002), 102, Some("C:/Tools/helper.exe")),
        row(900004, None, 103, Some("C:/Browser/chrome.exe")),
        row(900005, Some(900004), 104, Some("C:/Tools/helper.exe")),
        row(900006, None, 99, Some("C:/Different/chrome.exe")),
    ]
}

#[test]
fn selected_child_branch_and_all_instances_have_distinct_complete_targets() {
    let rows = fixture();
    let root = rows[1].identity().unwrap();
    let branch = Plan::build(&rows, root, Scope::SelectedTree).unwrap();
    assert_eq!(
        branch
            .targets()
            .iter()
            .map(|t| t.identity.pid)
            .collect::<Vec<_>>(),
        vec![900002, 900003]
    );
    assert_eq!(branch.targets()[1].parent, Some(root));
    assert_eq!(branch.targets()[1].depth, 1);
    let all = Plan::build(&rows, root, Scope::AllInstances).unwrap();
    assert_eq!(all.root(), root);
    assert_eq!(all.scope(), Scope::AllInstances);
    assert_eq!(
        all.targets()
            .iter()
            .map(|t| t.identity.pid)
            .collect::<Vec<_>>(),
        vec![900001, 900002, 900004, 900003, 900005]
    );
    assert!(!all.targets().iter().any(|t| t.identity.pid == 900006));
}

#[test]
fn review_is_stable_under_counter_updates_but_detects_membership_identity_and_name_changes() {
    let mut rows = fixture();
    let root = rows[0].identity().unwrap();
    let plan = Plan::build(&rows, root, Scope::SelectedTree).unwrap();
    rows.reverse();
    for row in &mut rows {
        row.cpu_percent = 55.0;
        row.memory_bytes = 12345;
    }
    assert!(plan.matches(&rows));
    let mut changed = rows.clone();
    changed.push(row(900010, Some(root.pid), 105, Some("C:/Helper.exe")));
    assert!(!plan.matches(&changed));
    changed = rows.clone();
    changed.retain(|row| row.pid != 900003);
    assert!(!plan.matches(&changed));
    changed = rows.clone();
    changed
        .iter_mut()
        .find(|row| row.pid == root.pid)
        .unwrap()
        .control
        .created_at_100ns = Some(200);
    assert!(!plan.matches(&changed));
    rows.iter_mut().find(|row| row.pid == 900003).unwrap().name = "changed.exe".into();
    assert!(!plan.matches(&rows));
}

#[test]
fn reuse_unknown_identity_duplicate_pid_and_missing_paths_fail_safely() {
    let mut rows = fixture();
    let root = rows[0].identity().unwrap();
    rows[1].control.created_at_100ns = Some(99);
    let plan = Plan::build(&rows, root, Scope::SelectedTree).unwrap();
    assert_eq!(
        plan.targets().len(),
        1,
        "older child is a reused-parent link"
    );
    rows[1].control.created_at_100ns = None;
    assert!(
        Plan::build(&rows, root, Scope::SelectedTree)
            .unwrap_err()
            .contains("creation time")
    );
    rows = fixture();
    rows.push(rows[0].clone());
    assert!(
        Plan::build(&rows, root, Scope::SelectedTree)
            .unwrap_err()
            .contains("duplicate PIDs")
    );
    rows = fixture();
    rows[0].executable = None;
    assert!(
        Plan::build(&rows, root, Scope::AllInstances)
            .unwrap_err()
            .contains("path is unavailable")
    );
    rows = fixture();
    rows[3].executable = None;
    assert!(
        Plan::build(&rows, root, Scope::AllInstances)
            .unwrap_err()
            .contains("no executable path")
    );
    rows = fixture();
    rows[0].control.created_at_100ns = Some(root.created_at_100ns + 1);
    assert!(Plan::build(&rows, root, Scope::SelectedTree).is_err());
}

#[test]
fn cycles_self_kernel_and_large_trees_cannot_be_confirmed() {
    let mut rows = vec![
        row(900001, Some(900002), 100, None),
        row(900002, Some(900001), 100, None),
    ];
    assert!(
        Plan::build(&rows, rows[0].identity().unwrap(), Scope::SelectedTree)
            .unwrap_err()
            .contains("cycle")
    );
    for pid in [0, 4, std::process::id()] {
        rows = vec![row(pid, None, 100, None)];
        assert!(Plan::build(&rows, rows[0].identity().unwrap(), Scope::SelectedTree).is_err());
    }
    rows = (0..=MAX_TARGETS)
        .map(|i| {
            row(
                1_900_001 + i as u32,
                if i == 0 { None } else { Some(1_900_001) },
                100 + i as u64,
                None,
            )
        })
        .collect();
    assert!(
        Plan::build(&rows, rows[0].identity().unwrap(), Scope::SelectedTree)
            .unwrap_err()
            .contains("4096")
    );
}
