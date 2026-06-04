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

    /// Cartesian point (mm) at forward arc-length `s_mm` (integer interpolation).
    pub fn point_at(&self, s_mm: i64) -> (i64, i64) {
        match self.polyline.len() {
            0 => return (0, 0),
            1 => return (self.polyline[0].x_mm, self.polyline[0].y_mm),
            _ => {}
        }
        let s = s_mm.clamp(0, self.length_mm());
        for i in 1..self.arclen_mm.len() {
            if s <= self.arclen_mm[i] {
                let seg_start = self.arclen_mm[i - 1];
                let seg_len = self.arclen_mm[i] - seg_start;
                let a = self.polyline[i - 1];
                let b = self.polyline[i];
                if seg_len <= 0 {
                    return (a.x_mm, a.y_mm);
                }
                let t = s - seg_start;
                return (
                    a.x_mm + (b.x_mm - a.x_mm) * t / seg_len,
                    a.y_mm + (b.y_mm - a.y_mm) * t / seg_len,
                );
            }
        }
        let last = self.polyline[self.polyline.len() - 1];
        (last.x_mm, last.y_mm)
    }

    /// Heading (radians, render-only float) of the segment at forward arc-length `s_mm`.
    pub fn heading_at(&self, s_mm: i64) -> f32 {
        if self.polyline.len() < 2 {
            return 0.0;
        }
        let s = s_mm.clamp(0, self.length_mm());
        for i in 1..self.arclen_mm.len() {
            if s <= self.arclen_mm[i] {
                let a = self.polyline[i - 1];
                let b = self.polyline[i];
                return ((b.y_mm - a.y_mm) as f32).atan2((b.x_mm - a.x_mm) as f32);
            }
        }
        0.0
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
