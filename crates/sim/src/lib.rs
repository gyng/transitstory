//! transitstory — deterministic simulation core.
//!
//! PURE: no IO, no wall-clock, no threads, no wasm-bindgen, no float-Mercator/lng-lat,
//! no `std::HashMap` iteration. Time is `i64` sim-milliseconds, positions are `i64`
//! millimetres. Same seed + same ordered command log => identical `state_hash` (the
//! determinism contract, re-gated at every commit). See AGENTS.md.
#![forbid(unsafe_code)]

pub mod agents;
pub mod army;
pub mod city;
pub mod command;
pub mod decadence;
pub mod decadence_field;
mod demand;
mod dispatch;
pub mod forge;
pub mod geo_local;
pub mod hash;
pub mod hexgrid;
pub mod ids;
pub mod journey;
pub mod line;
pub mod pax;
pub mod raider;
pub mod render_buf;
pub mod roadnav;
pub mod routing;
pub mod ruleset;
pub mod station;
pub mod spell;
pub mod stats;
pub mod tech;
mod tick;
pub mod tod;
pub mod track_graph;
pub mod trainset;
pub mod vehicle;
pub mod walkshed;
pub mod world;

pub use city::{BuildCell, BuildabilityGrid, CityData, DemandCell, DemandGrid};
pub use decadence_field::DecadenceField;
pub use command::{Command, Event};
pub use geo_local::PointMm;
pub use ids::{LineId, PaxId, StationId, TrackSegmentId, TrainsetId, VehicleId};
pub use line::{Branch, Line, Path};
pub use pax::Pax;
pub use routing::{plan_route, BfsRouter, Leg, RaptorRouter, Router, DEFAULT_MAX_LEGS};
pub use ruleset::{
    AgentDemand, ArcadiaRuleset, Demand, GravityDemand, Ruleset, SupplyChainDemand, TransitRuleset,
};
pub use station::Station;
pub use stats::{LineStat, ShedCell, StationStat, StatsSnapshot};
pub use trainset::{TrainsetAssignment, TrainsetSpec};
pub use vehicle::VehicleSoA;
pub use world::{
    replay, SaveGame, Signal, World, DEFAULT_HEADWAY_MS, FARE, MAX_HEADWAY_MS, MAX_TRAINS_PER_LINE,
    MIN_HEADWAY_MS, START_BUDGET,
};
