//! The core deserializes the mm-demand city JSON the frontend builds, and tolerates the
//! frontend-only manifest fields (lng/lat) by ignoring them — so the sim never sees lng/lat.
use sim::*;

#[test]
fn parses_core_city_json_with_mm_demand() {
    let json = r#"{"seed":7,"demand":{"cell_m":600.0,"cells":[
        {"x_mm":1000,"y_mm":2000,"origin_w":1.5,"dest_w":0.5},
        {"x_mm":-3000,"y_mm":42000,"origin_w":0.2,"dest_w":2.1}
    ]}}"#;
    let c = CityData::from_json(json).expect("core city json parses");
    assert_eq!(c.seed, 7);
    assert_eq!(c.demand.cells.len(), 2);
    assert_eq!(c.demand.cells[0].x_mm, 1000);
    assert!((c.demand.cells[1].dest_w - 2.1).abs() < 1e-6);

    let w = World::new(c.seed, c);
    assert_eq!(w.city.demand.cells.len(), 2);
}

#[test]
fn ignores_frontend_only_manifest_fields() {
    // The committed singapore_city.json carries lng/lat fields the core does not use.
    let json = r#"{"id":"singapore","name":"Singapore","originLngLat":[103.8,1.3],
        "bbox":[103.55,1.13,104.15,1.5],"center":[103.8,1.35],"zoom":11,"seed":42,
        "demandGridPath":"/data/singapore_demand.json"}"#;
    let c = CityData::from_json(json).expect("manifest parses, extra fields ignored");
    assert_eq!(c.seed, 42);
    assert_eq!(c.demand.cells.len(), 0); // no demand embedded in the frontend manifest
}
