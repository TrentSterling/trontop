//! System specifications in the spirit of Speccy: sidebar sections made of
//! collapsible label/value trees, Summary headlines and inline live values.
//!
//! Each section has one provider (`src/specs/<lane>.rs`) that runs on its own
//! background worker (`worker.rs`); the UI renders immutable snapshots and
//! resolves `LiveKey`s against the sampler snapshot and the sensor bridge
//! (`live.rs`) without native calls. Shared read-only native helpers live in
//! `native/`. See docs/SYSTEM_SPECS.md for the provider contract.
pub mod board;
pub mod bridge;
pub mod cpu;
pub mod devices;
pub mod graphics;
pub mod memory;
pub mod network;
pub mod os;
pub mod storage;

mod live;
mod model;
pub mod native;
mod report;
mod worker;

#[cfg(test)]
pub mod fixtures;

pub use live::{
    BRIDGE_STALE_AFTER, BridgeReading, BridgeReadings, GpuMetric, GpuRef, LiveKey, LiveUnit,
    TempBand, band, resolve,
};
#[cfg(test)]
pub use model::{Completeness, SectionHealth};
pub use model::{Group, Item, Row, Section, SectionId, SectionState, SummaryLine, Value};
#[cfg(test)]
pub use report::probe_text;
pub use report::{LiveSource, group_text, json, section_text, text};
pub use worker::{CADENCE, Context, Monitor, Snapshot};
