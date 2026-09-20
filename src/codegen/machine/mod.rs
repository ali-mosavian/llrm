//! Machine IR and target-independent register allocation.

mod liveness;
mod model;
mod verify;

pub use liveness::{
    BlockLiveness, MachineLiveness, MachineLivenessError, compute_liveness,
};
pub use model::*;
pub use verify::verify;
