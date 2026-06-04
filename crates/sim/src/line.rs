//! A transit line: an ordered list of station stops (out-and-back topology), a color,
//! a headway, and an optional trainset assignment. Polyline + cumulative arc-length are
//! cached from the stop positions (recomputed when stops change) for T14 vehicle motion.
use crate::geo_local::PointMm;
use crate::ids::StationId;
use crate::trainset::TrainsetAssignment;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Line {
    pub color: u32,
    pub stops: Vec<StationId>,
    pub headway_ms: i64,
    pub trainset: Option<TrainsetAssignment>,
    /// Stop positions in mm, in stop order. Cached from `World.stations` on change.
    pub polyline: Vec<PointMm>,
    /// Cumulative arc-length (mm) at each polyline vertex; `arclen[0] == 0`.
    pub arclen_mm: Vec<i64>,
}

impl Line {
    pub fn new(color: u32, default_headway_ms: i64) -> Self {
        Self {
            color,
            stops: Vec::new(),
            headway_ms: default_headway_ms,
            trainset: None,
            polyline: Vec::new(),
            arclen_mm: Vec::new(),
        }
    }

    /// One-way length of the line in mm (0 if fewer than 2 stops).
    pub fn length_mm(&self) -> i64 {
        self.arclen_mm.last().copied().unwrap_or(0)
    }

    /// Recompute the cached polyline + cumulative arc-length from current stop positions.
    pub fn rebuild_geometry(&mut self, station_pos: impl Fn(StationId) -> PointMm) {
        self.polyline = self.stops.iter().map(|&s| station_pos(s)).collect();
        self.arclen_mm.clear();
        let mut acc = 0i64;
        for (i, p) in self.polyline.iter().enumerate() {
            if i == 0 {
                self.arclen_mm.push(0);
            } else {
                acc += self.polyline[i - 1].dist_mm(p);
                self.arclen_mm.push(acc);
            }
        }
    }
}
