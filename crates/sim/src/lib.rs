//! onlytransits — deterministic simulation core.
//!
//! PURE: no IO, no wall-clock, no threads, no wasm-bindgen, no float-Mercator/lng-lat,
//! no `std::HashMap` iteration. Time is `i64` sim-milliseconds, positions are `i64`
//! millimetres. Same seed + same ordered command log => identical `state_hash` (the
//! determinism contract, re-gated at every commit). See AGENTS.md.
#![forbid(unsafe_code)]

pub mod city;
pub mod command;
mod demand;
mod dispatch;
pub mod geo_local;
pub mod hash;
pub mod ids;
pub mod line;
pub mod pax;
pub mod render_buf;
pub mod routing;
pub mod station;
pub mod stats;
mod tick;
pub mod tod;
pub mod trainset;
pub mod vehicle;
pub mod world;

pub use city::{CityData, DemandCell, DemandGrid};
pub use command::{Command, Event};
pub use geo_local::PointMm;
pub use ids::{LineId, PaxId, StationId, TrainsetId, VehicleId};
pub use line::Line;
pub use pax::Pax;
pub use routing::{plan_route, Leg};
pub use station::Station;
pub use stats::{LineStat, StationStat, StatsSnapshot};
pub use trainset::{TrainsetAssignment, TrainsetSpec};
pub use vehicle::VehicleSoA;
pub use world::{
    replay, SaveGame, World, DEFAULT_HEADWAY_MS, MAX_HEADWAY_MS, MAX_TRAINS_PER_LINE,
    MIN_HEADWAY_MS,
};
