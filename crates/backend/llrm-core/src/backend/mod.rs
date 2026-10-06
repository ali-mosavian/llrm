//! Ports of `qbopt/backend`.

pub mod addressforms;
pub mod classes;
pub mod affine;
pub mod addressvalues;
pub mod lirtext;
pub mod callregs;
pub mod assemble;
pub mod allocate;
pub mod arithmetic;
pub mod asm;
pub mod coalesce;
pub mod codeview;
pub mod comparefold;
pub mod constrain;
pub mod copyprop;
pub mod copysink;
pub mod cpu;
pub mod datagroup;
pub mod division;
pub mod ehprepare;
pub mod selects;
pub mod farcall;
pub mod farload;
pub mod constpool;
pub mod floatalloc;
pub mod floatassign;
pub mod floatregions;
pub mod inline_asm;
pub mod frame;
pub mod slots;
pub mod stackusage;
pub mod jumps;
pub mod lanes;
pub mod layout;
pub mod lifetimes;
pub mod liveness;
pub mod globals;
pub mod isel;
#[cfg(test)]
mod isel_tests;
pub mod loopslots;
pub mod lower_int64;
pub mod machinecse;
pub mod machinedce;
pub mod masm;
pub mod nativeframe;
pub mod nearcode;
pub mod objbuild;
pub mod overlap;
pub mod parcopy;
pub mod peep;
pub mod postings;
pub mod peephole;
pub mod phielim;
pub mod pointers;
pub mod prologue;
pub mod regthrash;
pub mod rmw;
pub mod exactaddress;
pub mod executed;
pub mod schedule;
pub mod shrinkwrap;
/// The x86 encoder is `llrm_x86::select`; the tests of this crate encode in real mode through `emit`.
pub mod select {
    pub use llrm_x86::select::*;

    #[cfg(test)]
    pub fn emit(what: &crate::model::ir::Semantics, at: u64, r#where: Option<Where<'_>>, short: bool, relocated: bool, held: Option<&HeldMap>) -> Option<Emitted> {
        emit_in(BITNESS, what, at, r#where, short, relocated, held)
    }
}
pub mod spiller;
pub mod ssarepair;
pub mod regclass;
pub mod ssaspill;
pub mod spillforward;
pub mod spillplacement;
pub mod splitkit;
pub mod sharedstores;
pub mod storecombine;
pub mod target;
pub mod timing;
pub mod twoaddr;
pub mod upperzero;
pub mod verify;

#[cfg(test)]
mod pricing_tests;
#[cfg(test)]
pub mod regalloc_input;
#[cfg(test)]
mod regalloc_total_tests;
#[cfg(test)]
mod regalloc_fuzz_tests;
#[cfg(test)]
mod ssaspill_tests;
