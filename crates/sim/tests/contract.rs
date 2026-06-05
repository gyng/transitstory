//! The hand-mirrored wire contract (AGENTS code-org: types.ts + codec.ts mirror crates/sim;
//! drift is a bug). These tests pin the externally-tagged Command/Event variant vocabulary so
//! adding or renaming a variant in Rust FORCES a deliberate update here — the canonical list the
//! frontend mirror (packages/app/src/types.ts + the codec contract test) is checked against.
use sim::*;

/// The single externally-tagged variant key of a serialized enum value.
fn sole_tag<T: serde::Serialize>(v: &T) -> String {
    let json = serde_json::to_string(v).expect("serialize");
    let val: serde_json::Value = serde_json::from_str(&json).expect("json");
    let obj = val.as_object().expect("externally-tagged enum is a JSON object");
    assert_eq!(obj.len(), 1, "exactly one variant tag in {json}");
    obj.keys().next().unwrap().clone()
}

fn sorted(mut v: Vec<&str>) -> Vec<String> {
    v.sort_unstable();
    v.into_iter().map(String::from).collect()
}

#[test]
fn command_variant_tags_match_the_frontend_mirror() {
    let samples = [
        Command::PlaceStation { x_mm: 0, y_mm: 0, name: None },
        Command::CreateLine { color: 0, name: None, loop_line: false, mode: 0 },
        Command::AddStop { line: LineId(0), station: StationId(0), after: None },
        Command::AssignTrainset { line: LineId(0), spec: 0, count: 1 },
        Command::SetHeadway { line: LineId(0), headway_ms: 0 },
        Command::SetSegmentMode { line: LineId(0), span: 0, mode: 0 },
        Command::SetRunning { running: false },
        Command::SetEconomy { enabled: false },
        Command::SetLineWaypoints { line: LineId(0), waypoints: vec![] },
        Command::SetDemandMode { agents: false },
        Command::RemoveStation { station: StationId(0) },
        Command::RemoveLine { line: LineId(0) },
    ];
    let mut tags: Vec<String> = samples.iter().map(sole_tag).collect();
    tags.sort();
    // Mirror: packages/app/src/types.ts `Command` + codec.ts `cmd.*`.
    let expected = sorted(vec![
        "AddStop", "AssignTrainset", "CreateLine", "PlaceStation", "RemoveLine", "RemoveStation",
        "SetDemandMode", "SetEconomy", "SetHeadway", "SetLineWaypoints", "SetRunning", "SetSegmentMode",
    ]);
    assert_eq!(tags, expected, "Command vocabulary drifted from the frontend mirror");
}

#[test]
fn event_variant_tags_match_the_frontend_mirror() {
    let samples = [
        Event::StationPlaced { id: StationId(0), name: String::new() },
        Event::LineCreated { id: LineId(0) },
        Event::StopAdded { line: LineId(0), station: StationId(0) },
        Event::TrainsetAssigned { line: LineId(0), count: 0 },
        Event::HeadwaySet { line: LineId(0), headway_ms: 0 },
        Event::SegmentModeSet { line: LineId(0), span: 0, mode: 0 },
        Event::RunningSet { running: false },
        Event::EconomySet { enabled: false },
        Event::WaypointsSet { line: LineId(0) },
        Event::DemandModeSet { agents: false },
        Event::StationRemoved { station: StationId(0) },
        Event::LineRemoved { line: LineId(0) },
        Event::Rejected { reason: String::new() },
    ];
    let mut tags: Vec<String> = samples.iter().map(sole_tag).collect();
    tags.sort();
    // Mirror: packages/app/src/types.ts `Event`.
    let expected = sorted(vec![
        "DemandModeSet", "EconomySet", "HeadwaySet", "LineCreated", "LineRemoved", "Rejected", "RunningSet",
        "SegmentModeSet", "StationPlaced", "StationRemoved", "StopAdded", "TrainsetAssigned", "WaypointsSet",
    ]);
    assert_eq!(tags, expected, "Event vocabulary drifted from the frontend mirror");
}
