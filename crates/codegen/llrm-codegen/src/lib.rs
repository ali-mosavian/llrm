#![forbid(unsafe_code)]

pub mod abi;
pub mod analysis;
pub mod backend;
pub mod coalescer;
pub mod driver;
pub mod flow;
pub mod greedy;
pub mod hir;
pub mod liveness;
pub mod model;
pub mod phielimination;
pub mod processimpdefs;
pub mod twoaddress;
pub use llrm_omf as objectfile;
pub use llrm_support as support;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
