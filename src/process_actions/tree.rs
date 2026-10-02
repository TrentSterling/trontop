//! A reviewed, immutable list, built only from an already collected snapshot.
use crate::model::{ProcessIdentity, ProcessRow};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

pub const MAX_TARGETS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scope {
    SelectedTree,
    AllInstances,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    pub identity: ProcessIdentity,
    pub name: String,
    pub parent: Option<ProcessIdentity>,
    pub depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    root: ProcessIdentity,
    scope: Scope,
    executable: Option<String>,
    targets: Vec<Target>,
}

pub(crate) fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

impl Plan {
    pub fn build(rows: &[ProcessRow], root: ProcessIdentity, scope: Scope) -> Result<Self, String> {
        let mut by_pid = HashMap::new();
        let mut children: HashMap<u32, Vec<&ProcessRow>> = HashMap::new();
        for row in rows {
            if by_pid.insert(row.pid, row).is_some() {
                return Err(
                    "The process sample contains duplicate PIDs. Wait for a new sample.".into(),
                );
            }
            if let Some(parent) = row.parent_pid {
                children.entry(parent).or_default().push(row);
            }
        }
        let selected = by_pid
            .get(&root.pid)
            .filter(|row| row.identity() == Some(root))
            .ok_or("The original process exited or changed. Cancel and select again.")?;
        let executable = selected.executable.as_deref().map(path_key);
        let mut roots = vec![*selected];
        if scope == Scope::AllInstances {
            let key = executable.as_ref().filter(|key| !key.is_empty()).ok_or(
                "The executable path is unavailable. All instances cannot be identified safely.",
            )?;
            roots.clear();
            for row in rows {
                match row.executable.as_deref() {
                    Some(path) if path_key(path) == *key => roots.push(row),
                    None if row.name.eq_ignore_ascii_case(&selected.name) => {
                        return Err(format!(
                            "PID {} ({}) has no executable path. Wait for a complete sample before ending all instances.",
                            row.pid, row.name
                        ));
                    }
                    _ => {}
                }
            }
        }
        roots.sort_by_key(|row| (row.control.created_at_100ns.unwrap_or(0), row.pid));
        let mut pending: VecDeque<_> = roots.into_iter().map(|row| (row, None, 0)).collect();
        let mut seen = HashSet::new();
        let mut targets = Vec::new();
        while let Some((row, parent, depth)) = pending.pop_front() {
            if !seen.insert(row.pid) {
                continue;
            }
            let identity = row.identity().ok_or_else(|| {
                format!(
                    "PID {} ({}) has no verified creation time. Wait for a complete sample.",
                    row.pid, row.name
                )
            })?;
            crate::platform::can_terminate(row.pid)?;
            if targets.len() == MAX_TARGETS {
                return Err("The tree exceeds 4096 processes. Select a smaller branch.".into());
            }
            targets.push(Target {
                identity,
                name: row.name.clone(),
                parent,
                depth,
            });
            if let Some(descendants) = children.get(&row.pid) {
                let mut descendants = descendants.clone();
                descendants.sort_by_key(|child| child.pid);
                for child in descendants {
                    let child_identity = child.identity().ok_or_else(|| format!(
                        "Child PID {} ({}) has no verified creation time. Wait for a complete sample.", child.pid, child.name))?;
                    // A reused parent PID must never attach an older process.
                    if child_identity.created_at_100ns < identity.created_at_100ns {
                        continue;
                    }
                    if child.pid == row.pid || seen.contains(&child.pid) {
                        // Matching-executable roots can also appear as descendants.
                        if scope == Scope::SelectedTree {
                            return Err("The process sample contains a parent cycle. Wait for a new sample.".into());
                        }
                        continue;
                    }
                    pending.push_back((child, Some(identity), depth + 1));
                }
            }
        }
        let plan = Self {
            root,
            scope,
            executable,
            targets,
        };
        plan.validate()?;
        Ok(plan)
    }

    pub fn root(&self) -> ProcessIdentity {
        self.root
    }
    pub fn scope(&self) -> Scope {
        self.scope
    }
    pub fn executable(&self) -> Option<&str> {
        self.executable.as_deref()
    }
    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if self.targets.is_empty() || self.targets.len() > MAX_TARGETS {
            return Err("The reviewed process list is empty or too large.".into());
        }
        let mut seen = HashSet::new();
        for target in &self.targets {
            crate::platform::can_terminate(target.identity.pid)?;
            if target.identity.created_at_100ns == 0 || !seen.insert(target.identity.pid) {
                return Err(
                    "The reviewed process list has missing or duplicate identities.".into(),
                );
            }
        }
        Ok(())
    }

    pub fn matches(&self, rows: &[ProcessRow]) -> bool {
        Self::build(rows, self.root, self.scope).is_ok_and(|current| current == *self)
    }
}

#[cfg(test)]
mod tests;
