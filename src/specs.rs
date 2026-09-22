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

// The scaffold exposes the full helper and builder surface before every
// provider lane consumes it. Drop these allowances at integration.
#[allow(dead_code)]
mod live;
#[allow(dead_code)]
mod model;
#[allow(dead_code, unused_imports)]
pub mod native;
#[allow(dead_code)]
mod report;
#[allow(dead_code)]
mod worker;

#[cfg(test)]
pub mod fixtures;

#[allow(unused_imports)]
pub use live::{
    BRIDGE_STALE_AFTER, BridgeReading, BridgeReadings, GpuMetric, GpuRef, LiveKey, LiveUnit,
    LiveValue, TempBand, band, resolve,
};
#[allow(unused_imports)]
pub use model::{
    Completeness, Group, Item, NOT_IMPLEMENTED, NOT_REPORTED, Row, Section, SectionHealth,
    SectionId, SectionState, SummaryLine, Value,
};
#[allow(unused_imports)]
pub use report::{HIDDEN, LiveSource, probe_text, section_text, text};
#[allow(unused_imports)]
pub use worker::{
    BUDGET, CADENCE, Context, Entry, LiveProvider, Monitor, Provider, SLOW_AFTER, Snapshot,
    provider,
};
