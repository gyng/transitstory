//! Strongly-typed entity ids. Newtype structs serialize transparently (as bare `u32`)
//! so JSON commands carry plain numbers while Rust keeps a LineId from being passed
//! where a StationId is expected.
use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub u32);

        impl $name {
            #[inline]
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

id_type!(StationId);
id_type!(LineId);
id_type!(TrainsetId);
id_type!(VehicleId);
id_type!(PaxId);
// TTD L3 (docs/ttd-l3-plan.md): the stable identity of a first-class TrackSegment. Introduced additively in
// A0 (a Line will reference `Vec<(TrackSegmentId, reverse)>` once segments own geometry); not hashed until
// the L3 C1 ownership-flip, where the id MUST stay a pure function of topology (sorted endpoint cells).
id_type!(TrackSegmentId);
