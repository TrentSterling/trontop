use super::{
    ProcessRow, ProcessTotals, ProcessTreeRow, SortColumn, SortDirection, sort_process_indices,
};
use std::collections::{HashMap, HashSet};

/// Snapshot-scoped display forest. Original parent IDs remain unchanged in the
/// source; missing, self, newer-parent and cyclic links do not imply ownership.
pub fn build_process_tree(
    processes: &[ProcessRow],
    matching_pids: &HashSet<u32>,
    expanded_pids: &HashSet<u32>,
    column: SortColumn,
    direction: SortDirection,
) -> Vec<ProcessTreeRow> {
    // Real snapshots have unique PIDs. For malformed duplicate rows, consistently
    // use the last observation once, including its parent and resource counters.
    let by_pid = processes
        .iter()
        .enumerate()
        .map(|(index, process)| (process.pid, index))
        .collect::<HashMap<_, _>>();
    let active = (0..processes.len())
        .filter(|&index| by_pid[&processes[index].pid] == index)
        .collect::<Vec<_>>();
    let mut parents = vec![None; processes.len()];
    for &index in &active {
        let process = &processes[index];
        parents[index] = process
            .parent_pid
            .and_then(|pid| by_pid.get(&pid).copied())
            .filter(|&parent| parent != index && !parent_is_newer(&processes[parent], process));
    }
    detach_cycles(&mut parents, &active);

    let mut children = vec![Vec::new(); processes.len()];
    let mut roots = Vec::new();
    for &index in &active {
        if let Some(parent) = parents[index] {
            children[parent].push(index);
        } else {
            roots.push(index);
        }
    }

    // Parent-before-child order, reversed for a bounded-stack postorder. Sum
    // children in source order, as before, even when display sorting differs.
    let mut traversal = roots.clone();
    let mut at = 0;
    while at < traversal.len() {
        let index = traversal[at];
        traversal.extend_from_slice(&children[index]);
        at += 1;
    }
    let mut totals = vec![ProcessTotals::default(); processes.len()];
    for &index in traversal.iter().rev() {
        let mut total = ProcessTotals::from_process(&processes[index]);
        for &child in &children[index] {
            total.add(totals[child]);
        }
        totals[index] = total;
    }

    let reveal_matches = active
        .iter()
        .any(|&index| !matching_pids.contains(&processes[index].pid));
    let mut included = vec![false; processes.len()];
    let mut expanded = vec![false; processes.len()];
    for &index in &active {
        expanded[index] = expanded_pids.contains(&processes[index].pid);
    }
    // Each ancestor link is traversed at most once. Stop at already included
    // context, but still reveal that parent when a later match reaches it.
    for &index in &active {
        if !matching_pids.contains(&processes[index].pid) {
            continue;
        }
        let mut cursor = index;
        while !included[cursor] {
            included[cursor] = true;
            let Some(parent) = parents[cursor] else {
                break;
            };
            if reveal_matches {
                expanded[parent] = true;
            }
            cursor = parent;
        }
    }
    roots.retain(|&index| included[index]);
    sort_process_indices(&mut roots, processes, column, direction);
    for nodes in &mut children {
        // Totals were computed before filtering: hidden descendants still count.
        nodes.retain(|&index| included[index]);
        sort_process_indices(nodes, processes, column, direction);
    }

    let mut pending = roots
        .into_iter()
        .rev()
        .map(|index| (index, 0))
        .collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(active.len());
    while let Some((index, depth)) = pending.pop() {
        let total = totals[index];
        let has_children = total.process_count > 1;
        let is_expanded = has_children && expanded[index];
        rows.push(ProcessTreeRow {
            process_index: index,
            depth,
            has_children,
            descendant_count: total.process_count.saturating_sub(1),
            expanded: is_expanded,
            totals: total,
        });
        if is_expanded {
            pending.extend(
                children[index]
                    .iter()
                    .rev()
                    .map(|&child| (child, depth + 1)),
            );
        }
    }
    rows
}

fn parent_is_newer(parent: &ProcessRow, child: &ProcessRow) -> bool {
    match (parent.identity(), child.identity()) {
        (Some(parent), Some(child)) => parent.created_at_100ns > child.created_at_100ns,
        _ => {
            parent.started_at_unix > 0
                && child.started_at_unix > 0
                && parent.started_at_unix > child.started_at_unix
        }
    }
}

fn detach_cycles(parents: &mut [Option<usize>], active: &[usize]) {
    // 0 = unseen, 1 = in this walk, 2 = complete. Every node is visited once.
    // All members of a malformed cycle become roots, rather than arbitrarily
    // attributing the entire cycle's resources to whichever PID sorted first.
    let mut state = vec![0u8; parents.len()];
    let mut walk = Vec::new();
    for &start in active {
        if state[start] != 0 {
            continue;
        }
        let mut cursor = Some(start);
        while let Some(index) = cursor {
            match state[index] {
                0 => {
                    state[index] = 1;
                    walk.push(index);
                    cursor = parents[index];
                }
                1 => {
                    let mut cycle = index;
                    loop {
                        let next = parents[cycle].take().expect("cycle has a parent");
                        cycle = next;
                        if cycle == index {
                            break;
                        }
                    }
                    break;
                }
                _ => break,
            }
        }
        for index in walk.drain(..) {
            state[index] = 2;
        }
    }
}
