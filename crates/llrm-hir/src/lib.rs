//! The small, source-language-neutral IR above MIR: its model, JSON codec,
//! verifier and escape facts. Lowering to MIR lives in `llrm-core`.

pub mod codec;
pub mod escape;
pub mod model;
pub mod verify;
