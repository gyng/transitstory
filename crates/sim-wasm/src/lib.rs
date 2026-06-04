//! onlytransits — wasm-bindgen facade (the ONLY wasm-aware crate).
//!
//! Thin translation membrane over `sim`: decode commands, drive ticks, copy-out SoA
//! buffers, marshal stats. NO game logic lives here. The real `Sim` facade lands in T8.
use wasm_bindgen::prelude::*;

/// Smoke export so the scaffold builds and a wasm-pack/node smoke can confirm the
/// module instantiates (guards wasm-bindgen crate/CLI version skew). Replaced in T8.
#[wasm_bindgen]
pub fn ping() -> i32 {
    1
}
