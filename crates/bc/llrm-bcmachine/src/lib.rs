//! The machine layer of BC objects: x86 decoded into the machine IR
//! (`Semantics`, `Loc`, `Mem`), blocks, extents, stack facts and the runtime
//! ABI. A crate of its own so that nothing here can reach MIR. Module paths
//! mirror `llrm-core`, which re-exports them.

#![forbid(unsafe_code)]

pub mod abi;
pub mod analysis;
pub mod frontends;
pub mod legacy;
pub mod model;
pub use llrm_omf as objectfile;
pub use llrm_support as support;
