//! The command/event vocabulary — the sim's only write port. Every player mutation is
//! one serializable `Command` applied via `World::apply`; the sim emits `Event`s the
//! frontend reads back (assigned ids, auto-names). Save = seed + ordered command log.
//!
//! Wire format: externally-tagged serde (the default), e.g.
//! `{"PlaceStation":{"x_mm":0,"y_mm":0,"name":null}}`. This is the one enum shape that
//! round-trips through BOTH JSON (the live command wire) and postcard (the save artifact);
//! internally-tagged would break postcard, which is not self-describing.
//!
//! Positions are local millimetres (i64) — the sim NEVER sees lng/lat.
use crate::ids::{LineId, StationId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    PlaceStation {
        x_mm: i64,
        y_mm: i64,
        #[serde(default)]
        name: Option<String>,
    },
    CreateLine {
        color: u32,
        #[serde(default)]
        name: Option<String>,
        /// Circular line (last stop connects back to the first) vs out-and-back.
        #[serde(default)]
        loop_line: bool,
    },
    AddStop {
        line: LineId,
        station: StationId,
        #[serde(default)]
        after: Option<usize>,
    },
    AssignTrainset {
        line: LineId,
        spec: u8,
        count: u16,
    },
    SetHeadway {
        line: LineId,
        headway_ms: i64,
    },
    /// Build mode for one inter-stop span (0=Surface,1=Elevated,2=Tunnel); span=u32::MAX sets
    /// every span of the line (whole-line toggle).
    SetSegmentMode {
        line: LineId,
        span: u32,
        mode: u8,
    },
    SetRunning {
        running: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    StationPlaced { id: StationId, name: String },
    LineCreated { id: LineId },
    StopAdded { line: LineId, station: StationId },
    TrainsetAssigned { line: LineId, count: u16 },
    HeadwaySet { line: LineId, headway_ms: i64 },
    SegmentModeSet { line: LineId, span: u32, mode: u8 },
    RunningSet { running: bool },
    Rejected { reason: String },
}
