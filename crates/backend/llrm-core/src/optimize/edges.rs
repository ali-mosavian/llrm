//! Port of `qbopt/optimize/edges.py`: place semantic operations on
//! conditional edges, not on either arm.

use crate::model::mir::MirBody;

pub fn fresh(body: &MirBody) -> i64 {
    ((body.entry + 1) << 32).max(body.blocks.iter().map(|block| block.at).max().expect("max() arg is an empty sequence"))
        + 1
}
