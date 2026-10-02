//! Exact registration/approval snapshots for confirmed, reversible controls.
use super::Source;
use crate::model::StartupRow;
use std::time::{Duration, Instant};

#[cfg(windows)]
pub mod native;
mod observations;
mod worker;
pub use observations::Observations;
pub use worker::{Controller, Outcome};

pub const VALUE_LIMIT: usize = 1024 * 1024;
pub const APPROVAL_LIMIT: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawValue {
    pub kind: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Approval {
    Missing,
    Value(RawValue),
    Unreadable(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Enabled,
    Disabled,
    Unsupported,
    Unavailable,
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Enabled => "Enabled",
            Self::Disabled => "Disabled",
            Self::Unsupported => "Unrecognized",
            Self::Unavailable => "Unavailable",
        }
    }
}

impl Approval {
    pub fn state(&self) -> State {
        match self {
            Self::Missing => State::Enabled,
            Self::Unreadable(_) => State::Unavailable,
            Self::Value(value) if value.kind == 3 && value.bytes.len() == 12 => {
                match u32::from_le_bytes(value.bytes[..4].try_into().unwrap()) {
                    2 | 6 => State::Enabled,
                    3 | 7 => State::Disabled,
                    _ => State::Unsupported,
                }
            }
            Self::Value(_) => State::Unsupported,
        }
    }

    pub fn changed(&self, enabled: bool, disabled_at_100ns: u64) -> Result<Self, String> {
        if !matches!(self.state(), State::Enabled | State::Disabled) {
            return Err(
                "Windows approval state is unreadable or unrecognized. No change was made.".into(),
            );
        }
        let base = match self {
            Self::Value(value) if value.bytes[0] >= 6 => 6u32,
            _ => 2,
        };
        let mut bytes = (base + u32::from(!enabled)).to_le_bytes().to_vec();
        bytes.extend_from_slice(&(if enabled { 0 } else { disabled_at_100ns }).to_le_bytes());
        Ok(Self::Value(RawValue { kind: 3, bytes }))
    }

    pub fn text_bytes(&self) -> usize {
        match self {
            Self::Missing => 0,
            Self::Value(value) => value.bytes.len(),
            Self::Unreadable(error) => error.len(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileIdentity {
    pub volume: u32,
    pub index: u64,
    pub created: u64,
    pub modified: u64,
    pub length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Registration {
    Run(RawValue),
    File(FileIdentity),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Control {
    pub registration: Option<Registration>,
    pub approval: Approval,
}

impl Control {
    pub fn editable(&self) -> bool {
        self.registration.is_some()
            && matches!(self.approval.state(), State::Enabled | State::Disabled)
    }
    pub fn text_bytes(&self) -> usize {
        self.approval.text_bytes()
            + match &self.registration {
                Some(Registration::Run(raw)) => raw.bytes.len(),
                _ => 0,
            }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Key {
    pub source: Source,
    pub name: String,
}
impl Key {
    pub fn of(row: &StartupRow) -> Self {
        Self {
            source: row.source,
            name: row.key.to_lowercase(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Enable,
    Disable,
    Restore(Approval),
}
impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Enable => "Enable startup",
            Self::Disable => "Disable startup",
            Self::Restore(_) => "Undo startup change",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub target: StartupRow,
    pub action: Action,
    pub confirmed_at: Instant,
}

impl Request {
    pub fn new(target: StartupRow, action: Action) -> Self {
        Self {
            target,
            action,
            confirmed_at: Instant::now(),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.confirmed_at.elapsed() >= Duration::from_secs(30) {
            return Err("The confirmation expired. Review this startup entry again.".into());
        }
        let row = &self.target;
        if row.key.is_empty()
            || row.key.contains('\0')
            || row.command.is_empty()
            || row.command.contains('\0')
            || row.key.encode_utf16().count() >= 32768
        {
            return Err(
                "The startup entry has an invalid name or command. No change was made.".into(),
            );
        }
        let control = row
            .control
            .as_ref()
            .filter(|control| control.editable())
            .ok_or(
                "A complete recognized startup record is required. Refresh before changing it.",
            )?;
        match (row.source, &control.registration) {
            (
                Source::UserRun | Source::MachineRun | Source::MachineRun32,
                Some(Registration::Run(raw)),
            ) if matches!(raw.kind, 1 | 2) && raw.bytes.len() <= VALUE_LIMIT => {}
            (Source::UserFolder | Source::MachineFolder, Some(Registration::File(file)))
                if file.index != 0
                    && file.created != 0
                    && !row.key.contains(['/', '\\', ':'])
                    && !matches!(row.key.as_str(), "." | "..") => {}
            _ => {
                return Err(
                    "The startup registration identity is invalid. No change was made.".into(),
                );
            }
        }
        if let Action::Restore(approval) = &self.action
            && !matches!(approval.state(), State::Enabled | State::Disabled)
        {
            return Err("The saved approval record cannot be restored safely.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Receipt {
    pub target: StartupRow,
    pub before: Approval,
    pub after: Approval,
    pub observed_at: Instant,
}
#[derive(Clone, Debug)]
pub struct Failure {
    pub message: String,
    pub uncertain: bool,
}
impl Failure {
    pub fn unchanged(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            uncertain: false,
        }
    }
    pub fn uncertain(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            uncertain: true,
        }
    }
}
impl Receipt {
    pub fn undo(&self) -> Request {
        let mut row = self.target.clone();
        if let Some(control) = &mut row.control {
            control.approval = self.after.clone();
        }
        Request::new(row, Action::Restore(self.before.clone()))
    }
}

#[cfg(test)]
mod tests;
