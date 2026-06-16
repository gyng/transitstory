//! A station. Its id is its index in `World.stations` (index-ordered, stable).
use crate::geo_local::PointMm;
use serde::{Deserialize, Serialize};

/// Maximum platform berths a station can be built to (TTD L2, docs/ttd-track-model.md). A small cap so a
/// station's berth-mutex key space stays tiny and the lateral render footprint stays sane.
pub const MAX_PLATFORMS: u8 = 4;

/// serde default for the L2 platform count — a station always has at least ONE berth, so an old save (or a
/// pre-L2 fixture) deserializes to the single-platform behaviour that's byte-identical to today.
fn default_platform_count() -> u8 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Station {
    pub pos: PointMm,
    pub name: String,
    /// Tombstone: a bulldozed station keeps its slot (ids are indices, never shifted) but is
    /// skipped by demand capture, routing, views, and counts. Determinism-safe (in state_hash).
    #[serde(default)]
    pub removed: bool,
    /// Platform berth count K (TTD L2): up to K consists dwell here in PARALLEL (a follower pulls into a
    /// free berth instead of holding the block gap behind a dwelling train). Hashed state (K changes
    /// behaviour); **default 1 ⇒ byte-identical to pre-L2** (the single platform every station has today).
    /// The per-berth occupancy is DERIVED each tick from the dwell order, never stored. Built via
    /// `Command::BuildPlatforms`, clamped to `[1, MAX_PLATFORMS]`.
    #[serde(default = "default_platform_count")]
    pub platform_count: u8,
}

impl Station {
    pub fn new(pos: PointMm, name: String) -> Self {
        Self { pos, name, removed: false, platform_count: 1 }
    }
}
