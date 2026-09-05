use super::*;

fn fixture(count: u32, shape: &str) -> Vec<ProcessRow> {
    (1..=count)
        .map(|pid| ProcessRow {
            pid,
            parent_pid: if pid == 1 {
                None
            } else {
                Some(match shape {
                    "chain" => pid - 1,
                    "balanced" => pid / 2,
                    "wide" => 1,
                    _ => panic!("unknown fixture shape"),
                })
            },
            name: format!("Fixture.{pid:06}.exe"),
            cpu_percent: 1.0,
            gpu_percent: Usage::Measured(0.0),
            memory_bytes: 16,
            read_bytes_per_sec: 2.0,
            write_bytes_per_sec: 3.0,
            ..Default::default()
        })
        .collect()
}

fn build(processes: &[ProcessRow], matches: &[u32], expanded: &[u32]) -> Vec<ProcessTreeRow> {
    build_process_tree(
        processes,
        &matches.iter().copied().collect(),
        &expanded.iter().copied().collect(),
        SortColumn::Pid,
        SortDirection::Ascending,
    )
}

fn pids(processes: &[ProcessRow], tree: &[ProcessTreeRow]) -> Vec<u32> {
    tree.iter()
        .map(|r| processes[r.process_index].pid)
        .collect()
}

#[test]
fn fifty_thousand_deep_chain_uses_bounded_stack_for_totals_expansion_and_search() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let count = 50_000;
            let mut processes = fixture(count, "chain");
            let all = (1..=count).collect::<Vec<_>>();
            let collapsed = build(&processes, &all, &[]);
            assert_eq!(collapsed.len(), 1);
            assert_eq!(collapsed[0].totals.process_count, count as usize);
            assert_eq!(collapsed[0].totals.memory_bytes, count as u64 * 16);
            assert_eq!(collapsed[0].totals.cpu_percent, count as f32);
            assert_eq!(collapsed[0].totals.read_bytes_per_sec, count as f64 * 2.0);
            assert_eq!(collapsed[0].totals.write_bytes_per_sec, count as f64 * 3.0);
            for tree in [
                build(&processes, &all, &all),
                build(&processes, &[count], &[]),
            ] {
                assert_eq!(tree.len(), count as usize);
                for (index, row) in tree.iter().enumerate() {
                    assert_eq!(row.process_index, index);
                    assert_eq!(row.depth, index);
                    assert_eq!(row.totals.process_count, count as usize - index);
                }
            }
            processes[0].parent_pid = Some(count);
            let cycle = build(&processes, &all, &all);
            assert_eq!(cycle.len(), count as usize);
            assert!(
                cycle
                    .iter()
                    .all(|row| row.depth == 0 && row.totals.process_count == 1)
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn cycles_become_separate_roots_with_valid_children_and_no_double_counting() {
    let mut processes = fixture(10, "chain");
    for (row, parent) in processes.iter_mut().zip([
        Some(3),
        Some(1),
        Some(2),
        Some(2),
        Some(4),
        Some(6),
        Some(999),
        None,
        Some(10),
        Some(9),
    ]) {
        row.parent_pid = parent;
    }
    let all = (1..=10).collect::<Vec<_>>();
    for _ in 0..2 {
        let tree = build(&processes, &all, &all);
        assert_eq!(pids(&processes, &tree), [1, 2, 4, 5, 3, 6, 7, 8, 9, 10]);
        assert_eq!(
            tree.iter().map(|r| r.depth).collect::<Vec<_>>(),
            [0, 0, 1, 2, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(tree[1].totals.process_count, 3);
        assert_eq!(
            tree.iter()
                .filter(|r| r.depth == 0)
                .map(|r| r.totals.process_count)
                .sum::<usize>(),
            10
        );
        assert_eq!(
            tree.iter()
                .filter(|r| r.depth == 0)
                .map(|r| r.totals.memory_bytes)
                .sum::<u64>(),
            160
        );
        let search = build(&processes, &[5], &[]);
        assert_eq!(pids(&processes, &search), [2, 4, 5]);
        assert_eq!(search[0].totals.process_count, 3);
        let root_only = build(&processes, &[2], &[]);
        assert_eq!(root_only.len(), 1);
        assert!(root_only[0].has_children);
        assert_eq!(root_only[0].descendant_count, 2);
        assert!(!root_only[0].expanded);
        processes.reverse();
    }
}

#[test]
fn arbitrary_parent_graphs_have_unique_rows_and_conserve_root_resources() {
    let mut random = 0x3a09_5452u32;
    for _ in 0..128 {
        let mut processes = fixture(128, "chain");
        for row in &mut processes {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            row.parent_pid = Some(random % 136); // Cycles, self and absent parents.
        }
        let all = (1..=128).collect::<Vec<_>>();
        let tree = build(&processes, &all, &all);
        assert_eq!(tree.len(), 128);
        assert_eq!(
            pids(&processes, &tree)
                .into_iter()
                .collect::<HashSet<_>>()
                .len(),
            128
        );
        let roots = tree.iter().filter(|r| r.depth == 0).collect::<Vec<_>>();
        assert_eq!(
            roots.iter().map(|r| r.totals.process_count).sum::<usize>(),
            128
        );
        assert_eq!(
            roots.iter().map(|r| r.totals.memory_bytes).sum::<u64>(),
            128 * 16
        );
        assert_eq!(
            roots.iter().map(|r| r.totals.cpu_percent).sum::<f32>(),
            128.0
        );
        assert!(tree.iter().all(|r| r.depth < 128));
    }
}

#[test]
fn newer_parent_identity_is_detached_even_with_equal_rounded_start_seconds() {
    let mut processes = fixture(3, "chain");
    for (row, created) in processes.iter_mut().zip([200, 100, 300]) {
        row.started_at_unix = 1;
        row.control.created_at_100ns = Some(created);
    }
    let tree = build(&processes, &[1, 2, 3], &[1, 2, 3]);
    assert_eq!(pids(&processes, &tree), [1, 2, 3]);
    assert_eq!(tree.iter().map(|r| r.depth).collect::<Vec<_>>(), [0, 0, 1]);
    assert_eq!(tree[0].totals.process_count, 1);
    assert_eq!(tree[1].totals.process_count, 2);
    assert_eq!(pids(&processes, &build(&processes, &[3], &[])), [2, 3]);
    assert_eq!(processes[1].parent_pid, Some(1)); // Raw provider data is unchanged.
}

#[test]
fn parent_time_fallback_rejects_only_known_newer_and_keeps_equal_or_unknown() {
    for (parent_time, child_time, connected) in [
        (20, 10, false),
        (10, 20, true),
        (10, 10, true),
        (0, 10, true),
        (10, 0, true),
    ] {
        let mut processes = fixture(2, "chain");
        processes[0].started_at_unix = parent_time;
        processes[1].started_at_unix = child_time;
        // A zero native identity is unavailable, not evidence of an older parent.
        processes[0].control.created_at_100ns = Some(0);
        let tree = build(&processes, &[1, 2], &[1, 2]);
        assert_eq!(tree[1].depth == 1, connected);
        assert_eq!(tree[0].totals.process_count, if connected { 2 } else { 1 });
    }
}

#[test]
fn duplicate_pid_uses_last_observation_once_for_links_and_resources() {
    let mut processes = fixture(3, "chain");
    let mut duplicate = processes[1].clone();
    duplicate.parent_pid = None;
    duplicate.memory_bytes = 64;
    processes.push(duplicate);
    let tree = build(&processes, &[1, 2, 3], &[1, 2, 3]);
    assert_eq!(pids(&processes, &tree), [1, 2, 3]);
    assert_eq!(tree[1].process_index, 3);
    assert_eq!(tree[0].totals.process_count, 1);
    assert_eq!(tree[1].totals.process_count, 2);
    assert_eq!(tree[1].totals.memory_bytes, 80);
}

#[test]
fn missing_matches_do_not_create_rows_or_hide_search_context() {
    assert!(build(&[], &[1], &[1]).is_empty());
    let processes = fixture(3, "chain");
    assert!(build(&processes, &[], &[1, 2]).is_empty());
    assert!(build(&processes, &[999], &[1, 2]).is_empty());
    // Same matching-set length as process count must not be mistaken for all matched.
    assert_eq!(
        pids(&processes, &build(&processes, &[3, 998, 999], &[])),
        [1, 2, 3]
    );
}

// Intentionally simple recursive oracle only for small, valid, unique-PID trees.
// It does not share the production normalization, traversal or totals algorithm.
fn reference_tree(
    processes: &[ProcessRow],
    matches: &HashSet<u32>,
    expanded: &HashSet<u32>,
    column: SortColumn,
    direction: SortDirection,
) -> Vec<ProcessTreeRow> {
    fn total(index: usize, processes: &[ProcessRow]) -> ProcessTotals {
        let mut result = ProcessTotals::from_process(&processes[index]);
        for (child, row) in processes.iter().enumerate() {
            if row.parent_pid == Some(processes[index].pid) {
                result.add(total(child, processes));
            }
        }
        result
    }
    let mut included = matches.clone();
    let mut expanded = expanded.clone();
    for &pid in matches {
        let mut cursor = pid;
        while let Some(parent) = processes
            .iter()
            .find(|p| p.pid == cursor)
            .and_then(|p| p.parent_pid)
        {
            included.insert(parent);
            if matches.len() != processes.len() {
                expanded.insert(parent);
            }
            cursor = parent;
        }
    }
    struct Reference<'a> {
        processes: &'a [ProcessRow],
        included: &'a HashSet<u32>,
        expanded: &'a HashSet<u32>,
        column: SortColumn,
        direction: SortDirection,
        rows: Vec<ProcessTreeRow>,
    }
    impl Reference<'_> {
        fn append(&mut self, parent: Option<u32>, depth: usize) {
            let mut children = self
                .processes
                .iter()
                .enumerate()
                .filter(|(_, p)| p.parent_pid == parent && self.included.contains(&p.pid))
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            children.sort_by(|&a, &b| {
                compare_processes(
                    &self.processes[a],
                    &self.processes[b],
                    self.column,
                    self.direction,
                )
            });
            for index in children {
                let total = total(index, self.processes);
                let pid = self.processes[index].pid;
                let is_expanded = total.process_count > 1 && self.expanded.contains(&pid);
                self.rows.push(ProcessTreeRow {
                    process_index: index,
                    depth,
                    has_children: total.process_count > 1,
                    descendant_count: total.process_count - 1,
                    expanded: is_expanded,
                    totals: total,
                });
                if is_expanded {
                    self.append(Some(pid), depth + 1);
                }
            }
        }
    }
    let mut reference = Reference {
        processes,
        included: &included,
        expanded: &expanded,
        column,
        direction,
        rows: Vec::new(),
    };
    reference.append(None, 0);
    reference.rows
}

#[test]
fn valid_forests_match_reference_order_filter_expansion_and_all_totals() {
    for seed in 0..24u32 {
        let mut processes = fixture(24, "balanced");
        for row in &mut processes {
            row.parent_pid = if row.pid <= 3 {
                None
            } else {
                Some(1 + (row.pid * 7 + seed * 13) % (row.pid - 1))
            };
            row.name = format!("Fixture.{}", row.pid % 4);
            row.cpu_percent = row.pid as f32 / 7.0;
            row.gpu_percent = match row.pid % 5 {
                0 => Usage::Unreported,
                1 => Usage::Measured(0.0),
                2 => Usage::Partial(1.25),
                3 => Usage::Warming,
                _ => Usage::Measured(2.5),
            };
        }
        processes.rotate_left(seed as usize);
        if seed % 2 == 0 {
            processes.reverse();
        }
        let all = processes.iter().map(|p| p.pid).collect::<HashSet<_>>();
        let subset = processes
            .iter()
            .filter(|p| (p.pid + seed) % 3 == 0)
            .map(|p| p.pid)
            .collect::<HashSet<_>>();
        let expanded = processes
            .iter()
            .filter(|p| p.pid % 2 == 0)
            .map(|p| p.pid)
            .collect::<HashSet<_>>();
        for matches in [&all, &subset] {
            for expanded in [&all, &expanded, &HashSet::new()] {
                for column in [
                    SortColumn::Name,
                    SortColumn::Pid,
                    SortColumn::Cpu,
                    SortColumn::Gpu,
                    SortColumn::Memory,
                    SortColumn::ReadRate,
                    SortColumn::WriteRate,
                    SortColumn::CpuTime,
                    SortColumn::User,
                    SortColumn::Status,
                ] {
                    for direction in [SortDirection::Ascending, SortDirection::Descending] {
                        let actual =
                            build_process_tree(&processes, matches, expanded, column, direction);
                        let expected =
                            reference_tree(&processes, matches, expanded, column, direction);
                        assert_eq!(actual.len(), expected.len());
                        for (a, b) in actual.iter().zip(expected) {
                            assert_eq!(
                                (
                                    a.process_index,
                                    a.depth,
                                    a.has_children,
                                    a.expanded,
                                    a.descendant_count
                                ),
                                (
                                    b.process_index,
                                    b.depth,
                                    b.has_children,
                                    b.expanded,
                                    b.descendant_count
                                )
                            );
                            assert_eq!(a.totals.process_count, b.totals.process_count);
                            assert_eq!(a.totals.cpu_percent, b.totals.cpu_percent);
                            assert_eq!(a.totals.gpu_percent, b.totals.gpu_percent);
                            assert_eq!(a.totals.memory_bytes, b.totals.memory_bytes);
                            assert_eq!(a.totals.read_bytes_per_sec, b.totals.read_bytes_per_sec);
                            assert_eq!(a.totals.write_bytes_per_sec, b.totals.write_bytes_per_sec);
                        }
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "synthetic hierarchy timing; no native process, providers, window or input"]
fn process_tree_timing_probe() {
    // Give the previous recursive implementation a safe, explicit stack for A/B
    // comparison. Bounded-stack correctness has a separate ordinary test.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            use std::hint::black_box;
            use std::time::Instant;
            for count in [500, 1000] {
                for shape in ["wide", "balanced", "chain"] {
                    let processes = fixture(count, shape);
                    let all = (1..=count).collect::<HashSet<_>>();
                    let leaf = HashSet::from([count]);
                    let none = HashSet::new();
                    for (mode, matches, expanded) in [
                        ("collapsed", &all, &none),
                        ("expanded", &all, &all),
                        ("leaf-search", &leaf, &none),
                    ] {
                        let build = || {
                            build_process_tree(
                                &processes,
                                matches,
                                expanded,
                                SortColumn::Name,
                                SortDirection::Ascending,
                            )
                        };
                        for _ in 0..3 {
                            black_box(build());
                        }
                        let mut times = Vec::new();
                        for _ in 0..20 {
                            let at = Instant::now();
                            black_box(build());
                            times.push(at.elapsed().as_secs_f64() * 1e6);
                        }
                        times.sort_by(f64::total_cmp);
                        println!(
                            "PROCESS_TREE count={count} shape={shape} mode={mode} median_us={:.1} p95_us={:.1}",
                            times[10], times[19]
                        );
                    }
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
