//! The backend: MIR lowered for selection, instruction selection, the machine
//! phases over LIR, and emission to an object.
//!
//! # How it works
//! Each module here is one phase, one analysis a phase uses, or a helper; its
//! own header says which. The order of the phases is below. Everything after
//! selection works on LIR (`LirBody`): in SSA over virtual registers until
//! `phielim`, over virtual registers with copy groups until `allocate`, then
//! over physical registers.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   subgraph g_drv["DRIVER::OPTIMIZED"]
//!     mir["<b>MIR after</b><br/><small>llrm_transforms::pipeline</small>"]:::mid
//!     prep["<b>assumptions dropped</b><br/><small>ehprepare, fpconvert*<br/>selects*  (* if the target<br/>needs it)</small>"]:::mid
//!   end
//!   subgraph g_asm["ASSEMBLE"]
//!     near["<b>nearcode</b><br/><small>globals::names<br/>functions callees first</small>"]:::mid
//!     staged["<b>machined: staged</b><br/><small>isel, framefree, frame::of</small>"]:::mid
//!     once["<b>machined_once</b><br/><small>cheaper of allocator alone<br/>and spiller route<br/>(flow::machine_with)</small>"]:::mid
//!     relay["<b>relayout::laid_again</b><br/><small>if far frame operands remain</small>"]:::mid
//!     omax["<b>-Omax: again without</b><br/><small>callee facts, keep cheaper</small>"]:::mid
//!     proc["<b>per procedure</b><br/><small>addressvalues::converted<br/>masm::cleaned_returns<br/>jumps::duplicated_returns</small>"]:::mid
//!     all["<b>all procedures</b><br/><small>stackusage::elide_checks<br/>debuginfo::described</small>"]:::mid
//!   end
//!   mod["<b>masm::Module</b>"]:::opt
//!   obj["<b>objbuild</b>"]:::out
//!   wr["<b>writer crate</b>"]:::out
//!   mir --> prep
//!   prep --> near
//!   near -->|"per function"| staged
//!   staged --> once
//!   once --> relay
//!   relay --> omax
//!   omax --> proc
//!   proc --> all
//!   all --> mod
//!   mod -->|"data layout (driver)"| obj
//!   obj --> wr
//!   style g_drv fill:#47556912,stroke:#475569,stroke-width:1.6px,stroke-dasharray:7 5,color:#475569
//!   style g_asm fill:#47556912,stroke:#475569,stroke-width:1.6px,stroke-dasharray:7 5,color:#475569
//! ```
//!
//! *From optimized MIR to the object writer*
//!
//! The machine phases, in the order of [`crate::flow::machine_with`]. Each
//! runs on the whole body and is verified afterwards under `LLRM_DEBUG=verify`
//! (`flow::checked`).
//!
//! ```text
//!   #  phase (name)             module        notes
//!   1  far-indirect-calls       farcall       memory operand for a far call
//!   2  SharedConstants          phiconst      one value for a constant several phi edges carry
//!   4  liverangeshrink          liverangeshrink  instruction up to its operands' last def
//!   4  phielim                  phielim       phis -> copy groups; SSA ends
//!   6  floatfold                floatfold     float load as memory operand
//!   7  floatassign              floatassign   x87 stack slots per value
//!   8  floatalloc               floatalloc    x87 stack form
//!   9  TwoAddress               twoaddr       c := a+b  ->  c := a; c += b
//!  10  Coalescer                coalesce      remove a copy that can share
//!  11  RegAlloc                 allocate      assign registers, split, spill
//!  12  parcopy                  parcopy       sequence the copy groups
//!  13  prologue                 prologue      reserve the frame; skipped by
//!                                             assemble, masm writes it
//!  14  peephole                 peephole      one sweep of local rewrites
//!  15  loopslots                loopslots     loop spill slots in registers
//!  16  schedule                 schedule      latency-aware order
//!  17  jumps                    jumps         block order, jumps, tails
//! ```
//!
//! Phases 4 to 9 are in `machine_with` before `allocate`; `allocate` calls
//! `spillplacement`, `slots` and `postings` for live-range splitting and
//! spilling, and consults `constrain`, `classes`,
//! `registerinfo` and `datagroup` for what each value may be assigned.
//! `peephole` calls the sub-passes `machinecp`, `copysink`,
//! `postrasink`, `storecombine`, `sharedstores`,
//! `exactaddress`, `machinedce` and `upperzero`, and the rule
//! groups of `peep`.
//!
//! The analyses the phases share: `liveness` and `lanes` (register and flag
//! lanes), `live` and `neighbours` (intervals and interference for
//! the allocator), `overlap` (memory operands),
//! `lifetimes` and `slots` (frame-slot sharing), `executed` and `timing`
//! (what a candidate costs) and `effects` (what an instruction reads and
//! writes). `needs` lets the peephole skip a sub-pass with nothing to act on.
//!
//! # Example
//! The LIR of one function after each machine phase on the allocator route, as
//! `ISEL_DUMP=1` prints it (the instruction count follows the stage name):
//!
//! ```text
//! === FarIndirectCalls   total: 18 instructions
//! === PhiElimination     total: 27 instructions
//! === FloatFold          total: 27 instructions
//! === FloatAssign        total: 27 instructions
//! === FloatAlloc         total: 27 instructions
//! === TwoAddress         total: 27 instructions
//! === Coalescer          total: 19 instructions
//! === RegAlloc           total: 21 instructions
//! === ParallelCopy       total: 21 instructions
//! === Peephole           total: 21 instructions
//! === LoopSlots          total: 21 instructions
//! === Scheduler          total: 21 instructions
//! === ControlFlow        total: 25 instructions
//! ```
//!
//! From `ISEL_DUMP=1` of a `total` over a `&[u16]` (`-fno-inline-functions`) at
//! `-m16 -O2`.
//!
//! # Runs
//! `assemble::assembled_by` runs it once per module. The gating of the
//! allocation search (`-fallocation-search`,
//! `-fallocation-search-all`, at -Os candidates priced in bytes) is described
//! in `assemble` and `allocate`. `ISEL_DUMP=1` prints the LIR after each
//! machine phase (`lirtext`).
//!
//! # See also
//! [`crate::flow`] (the phase list and its gate) and
//! The phase map: [`crate::backend`].

pub mod absolute;
pub mod addressforms;
pub mod addressvalues;
pub mod affine;
pub mod allocate;
pub mod allocresult;

pub mod arithmetic;
pub mod arrival;
pub mod asm;
pub mod assemble;
pub mod bitcounts;
pub mod bytes;
pub mod calleefacts;
pub mod callregs;
pub mod cfi;
pub mod classes;
pub mod combiner;
pub mod comparefold;
pub mod compares;
pub mod constpool;
pub mod constrain;
pub mod copysink;
pub mod cpu;
pub mod datagroup;
pub mod debuginfo;
pub mod division;
pub mod effects;
pub mod ehprepare;
pub mod exactaddress;
pub mod executed;
pub use llrm_transforms::expansion;
pub mod farcall;
pub mod farload;
pub mod floatalloc;
pub mod floatassign;
pub mod floatfold;
pub mod floatregions;
pub mod fpconvert;
pub mod fpcopyprop;
pub mod fpregs;
pub mod fpstack;
pub mod frame;
pub mod framefree;
pub mod framelowering;
pub mod frameobjects;
pub mod funnel;
pub mod globals;
pub mod inline_asm;
pub mod isel;
pub mod liverows;
pub mod masks;
pub mod valuefacts;

/// Every target's peephole rules, as its definition directory is named.
pub mod targets {
    include!(concat!(env!("OUT_DIR"), "/peep_targets.rs"));
}

/// Every target's generated selector, bound by name (`isel::selector`).
pub mod selectors {
    use super::isel::api::Compiled;

    include!(concat!(env!("OUT_DIR"), "/selectors.rs"));
}
pub mod booleans;
pub mod callframe;
pub mod cmpsub;
pub mod deadmi;
pub mod earlytaildup;
pub mod edges;
pub mod fixupbw;
pub mod fixupleas;
pub mod fixupsetcc;
pub mod flagscopy;
pub mod incremental;
pub mod intbits;
#[cfg(test)]
mod isel_tests;
pub mod jumps;
pub mod lanes;
pub mod latecleanup;
pub mod layout;
pub mod leaopt;
pub mod legalize;
pub mod libmcalls;
#[cfg(test)]
mod libmcalls_tests;
pub mod lifetimes;
pub mod lirtext;
pub mod live;
pub mod liveness;
pub mod liverangeshrink;
pub mod loopslots;
pub mod lower_int64;
pub mod machinecp;
pub mod machinedce;
pub mod machinelicm;
pub mod machinesched;
pub mod machinesink;
pub mod masm;
pub mod memcpycall;
#[cfg(test)]
mod memcpycall_tests;
pub mod memsetpattern;
#[cfg(test)]
mod memsetpattern_tests;
pub mod mirprint;
pub mod nativeframe;
pub mod nearcode;
pub mod needs;
pub mod objbuild;
pub mod optimizephis;
pub mod overlap;
pub mod parcopy;
pub mod peep;
pub mod peephole;
pub mod peepholeopt;
pub mod phiconst;
pub mod phielim;
pub mod postings;
pub mod postrasink;
pub mod precse;
pub mod prezeroed;
pub mod prologue;
pub mod pushfold;
pub mod regpressure;
pub mod regthrash;
pub mod relayout;
pub mod returnsplit;
pub mod rmw;
pub mod saverestore;
pub mod schedule;
pub mod selectconstants;
pub mod selectiondag;
pub mod selects;
pub mod shadow;
#[cfg(test)]
mod shadow_tests;
pub mod shrinkwrap;
pub mod slotcopies;
pub mod slots;
pub mod stackusage;
pub mod stepflags;
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
        let moves = |place: &Loc| place.address().is_some_and(|cell| placed(&cell.addr, place.in_frame()));
        if !what.dests.iter().chain(&what.sources).any(moves) {
            return emit_in(bits, what, at, r#where, short, relocated, held);
        }
        let near = |place: &Loc| {
            if !moves(place) {
                return place.clone();
            }
            place.map_address(|cell| crate::model::ir::AddressRef {
                addr: cell.addr.map(|addr| Addr { disp: NEAR, ..addr }),
                ..cell
            })
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
pub mod bodyfacts;
pub mod regclass;
pub mod registerinfo;
pub mod repbsf;
pub mod rewriter;
pub mod sharedstores;
pub mod spillplacement;
pub mod storecombine;
pub mod storedhomes;
pub mod target;
pub mod timing;
pub mod twoaddr;
pub mod upperzero;
pub mod verify;
pub mod widths;

#[cfg(test)]
mod pricing_tests;
#[cfg(test)]
pub(crate) mod regalloc_fuzz_tests;
#[cfg(test)]
pub mod regalloc_input;
#[cfg(test)]
mod regalloc_total_tests;
