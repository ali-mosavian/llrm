//! The arm64 target of the DAG selector: the pin's register numbers, then the
//! rest of what the shared stages ask of a target.

pub mod addr;
pub mod bitfield;
pub mod call;
pub mod ccmp;
pub mod combine;
pub mod emit;
pub mod function;
pub mod globals;
pub mod hooks;
pub mod ids;
pub mod lowering;
pub mod pipeline;
