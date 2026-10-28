//! The MIR optimization pipeline every driver runs: the order of the passes,
//! the per-body fixed point, and which options switch them off.
//!
//! # How it works
//! [`recorded`] (and [`applied`], which drops its result) builds a
//! `PassManager` and adds, in this order: whole-module steps, [`Fixed`] over
//! every body, the whole-program [`crate::interprocedural`] step, and loop and
//! address passes that run once, after the program is settled. Every pass
//! after `globalopt` is a function, module or program pass of the manager.
//! With `LLRM_VERIFY` set the module is verified after each manager pass;
//! with `Applied::dump` set each changed step of each body is written there.
//! A body sent back through [`Fixed`] by a later stage starts only the passes
//! that found nothing last time and read nothing that changed since
//! ([`crate::settled`]); `LLRM_CHECK_SETTLED=1` runs them and fails where one
//! changes the body.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef note fill:transparent,stroke:transparent,color:#64748b
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   o0["-O0 (Options::optimize = false)<br/>available-externally only"]:::note
//!   a["<b>A  once, whole module</b><br/><small>globalopt, ports (every body),<br/>stamp</small>"]:::mid
//!   subgraph g_b["B  EACH BODY, TO A FIXED POINT (FIXED, MIR PIPELINE)"]
//!     sroa["<b>sroa</b><br/><small>(boundary#59; also after a copy)</small>"]:::mid
//!     rnd["<b>round, until no pass changes</b><br/><small>the body: fold ports simplifycfg<br/>decide equalcalls tailrec<br/>loopsimplify lcssa floatloop<br/>licm<br/>trivialunswitch inferspace<br/>dse gvn promote indvars<br/>algebraic dead#59; unroll once<br/>per round (a change sends<br/>the copy to sroa)</small>"]:::mid
//!     peel["<b>peel</b><br/><small>while it removes loops:<br/>sroa, then B's rounds</small>"]:::mid
//!     unsw["<b>unswitch (-O3, -Omax)</b><br/><small>simple-loop-unswitch,<br/>non-trivial</small>"]:::mid
//!     swap["<b>interchange (rounds only)</b><br/><small>sroa, then B's rounds</small>"]:::mid
//!     mrg["<b>merge (once)</b>"]:::mid
//!   end
//!   c["<b>C  whole program,</b><br/><small>interprocedural: inline,<br/>constants, dead pure calls<br/>(stages: LLRM_DEBUG=runs)</small>"]:::mid
//!   d["<b>D  globaldce,</b><br/><small>available-externally,<br/>calleepop#59; freeze the<br/>Summaries and GlobalsAA</small>"]:::mid
//!   e["<b>E  once, each body, with the</b><br/><small>analyses frozen<br/>[rotate#123;copy#125; loopsimplify<br/>lcssa fold algebraic<br/>indvars#123;exit#125; dead]: only<br/>with -ftree-ch, not at -Os<br/>fixednarrow lsr differences<br/>window rotate#123;proven#125;<br/>hoist gepoffset<br/>cgpaddr sink</small>"]:::mid
//!   f["<b>F  last: spares, homes</b><br/><small>(instruction ids selection sees)</small>"]:::mid
//!   a --> sroa
//!   sroa --> rnd
//!   rnd --> peel
//!   peel --> unsw
//!   unsw --> swap
//!   swap --> mrg
//!   mrg --> c
//!   c -.->|"a changed body goes<br/>back through B"| sroa
//!   c --> d
//!   d --> e
//!   e --> f
//!   style g_b fill:#47556912,stroke:#475569,stroke-width:1.6px,stroke-dasharray:7 5,color:#475569
//! ```
//!
//! *The mir pipeline, stages A to F*
//!
//! The order of B is the `every` list of [`pipeline`]; [`passes`] returns it
//! as names. [`Fixed`] takes two passes out of it: `sroa` runs at structural
//! boundaries, `merge` once at the end. A pass that is off is not in the list.
//!
//! A round runs the remaining passes in order. The body is at a fixed point
//! when a round changes nothing; a pass reports a change by preserving less
//! than every analysis. A pass that changed nothing is skipped until an edit
//! reaches what it reads (`FunctionPass::reads`, `footprint::of`). The limit
//! is `max(16, size + 1)` rounds, and a round that reproduces an earlier
//! printed body is an error, so an oscillating pair fails at once. `sroa` and
//! `promote` are skipped on a body with an irreducible loop: promotion needs
//! dominators.
//!
//! Peeling repeats while it leaves fewer loops, so an outer loop whose inner
//! loop was peeled is peeled in turn. The passes that make a candidate
//! re-optimize it with a `Fixed` of its own that has unswitching
//! off.
//!
//! In E, `lsr` is after the interprocedural step because nothing after it may
//! choose a loop's counters again; `rotate{proven}` is last of the loop
//! passes; `hoist`
//! runs again to lift what a rotated loop (run at least once) may now leave;
//! `gepoffset` follows it because it would move a constant `gep` out of its
//! loop; `cgpaddr` follows the last `gvn`, which makes the phis it sinks;
//! `sink` follows `cgpaddr`, once the passes that move work up have run.
//!
//! # Example
//! No honest example: the pipeline is the order of the passes, not one rewrite,
//! and no one function shows it. `llrm-c --dump DIR` writes the module after
//! each stage (`NN-stage.ll`) and, for each body, each changed step of the
//! fixed point (`N/NNN-rRR-pass.ll`); the pass examples come from those.
//!
//! # Runs
//! Options are `Options`; `Level::options` in llrm-codegen's `driver/flags.rs`
//! maps `-O0`, `-O1`, `-O2`, `-O3`, `-Omax`, `-Os`, `-Oz`, `-Og` to
//! `Options::none`, `basic`, `standard`, `speed`, `aggressive`, `size`,
//! `min_size` (`-Og`: `basic` without `loopidioms`). `-f` flags set single
//! fields (`PASSES` in the same file). The per-level table, with the gcc and
//! LLVM counterpart of each limit, is
//! [docs/reference/levels.md](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/docs/reference/levels.md).
//!
//! `Options::wanted` gates passes by name, so every pass can be bisected off:
//!
//! | field | passes |
//! |---|---|
//! | `optimize` | all; off, only `available-externally` runs |
//! | `lcssa`, `floatloop`, `fold`, `simplifycfg`, `decide`, `dead` | the pass of that name |
//! | `hoist` | `hoist`, `loopmotion` |
//! | `forward` and `drop_loads` | `gvn`, `equalcalls` |
//! | `drop_stores` | `dse` |
//! | `sink` | `sink` |
//! | `promote` | `sroa`, `promote` |
//! | `strength` | `indvars`, `lsr`, `canonfreeze`, `window`, `gepoffset`, `cgpaddr` |
//! | `loopidioms` | `loopidioms`, `merge`, `memcpyopt` |
//! | `sibcalls` | `tailrec` |
//! | `unswitch`, `interchange`, `unroll_and_jam` | the passes of that name (`unswitch` and `unroll_and_jam` on at -O3 and -Omax; `interchange` on at every level but -O0) |
//! | `copy_headers` | the header copy in E (off by default; `-ftree-ch`) |
//!
//! `pre` makes `gvn` insert on the path that lacks a value; `for_size` prices
//! `hoist`, `algebraic`, `loopidioms`, `lsr`, `window` and
//! `calleepop` in bytes. `limits` are the copy budgets of the unroller.
//!
//! # Needs
//! `Summaries`, `GlobalsAA` and `ProgramSummaries` are required
//! up front, and frozen at the end of D so the passes of E read them without
//! recomputing after each edit. In LLVM's order `GlobalsAA` and
//! `GlobalsModRefAnalysis` are made before the walk and again at the end of D
//! alone (`RequireAnalysisPass`, `RecomputeGlobalsAAPass`), read cached
//! between.
//!
//! # See also
//! gcc's pass lists, `passes.def`; LLVM's `PassBuilderPipelines.cpp`.
//! `LLRM_DEBUG=time` times each step, `runs` counts rounds, skips and the
//! work of passes that changed nothing.

use std::collections::BTreeSet;
use std::path::PathBuf;

use llrm_analysis::cfg;
use llrm_analysis::manager::{GlobalsAA, GlobalsModRefAnalysis, ProgramSummaries, Summaries};
use llrm_analysis::peelsize::Limits;
use llrm_mir::context::GlobalId;
use llrm_mir::module::{GlobalKind, GlobalValue, Linkage, Module, UnnamedAddr};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, ModuleAnalyses, PassManager, PreservedAnalyses, Stage, Unit};
use llrm_mir::print;
use llrm_mir::program::Program;

use crate::interprocedural::Interprocedural;
use crate::parity::Phase;
use crate::settled::{self, Settled};
use crate::{
    adce, algebraic, availableexternally, bdce, calleepop, cgpaddr, constmerge, dead, deadstore, decide, extload,
    fixednarrow, floatloop, fold, gepoffset, globaldce, globalopt, gvn, indvars, inferfunctionattrs, inferspace,
    inline, lcssa, loopdeletion, loopidioms, loopinterchange, looprotate, loopsimplify, loopunrollandjam,
    loopunrollpass, mem2reg, memcpyopt, mldstmotion, ports, promote, rotate, sccp, simpleloopunswitch, simplifycfg,
    switchlower, tailcallelim, trivialunswitch, window,
};

/// Which passes run, and the copy budgets: the old `Options`. The default
/// is -O2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    /// The library routines the compile states the program links (the pin's
    /// `TargetLibraryInfo`, which it reads from the triple: here the front
    /// end's profile states them, as the ABI chosen by `-mabi` spells and
    /// calls them). DEVIATE D11: llrm-c states `strlen` for `-m32` under the
    /// System V convention only, llrm-qb and llrm-nib none, so no other
    /// compile forms a call of a library routine. Exit: a `Target::library`
    /// of each target that states its routines. Owner w-6, review 2026-11-30.
    pub library: Vec<llrm_mir::libfunc::Routine>,
    /// Off at -O0: no pass runs.
    pub optimize: bool,
    pub limits: Limits,
    pub inline: inline::Threshold,
    pub lcssa: bool,
    pub floatloop: bool,
    pub fold: bool,
    pub simplifycfg: bool,
    pub decide: bool,
    pub dead: bool,
    pub hoist: bool,
    pub forward: bool,
    pub drop_loads: bool,
    /// Partial redundancies are made full by inserting a computation or load on
    /// the path that lacks it: gcc's `-ftree-pre`, -O2 and up. Without it
    /// `gvn` only removes what is redundant on every path (`-ftree-fre`, -O1).
    pub pre: bool,
    /// A switch that only picks constants is a load from an array: gcc's
    /// `-ftree-switch-conversion`, -O2 and up.
    pub switch_conversion: bool,
    /// A dense switch may be a jump table: gcc's `-fjump-tables`, on at every
    /// level.
    pub jump_tables: bool,
    /// The MIR `switchlower` pass lowers the switches that remain: the old
    /// selector reads its table ops. Off under `--isel=dag`, where the DAG
    /// builder lowers them as `SelectionDAGBuilder::visitSwitch` (it needs the
    /// switch, with its `unreachable` default, as the pin's passes leave it).
    pub lower_switches: bool,
    pub drop_stores: bool,
    /// Work read below a branch moves below it: gcc's `-ftree-sink`.
    pub sink: bool,
    /// `Spares`: a fixed-cell load rematerialized past the writes alias proves
    /// disjoint. llrm only (the pin rematerializes invariant loads alone); off
    /// by default, `-fllrm-spares`. DEVIATE (no number), 8.0 M Ir at d_faces.
    pub spare_loads: bool,
    /// `Homes`: a phi that also sits in a stored cell is spilled to that cell
    /// (gcc `REG_EQUIV`). llrm only; off by default, `-fllrm-homes`. DEVIATE
    /// (no number), 8.4 M Ir at d_faces.
    pub stored_homes: bool,
    /// The spiller makes a load of a cell nothing writes again where it is read
    /// (D-I). llrm only; off by default, `-fllrm-stable-loads`. Read by the
    /// backend through the CPU profile.
    pub stable_loads: bool,
    pub promote: bool,
    pub strength: bool,
    /// The translated `LoopStrengthReduce` slot (LLVM's `-disable-lsr` turns it
    /// off); `strength` turns it off with the other address passes.
    pub lsr: bool,
    pub unroll: bool,
    /// clang's `PTO.LoopUnrolling` (`UnrollLoops`, on above -O1,
    /// [CompilerInvocation.cpp#L1936](https://github.com/llvm/llvm-project/blob/d1106deb71cc/clang/lib/Frontend/CompilerInvocation.cpp#L1936)):
    /// off, the full unroll of the LLVM order only takes a loop that asks
    /// (`OnlyWhenForced`). -Os keeps it on: the unroller's own size threshold
    /// of 0 stops the copies.
    pub loop_unrolling: bool,
    pub peel: bool,
    pub loopidioms: bool,
    pub sibcalls: bool,
    pub unswitch: bool,
    /// `-floop-interchange`: LLVM's `LoopInterchange.cpp`, in the pin's
    /// default pipelines (`enable-loopinterchange`) at every level but -O0.
    pub interchange: bool,
    /// `-floop-unroll-and-jam`: LLVM's `LoopUnrollAndJamPass`, in the Late
    /// stage (L1474). DEVIATE: the pin's default pipelines have it off
    /// (`enable-unroll-and-jam`); it is on at -O3 and -Omax, where the
    /// particle row gains from it.
    pub unroll_and_jam: bool,
    /// The passes run in LLVM's order, once each (`Fixed::linear`:
    /// [`llvm_order`]), not in the rounds; on by default, `LLRM_NO_LLVM_ORDER`
    /// turns it off.
    pub llvm_order: bool,
    /// With `llvm_order`, LPM1's `loop-rotate` between the two LICM runs
    /// (PassBuilderPipelines.cpp L728), the first LICM not speculating; on, as
    /// in the pin.
    pub rotate_slot: bool,
    /// The round's `Promote` is the translated `PromoteMemToReg`
    /// ([`crate::mem2reg`]); on by default, `LLRM_NO_MEM2REG` turns it off.
    pub mem2reg: bool,
    /// Under `llvm_order` the SROA slots run the translated unit (`SROA.cpp`,
    /// [`crate::sroa`], with [`crate::mem2reg`]); on by default,
    /// `LLRM_NO_SROA` turns it off.
    pub sroa: bool,
    /// The InstCombine slots (`combine@N`) run the translated families
    /// ([`crate::combine_pass`]); on by default, `LLRM_NO_COMBINE` turns it
    /// off.
    pub combine: bool,
    /// The loop-invariant passes are the translated LICM ([`crate::licm`]):
    /// `licm` in the LICM slots and where `hoist` ran; on by default,
    /// `LLRM_NO_LICM` turns it off.
    pub licm: bool,
    /// Loops not proven to run are entered behind a copy of their test, before
    /// the loop passes (gcc's `-ftree-ch`; not at -Os).
    pub copy_headers: bool,
    /// Code size outranks speed where they conflict: -Os and -Oz. (Whether a
    /// complete copy of a loop may grow the code is `limits.grows`, which
    /// gcc lets only -O3 do.)
    pub for_size: bool,
    /// `Function::hasMinSize`: -Oz. The only level where `LoopRotate` copies no
    /// header
    /// ([LoopRotation.cpp#L80](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LoopRotation.cpp#L80)).
    pub has_min_size: bool,
    /// gcc's `-fexpensive-optimizations` (-O2 and up): the passes it gates.
    pub expensive: bool,
    /// The allocator tries other shapes of a body and keeps the cheapest
    /// (`-fallocation-search`; -Omax: gcc and LLVM allocate once).
    pub search: bool,
    /// With `search`, every shape rather than the one the spills suggest
    /// (`-fallocation-search-all`; -Omax).
    pub exhaustive: bool,
    /// How many rounds of the body's passes run at most. A round after the
    /// third found nothing on the 71 programs of the vsgcc tables and
    /// QCport (the objects are the same), and the fourth and fifth of
    /// d_faces were 354 Minstr; -Omax runs to the fixed point. A round that
    /// follows an unroll which changed the body runs past the budget, to clean
    /// the straight line it made.
    pub rounds: usize,
}

/// The rounds a body's passes run, short of -Omax's fixed point.
const ROUNDS: usize = 4;

impl Default for Options {
    fn default() -> Self {
        Self {
            library: Vec::new(),
            optimize: true,
            llvm_order: llvm_order_default(),
            rotate_slot: true,
            limits: Limits { target_percent: 100, ..Limits::default() },
            inline: inline::Threshold::default(),
            lcssa: true,
            floatloop: true,
            rounds: ROUNDS,
            fold: true,
            simplifycfg: true,
            decide: true,
            dead: true,
            hoist: true,
            forward: true,
            drop_loads: true,
            pre: true,
            drop_stores: true,
            sink: true,
            spare_loads: false,
            stored_homes: false,
            stable_loads: false,
            promote: true,
            strength: true,
            lsr: true,
            unroll: true,
            loop_unrolling: true,
            peel: true,
            loopidioms: true,
            sibcalls: true,
            unswitch: false,
            interchange: true,
            unroll_and_jam: false,
            mem2reg: experiment_mem2reg(),
            sroa: !llrm_support::env_flag!("LLRM_NO_SROA"),
            combine: !llrm_support::env_flag!("LLRM_NO_COMBINE"),
            licm: !llrm_support::env_flag!("LLRM_NO_LICM"),
            copy_headers: true,
            for_size: false,
            has_min_size: false,
            expensive: true,
            switch_conversion: true,
            jump_tables: true,
            lower_switches: true,
            search: false,
            exhaustive: false,
        }
    }
}

impl Options {
    /// -O0.
    pub fn none() -> Self {
        Self { optimize: false, ..Self::default() }
    }

    /// -O1: the scalar passes, a loop copied out completely only where it
    /// asks (`loop_unrolling` off, as clang's `UnrollLoops` is), the pin's
    /// inliner (`getInlineParamsFromOptLevel`: 225 as -O2,
    /// PassBuilderPipelines.cpp L968), no allocation search or second route
    /// (compile -34% on d_faces), value numbering that inserts nothing, and
    /// no sibling calls, peeling or unswitching. Pattern fill is
    /// on (`LoopIdiomRecognize` is in LLVM's O1 pipeline,
    /// PassBuilderPipelines.cpp L562), the user's decision of 2026-10-10
    /// (docs/reference/levels.md).
    pub fn basic() -> Self {
        Self {
            limits: Limits { grows: false, ..Self::default().limits },
            inline: inline::Threshold::default(),
            // gcc runs each pass once: the third round changed five passes of
            // d_faces and the fourth none, 253 of 3,084 Minstr at -O1.
            rounds: 2,
            pre: false,
            peel: false,
            loop_unrolling: false,
            unswitch: false,
            expensive: false,
            switch_conversion: false,
            ..Self::default()
        }
    }

    /// -O2: gcc's: -O1 with inlining, gcse, sibling calls and pattern fill; a
    /// complete copy of a loop still must not grow the code
    /// (`flag_cunroll_grow_size` is on at -O3, `-funroll-loops` and
    /// `-fpeel-loops` only).
    pub fn standard() -> Self {
        Self {
            limits: Limits { grows: false, ..Self::default().limits },
            peel: false,
            unswitch: false,
            ..Self::default()
        }
    }

    /// -O3: gcc's: -O2 with peeling, unswitching, complete copies that grow the
    /// code, and the larger inline threshold; the copy budget is LLVM's -O3
    /// (`UnrollThresholdAggressive`, twice the target's), not gcc's. No ipa-cp
    /// clones: the pin makes none, and they grew recchop 1541 lines against
    /// 171.
    pub fn speed() -> Self {
        Self {
            inline: inline::Threshold::new(250),
            unswitch: true,
            unroll_and_jam: true,
            limits: Limits { target_percent: 200, ..Limits::default() },
            ..Self::default()
        }
    }

    /// -Omax: every pass the default has on, LLVM's -O3 budgets, twice the
    /// target's unroll budget, loops of up to 64 trips copied out completely
    /// (gcc's `max-completely-peel-times` is 16; clang has no cap) and a 250
    /// inline threshold.
    pub fn aggressive() -> Self {
        Self {
            limits: Limits { target_percent: 200, max_unroll_iterations: 64, ..Limits::default() },
            inline: inline::Threshold { cp_clone: true, ..inline::Threshold::new(250) },
            unswitch: true,
            unroll_and_jam: true,
            search: true,
            exhaustive: true,
            rounds: usize::MAX,
            ..Self::default()
        }
    }

    /// -Os: no copy grows the code. The limit stays -O2's 225, but every
    /// caller is `optsize` (D8), so a call is held to 50, 487 with an
    /// `inlinehint`, as the pin holds it (audit row 3: mirrored).
    pub fn size() -> Self {
        Self {
            limits: Limits { grows: false, target_percent: 100, ..Limits::default() },
            inline: inline::Threshold::default().for_size(),
            for_size: true,
            ..Self::default()
        }
    }

    /// Whether the allocator tries other shapes of a body and keeps the
    /// cheapest.
    pub fn searches(&self) -> bool {
        self.search
    }

    /// Whether the search tries every shape of a body.
    pub fn searches_all(&self) -> bool {
        self.exhaustive
    }

    /// Whether code size outranks speed where they conflict: -Os and -Oz.
    pub fn prefers_size(&self) -> bool {
        self.for_size
    }

    /// Whether the code generator may grow code for speed: -O3 and -Omax
    /// (`CodeGenOptLevel::Aggressive`).
    pub fn aggressive_codegen(&self) -> bool {
        self.limits.target_percent >= 200
    }

    /// -Oz: no loop is copied.
    pub fn min_size() -> Self {
        Self {
            unroll: false,
            peel: false,
            has_min_size: true,
            inline: inline::Threshold::default().for_min_size(),
            ..Self::size()
        }
    }

    /// Whether pass `name` is on. Every pass can be turned off, which is
    /// how a miscompile is bisected.
    pub(crate) fn wanted(
        &self,
        name: &str,
    ) -> bool {
        match base(name) {
            "lcssa" => self.lcssa,
            "floatloop" => self.floatloop,
            "fold" => self.fold,
            "simplifycfg" => self.simplifycfg,
            "decide" => self.decide,
            "dead" => self.dead,
            "hoist" | "loopmotion" | "licm" => self.hoist && self.licm,
            "gvn" | "equalcalls" | "earlycse" => self.forward && self.drop_loads,
            "dse" => self.drop_stores,
            "sink" => self.sink,
            "sroa" | "promote" => self.promote,
            "lsr" | "canonfreeze" => self.strength && self.lsr,
            "indvars" | "window" | "gepoffset" | "cgpaddr" => self.strength,
            "unroll" => self.unroll,
            "peel" => self.peel,
            "loopidiom" | "loopidioms" | "merge" | "memcpyopt" => self.loopidioms,
            "tailrec" => self.sibcalls,
            "loop-interchange" => self.interchange,
            "loop-unroll-and-jam" => self.unroll_and_jam,
            _ => true,
        }
    }
}

/// `applied`'s arguments: the old ones less the BC frontend's.
#[derive(Clone, Default)]
pub struct Applied {
    pub options: Options,
    /// The one pass to run, by name.
    pub only: Option<String>,
    /// Where each body's changed steps are written, `N/NNN-stage.ll`, the
    /// Nth body run; the manager writes the module after it there.
    pub dump: Option<PathBuf>,
}

/// `LLRM_NO_MEM2REG`, read once: set, the round's `Promote` stays the
/// write-through stand-in; otherwise it is the translated `PromoteMemToReg`
/// ([`crate::mem2reg`]).
fn experiment_mem2reg() -> bool {
    !llrm_support::env_flag!("LLRM_NO_MEM2REG")
}

/// The passes, in order, that `applied` leaves on.
///
/// A pass that is off is not in it, rather than in it and skipped, so what
/// runs is what this returns.
pub fn pipeline(applied: &Applied) -> Vec<Box<dyn FunctionPass>> {
    let every: Vec<Box<dyn FunctionPass>> = vec![
        // Aggregate/object leaves become ordinary SSA before any scalar or
        // CFG pass asks what is constant, redundant, or loop invariant.
        Box::new(promote::Sroa),
        Box::new(fold::Fold),
        // Before anything asks what a port call does to memory.
        Box::new(ports::Ports),
        // A terminator on a literal is a jump before `decide` asks what the
        // body's values are.
        Box::new(simplifycfg::SimplifyCfg::default()),
        Box::new(decide::Decide { constants: true }),
        // Once the arguments are values rather than frame cells; the loop it
        // makes goes to the loop passes below.
        // Before a call is made a loop: two equal calls are one first, as
        // LLVM's EarlyCSE and gcc's FRE precede tail-recursion elimination.
        Box::new(gvn::EqualCalls),
        Box::new(tailcallelim::TailCallElim),
        Box::new(loopsimplify::LoopSimplify),
        Box::new(lcssa::LoopClosedSSA),
        // Strict floating recurrences must retain their original iteration
        // order; LICM may move invariant preparation out afterwards.
        Box::new(floatloop::FloatLoop),
        Box::new(crate::licm::Licm { apply: true, speculate: true }),
        // The loop's exit tests that nothing in it changes are made once, with
        // the loop's entry.
        Box::new(trivialunswitch::TrivialUnswitch),
        // Before gvn: a far pointer cast from a near one is read as the near
        // one.
        Box::new(inferspace::InferAddressSpaces),
        Box::new(deadstore::DeadStoreElimination),
        Box::new(gvn::Gvn { pre: applied.options.pre }),
        // Ordinary scalar write-through promotion remains after memory GVN;
        // `LLRM_NO_MEM2REG` keeps the write-through stand-in instead of the
        // translated PromoteMemToReg.
        if applied.options.mem2reg { Box::new(mem2reg::Mem2Reg) } else { Box::new(promote::Promote) },
        Box::new(indvars::IndVars { lftr: indvars::lftr_default(), size: applied.options.prefers_size() }),
        Box::new(algebraic::Algebraic { size: applied.options.prefers_size(), llvm: false }),
        Box::new(dead::Dead),
        Box::new(loopidioms::LoopIdioms { size: applied.options.prefers_size() }),
        Box::new(loopidioms::Merge { size: applied.options.prefers_size() }),
        Box::new(memcpyopt::MemCpyOpt),
    ];
    every.into_iter().filter(|one| applied.options.wanted(one.name())).collect()
}

/// Under LLVM's order the translated LICM does the moving of code out of and
/// into loops, in its three slots: the passes after the fixed point that
/// stood in for MachineLICM and gcc's `pass_sink_code` (`hoist` again) have no
/// counterpart there, and run only without the order or the LICM.
pub fn llvm_motion(applied: &Applied) -> bool {
    applied.options.llvm_order && applied.options.licm
}

/// Translations that have not joined the pipeline yet: `llrm-pass` and
/// `Applied::only` run them alone, which is what `tools/difftest` compares
/// with LLVM's counterpart. A pass leaves this list when it joins `pipeline`.
fn translated() -> Vec<Box<dyn FunctionPass>> {
    vec![
        Box::new(crate::cvp::Cvp),
        Box::new(crate::jumpthreading::JumpThreading::from_env()),
        Box::new(crate::dfajumpthreading::DfaJumpThreading::from_env()),
        Box::new(crate::tailcallelim::TailCallElim),
    ]
}

/// `name` without the slot it stands in (`gvn@787` is `gvn`).
fn base(name: &str) -> &str {
    name.split_once('@').map_or(name, |split| split.0)
}

/// A pass in one place of LLVM's function pipeline, named by it so that its
/// settled record and its line of `LLRM_DEBUG=runs` are its own.
struct Slot {
    name: &'static str,
    inner: Box<dyn FunctionPass>,
}

impl FunctionPass for Slot {
    fn name(&self) -> &'static str {
        self.name
    }

    fn translations(&self) -> Vec<&'static str> {
        self.inner.translations()
    }

    fn adds_memory_operations(&self) -> bool {
        self.inner.adds_memory_operations()
    }

    fn reads(&self) -> llrm_mir::footprint::Footprint {
        self.inner.reads()
    }

    fn wants_sizes(&self) -> bool {
        self.inner.wants_sizes()
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        self.inner.run(unit, analyses)
    }
}

/// Passes that stand for one pass of LLVM, run one after the other and named
/// as one: SROA runs mem2reg on what it splits, so the slot is SROA, then
/// Promote, with nothing between (SROA.cpp `promoteAllocas`).
struct Together {
    name: &'static str,
    parts: Vec<Box<dyn FunctionPass>>,
}

impl FunctionPass for Together {
    fn name(&self) -> &'static str {
        self.name
    }

    fn parts(&self) -> Vec<&'static str> {
        self.parts.iter().flat_map(|part| part.parts()).collect()
    }

    fn translations(&self) -> Vec<&'static str> {
        self.parts.iter().flat_map(|part| part.translations()).collect()
    }

    fn adds_memory_operations(&self) -> bool {
        self.parts.iter().any(|part| part.adds_memory_operations())
    }

    fn reads(&self) -> llrm_mir::footprint::Footprint {
        self.parts.iter().fold(llrm_mir::footprint::Footprint::default(), |all, part| all.with(part.reads()))
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let mut changed = false;
        for part in &mut self.parts {
            let preserved = llrm_mir::passes::run_checked(part.as_mut(), unit, analyses);
            if !preserved.are_all_preserved() {
                changed = true;
                analyses.invalidate(unit.function, &preserved);
            }
        }
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// `LLRM_NO_LLVM_ORDER`, read once: set, [`Options::llvm_order`] is off by
/// default (PassBuilderPipelines.cpp at d1106deb71cc is the order otherwise).
fn llvm_order_default() -> bool {
    !llrm_support::env_flag!("LLRM_NO_LLVM_ORDER")
}

/// The passes of `phase`, each once in the place LLVM runs it (the line is the
/// slot's name).
pub(crate) fn llvm_order(
    applied: &Applied,
    phase: Phase,
) -> Vec<Box<dyn FunctionPass>> {
    let size = applied.options.prefers_size();
    let slot =
        |name: &'static str, inner: Box<dyn FunctionPass>| -> Box<dyn FunctionPass> { Box::new(Slot { name, inner }) };
    // `-fno-jump-tables`: clang's "no-jump-tables" on every function, which
    // stops SimplifyCFG's lookup tables as well as the jump tables.
    let no_jump_tables = !applied.options.jump_tables;
    // Each runs once: nothing undoes a loop's simple form between rounds (D7).
    let simplify_cfg = |name| {
        slot(
            name,
            Box::new(simplifycfg::SimplifyCfg {
                options: simplifycfg::Options { loop_form_kept: false, ..Default::default() },
            }),
        )
    };
    let simplify_cfg_with = |name, options: simplifycfg::Options| {
        slot(
            name,
            Box::new(simplifycfg::SimplifyCfg {
                options: simplifycfg::Options { no_jump_tables, loop_form_kept: false, ..options },
            }),
        )
    };
    // `visitCallInst`'s `LibCallSimplifier` runs with every InstCombine, so it
    // is a part of the slot.
    let combine = |name| {
        slot(
            name,
            Box::new(Together {
                name,
                parts: vec![
                    Box::new(crate::combine_pass::Combine { enabled: applied.options.combine }),
                    Box::new(crate::simplifylibcalls::SimplifyLibCalls::with_routines(applied.options.library.clone())),
                ],
            }),
        )
    };
    let dead = |name| slot(name, Box::new(dead::Dead));
    // SROA runs mem2reg on what it splits: one slot, nothing between
    // (SROA.cpp `promoteAllocas`).
    let translated = applied.options.sroa;
    // DEVIATE, -O1 and -Og only (no GVN `pre`): the write-through `Promote`
    // after PromoteMemToReg forwards a store of a global to the load after it,
    // which LLVM's InstCombine does in `visitLoadInst`
    // (FindAvailableLoadedValue, not translated); GVN does it at the other
    // levels. Exit: that translation.
    let write_through = || -> Vec<Box<dyn FunctionPass>> {
        if applied.options.pre { Vec::new() } else { vec![Box::new(promote::Promote), Box::new(promote::DeadSlots)] }
    };
    let sroa = |name: &'static str| -> Box<dyn FunctionPass> {
        let parts: Vec<Box<dyn FunctionPass>> = if translated {
            let mut parts: Vec<Box<dyn FunctionPass>> =
                vec![Box::new(crate::sroa::Sroa), Box::new(crate::mem2reg::Mem2Reg)];
            parts.extend(write_through());
            parts
        } else {
            vec![Box::new(promote::Sroa), Box::new(promote::Promote), Box::new(promote::DeadSlots)]
        };
        Box::new(Together { name, parts })
    };
    let earlycse = |name, memory_ssa| slot(name, Box::new(crate::earlycse::EarlyCse { memory_ssa }));
    // `licm<allowspeculation>` or `licm<no-allowspeculation>`; under
    // `LLRM_NO_LICM` the slot stays and holds nothing.
    let licm = |name, speculate| -> Box<dyn FunctionPass> {
        if applied.options.licm {
            slot(name, Box::new(crate::licm::Licm { apply: true, speculate }))
        } else {
            Box::new(Together { name, parts: Vec::new() })
        }
    };
    let canonical = |a: &'static str, b: &'static str| {
        [slot(a, Box::new(loopsimplify::LoopSimplify)), slot(b, Box::new(lcssa::LoopClosedSSA))]
    };
    let list: Vec<Box<dyn FunctionPass>> = match phase {
        // Not function lists: their builders call `parity::check_phase`
        // themselves.
        Phase::Module | Phase::Backend => Vec::new(),
        Phase::Early => {
            // EarlyFPM's `SimplifyCFGPass()` (PassBuilderPipelines.cpp#L1178),
            // at the default options. The slot keeps the name
            // `decide@1178`.
            vec![
                // `EarlyFPM.addPass(LowerExpectIntrinsicPass())` (#L1177).
                slot("lowerexpect@1177", Box::new(crate::lowerexpect::LowerExpect)),
                simplify_cfg("decide@1178"),
                sroa("sroa@1179"),
                earlycse("earlycse@1180", false),
            ]
        }
        Phase::Cleanup => vec![
            Box::new(Together {
                name: "promote@1241",
                // DEVIATE as in the SROA slot.
                parts: if applied.options.mem2reg {
                    let mut parts: Vec<Box<dyn FunctionPass>> = vec![Box::new(mem2reg::Mem2Reg)];
                    parts.extend(write_through());
                    parts
                } else {
                    vec![Box::new(promote::Promote), Box::new(promote::DeadSlots)]
                },
            }),
            combine("combine@1242"),
            dead("dead@1242"),
            // `SimplifyCFGOptions().convertSwitchRangeToICmp(true)`
            // (PassBuilderPipelines.cpp#L1245): the option is read by
            // `simplifySwitch`, not translated yet, so these are the defaults.
            simplify_cfg("decide@1244"),
        ],
        // The Late stage follows the Simplify list, in `OptimizePM`'s order
        // (the pin's printed pipeline): Float2IntPass (L1594) first, then the
        // loop passes, L1438, LICM (L1508), InstSimplifyPass
        // (PassBuilderPipelines.cpp#L1662, the last function pass, after
        // LoopSink) and L1682.
        Phase::Late => vec![
            slot("float2int@1594", Box::new(crate::float2int::Float2Int)),
            // `LowerConstantIntrinsicsPass` (l.1597), right after Float2Int.
            slot("lower-constant-intrinsics@1597", Box::new(crate::lowerconstantintrinsics::LowerConstantIntrinsics)),
            // LoopInterchangePass (l.1631), in the loop pass manager after
            // LoopRotate (l.1621) and LoopDeletion (l.1628).
            slot("loop-interchange@1631", Box::new(loopinterchange::Interchange::default())),
            // InferAlignmentPass (l.1362), before LoopLoadElimination.
            slot("infer-alignment@1362", Box::new(crate::inferalignment::InferAlignment)),
            // LoopLoadEliminationPass (l.1400), before the SimplifyCFG of
            // L1438; llrm's targets have no vector registers.
            slot("loop-load-elim@1400", Box::new(crate::looploadelim::LoopLoadElim::default())),
            // InstCombinePass (l.1403), l.1464 and l.1499: the three late runs
            // that clean what SimplifyCFG (l.1438) and the unroll leave, as the
            // `not` of a condition it makes before a branch.
            combine("combine@1403"),
            // The slots' options, with the pin's lines,
            // are `simplifycfg::Options::{L1438, L1682}`.
            simplify_cfg_with("simplifycfg@1438", simplifycfg::Options::L1438),
            // SLPVectorizerPass (l.1455): the compares of adjacent bytes
            // against constants, on a target with vector registers.
            slot("slp@1455", Box::new(crate::slpcompares::SlpCompares)),
            combine("combine@1464"),
            // LoopUnrollAndJamPass (l.1474), in its own loop pass manager
            // before LoopUnrollPass; the nest is in simplify form and LCSSA.
            slot(
                "loop-unroll-and-jam@1474",
                Box::new(loopunrollandjam::LoopUnrollAndJam {
                    opt_level: if applied.options.limits.target_percent >= 200 { 3 } else { 2 },
                    opt_size: size,
                    options: loopunrollandjam::JamOptions { on: true, ..Default::default() },
                }),
            ),
            // LoopUnrollPass (l.1476) with the full unroll the target allows
            // (X86 sets no partial or runtime unrolling), then SROA (l.1494)
            // on the allocas the unroll made constant-indexed. The loops the
            // early unroll refused are looked at again after the combines.
            slot(
                "unroll@1476",
                Box::new(loopunrollpass::LoopFullUnroll {
                    opt_level: if applied.options.limits.target_percent >= 200 { 3 } else { 2 },
                    opt_size: size,
                    only_when_forced: !applied.options.loop_unrolling,
                    created: Default::default(),
                }),
            ),
            sroa("sroa@1494"),
            // LICMPass again (l.1508,
            // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L1508,
            // `AllowSpeculation=true`), in a loop pass manager of its own after
            // the unroll and the combine that follow it: "helps to clean up
            // some loop-invariant code created by the loop unroll pass".
            // InferAlignmentPass (l.1498), after the unroll and SROA.
            slot("infer-alignment@1498", Box::new(crate::inferalignment::InferAlignment)),
            combine("combine@1499"),
            licm("licm@1508", true),
            slot("instsimplify@1662", Box::new(crate::instsimplify::InstSimplify)),
            slot("divrempairs@1667", Box::new(crate::divrempairs::DivRemPairs)),
            // TailCallElim (l.1677, the module optimization pipeline's
            // "calls created during optimization"), at every level: at -O1 it
            // is the only one, so a self-recursive tail call is a loop there
            // (recmany -O1 -m32 ran 1500 frames deep, 60 KB, where the pin's
            // runs in one). ExpandMemCmp, between the two at the pin, is not
            // a slot.
            slot("tailrec@1677", Box::new(tailcallelim::TailCallElim)),
            simplify_cfg_with("simplifycfg@1682", simplifycfg::Options::L1682),
        ],
        Phase::Simplify => {
            let mut list = vec![sroa("sroa@646"), earlycse("earlycse@649", true)];
            list.extend([
                slot("jumpthreading@668", Box::new(crate::jumpthreading::JumpThreading::from_env())),
                slot("cvp@669", Box::new(crate::cvp::Cvp)),
                // JumpTableToSwitchPass, PassBuilderPipelines.cpp#L672.
                slot("jumptableswitch@672", Box::new(crate::jumptableswitch::JumpTableToSwitch)),
                // SimplifyCFGOptions().convertSwitchRangeToICmp(true), L675:
                // the option is read by `simplifySwitch`, untranslated.
                simplify_cfg("decide@675"),
                combine("combine@677"),
                slot("aggressive-instcombine@678", Box::new(crate::trunc_inst_combine::TruncInstCombine)),
                slot("tailrec@688", Box::new(tailcallelim::TailCallElim)),
                // SimplifyCFGOptions().convertSwitchRangeToICmp(true), L691:
                // the option is read by `simplifySwitch`, untranslated.
                simplify_cfg("decide@690"),
                slot("reassociate@696", Box::new(crate::reassociate::Reassociate)),
                slot("constraint-elimination@699", Box::new(crate::constraintelim::ConstraintElimination)),
            ]);
            list.extend(canonical("loopsimplify@764", "lcssa@764"));
            // Without the rotate slot the first LICM keeps speculating, as the
            // single hoist did.
            let rotating = applied.options.rotate_slot;
            list.extend([
                // LPM1's first pass, PassBuilderPipelines.cpp#L715.
                slot("loop-instsimplify@715", Box::new(crate::loopinstsimplify::LoopInstSimplify)),
                // LPM1's second pass, PassBuilderPipelines.cpp#L716.
                slot("loop-simplifycfg@716", Box::new(crate::loopsimplifycfg::LoopSimplifyCfg)),
                slot("floatloop@724", Box::new(floatloop::FloatLoop)),
                // LPM1
                // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L724:
                // licm<no-allowspeculation> L724, loop-rotate L728,
                // licm<allowspeculation> L730. The first hoists only what the
                // loop certainly runs; the rotate puts a guard before a loop
                // not proven to run, so that the second hoists into a preheader
                // behind it (#2194).
                licm("licm@724", !rotating),
            ]);
            if rotating {
                // The header is copied at every level but -Oz (`hasMinSize`).
                list.push(slot(
                    "rotate@728",
                    Box::new(looprotate::LoopRotate {
                        duplicate_headers: applied.options.copy_headers && !applied.options.has_min_size,
                        size: applied.options.prefers_size(),
                    }),
                ));
            }
            list.extend([
                licm("licm@730", true),
                // The pin's `SimpleLoopUnswitchPass(NonTrivial = Level == O3)`:
                // the options say -O3 and -Omax.
                slot(
                    "trivialunswitch@732",
                    Box::new(simpleloopunswitch::SimpleLoopUnswitch { non_trivial: applied.options.unswitch }),
                ),
                // SimplifyCFGOptions().convertSwitchRangeToICmp(true), L766:
                // the option is read by `simplifySwitch`, untranslated.
                simplify_cfg("decide@766"),
                combine("combine@768"),
            ]);
            list.extend(canonical("loopsimplify@772", "lcssa@772"));
            // What the unroll makes, for the slot after it.
            let created: std::rc::Rc<std::cell::RefCell<Vec<llrm_mir::module::BlockId>>> = Default::default();
            list.extend([
                slot(
                    "loopidiom@737",
                    Box::new(crate::loopidiom::pass::LoopIdiom {
                        for_size: size,
                        ..crate::loopidiom::pass::LoopIdiom::with_routines(applied.options.library.clone())
                    }),
                ),
                slot("loopidioms@737", Box::new(loopidioms::LoopIdioms { size })),
                slot("indvars@738", Box::new(indvars::IndVars { lftr: indvars::lftr_default(), size })),
                slot("loopdeletion@749", Box::new(loopdeletion::LoopDeletion)),
                slot(
                    "unroll@753",
                    Box::new(loopunrollpass::LoopFullUnroll {
                        opt_level: if applied.options.limits.target_percent >= 200 { 3 } else { 2 },
                        opt_size: size,
                        only_when_forced: !applied.options.loop_unrolling,
                        created: std::rc::Rc::clone(&created),
                    }),
                ),
                // The loops the unroll made go on to LPM2's passes: indvars on
                // them.
                slot("indvars@753", Box::new(indvars::IndVarsOnCreated { created })),
                sroa("sroa@776"),
            ]);
            // Not in the pin's `default<O1>` (a pipeline of its own); `pre` is
            // off exactly at -O1 and -Og.
            if applied.options.pre {
                list.push(slot("mldst-motion@783", Box::new(mldstmotion::MergedLoadStoreMotion)));
            }
            list.extend([
                slot("inferspace@787", Box::new(inferspace::InferAddressSpaces)),
                // The pin's `default<O1>` has no GVN (`pre` is off exactly at
                // -O1 and -Og); rows against clang's -O1 are pending, a loss
                // there becomes a numbered DEVIATE.
                slot("gvn@787", Box::new(gvn::Gvn { pre: applied.options.pre })),
                slot("sccp@792", Box::new(sccp::Sccp)),
                slot("bdce@797", Box::new(bdce::Bdce)),
                combine("combine@801"),
                // DFAJumpThreadingPass, PassBuilderPipelines.cpp#L806.
                slot(
                    "dfajumpthreading@806",
                    Box::new(
                        crate::dfajumpthreading::DfaJumpThreading::from_env().for_size(applied.options.prefers_size()),
                    ),
                ),
                slot("jumpthreading@809", Box::new(crate::jumpthreading::JumpThreading::from_env())),
                slot("cvp@810", Box::new(crate::cvp::Cvp)),
                slot("adce@815", Box::new(adce::Adce)),
                slot("memcpyopt@818", Box::new(memcpyopt::MemCpyOpt)),
                slot("merge@818", Box::new(loopidioms::Merge { size })),
                slot("dse@820", Box::new(deadstore::DeadStoreElimination)),
            ]);
            list.extend(canonical("loopsimplify@823", "lcssa@823"));
            list.extend([
                licm("licm@823", true),
                // L832: hoistCommonInsts and sinkCommonInsts on, besides
                // convertSwitchRangeToICmp and convertSwitchToArithmetic.
                simplify_cfg_with(
                    "decide@832",
                    simplifycfg::Options {
                        hoist_common_insts: true,
                        sink_common_insts: true,
                        convert_switch_to_arithmetic: true,
                        ..Default::default()
                    },
                ),
                combine("combine@837"),
            ]);
            list
        }
    };
    list.into_iter()
        .filter(|one| applied.options.wanted(one.name()))
        .filter(|one| applied.options.pre || !crate::parity::ABSENT_AT_O1.contains(&one.name()))
        .collect()
}

/// The order, from the pipeline itself rather than beside it, then the
/// translations not yet in it.
pub fn passes() -> Vec<&'static str> {
    pipeline(&Applied::default()).iter().chain(translated().iter()).map(|one| one.name()).collect()
}

/// Whether the options of `applied` build the slot `slot` of LLVM's order
/// (`-O1` lacks some; an off switch puts the stand-in in the slot, which is
/// still built).
pub fn slot_built(
    applied: &Applied,
    slot: &str,
) -> bool {
    let mut options = applied.options.clone();
    options.llvm_order = true;
    crate::parity::describe(slot).is_some_and(|(phase, ..)| {
        llvm_order(&Applied { options, ..Applied::default() }, phase).iter().any(|one| one.name() == slot)
    })
}

/// Plugs the pass manager's steps into `LLRM_DEBUG=time` and `runs`, if either
/// is on: `time` times them, `runs` tells a pass's own work from the analyses
/// it computed.
pub fn timed() {
    use llrm_support::debug;
    if !debug::enabled("time") && !debug::enabled("runs") {
        return;
    }
    llrm_mir::passes::observe(llrm_mir::passes::Observer {
        span: |kind, name, run| {
            let timed = |run: &mut dyn FnMut()| {
                if debug::enabled("time") { debug::timed_by(|| format!("{kind} {name}"), run) } else { run() }
            };
            if kind == "analysis" { debug::analysed(|| timed(run)) } else { timed(run) }
        },
        function: |name, run| debug::in_function(name, run),
        count: |what, hit| debug::counted(what, hit),
    });
}

/// `program` through the pipeline.
pub fn applied(
    program: &mut Program,
    applied: &Applied,
) -> Result<(), String> {
    recorded(program, applied).map(|_| ())
}

/// What runs on each body before the interprocedural step: the rounds, or
/// LLVM's early function pipeline, GlobalCleanup and function simplification
/// once each (`Options::llvm_order`).
pub(crate) fn function_stages(applied: &Applied) -> Vec<Fixed> {
    if applied.options.llvm_order {
        vec![
            Fixed::linear(applied, Phase::Early),
            Fixed::linear(applied, Phase::Cleanup),
            Fixed::linear(applied, Phase::Simplify),
            // DEVIATE: LLVM runs it after the interprocedural passes; here it
            // closes the function stages.
            Fixed::linear(applied, Phase::Late),
        ]
    } else {
        vec![Fixed::new(applied)]
    }
}

/// `applied`, with what the pipeline did to each function.
pub fn recorded(
    program: &mut Program,
    applied: &Applied,
) -> Result<Vec<Stage>, String> {
    timed();
    let mut manager = PassManager::default();
    manager.verify_each = llrm_support::debug::verifying();
    manager.dump = applied.dump.clone();
    manager.dump_inputs = llrm_support::env_flag!("LLRM_DUMP_INPUTS");
    if !applied.options.optimize {
        manager.add_program(availableexternally::EliminateAvailableExternally);
        return manager.run(program);
    }
    let llvm_order = applied.options.llvm_order;
    // LLVM's GlobalsAA is made at `RequireAnalysisPass<GlobalsAA>` and
    // `RecomputeGlobalsAAPass` alone, read cached in between; the rounds make
    // it before each function pass.
    if llvm_order {
        manager.require_cached::<GlobalsAA>();
        manager.require_cached::<GlobalsModRefAnalysis>();
    } else {
        manager.require::<GlobalsAA>();
        manager.require::<GlobalsModRefAnalysis>();
    }
    manager.require::<Summaries>();
    manager.require_program::<ProgramSummaries>();
    // As LLVM's module pipeline starts with InferFunctionAttrs
    // (PassBuilderPipelines.cpp#L1169), then GlobalOpt.
    manager.add_module(inferfunctionattrs::InferFunctionAttrs);
    manager.add_module(globalopt::GlobalOpt);
    // Over every body first, so a caller's pipeline sees each callee's
    // port calls narrowed.
    if applied.options.wanted("ports") {
        manager.add(ports::Ports);
    }
    manager.add_program(crate::interprocedural::Stamp);
    let walked = |dump: &str| Applied { dump: applied.dump.as_ref().map(|one| one.join(dump)), ..applied.clone() };
    for (at, stage) in function_stages(applied).into_iter().enumerate() {
        // The walk simplifies each body: not a module-wide stage.
        if at != 2 || !applied.options.llvm_order {
            manager.add(stage);
        }
        // LLVM's CGSCC walk: each body inlined into, then simplified, callees
        // first. `Phase::Simplify` runs there and not module-wide.
        if at == 1 && applied.options.llvm_order {
            // `buildInlinerPipeline` (PassBuilderPipelines.cpp:997,
            // d1106deb71cc).
            manager.compute::<GlobalsAA>();
            manager.compute::<GlobalsModRefAnalysis>();
            let mut simplify = Fixed::linear(&walked("walk"), Phase::Simplify);
            manager.add_program(crate::walk::Walk {
                inline: applied.options.inline,
                simplify: Box::new(move |module, analyses, id, stage| {
                    TRIGGER.with(|trigger| *trigger.borrow_mut() = stage.to_owned());
                    rerun(module, analyses, id, &mut simplify, true)
                        .unwrap_or_else(|error| panic!("pipeline: {error}"));
                    TRIGGER.with(|trigger| trigger.borrow_mut().clear());
                }),
            });
        }
        // LLVM's IPSCCP and DeadArgumentElimination run after the early
        // function pipeline, before the Simplify list.
        if at == 0 && applied.options.llvm_order {
            manager.add_program(crate::walk::ModuleIpo { inline: applied.options.inline });
        }
    }
    // Once every body has reached its own fixed point, as the old Nib
    // driver's whole-module step: a body it changes goes back through.
    let again_applied =
        Applied { dump: applied.dump.as_ref().map(|one| one.join("interprocedural")), ..applied.clone() };
    let mut again = if applied.options.llvm_order {
        Fixed::linear(&again_applied, Phase::Simplify)
    } else {
        Fixed::new(&again_applied)
    };
    // The copies of a tree recursion in itself and then its tail-call
    // elimination, after the clones ipa-cp made (D10).
    let late = (applied.options.llvm_order && applied.options.inline.recursive).then(|| {
        let mut simplify = Fixed::linear(&again_applied, Phase::Simplify);
        crate::walk::SelfInline {
            simplify: Box::new(move |module, analyses, id, stage| {
                TRIGGER.with(|trigger| *trigger.borrow_mut() = stage.to_owned());
                rerun(module, analyses, id, &mut simplify, true).unwrap_or_else(|error| panic!("pipeline: {error}"));
                TRIGGER.with(|trigger| trigger.borrow_mut().clear());
            }),
            for_size: applied.options.prefers_size(),
        }
    });
    manager.add_program(Interprocedural {
        late,
        pipeline: Box::new(move |module, analyses, id, stage| {
            TRIGGER.with(|trigger| *trigger.borrow_mut() = stage.to_owned());
            let done =
                // The stage that only retargets loads and stores of globals
                // keeps GlobalsAA past its bodies, as the walk's runs do.
                rerun(module, analyses, id, &mut again, stage == "globalsra.")
                    .unwrap_or_else(|error| panic!("pipeline: {error}"));
            TRIGGER.with(|trigger| trigger.borrow_mut().clear());
            done
        }),
        proved: None,
        inline: applied.options.inline,
        rate: Some(if applied.options.prefers_size() { 0 } else { applied.options.limits.milliclocks_per_byte }),
    });
    // A global no body loads from any more (the loads were forwarded) needs
    // none of its stores, as LLVM's GlobalOpt and gcc's ipa find a variable
    // only written; the dead-code pass then finds nothing names it.
    manager.add_module(globalopt::WriteOnly);
    // What only fed those stores (their loads, arithmetic) is dead with them;
    // LLVM's GlobalOpt runs before the late function passes that sweep it.
    manager.add(dead::Dead);
    // A loop whose only effect was those stores is dead now: LLVM's second
    // function round (LoopDeletion, PassBuilderPipelines.cpp:1628) follows its
    // GlobalOpt.
    // It reads the live-outs from exit phis, so the bodies are closed first.
    manager.add(loopsimplify::LoopSimplify);
    manager.add(lcssa::LoopClosedSSA);
    manager.add(loopdeletion::LoopDeletion);
    // What no live code names any more goes before selection, as LLVM runs
    // GlobalDCE after inlining.
    manager.add_program(globaldce::GlobalDce);
    manager.add_program(availableexternally::EliminateAvailableExternally);
    // `ReversePostOrderFunctionAttrsPass` (PassBuilderPipelines.cpp:1542).
    if llvm_order {
        manager.add_module(crate::rpoattrs::ReversePostOrderFunctionAttrs);
    }
    // Once the callers that remain are the ones that stay: an internal function
    // they all call directly pops its own arguments.
    if applied.options.wanted("calleepop") {
        manager.add_module(calleepop::CalleePop { size: applied.options.prefers_size() });
    }
    if llvm_order {
        // `buildModuleOptimizationPipeline` (PassBuilderPipelines.cpp:1567,
        // d1106deb71cc). DEVIATE: made even where not held, as GlobalDCE, which
        // LLVM runs after this point, abandoned it here.
        manager.recompute::<GlobalsAA>();
        manager.recompute::<GlobalsModRefAnalysis>();
    } else {
        manager.freeze::<GlobalsAA>();
        manager.freeze::<GlobalsModRefAnalysis>();
    }
    // The summaries are made. The passes below move and rewrite what a function
    // does and add nothing to it, so they read these and do not make them
    // again after each edit, as gcc's passes after IPA read the modref
    // summaries it made there.
    manager.freeze::<Summaries>();
    // gcc's `pass_ch` (passes.def:232, in `pass_all_optimizations`) runs after
    // `pass_ipa_inline`: the inliner sizes a body before its header is copied.
    // The loops it guards are re-simplified for the passes that follow. Not at
    // -Os (`optimize_loop_for_size_p`): there only a loop proven to run is
    // entered at its body, which the last `Rotate` does.
    let guarded = copies_headers_after_ipa(applied);
    for pass in header_copied(applied) {
        manager.add_boxed(pass);
    }
    // Before LSR: a factor of two or a scale the product carries still shows as
    // a shift.
    if applied.options.wanted("fixednarrow") {
        manager.add(fixednarrow::FixedNarrow);
    }
    // Each loop's counters chosen once, on the loop the passes above leave.
    for pass in strength_slot(applied) {
        manager.add_boxed(pass);
    }
    // On the pointers LSR chose: a huge one a loop keeps in one window is far
    // there.
    if applied.options.wanted("window") {
        manager.add(window::Window { size: applied.options.prefers_size() });
    }
    // Last, as the old drivers rotated in lowering: unroll and peel refuse
    // a rotated loop.
    manager.add(rotate::Rotate { proven: true, copy: false });
    // A loop entered at its body runs it at least once: what it loads
    // unchanged may now leave it, as MachineLICM follows LLVM's LSR.
    if !guarded && applied.options.wanted("hoist") && !llvm_motion(applied) {
        manager.add(crate::licm::Licm { apply: true, speculate: true });
    }
    // After hoist, which would move a constant `gep` out of its loop.
    if applied.options.wanted("gepoffset") {
        manager.add(gepoffset::GepOffset);
    }
    // After gvn's last partial redundancy elimination, which makes the phis it
    // sinks.
    // CodeGenPrepare's sinking: compares, masks, address phis and the address
    // arithmetic of a block's accesses, made in the block that uses them.
    if applied.options.wanted("cgpaddr") {
        manager.add(cgpaddr::CgpAddr);
    }
    // After cgpaddr, with the other passes that make the forms isel sees:
    // the pin's DAGCombiner makes one extending load for a load and its
    // extension (`ExtendUsesToFormExtLoad`), which isel cannot.
    if applied.options.wanted("extload") {
        manager.add(extload::ExtLoad);
    }
    // Last of the passes that reshape the body, as gcc's `pass_lower_switch`
    // follows its optimizations.
    if applied.options.switch_conversion && applied.options.wanted("switchconv") {
        manager.add(switchlower::SwitchLower {
            convert: true,
            jump_tables: applied.options.jump_tables,
            size: applied.options.prefers_size(),
        });
    }
    if applied.options.lower_switches && applied.options.wanted("switchlower") {
        manager.add(switchlower::SwitchLower {
            convert: false,
            jump_tables: applied.options.jump_tables,
            size: applied.options.prefers_size(),
        });
    }
    // The pin's last module pass (PassBuilderPipelines.cpp#L1714), after every
    // body is final.
    if applied.options.llvm_order && applied.options.optimize {
        manager.add_program(constmerge::ConstantMerge);
    }
    // Last: what it names are the instructions selection sees.
    if applied.options.spare_loads {
        manager.add_module(crate::spares::Spares);
    }
    if applied.options.stored_homes {
        manager.add_module(crate::homes::Homes);
    }
    let done = manager.run(program);
    if llrm_support::env_flag!("LLRM_CHECK_CAPTURES_MODULE") {
        for module in &program.modules {
            llrm_analysis::capturecheck::check_module(module, &program.layout);
        }
    }
    done
}

/// Whether gcc's `pass_ch` stage (`header_copied`) runs after the
/// interprocedural step. Not under LLVM's order: its LoopRotate copies the
/// headers in its own slots (`duplicate_headers`), and the pin has no second
/// pass over the loops there (d_faces: 52 M instructions at -O1, 41 M at -O2,
/// 72 M at -O3).
fn copies_headers_after_ipa(applied: &Applied) -> bool {
    applied.options.copy_headers && !applied.options.prefers_size() && !applied.options.llvm_order
}

/// Each loop's counters chosen once, on the loop the passes before leave. LSR
/// requires LoopSimplify and LCSSA (`getAnalysisUsage`): an exit value read
/// through a phi that is not LCSSA is not a use outside the loop, and LSR gave
/// it `-1` scales (jpeg_single's `DECODE%` -Os grew a second counter).
pub fn strength_slot(applied: &Applied) -> Vec<Box<dyn FunctionPass>> {
    let mut passes: Vec<Box<dyn FunctionPass>> = Vec::new();
    if applied.options.wanted("lsr") {
        passes.push(Box::new(loopsimplify::LoopSimplify));
        // The pin's llc takes the IR after opt's closing InstSimplify (l.1662,
        // "clean up LCSSA form before generating code").
        passes.push(Box::new(crate::instsimplify::InstSimplify));
        // The pin runs canon-freeze with loop-reduce, before it.
        passes.push(Box::new(crate::canonfreeze::CanonicalizeFreezeInLoops));
        passes.push(Box::new(crate::lsr_pass::LoopStrengthReduce { size: applied.options.prefers_size() }));
        // What the counters it chose leave behind (a bound subtracted from a
        // counter rebased by it), as LLVM's LSR cleans with
        // SimplifyInstructions.
        passes.push(Box::new(algebraic::Differences));
        // The remainder LSR wrote out as a quotient and a product, back to one
        // where the divide stays.
        passes.push(Box::new(crate::remidiom::RemainderIdiom { size: applied.options.prefers_size() }));
    }
    passes
}

/// What `pass_ch` runs and what it leaves to clean: the loops' headers copied,
/// the loops simplified again, what the copy leaves in the latch cleaned as
/// gcc's DOM, forwprop and DCE do, and what a loop entered at its body runs
/// at least once loads unchanged moved out before its counters are chosen, as
/// LLVM runs LICM before LoopStrengthReduce. Only the copy has entered loops at
/// their body by here; without it the last `Rotate` has not, and hoist follows
/// it. Not at -Os (`optimize_loop_for_size_p`): there only a loop proven to run
/// is entered at its body.
pub fn header_copied(applied: &Applied) -> Vec<Box<dyn FunctionPass>> {
    let mut passes: Vec<Box<dyn FunctionPass>> = Vec::new();
    if !copies_headers_after_ipa(applied) {
        return passes;
    }
    passes.push(Box::new(rotate::Rotate { proven: false, copy: true }));
    passes.push(Box::new(loopsimplify::LoopSimplify));
    passes.push(Box::new(lcssa::LoopClosedSSA));
    passes.push(Box::new(fold::Fold));
    passes.push(Box::new(algebraic::Algebraic { size: false, llvm: false }));
    // A counted exit's values are numbers where the copy made its test the one
    // that leaves.
    passes.push(Box::new(indvars::IndVars { lftr: indvars::lftr_default(), size: false }));
    passes.push(Box::new(dead::Dead));
    if applied.options.wanted("hoist") {
        passes.push(Box::new(crate::licm::Licm { apply: true, speculate: true }));
    }
    passes
}

thread_local! {
    /// What made the interprocedural step run a body's pipeline again (empty:
    /// the first run), for `LLRM_DEBUG=runs`.
    static TRIGGER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// `fixed` over body `id` alone, as the manager runs a function pass, its
/// module's analyses those `analyses` holds.
fn rerun(
    module: &mut Module,
    analyses: &mut ModuleAnalyses,
    id: GlobalId,
    fixed: &mut Fixed,
    globals_aa_held: bool,
) -> Result<(), String> {
    let layout = analyses.program().layout.clone();
    let outer = analyses.outer(module);
    let mut declared = Declared::over(std::rc::Rc::clone(&outer.globals), module.metadata.len());
    if fixed.wants_sizes() {
        declared.count_referenced(module);
    }
    let Module { context, globals, metadata, .. } = &mut *module;
    llrm_mir::passes::set_running_function(globals[id.0 as usize].name.as_deref().unwrap_or_default());
    let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else {
        return Err(format!("@{}: not a function", id.0));
    };
    let mut unit = Unit { context, layout: &layout, function, id: Some(id), metadata, declared: &mut declared };
    let mut preserved = fixed.run(&mut unit, analyses.manager(id, &outer));
    if globals_aa_held {
        preserved = preserved.preserve_module::<GlobalsAA>();
    }
    analyses.body_edited(id, &preserved);
    if declared.tables_pending() {
        analyses.abandon_stateless();
    }
    if declared.place(module)? > 0 {
        let mut dropped = PreservedAnalyses::none();
        if globals_aa_held {
            dropped = dropped.preserve_module::<GlobalsAA>();
        }
        analyses.invalidate(&dropped);
    }
    Ok(())
}

/// The pipeline over one body, the old `_Transaction`: the structural
/// passes at its boundaries, the scalar ones to a fixed point with
/// unrolling after each round, peeling once that settles, then
/// unswitching.
pub struct Fixed {
    boundary: Vec<Box<dyn FunctionPass>>,
    passes: Vec<Box<dyn FunctionPass>>,
    /// Once, after everything else.
    last: Vec<Box<dyn FunctionPass>>,
    unswitch: Option<simpleloopunswitch::SimpleLoopUnswitch>,
    interchange: Option<loopinterchange::Interchange>,
    only: bool,
    rounds: usize,
    dump: Option<PathBuf>,
    /// Bodies run, naming each one's dump.
    runs: usize,
}

impl Fixed {
    pub fn new(applied: &Applied) -> Self {
        if let Some(slot) = applied.only.as_deref().filter(|one| one.contains('@')) {
            return Self::slot(applied, slot);
        }
        // Public debugging selectors from before value reuse became one pass.
        let only = match applied.only.as_deref() {
            Some("forward" | "drop_loads" | "reuse" | "cse") => Some("gvn"),
            Some("drop_stores") => Some("dse"),
            other => other,
        };
        let translated = only.map(|_| translated()).unwrap_or_default();
        let passes =
            pipeline(applied).into_iter().chain(translated).filter(|one| only.is_none_or(|only| one.name() == only));
        // SROA establishes the scalar memory shape at structural
        // boundaries; it is no member of the scalar fixed point.
        let (boundary, passes): (Vec<_>, Vec<_>) = passes.partition(|one| one.name() == "sroa");
        let (last, passes): (Vec<_>, Vec<_>) =
            passes.into_iter().partition(|one| matches!(one.name(), "merge" | "memcpyopt"));
        let unswitch = applied.options.unswitch.then_some(simpleloopunswitch::SimpleLoopUnswitch { non_trivial: true });
        Self {
            boundary,
            passes,
            last,
            unswitch,
            interchange: (applied.options.interchange && only.is_none()).then(loopinterchange::Interchange::default),
            only: only.is_some(),
            rounds: applied.options.rounds,
            dump: applied.dump.clone(),
            runs: 0,
        }
    }

    /// The one slot `slot` (`sroa@1179`) of LLVM's order, run alone once: what
    /// `llrm-pass --order` compares with `opt`. [`slot_built`] says first
    /// whether the options build it.
    fn slot(
        applied: &Applied,
        slot: &str,
    ) -> Self {
        let (phase, ..) = crate::parity::describe(slot).expect("a slot of the registry");
        let mut quiet = applied.clone();
        quiet.only = None;
        quiet.options.llvm_order = true;
        let mut one = Self::linear(&quiet, phase);
        one.passes.retain(|pass| pass.name() == slot);
        one.only = true;
        one
    }

    /// `passes` once each in order, no fixed point: LLVM's function pipelines
    /// run a pass where it stands (`Options::llvm_order`).
    fn linear(
        applied: &Applied,
        phase: Phase,
    ) -> Self {
        // The passes that make a candidate and judge it stay after the list, as
        // in the rounds.
        let mut linear = Self::new(applied);
        linear.boundary.clear();
        linear.last.clear();
        linear.passes = llvm_order(applied, phase);
        let built = linear.passes.iter().map(|one| one.name()).collect::<Vec<_>>();
        let level = if applied.options.pre { crate::parity::Level::O2 } else { crate::parity::Level::O1 };
        let off = crate::parity::REGISTRY
            .iter()
            .filter(|one| one.phase == phase && !applied.options.wanted(one.slot))
            .map(|one| one.slot)
            .collect::<Vec<_>>();
        let held = linear.passes.iter().map(|one| (one.name(), one.translations())).collect::<Vec<_>>();
        let checked = crate::parity::check_phase_with(
            phase,
            &built,
            level,
            applied.options.sroa,
            applied.options.mem2reg,
            applied.options.rotate_slot,
            &off,
            &held,
        );
        debug_assert!(checked.is_ok(), "{checked:?}");
        linear.rounds = 1;
        // The slot of the order unswitches, where the pin does.
        linear.unswitch = None;
        // It has its own slot (`loop-interchange@1631`) in `Phase::Late`.
        linear.interchange = None;
        linear
    }

    fn transacted(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
        run: &mut Run,
    ) -> Result<(), String> {
        self.scalarized(unit, analyses, run, "r01");
        if self.only && !self.boundary.is_empty() {
            return Ok(run.settled(unit, analyses));
        }
        self.fixed(unit, analyses, run, "")?;
        if let Some(unswitch) = &mut self.unswitch {
            run.step(unswitch, &"unswitch", unit, analyses);
        }
        // The swapped nest is cleaned as a peeled loop is.
        if let Some(interchange) = &mut self.interchange {
            if run.step(interchange, &"interchange", unit, analyses) {
                self.scalarized(unit, analyses, run, "interchanged");
                self.fixed(unit, analyses, run, "interchanged-")?;
            }
        }
        for one in &mut self.last {
            let name = one.name();
            run.step(&mut **one, &name, unit, analyses);
        }
        Ok(run.settled(unit, analyses))
    }

    fn scalarized(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
        run: &mut Run,
        stage: &str,
    ) {
        for one in &mut self.boundary {
            let name = one.name();
            run.step(&mut **one, &format_args!("{stage}-{name}"), unit, analyses);
        }
    }

    fn fixed(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
        run: &mut Run,
        prefix: &str,
    ) -> Result<(), String> {
        // A monotone chain may expose one simplification per operation.
        let size = unit.function.layout().len() + unit.function.walk().count();
        let limit = std::cmp::max(16, size + 1);
        // The change each pass last left the body at, unchanged: handed that
        // body again it would change nothing, so it is skipped.
        let mut settled: Vec<Option<(usize, usize)>> = vec![None; self.passes.len()];
        // Separately reject a repeated state, so an oscillator fails at once
        // instead of consuming the limit.
        // Only a list that rounds more than once can come back to a state, so
        // only that one hashes the body (LLVM's order runs its lists once).
        let mut history = (self.rounds > 1).then(|| BTreeSet::from([print::fingerprint(unit.function)]));

        run.fixed += 1;
        for iteration in 0..limit {
            if iteration >= self.rounds {
                self.report_unspent(unit, analyses, iteration);
                run.settled(unit, analyses);
                return Ok(());
            }
            // A named point: what was kept as sound is worked out afresh.
            analyses.refresh_soft();
            let before = run.version;
            run.rounds += 1;
            for (one, settled) in self.passes.iter_mut().zip(&mut settled) {
                if let Some((version, stretches)) = *settled {
                    if version == run.version {
                        run.skipped += 1;
                        run.check_skipped(&mut **one, unit, analyses);
                        continue;
                    }
                    // Edits since it found nothing to do, none of which reaches
                    // what it reads.
                    run.classify(unit);
                    let reached = run.reached_since(stretches);
                    if !reached.meets(one.reads()) {
                        run.unreached += 1;
                        run.check_skipped(&mut **one, unit, analyses);
                        continue;
                    }
                }
                let pass = one.name();
                let name = format_args!("{prefix}r{:02}-{pass}", iteration + 1);
                *settled = (!run.step(&mut **one, &name, unit, analyses)).then(|| (run.version, run.stretches.len()));
            }
            if self.only || run.version == before {
                run.settled(unit, analyses);
                return Ok(());
            }
            // A list that runs once (LLVM's order) may end where it began:
            // EarlyCSE drops the LCSSA phis that `lcssa` makes
            // again.
            if history.as_mut().is_some_and(|seen| !seen.insert(print::fingerprint(unit.function))) {
                return Err(format!("MIR optimization did not converge: cycle after {} rounds", iteration + 1));
            }
        }
        Err(format!("MIR optimization did not converge after {limit} size-scaled rounds"))
    }
}

impl Fixed {
    /// `LLRM_CHECK_ROUNDS`: the pass that a round after the budget would have
    /// changed the body with, said on stderr; nothing fails, as the budget is a
    /// choice and a body may need more.
    fn report_unspent(
        &mut self,
        unit: &Unit,
        analyses: &Analyses,
        rounds: usize,
    ) {
        if !llrm_support::env_flag!("LLRM_CHECK_ROUNDS") {
            return;
        }
        let mut context = unit.context.clone();
        let mut function = unit.function.clone();
        let mut declared = unit.declared.clone();
        let mut copy = Unit {
            context: &mut context,
            layout: unit.layout,
            function: &mut function,
            id: unit.id,
            metadata: unit.metadata,
            declared: &mut declared,
        };
        for pass in &mut self.passes {
            let preserved = pass.run(&mut copy, &mut analyses.fresh());
            if !preserved.are_all_preserved() {
                eprintln!("LLRM_CHECK_ROUNDS: round {} would change the body with {}", rounds + 1, pass.name());
                return;
            }
        }
    }
}

impl FunctionPass for Fixed {
    fn dumps_inputs_itself(&self) -> bool {
        true
    }

    fn wants_sizes(&self) -> bool {
        self.passes.iter().any(|pass| pass.wants_sizes())
    }

    fn name(&self) -> &'static str {
        "pipeline"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        self.runs += 1;
        let promotes = !analyses
            .get::<cfg::Shape>(unit.context, unit.layout, unit.function)
            .dominance
            .is_irreducible(unit.function);
        let dump = self.dump.as_ref().map(|directory| directory.join(format!("{:02}", self.runs)));
        let mut run = Run {
            version: 0,
            promotes,
            inputs: dump.is_some() && llrm_support::env_flag!("LLRM_DUMP_INPUTS"),
            dump,
            rounds: 0,
            steps: 0,
            skipped: 0,
            fixed: 0,
            billing: runs_billed(),
            idle: 0,
            useful: 0,
            idle_with_analyses: 0,
            useful_with_analyses: 0,
            classified: unit.function.mark(),
            stretches: Vec::new(),
            unreached: 0,
            slice: None,
            persisted: 0,
            records: unit.context.extension::<Settled>(),
            rerun: false,
            idle_at: Vec::new(),
        };
        run.rerun = run.records.knows(unit.function);
        self.transacted(unit, analyses, &mut run).unwrap_or_else(|error| panic!("pipeline: {error}"));
        run.remember(unit, analyses);
        runs_line(format_args!(
            "body {} trigger {:?}: {} fixed points, {} rounds, {} pass runs, {} skipped as settled, {} skipped as unreached, {} skipped as settled in an earlier run, {} changes, work idle {} useful {} (with the analyses the passes computed first: idle {} useful {}) name {}",
            unit.id.map_or(-1, |id| i64::from(id.0)),
            TRIGGER.with(|trigger| trigger.borrow().clone()),
            run.fixed,
            run.rounds,
            run.steps,
            run.skipped,
            run.unreached,
            run.persisted,
            run.version,
            run.idle,
            run.useful,
            run.idle_with_analyses,
            run.useful_with_analyses,
            llrm_mir::passes::running_function()
        ));
        if run.version == 0 { PreservedAnalyses::all() } else { PreservedAnalyses::none() }
    }
}

/// The instructions in the function's layout: erased ones stay in the arena
/// and are not counted.
fn placed_instructions(function: &llrm_mir::module::Function) -> usize {
    function.layout().iter().map(|&block| function.block(block).len()).sum()
}

thread_local! {
    /// Tests: the `runs` lines go here, and are billed without `LLRM_DEBUG`.
    static RUNS_CAPTURE: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

fn runs_billed() -> bool {
    llrm_support::debug::enabled("runs") || RUNS_CAPTURE.with(|capture| capture.borrow().is_some())
}

fn runs_line(line: std::fmt::Arguments) {
    let captured = RUNS_CAPTURE.with(|capture| capture.borrow_mut().as_mut().map(|lines| lines.push(line.to_string())));
    if captured.is_none() {
        llrm_support::debug!("runs", "{}", line);
    }
}

/// `run` with the `runs` lines kept, not printed.
#[cfg(test)]
pub(crate) fn capturing_runs<T>(run: impl FnOnce() -> T) -> (T, Vec<String>) {
    RUNS_CAPTURE.with(|capture| *capture.borrow_mut() = Some(Vec::new()));
    let done = run();
    (done, RUNS_CAPTURE.with(|capture| capture.borrow_mut().take()).unwrap_or_default())
}

/// `LLRM_CHECK_SETTLED`: a pass skipped as settled by an earlier run is run,
/// and fails where it changes the body.
fn settled_checked() -> bool {
    llrm_support::env_flag!("LLRM_CHECK_SETTLED")
}

/// One body's run through `Fixed`.
struct Run {
    /// Changes so far.
    version: usize,
    /// Whether promotion may run: not over an irreducible CFG.
    promotes: bool,
    dump: Option<PathBuf>,
    /// `LLRM_DUMP_INPUTS`: the body at the input of every pass run is dumped.
    inputs: bool,
    /// For `LLRM_DEBUG=runs`: fixed points reached, rounds in them, passes run,
    /// passes skipped as settled.
    fixed: usize,
    rounds: usize,
    steps: usize,
    skipped: usize,
    /// The work of passes that changed nothing, and of those that did (only
    /// where `runs` is on).
    billing: bool,
    idle: u64,
    useful: u64,
    /// The same two with the analyses the passes computed first: what skipping
    /// a pass would not save, since the next pass to ask would pay it.
    idle_with_analyses: u64,
    useful_with_analyses: u64,
    /// Where the log of edits has been classified to, and what each stretch of
    /// it reached (`footprint::of`), found once for every pass that asks.
    classified: llrm_mir::module::Mark,
    stretches: Vec<llrm_mir::footprint::Footprint>,
    /// Passes not run because no edit since they found nothing reached what
    /// they read.
    unreached: usize,
    /// The facts outside the body, as of a point in its history
    /// (`settled::slice`).
    slice: Option<(llrm_mir::module::Mark, settled::Slice)>,
    /// Passes not run because they found nothing in an earlier run, under the
    /// same facts (`settled`).
    persisted: usize,
    /// The records of the earlier runs of this body, and whether there are any.
    records: std::rc::Rc<Settled>,
    rerun: bool,
    /// What each pass last found nothing on in this run: where, and what it
    /// read.
    idle_at: Vec<(&'static str, llrm_mir::module::Mark, u8)>,
}

impl Run {
    /// The edits made since the last stretch, classified as one: after every
    /// pass, so that no stretch holds the edits of two passes. A function
    /// put in place of the body (a copy has a history of its own) is every
    /// part changed.
    fn classify(
        &mut self,
        unit: &Unit,
    ) {
        let now = unit.function.mark();
        let reached = if !now.same_history(self.classified) {
            llrm_mir::footprint::Footprint::ALL
        } else if now.position() != self.classified.position() {
            unit.function
                .changes_since(self.classified)
                .map_or(llrm_mir::footprint::Footprint::ALL, |changes| llrm_mir::footprint::of(unit.function, changes))
        } else {
            return;
        };
        self.stretches.push(reached);
        self.classified = now;
    }

    /// What the stretches since the `len`th reached.
    fn reached_since(
        &self,
        len: usize,
    ) -> llrm_mir::footprint::Footprint {
        self.stretches[len..].iter().fold(llrm_mir::footprint::Footprint::NONE, |all, reached| all | *reached)
    }

    /// The passes that found nothing are recorded as settled on the body as
    /// they found it: a later run starts none of them where what they read is
    /// as it was (`settled`), which the edits since are asked of then, not
    /// here.
    fn remember(
        &mut self,
        unit: &Unit,
        analyses: &Analyses,
    ) {
        if self.idle_at.is_empty() {
            return;
        }
        let _span = llrm_support::debug::span("settled remember");
        let slice = self.slice(unit, analyses);
        for &(name, mark, bits) in &self.idle_at {
            self.records.note(name, mark, slice, bits);
        }
    }

    /// The facts outside the body its passes read, as it stands.
    fn slice(
        &mut self,
        unit: &Unit,
        analyses: &Analyses,
    ) -> settled::Slice {
        let now = unit.function.mark();
        match self.slice {
            Some((at, slice)) if at == now => slice,
            _ => {
                let _span = llrm_support::debug::span("settled slice");
                let slice = settled::slice(unit, analyses.outer_unread());
                self.slice = Some((now, slice));
                slice
            }
        }
    }

    /// `pass` over the body; whether it changed it.
    fn step(
        &mut self,
        pass: &mut dyn FunctionPass,
        stage: &dyn std::fmt::Display,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> bool {
        if !self.promotes && matches!(base(pass.name()), "sroa" | "promote") {
            return false;
        }
        let name = pass.name();
        let mut skipped = false;
        if self.rerun {
            let slice = self.slice(unit, analyses);
            skipped = self.records.holds(name, unit.function, slice, pass.reads());
        }
        let checking = skipped && settled_checked();
        if skipped && !checking {
            self.persisted += 1;
            llrm_mir::passes::read::note(self.records.bits(name, unit.function));
            return false;
        }
        self.steps += 1;
        self.input(name, unit, analyses);
        // `LLRM_CHECK_ORDER=SEED`: some analyses asked at this pass's entry,
        // which ones by the seed, the step and the body, so that each
        // is first asked earlier than a pass needs it, somewhere else
        // on every seed. A result a pass reads is a function of the
        // body: the object is the lazy one's.
        if let Some(seed) = order_seed() {
            let mix = seed
                ^ (self.steps as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (unit.function.mark().position() as u64);
            llrm_analysis::manager::ask_some(unit.context, unit.layout, unit.function, analyses, mix);
        }
        let outside = llrm_mir::passes::read::swap(0);
        let billed = self.billing.then(|| {
            (llrm_support::debug::work(), llrm_support::debug::analysed_work(), placed_instructions(unit.function))
        });
        let running = llrm_mir::counts::pass_run_over(pass.name(), unit.function);
        let preserved =
            llrm_mir::passes::spanned(pass.name(), || llrm_mir::passes::run_checked(&mut *pass, unit, analyses));
        drop(running);
        analyses.declared(unit.context, unit.declared);
        let read = llrm_mir::passes::read::swap(outside);
        llrm_mir::passes::read::note(read);
        self.classify(unit);
        if let Some((billed, analysed, entering)) = billed {
            let with_analyses = llrm_support::debug::work() - billed;
            let own = with_analyses.saturating_sub(llrm_support::debug::analysed_work() - analysed);
            let idle = preserved.are_all_preserved();
            *(if idle { &mut self.idle } else { &mut self.useful }) += own;
            *(if idle { &mut self.idle_with_analyses } else { &mut self.useful_with_analyses }) += with_analyses;
            runs_line(format_args!(
                "step {stage} {} {own} with-analyses {with_analyses} in={entering} out={}",
                if idle { "idle" } else { "changed" },
                placed_instructions(unit.function)
            ));
        }
        assert!(
            !checking || preserved.are_all_preserved(),
            "{}: skipped as settled by an earlier run, it changed the body ({stage})",
            pass.name()
        );
        if preserved.are_all_preserved() {
            // Every result stands, as the body is now (it may have edited and
            // put back).
            analyses.vouch(unit.function);
            self.idle_at.retain(|one| one.0 != name);
            self.idle_at.push((name, unit.function.mark(), read));
            return false;
        }
        self.idle_at.retain(|one| one.0 != name);
        self.records.forget(name, unit.function);
        llrm_mir::passes::note_pass(pass.name());
        llrm_mir::passes::spanned("invalidate", || analyses.invalidate(unit.function, &preserved));
        analyses.check_kept(pass.name(), unit.context, unit.layout, unit.function);
        self.changed(stage, unit, analyses);
        true
    }

    /// `LLRM_CHECK_SKIPPED`: a pass skipped as settled or as reading nothing
    /// the edits since reached is run on a copy, and finds nothing to do.
    fn check_skipped(
        &self,
        pass: &mut dyn FunctionPass,
        unit: &Unit,
        analyses: &Analyses,
    ) {
        if !llrm_support::env_flag!("LLRM_CHECK_SKIPPED")
            || (!self.promotes && matches!(base(pass.name()), "sroa" | "promote"))
        {
            return;
        }
        let mut context = unit.context.clone();
        let mut function = unit.function.clone();
        let mut declared = unit.declared.clone();
        let mut copy = Unit {
            context: &mut context,
            layout: unit.layout,
            function: &mut function,
            id: unit.id,
            metadata: unit.metadata,
            declared: &mut declared,
        };
        let preserved = pass.run(&mut copy, &mut analyses.fresh());
        assert!(preserved.are_all_preserved(), "{} was skipped and would have changed the body", pass.name());
    }

    /// The body a pipeline hands out, with its unreachable blocks settled.
    fn settled(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) {
        if cfg::_unreachable(unit.context, unit.function) {
            analyses.invalidate(unit.function, &PreservedAnalyses::none());
            self.changed(&"unreachable", unit, analyses);
        }
    }

    fn changed(
        &mut self,
        stage: &dyn std::fmt::Display,
        unit: &Unit,
        analyses: &Analyses,
    ) {
        self.version += 1;
        let Some(directory) = &self.dump else { return };
        let file = directory.join(format!("{:03}-{stage}.ll", self.version));
        Self::write(&file, unit, analyses);
    }

    /// The body beside every global's declaration, which is what it names.
    fn write(
        file: &std::path::Path,
        unit: &Unit,
        analyses: &Analyses,
    ) {
        let mut globals = analyses.outer().globals.to_vec();
        globals.push(GlobalValue {
            name: "pipeline.body".to_owned().into(),
            linkage: Linkage::Internal,
            unnamed_addr: UnnamedAddr::default(),
            address_space: 0,
            kind: GlobalKind::Function(Box::new(unit.function.clone())),
        });
        let module = Module {
            context: unit.context.clone(),
            datalayout: None,
            globals,
            metadata: unit.metadata.to_vec(),
            named_metadata: Vec::new(),
            sections: Default::default(),
            special: Default::default(),
            global_metadata: Default::default(),
        };
        std::fs::create_dir_all(file.parent().expect("a file in a directory"))
            .and_then(|()| std::fs::write(file, llrm_mir::print::module(&module)))
            .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
    }

    /// `LLRM_DUMP_INPUTS`: the body each pass is about to run over, a run that
    /// changes nothing included, with a line in `manifest.txt` (pass, run,
    /// file, instructions) beside the dump.
    fn input(
        &self,
        name: &str,
        unit: &Unit,
        analyses: &Analyses,
    ) {
        let Some(directory) = &self.dump else { return };
        if !self.inputs {
            return;
        }
        let file = format!("{:04}-in-{name}.ll", self.steps);
        Self::write(&directory.join(&file), unit, analyses);
        let line = format!("{name} {} {file} {}\n", self.steps, unit.function.walk().count());
        let manifest = directory.join("manifest.txt");
        let mut out = std::fs::OpenOptions::new().create(true).append(true).open(&manifest).expect("a manifest");
        std::io::Write::write_all(&mut out, line.as_bytes()).expect("a manifest line");
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod pipeline_tests;

/// The seed of `LLRM_CHECK_ORDER`, where it is on and a number.
fn order_seed() -> Option<u64> {
    static SEED: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *SEED.get_or_init(|| {
        llrm_support::env_flag!("LLRM_CHECK_ORDER")
            .then(|| std::env::var("LLRM_CHECK_ORDER").ok().and_then(|value| value.parse().ok()))
            .flatten()
    })
}
