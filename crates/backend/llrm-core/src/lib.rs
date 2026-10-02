#![forbid(unsafe_code)]

pub mod abi;
pub mod analysis;
pub mod backend;
pub mod driver;
pub mod flow;
pub mod frontends;
pub mod hir;
pub use llrm_bcmachine::legacy;
pub mod model;
pub use llrm_omf as objectfile;
pub mod optimize;
pub use llrm_support as support;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
