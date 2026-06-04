//! onlytransits — deterministic simulation core.
//!
//! PURE: no IO, no wall-clock, no threads, no wasm-bindgen, no float-Mercator/lng-lat,
//! no `std::HashMap` iteration. Time is `i64` sim-milliseconds, positions are `i64`
//! millimetres. Same seed + same ordered command log => identical `state_hash` (the
//! determinism contract, re-gated at every commit). See AGENTS.md.
#![forbid(unsafe_code)]

// Real modules land in T2+ (world, command, tick, ids, ...). Kept minimal so the
// scaffold commit builds.
