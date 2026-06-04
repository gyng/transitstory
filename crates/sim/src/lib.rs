//! onlytransits — deterministic simulation core.
//!
//! PURE: no IO, no wall-clock, no threads, no wasm-bindgen, no float-Mercator/lng-lat,
//! no `std::HashMap` iteration. Time is `i64` sim-milliseconds, positions are `i64`
//! millimetres. Same seed + same ordered command log => identical `state_hash` (the
//! determinism contract, re-gated at every commit). See AGENTS.md.
#![forbid(unsafe_code)]

pub mod city;
pub mod command;
pub mod geo_local;
pub mod hash;
pub mod ids;
pub mod line;
pub mod station;
mod tick;
pub mod trainset;
pub mod world;

pub use city::{CityData, DemandCell, DemandGrid};
pub use command::{Command, Event};
pub use geo_local::PointMm;
pub use ids::{LineId, PaxId, StationId, TrainsetId, VehicleId};
pub use line::Line;
pub use station::Station;
pub use trainset::{TrainsetAssignment, TrainsetSpec};
pub use world::{
    replay, SaveGame, World, DEFAULT_HEADWAY_MS, MAX_HEADWAY_MS, MAX_TRAINS_PER_LINE,
    MIN_HEADWAY_MS,
};
