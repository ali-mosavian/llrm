//! Port of `qbopt/frontend`'s BC object raise.
//!
//! Machine form, in `llrm-x86-bcmachine` so it cannot reach MIR: `blocks`,
//! `declen`, `escaped`, `extent`, `fppatches`, `raising_control`,
//! `raising_returns`, `stack`. The modules declared here raise into the
//! old MIR and are to be ported.

pub use llrm_x86_bcmachine::frontends::bc::{blocks, declen, extent, fppatches, raising_control, stack};
