//! The SelectionDAG's type and action tables, below the passes that ask them:
//! the pin's `MVT`, `ISD`, `TargetLoweringInfo` and the x86 `TargetLowering`
//! action tables. `llrm-codegen`'s `selectiondag` re-exports them; `Lowering`
//! reads the same tables, so there is one copy.
//!
//! # Example
//! ```
//! use llrm_x86::dag::mvt::MVT;
//! assert_eq!(MVT::I32 as u8, MVT::I32 as u8);
//! ```
//!
//! # Runs
//! Built once per target, read by LegalizeDAG and by `Lowering`.

pub mod actions;
pub mod isd;
pub mod lowering;
pub mod mvt;
pub mod x86isd;
