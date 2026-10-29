//! The arm64 target of the DAG selector: the pin's register numbers, then the
//! rest of what the shared stages ask of a target.

pub mod addr;
pub mod call;
pub mod emit;
pub mod hooks;
pub mod function;
pub mod globals;
pub mod ids;
pub mod pipeline;
