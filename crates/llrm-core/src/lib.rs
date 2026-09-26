#![forbid(unsafe_code)]

pub mod abi;
pub mod analysis;
pub mod backend;
pub use llrm_cycles as cycles;
pub mod flow;
pub mod frontends;
pub mod hir;
pub mod legacy;
pub mod model;
pub use llrm_omf as objectfile;
pub mod optimize;
pub mod rewrite;
pub use llrm_support as support;
pub mod tools;
pub mod wholeseg;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
