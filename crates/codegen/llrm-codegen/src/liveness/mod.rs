//! LLVM's live-range data structures, translated from llvm/lib/CodeGen and
//! llvm/include/llvm/CodeGen at d1106deb71cc (see each file for the C++ it
//! translates and its DEVIATEs).

pub mod blockfrequency;
pub mod calcspillweights;
pub mod cfg;
pub mod eqclasses;
pub mod intervalmap;
pub mod liveintervals;
pub mod liveintervalunion;
pub mod liverange;
pub mod liverangecalc;
pub mod livevariables;
pub mod slotindex;
