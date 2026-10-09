//! Ports of `qbopt/backend`.

pub mod addressforms;
pub mod addressvalues;
pub mod affine;
pub mod allocate;
pub mod arithmetic;
pub mod arrival;
pub mod asm;
pub mod assemble;
pub mod calleefacts;
pub mod callregs;
pub mod cfi;
pub mod classes;
pub mod coalesce;
pub mod comparefold;
pub mod constpool;
pub mod constrain;
pub mod copyprop;
pub mod copysink;
pub mod cpu;
pub mod datagroup;
pub mod debuginfo;
pub mod division;
pub mod ehprepare;
pub mod exactaddress;
pub mod executed;
pub mod farcall;
pub mod farload;
pub mod floatalloc;
pub mod floatassign;
pub mod floatfold;
pub mod floatregions;
pub mod fpconvert;
pub mod frame;
pub mod framefree;
pub mod globals;
pub mod inline_asm;
pub mod isel;
#[cfg(test)]
mod isel_tests;
pub mod jumps;
pub mod lanes;
pub mod layout;
pub mod lifetimes;
pub mod lirtext;
pub mod liveness;
pub mod liveunion;
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
pub mod peephole;
pub mod phielim;
pub mod pointers;
pub mod postings;
pub mod postrasink;
pub mod pressuresink;
pub mod prologue;
pub mod regthrash;
pub mod relayout;
pub mod rmw;
pub mod schedule;
pub mod selects;
pub mod shrinkwrap;
pub mod slots;
pub mod stackusage;
pub mod valuetrack;
/// The x86 encoder is `llrm_x86::select`; the tests of this crate encode in
/// real mode through `emit`.
pub mod select {
    pub use llrm_x86::select::*;

    /// A deliberate model simplification (pricing every frame cell as near
    /// costs two bytes of the objects, accepted): see below. `emit_in`, for
    /// a price: the bytes an instruction takes, or whether it encodes. A frame
    /// cell is priced as a near one (a one-byte displacement): where it
    /// ends up is for the frame layout to decide after the machine phases,
    /// and a price that knew a displacement would be wrong the moment the
    /// layout moved. Incoming arguments, whose place is fixed, keep theirs.
    #[allow(clippy::too_many_arguments)]
    pub fn priced_in(
        bits: u32,
        what: &crate::model::ir::Semantics,
        at: u64,
        r#where: Option<Where<'_>>,
        short: bool,
        relocated: bool,
        held: Option<&HeldMap>,
    ) -> Option<Emitted> {
        use crate::model::ir::{Addr, Loc};
        /// Where a frame cell is priced.
        const NEAR: i64 = -8;
        // A frame cell below BP whose place is not fixed: not an incoming
        // argument.
        let placed = |addr: &Option<Addr>, in_frame: bool| {
            addr.is_some_and(|addr| in_frame && addr.disp < 0 && addr.slot_home() != Some(0))
        };
        let moves = |place: &Loc| match place {
            Loc::Mem(cell) => placed(&cell.addr, cell.in_frame()),
            Loc::Address(cell) => placed(&cell.addr, cell.in_frame()),
            _ => false,
        };
        if !what.dests.iter().chain(&what.sources).any(moves) {
            return emit_in(bits, what, at, r#where, short, relocated, held);
        }
        let near = |place: &Loc| match place {
            Loc::Mem(cell) if placed(&cell.addr, cell.in_frame()) => Loc::Mem(crate::model::ir::Mem {
                addr: cell.addr.map(|addr| Addr { disp: NEAR, ..addr }),
                ..cell.clone()
            }),
            Loc::Address(cell) if placed(&cell.addr, cell.in_frame()) => Loc::Address(crate::model::ir::Address {
                addr: cell.addr.map(|addr| Addr { disp: NEAR, ..addr }),
                ..cell.clone()
            }),
            other => other.clone(),
        };
        let priced = crate::model::ir::Semantics {
            dests: what.dests.iter().map(&near).collect(),
            sources: what.sources.iter().map(&near).collect(),
            ..what.clone()
        };
        emit_in(bits, &priced, at, r#where, short, relocated, held)
    }

    #[cfg(test)]
    pub fn emit(
        what: &crate::model::ir::Semantics,
        at: u64,
        r#where: Option<Where<'_>>,
        short: bool,
        relocated: bool,
        held: Option<&HeldMap>,
    ) -> Option<Emitted> {
        emit_in(BITNESS, what, at, r#where, short, relocated, held)
    }
}
pub mod regclass;
pub mod sharedstores;
pub mod spiller;
pub mod spillforward;
pub mod spillplacement;
pub mod splitkit;
pub mod ssarepair;
pub mod ssaspill;
pub mod storecombine;
pub mod storedhomes;
pub mod target;
pub mod timing;
pub mod twoaddr;
pub mod upperzero;
pub mod verify;

#[cfg(test)]
mod pricing_tests;
#[cfg(test)]
mod regalloc_fuzz_tests;
#[cfg(test)]
pub mod regalloc_input;
#[cfg(test)]
mod regalloc_total_tests;
#[cfg(test)]
mod ssaspill_tests;
