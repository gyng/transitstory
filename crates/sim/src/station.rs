//! A station. Its id is its index in `World.stations` (index-ordered, stable).
use crate::geo_local::PointMm;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Station {
    pub pos: PointMm,
    pub name: String,
}

impl Station {
    pub fn new(pos: PointMm, name: String) -> Self {
        Self { pos, name }
    }
}
