//! Direct ports of Python's production compiler models.
//!
//! These types intentionally remain distinct from the newer portable SSA and
//! Machine IR scaffolding. Python stage parity is established here before any
//! consumer is migrated.

pub mod floating;
pub mod memory;
pub mod mir;
pub(crate) mod mir_loops;
