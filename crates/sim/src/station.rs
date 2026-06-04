//! A station. Its id is its index in `World.stations` (index-ordered, stable).
use crate::geo_local::PointMm;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Station {
    pub pos: PointMm,
    pub name: String,
    /// Tombstone: a bulldozed station keeps its slot (ids are indices, never shifted) but is
    /// skipped by demand capture, routing, views, and counts. Determinism-safe (in state_hash).
    #[serde(default)]
    pub removed: bool,
}

impl Station {
    pub fn new(pos: PointMm, name: String) -> Self {
        Self { pos, name, removed: false }
    }
}
