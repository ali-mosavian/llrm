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
//!     rnd["<b>round, until no pass changes</b><br/><small>the body: fold ports simplifycfg<br/>decide phiopt equalcalls tailrec<br/>loopsimplify lcssa floatloop<br/>licm<br/>trivialunswitch inferspace<br/>dse gvn promote indvars<br/>algebraic dead#59; unroll once<br/>per round (a change sends<br/>the copy to sroa)</small>"]:::mid
//!     peel["<b>peel</b><br/><small>while it removes loops:<br/>sroa, then B's rounds</small>"]:::mid
//!     unsw["<b>unswitch (-O3, -Omax)</b><br/><small>simple-loop-unswitch,<br/>non-trivial</small>"]:::mid
//!     dist["<b>distribute (-O3, -Omax)</b><br/><small>candidate judged on a<br/>full pipeline</small>"]:::mid
//!     ver["<b>version (-O3, -Omax)</b><br/><small>candidate judged on a<br/>full pipeline</small>"]:::mid
//!     fuse["<b>fuse (-Omax)</b><br/><small>candidate judged on a<br/>full pipeline</small>"]:::mid
//!     swap["<b>interchange (-O3, -Omax)</b><br/><small>sroa, then B's rounds</small>"]:::mid
//!     jam["<b>unroll-jam (-O3, -Omax)</b><br/><small>sroa, then B's rounds</small>"]:::mid
//!     mrg["<b>merge (once)</b>"]:::mid
//!   end
//!   c["<b>C  whole program,</b><br/><small>interprocedural: inline,<br/>constants, dead pure calls<br/>(stages: LLRM_DEBUG=runs)</small>"]:::mid
//!   d["<b>D  globaldce,</b><br/><small>available-externally,<br/>calleepop#59; freeze the<br/>Summaries and GlobalsAA</small>"]:::mid
//!   e["<b>E  once, each body, with the</b><br/><small>analyses frozen<br/>[rotate#123;copy#125; loopsimplify<br/>lcssa fold algebraic<br/>indvars#123;exit#125; dead]: only<br/>with -ftree-ch, not at -Os<br/>fixednarrow lsr differences<br/>window rotate#123;proven#125;<br/>hoist gepoffset<br/>addresssink sink</small>"]:::mid
//!   f["<b>F  last: spares, homes</b><br/><small>(instruction ids selection sees)</small>"]:::mid
//!   a --> sroa
//!   sroa --> rnd
//!   rnd --> peel
//!   peel --> unsw
//!   unsw --> fuse
//!   fuse --> dist
//!   dist --> ver
//!   ver --> swap
//!   swap --> jam
//!   jam --> mrg
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
//! as names. [`Fixed`] takes four passes out of it: `sroa` runs at structural
//! boundaries, `unroll` and `peel` run at the places drawn above, `merge`
//! once at the end. A pass that is off is not in the list.
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
//! re-optimize it with a `Fixed` of its own that has unswitching and jamming
//! off.
//!
//! In E, `lsr` is after the interprocedural step because nothing after it may
//! choose a loop's counters again; `rotate{proven}` is last of the loop
//! passes because `unroll` and `peel` refuse a rotated loop; `hoist`
//! runs again to lift what a rotated loop (run at least once) may now leave;
//! `gepoffset` follows it because it would move a constant `gep` out of its
//! loop; `addresssink` follows the last `gvn`, which makes the phis it sinks;
//! `sink` follows `addresssink`, once the passes that move work up have run.
//!
//! # Example
//! No honest example: the pipeline is the order of the passes, not one rewrite,
//! and no one function shows it. `llrm-c --dump DIR` writes the module after
//! each stage (`NN-stage.ll`) and, for each body, each changed step of the
//! fixed point (`N/NNN-rRR-pass.ll`); the pass examples come from those.
//!
//! # Runs
//! Options are `Options`; `Level::options` in llrm-core's `driver/flags.rs`
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
//! | `strength` | `indvars`, `lsr`, `window`, `gepoffset`, `addresssink` |
//! | `unroll`, `peel` | the pass of that name |
//! | `loopidioms` | `loopidioms`, `merge`, `memcpyopt` |
//! | `sibcalls` | `tailrec` |
//! | `ipa_ranges` | argument ranges in the interprocedural step |
//! | `unswitch`, `split`, `interchange`, `unroll_jam` | the passes of that name (off by default; on at -O3 and -Omax) |
//! | `distribute` | `distribute` (off by default; on at -O3 and -Omax) |
//! | `version` | `version` (off by default; on at -O3 and -Omax) |
//! | `fuse` | `fuse` (off by default; on at -Omax) |
//! | `copy_headers` | the header copy in E (off by default; `-ftree-ch`) |
//!
//! `pre` makes `gvn` insert on the path that lacks a value; `for_size` prices `phiopt`, `hoist`, `algebraic`, `loopidioms`,
//! `lsr`, `window` and `calleepop` in bytes; `loop_intervals`
//! lets alias and branch decisions use the intervals of counted loops. `limits`
//! are the copy budgets of `unroll` and `peel`.
//!
//! # Needs
//! `Summaries`, `GlobalsAA`, `Callbacks` and `ProgramSummaries` are required
//! up front, and frozen at the end of D so the passes of E read them without
//! recomputing after each edit.
//!
//! # See also
//! gcc's pass lists, `passes.def`; LLVM's `PassBuilderPipelines.cpp`.
//! `LLRM_DEBUG=time` times each step, `runs` counts rounds, skips and the
//! work of passes that changed nothing.

use std::collections::BTreeSet;
use std::path::PathBuf;

use llrm_analysis::cfg;
use llrm_analysis::manager::{Callbacks, GlobalsAA, ProgramSummaries, Summaries};
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
    adce, addresssink, algebraic, availableexternally, bdce, calleepop, dead, deadstore, decide, dse, fixednarrow,
    floatloop, fold, gepoffset, globaldce, globalopt, gvn, indvars, inferspace, inline, lcssa,
    loopdeletion, loopdistribute, loopfuse, loopidioms, loopinterchange, looprotate, loopsimplify, loopsplit,
    loopunrollpass, loopversion, mem2reg, memcpyopt, peel, phiopt, ports, promote, rotate, sccp, simpleloopunswitch,
    simplifycfg, switchlower, tailcallelim, trivialunswitch, unroll, unroll_jam, window,
};

/// Which passes run, and the copy budgets: the old `Options`. The default
/// is -O2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    /// Off at -O0: no pass runs.
    pub optimize: bool,
    pub limits: Limits,
    pub inline: inline::Threshold,
    /// Whether the callers' arguments are stated as ranges on a body's
    /// parameters: gcc's `-fipa-vrp`, -O2 and up.
    pub ipa_ranges: bool,
    /// Alias and branch decisions use the intervals of counted loops (gcc's
    /// `-ftree-vrp`, -O2; its -O1 has them for induction variables only).
    pub loop_intervals: bool,
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
    pub drop_stores: bool,
    /// Work read below a branch moves below it: gcc's `-ftree-sink`.
    pub sink: bool,
    pub promote: bool,
    pub strength: bool,
    pub unroll: bool,
    pub peel: bool,
    pub loopidioms: bool,
    pub sibcalls: bool,
    pub unswitch: bool,
    /// A loop with a test of its counter against an invariant becomes two,
    /// one either side of where it flips (gcc's `-fsplit-loops`, -O3; -Omax
    /// too).
    pub split: bool,
    /// Two adjacent loops of the same trips become one (LLVM's `LoopFuse`;
    /// -Omax only, `-floop-fusion`).
    pub fuse: bool,
    /// A loop whose stores do not need each other's trips becomes a loop
    /// for each (gcc's `-ftree-loop-distribution`, -O3; -Omax too).
    pub distribute: bool,
    /// A loop is copied for what is true of it at run time, behind a test of
    /// it: a stride of one, pointers apart (gcc's
    /// `-fversion-loops-for-strides`, -O3; LLVM's `LoopVersioning`; -Omax
    /// too).
    pub version: bool,
    /// A nest's outer loop unrolled and its inner copies fused (gcc's
    /// `-floop-unroll-and-jam`, -O3; -Omax too).
    pub unroll_jam: bool,
    /// `-floop-interchange`, -O3 (gcc's `gimple-loop-interchange.cc`); -Omax
    /// too. LLVM's `LoopInterchange` is off in its pipelines.
    pub interchange: bool,
    /// The passes run in LLVM's order, once each (`Fixed::linear`:
    /// [`llvm_order`]), not in the rounds; on by default, `LLRM_NO_LLVM_ORDER`
    /// turns it off.
    pub llvm_order: bool,
    /// With `llvm_order`, LPM1's `loop-rotate` between the two LICM runs
    /// (PassBuilderPipelines.cpp L728), the first LICM not speculating; off by
    /// default (it miscompiles, #2585), `LLRM_ROTATE` turns it on. Not run
    /// where `unroll_jam` is (see [`Options::rotating`]).
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
    /// gcc's `-fexpensive-optimizations` (-O2 and up): the passes it gates.
    pub expensive: bool,
    /// The allocator tries other shapes of a body and keeps the cheapest
    /// (`-fallocation-search`; -Omax: gcc and LLVM allocate once).
    pub search: bool,
    /// Both routes through the machine phases are made and the cheaper kept
    /// (`-fallocation-routes`; -Omax); else the allocator alone.
    pub routes: bool,
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
            optimize: true,
            llvm_order: llvm_order_default(),
            rotate_slot: rotate_default(),
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
            ipa_ranges: true,
            loop_intervals: true,
            forward: true,
            drop_loads: true,
            pre: true,
            drop_stores: true,
            sink: true,
            promote: true,
            strength: true,
            unroll: true,
            peel: true,
            loopidioms: true,
            sibcalls: true,
            unswitch: false,
            split: false,
            fuse: false,
            distribute: false,
            version: false,
            unroll_jam: false,
            interchange: false,
            mem2reg: experiment_mem2reg(),
            sroa: !llrm_support::env_flag!("LLRM_NO_SROA"),
            combine: !llrm_support::env_flag!("LLRM_NO_COMBINE"),
            licm: !llrm_support::env_flag!("LLRM_NO_LICM"),
            copy_headers: true,
            for_size: false,
            expensive: true,
            switch_conversion: true,
            jump_tables: true,
            search: false,
            routes: false,
            exhaustive: false,
        }
    }
}

impl Options {
    /// -O0.
    pub fn none() -> Self {
        Self { optimize: false, ..Self::default() }
    }

    /// -O1: gcc's: the scalar passes and `-finline-functions-called-once`; a
    /// loop is copied out completely only where the code does not grow;
    /// nothing is inlined but the last call of a function (gcc has no
    /// `-finline-small-functions` below -O2), no allocation search or second
    /// route (gcc has neither: compile -34% on d_faces), value numbering that
    /// inserts nothing (`-ftree-fre`; no `-ftree-pre`), and no sibling
    /// calls, peeling or unswitching. Pattern fill is on
    /// (`LoopIdiomRecognize` is in LLVM's O1 pipeline,
    /// PassBuilderPipelines.cpp L562): the one departure from gcc's -O1
    /// here, the user's decision of 2026-10-10 (docs/reference/levels.md).
    pub fn basic() -> Self {
        Self {
            limits: Limits { grows: false, ..Self::default().limits },
            // gcc's -O1 has `-finline-functions-called-once` and no
            // `-finline-small-functions`, and no allocation search or second
            // route: with them off compile is 34% less on d_faces.
            inline: inline::Threshold::new(0),
            // gcc runs each pass once: the third round changed five passes of
            // d_faces and the fourth none, 253 of 3,084 Minstr at -O1.
            rounds: 2,
            ipa_ranges: false,
            loop_intervals: false,
            pre: false,
            sibcalls: false,
            peel: false,
            unswitch: false,
            split: false,
            fuse: false,
            distribute: false,
            version: false,
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
            split: false,
            fuse: false,
            distribute: false,
            version: false,
            ..Self::default()
        }
    }

    /// Whether the rotate slot runs: it does not where `unroll_jam` does,
    /// whose nests are the header-tested form (RJ1: `loopnest.rs` and
    /// `loopclone::chained` take a loop that leaves only from its header).
    pub fn rotating(&self) -> bool {
        self.rotate_slot && !self.unroll_jam
    }

    /// -O3: gcc's: -O2 with peeling, unswitching, complete copies that grow the
    /// code, and the larger inline threshold; the copy budget is LLVM's -O3
    /// (`UnrollThresholdAggressive`, twice the target's), not gcc's.
    pub fn speed() -> Self {
        Self {
            inline: inline::Threshold { cp_clone: true, ..inline::Threshold::new(250) },
            unswitch: true,
            split: true,
            fuse: false,
            distribute: true,
            version: true,
            unroll_jam: true,
            interchange: true,
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
            split: true,
            fuse: true,
            distribute: true,
            version: true,
            unroll_jam: true,
            interchange: true,
            search: true,
            routes: true,
            exhaustive: true,
            rounds: usize::MAX,
            ..Self::default()
        }
    }

    /// -Os: no copy grows the code. Inlining keeps -O2's threshold: the
    /// threshold bounds a body's size, and what it admits, a body called
    /// once or one whose copies cost less than the calls they remove,
    /// shrinks the code here. A lower one would also refuse a constant-site
    /// clone that folds away.
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

    /// Whether both routes are made and the cheaper kept.
    pub fn compares_routes(&self) -> bool {
        self.routes
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
        Self { unroll: false, peel: false, ..Self::size() }
    }

    /// Whether pass `name` is on. Every pass can be turned off, which is
    /// how a miscompile is bisected.
    fn wanted(
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
            "indvars" | "lsr" | "window" | "gepoffset" | "addresssink" => self.strength,
            "unroll" => self.unroll,
            "peel" => self.peel,
            "loopidiom" | "loopidioms" | "merge" | "memcpyopt" => self.loopidioms,
            "tailrec" => self.sibcalls,
            "ipa-ranges" => self.ipa_ranges,
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
    let limits = || applied.options.limits.clone();
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
        Box::new(decide::Decide),
        // gcc runs phiopt at -O1 and above (opts.cc:609); it is priced in bytes
        // at -Os.
        Box::new(phiopt::PhiOpt { size: applied.options.prefers_size() }),
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
        Box::new(dse::Dse),
        Box::new(gvn::Gvn { pre: applied.options.pre }),
        // Ordinary scalar write-through promotion remains after memory GVN;
        // `LLRM_NO_MEM2REG` keeps the write-through stand-in instead of the
        // translated PromoteMemToReg.
        if applied.options.mem2reg { Box::new(mem2reg::Mem2Reg) } else { Box::new(promote::Promote) },
        Box::new(indvars::IndVars { lftr: indvars::lftr_default() }),
        Box::new(algebraic::Algebraic { size: applied.options.prefers_size(), llvm: false }),
        Box::new(dead::Dead),
        Box::new(unroll::Unroll { limits: limits() }),
        Box::new(peel::Peel { limits: limits() }),
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

/// `LLRM_ROTATE`, read once: set, the rotate slot of [`llvm_order`] runs.
fn rotate_default() -> bool {
    llrm_support::env_flag!("LLRM_ROTATE")
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
    let decide = |name| slot(name, Box::new(decide::Decide));
    let simplify_cfg = |name| slot(name, Box::new(simplifycfg::SimplifyCfg::default()));
    let simplify_cfg_with = |name, options| slot(name, Box::new(simplifycfg::SimplifyCfg { options }));
    let phiopt = |name| slot(name, Box::new(phiopt::PhiOpt { size }));
    let fold = |name| slot(name, Box::new(fold::Fold));
    let combine = |name| slot(name, Box::new(crate::combine_pass::Combine { enabled: applied.options.combine }));
    let dead = |name| slot(name, Box::new(dead::Dead));
    // SROA runs mem2reg on what it splits: one slot, nothing between
    // (SROA.cpp `promoteAllocas`).
    let translated = applied.options.sroa;
    let sroa = |name: &'static str| -> Box<dyn FunctionPass> {
        let parts: Vec<Box<dyn FunctionPass>> = if translated {
            // DEVIATE: PromoteMemToReg promotes allocas only; the write-through
            // `Promote` after it still promotes the cells it declines (an
            // internal global only `main` uses: LLVM localizes it to an alloca
            // in GlobalOpt, which llrm's does not yet).
            vec![
                Box::new(crate::sroa::Sroa),
                Box::new(crate::mem2reg::Mem2Reg),
                Box::new(promote::Promote),
                Box::new(promote::DeadSlots),
            ]
        } else {
            vec![Box::new(promote::Sroa), Box::new(promote::Promote), Box::new(promote::DeadSlots)]
        };
        Box::new(Together { name, parts })
    };
    let earlycse = |name| slot(name, Box::new(crate::earlycse::EarlyCse));
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
            vec![simplify_cfg("decide@1178"), phiopt("phiopt@1178"), sroa("sroa@1179"), earlycse("earlycse@1180")]
        }
        Phase::Cleanup => vec![
            Box::new(Together {
                name: "promote@1241",
                // DEVIATE as in the SROA slot: `Promote` keeps promoting the
                // cells (globals) PromoteMemToReg does not.
                parts: if applied.options.mem2reg {
                    vec![Box::new(mem2reg::Mem2Reg), Box::new(promote::Promote), Box::new(promote::DeadSlots)]
                } else {
                    vec![Box::new(promote::Promote), Box::new(promote::DeadSlots)]
                },
            }),
            combine("combine@1242"),
            fold("fold@1242"),
            dead("dead@1242"),
            // `SimplifyCFGOptions().convertSwitchRangeToICmp(true)`
            // (PassBuilderPipelines.cpp#L1245): the option is read by
            // `simplifySwitch`, not translated yet, so these are the defaults.
            simplify_cfg("decide@1244"),
            phiopt("phiopt@1244"),
        ],
        // The Late stage follows the Simplify list and its unroll-and-jam:
        // L1438, Float2IntPass (L1594), InstSimplifyPass (PassBuilderPipelines.cpp#L1662, the last
        // function pass of `OptimizePM`, after LoopSink) and L2376.
        // DEVIATE LS1: the jam is not in the pin's default pipeline
        // (`enable-unroll-and-jam` is false, L222); at the pin, if enabled,
        // L1438 comes first, then the jam at L1474. llrm runs the jam first
        // because its matcher needs the pre-L1438 nest: `needCanonicalLoops
        // (false)` merges the blocks it matches (a testing outer header, a
        // chain `pre` and `post`; hoist and sink alone leave them), and
        // `only_o3_jams_a_nest_that_sweeps_the_same_cells` fails with the slot
        // at the end of the Simplify list. Exit: the matcher accepts the
        // L1438 form, no date.
        Phase::Late => vec![
            // The slots' options, with the pin's lines,
            // are `simplifycfg::Options::{L1438, L2376}`.
            simplify_cfg_with("simplifycfg@1438", simplifycfg::Options::L1438),
            slot("float2int@1594", Box::new(crate::float2int::Float2Int)),
            slot("instsimplify@1662", Box::new(crate::instsimplify::InstSimplify)),
            simplify_cfg_with("simplifycfg@2376", simplifycfg::Options::L2376),
        ],
        Phase::Simplify => {
            let mut list = vec![sroa("sroa@646"), earlycse("earlycse@649")];
            list.extend([
                slot("jumpthreading@668", Box::new(crate::jumpthreading::JumpThreading::from_env())),
                slot("cvp@669", Box::new(crate::cvp::Cvp)),
                // SimplifyCFGOptions().convertSwitchRangeToICmp(true), L675:
                // the option is read by `simplifySwitch`, untranslated.
                simplify_cfg("decide@675"),
                phiopt("phiopt@675"),
                combine("combine@677"),
                fold("fold@677"),
                dead("dead@677"),
                slot("tailrec@688", Box::new(tailcallelim::TailCallElim)),
                // SimplifyCFGOptions().convertSwitchRangeToICmp(true), L691:
                // the option is read by `simplifySwitch`, untranslated.
                simplify_cfg("decide@690"),
                phiopt("phiopt@690"),
                slot("reassociate@696", Box::new(crate::reassociate::Reassociate)),
                slot("algebraic@696", Box::new(algebraic::Algebraic { size, llvm: true })),
            ]);
            list.extend(canonical("loopsimplify@764", "lcssa@764"));
            // Without the rotate slot the first LICM keeps speculating, as the
            // single hoist did.
            let rotating = applied.options.rotating();
            list.extend([
                slot("floatloop@724", Box::new(floatloop::FloatLoop)),
                // LPM1
                // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L724:
                // licm<no-allowspeculation> L724, loop-rotate L728,
                // licm<allowspeculation> L730. The first hoists only what the
                // loop certainly runs; the rotate puts a guard before a loop
                // not proven to run, so that the second hoists into a preheader
                // behind it (#2194). llrm skips the header copy at -Os, as
                // gcc's -ftree-ch.
                licm("licm@724", !rotating),
            ]);
            if rotating {
                // DEVIATE (D10 R7): the pin duplicates headers at -Os; `size`
                // is -Os and -Oz.
                list.push(slot(
                    "rotate@728",
                    Box::new(looprotate::LoopRotate { duplicate_headers: applied.options.copy_headers && !size }),
                ));
                list.extend(canonical("loopsimplify@728", "lcssa@728"));
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
                phiopt("phiopt@766"),
                combine("combine@768"),
                fold("fold@768"),
                dead("dead@768"),
            ]);
            list.extend(canonical("loopsimplify@772", "lcssa@772"));
            list.extend([
                slot("loopidiom@737", Box::new(crate::loopidiom::pass::LoopIdiom::new())),
                slot("loopidioms@737", Box::new(loopidioms::LoopIdioms { size })),
                slot("indvars@738", Box::new(indvars::IndVars { lftr: indvars::lftr_default() })),
                slot("loopdeletion@749", Box::new(loopdeletion::LoopDeletion)),
                slot(
                    "unroll@753",
                    Box::new(loopunrollpass::LoopFullUnroll {
                        opt_level: if applied.options.limits.target_percent >= 200 { 3 } else { 2 },
                        opt_size: size,
                    }),
                ),
                sroa("sroa@776"),
                slot("inferspace@787", Box::new(inferspace::InferAddressSpaces)),
                // The pin's `default<O1>` has no GVN (`pre` is off exactly at
                // -O1 and -Og); rows against clang's -O1 are pending, a loss
                // there becomes a numbered DEVIATE.
                slot("gvn@787", Box::new(gvn::Gvn { pre: applied.options.pre })),
                slot("sccp@792", Box::new(sccp::Sccp)),
                slot("bdce@797", Box::new(bdce::Bdce)),
                combine("combine@801"),
                fold("fold@801"),
                dead("dead@801"),
                // DFAJumpThreadingPass, PassBuilderPipelines.cpp#L806.
                slot(
                    "dfajumpthreading@806",
                    Box::new(
                        crate::dfajumpthreading::DfaJumpThreading::from_env().for_size(applied.options.prefers_size()),
                    ),
                ),
                slot("jumpthreading@809", Box::new(crate::jumpthreading::JumpThreading::from_env())),
                decide("decide@809"),
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
                // convertSwitchRangeToICmp and convertSwitchToArithmetic
                // (read by `simplifySwitch`, untranslated).
                simplify_cfg_with(
                    "decide@832",
                    simplifycfg::Options { hoist_common_insts: true, sink_common_insts: true, ..Default::default() },
                ),
                phiopt("phiopt@832"),
                combine("combine@837"),
                fold("fold@837"),
                dead("dead@837"),
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
            // closes the function stages, before `ipa-range`.
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
    manager.without_loop_intervals = !applied.options.loop_intervals;
    manager.verify_each = llrm_support::debug::verifying();
    manager.dump = applied.dump.clone();
    manager.dump_inputs = llrm_support::env_flag!("LLRM_DUMP_INPUTS");
    if !applied.options.optimize {
        manager.add_program(availableexternally::EliminateAvailableExternally);
        return manager.run(program);
    }
    // As LLVM's O2 requires GlobalsAA before the function pipeline.
    manager.require::<GlobalsAA>();
    manager.require::<Summaries>();
    manager.require::<Callbacks>();
    manager.require_program::<ProgramSummaries>();
    // Before any summary is made: the module names what a loop's copy is told
    // its pointers are by.
    if applied.options.version {
        manager.add_module(loopversion::DeclareRestrict);
    }
    // As LLVM's O2 runs GlobalOpt before the function pipeline.
    manager.add_module(globalopt::GlobalOpt);
    // Over every body first, so a caller's pipeline sees each callee's
    // port calls narrowed.
    if applied.options.wanted("ports") {
        manager.add(ports::Ports);
    }
    manager.add_program(crate::interprocedural::Stamp);
    // LLVM keeps GlobalsAA past every function pass
    // (`GlobalsAAResult::invalidate`, GlobalsModRef.cpp:825, d1106deb71cc):
    // made once, not again after each pass that changed a body. Only the
    // order has it so; the rounds keep theirs.
    if applied.options.llvm_order {
        manager.hold::<GlobalsAA>();
    }
    let walked = |dump: &str| Applied { dump: applied.dump.as_ref().map(|one| one.join(dump)), ..applied.clone() };
    for (at, stage) in function_stages(applied).into_iter().enumerate() {
        // The walk simplifies each body: not a module-wide stage.
        if at != 2 || !applied.options.llvm_order {
            manager.add(stage);
        }
        // LLVM's CGSCC walk: each body inlined into, then simplified, callees
        // first. `Phase::Simplify` runs there and not module-wide.
        if at == 1 && applied.options.llvm_order {
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
            manager.hold::<GlobalsAA>();
        }
        // LLVM's IPSCCP and DeadArgumentElimination run after the early
        // function pipeline, before the Simplify list.
        if at == 0 && applied.options.llvm_order {
            manager.add_program(crate::walk::ModuleIpo {
                inline: applied.options.inline,
                ranges: applied.options.ipa_ranges,
            });
            // A program pass abandons it.
            manager.hold::<GlobalsAA>();
        }
    }
    // Once every body has reached its own fixed point, as the old Nib
    // driver's whole-module step: a body it changes goes back through.
    // DEVIATE from LLVM: `ipa-range` runs after the function pipeline here,
    // LLVM's `ipsccp` before it, so a body it changes runs the Simplify list a
    // second time (d_faces -O2: one body, `gvn` 12 runs over 11 functions
    // against clang's 11, `parity::RUNS_PER_BODY`).
    let again_applied =
        Applied { dump: applied.dump.as_ref().map(|one| one.join("interprocedural")), ..applied.clone() };
    let mut again = if applied.options.llvm_order {
        Fixed::linear(&again_applied, Phase::Simplify)
    } else {
        Fixed::new(&again_applied)
    };
    manager.add_program(Interprocedural {
        pipeline: Box::new(move |module, analyses, id, stage| {
            TRIGGER.with(|trigger| *trigger.borrow_mut() = stage.to_owned());
            let done =
                rerun(module, analyses, id, &mut again, false).unwrap_or_else(|error| panic!("pipeline: {error}"));
            TRIGGER.with(|trigger| trigger.borrow_mut().clear());
            done
        }),
        specialise: {
            let mut alone = Fixed::new(&Applied { dump: None, ..applied.clone() });
            Box::new(move |module, analyses, mut function| {
                let layout = analyses.program().layout.clone();
                let outer = analyses.outer_held(module);
                let mut declared = Declared::over(std::rc::Rc::clone(&outer.globals), module.metadata.len());
                let Module { context, metadata, .. } = &mut *module;
                // A specialised copy has no global of its own yet.
                llrm_mir::passes::set_running_function("(specialised copy)");
                let mut unit = Unit {
                    context,
                    layout: &layout,
                    function: &mut function,
                    id: None,
                    metadata,
                    declared: &mut declared,
                };
                alone.run(&mut unit, &mut Analyses::new(std::rc::Rc::clone(&outer)));
                (function, declared)
            })
        },
        proved: None,
        inline: applied.options.inline,
        ranges: applied.options.ipa_ranges,
        rate: Some(if applied.options.prefers_size() { 0 } else { applied.options.limits.milliclocks_per_byte }),
    });
    // A global no body loads from any more (the loads were forwarded) needs
    // none of its stores, as LLVM's GlobalOpt and gcc's ipa find a variable
    // only written; the dead-code pass then finds nothing names it.
    manager.add_module(globalopt::WriteOnly);
    // What no live code names any more goes before selection, as LLVM runs
    // GlobalDCE after inlining.
    manager.add_program(globaldce::GlobalDce);
    manager.add_program(availableexternally::EliminateAvailableExternally);
    // Once the callers that remain are the ones that stay: an internal function
    // they all call directly pops its own arguments.
    if applied.options.wanted("calleepop") {
        manager.add_module(calleepop::CalleePop { size: applied.options.prefers_size() });
    }
    // The summaries are made. The passes below move and rewrite what a function
    // does and add nothing to it, so they read these and do not make them
    // again after each edit, as gcc's passes after IPA read the modref
    // summaries it made there.
    manager.freeze::<Summaries>();
    manager.freeze::<GlobalsAA>();
    // gcc's `pass_ch` (passes.def:232, in `pass_all_optimizations`) runs after
    // `pass_ipa_inline`: the inliner sizes a body before its header is copied.
    // The loops it guards are re-simplified for the passes that follow. Not at
    // -Os (`optimize_loop_for_size_p`): there only a loop proven to run is
    // entered at its body, which the last `Rotate` does.
    let guarded = applied.options.copy_headers && !applied.options.prefers_size();
    for pass in header_copied(applied) {
        manager.add_boxed(pass);
    }
    // Before LSR: a factor of two or a scale the product carries still shows as
    // a shift.
    if applied.options.wanted("fixednarrow") {
        manager.add(fixednarrow::FixedNarrow);
    }
    // Each loop's counters chosen once, on the loop the passes above leave.
    if applied.options.wanted("lsr") {
        manager.add(crate::lsr_pass::LoopStrengthReduce);
        // What the counters it chose leave behind (a bound subtracted from a
        // counter rebased by it), as LLVM's LSR cleans with
        // SimplifyInstructions.
        manager.add(algebraic::Differences);
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
    if applied.options.wanted("addresssink") {
        manager.add(addresssink::AddressSink);
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
    if applied.options.wanted("switchlower") {
        manager.add(switchlower::SwitchLower {
            convert: false,
            jump_tables: applied.options.jump_tables,
            size: applied.options.prefers_size(),
        });
    }
    // Last: what it names are the instructions selection sees.
    manager.add_module(crate::spares::Spares);
    manager.add_module(crate::homes::Homes);
    let done = manager.run(program);
    if llrm_support::env_flag!("LLRM_CHECK_CAPTURES_MODULE") {
        for module in &program.modules {
            llrm_analysis::capturecheck::check_module(module, &program.layout);
        }
    }
    done
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
    if !(applied.options.copy_headers && !applied.options.prefers_size()) {
        return passes;
    }
    passes.push(Box::new(rotate::Rotate { proven: false, copy: true }));
    passes.push(Box::new(loopsimplify::LoopSimplify));
    passes.push(Box::new(lcssa::LoopClosedSSA));
    passes.push(Box::new(fold::Fold));
    passes.push(Box::new(algebraic::Algebraic { size: false, llvm: false }));
    // A counted exit's values are numbers where the copy made its test the one
    // that leaves.
    passes.push(Box::new(indvars::IndVars { lftr: indvars::lftr_default() }));
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
    unrollers: Vec<Box<dyn FunctionPass>>,
    peelers: Vec<Box<dyn FunctionPass>>,
    /// Once, after everything else.
    last: Vec<Box<dyn FunctionPass>>,
    unswitch: Option<simpleloopunswitch::SimpleLoopUnswitch>,
    split: Option<loopsplit::Split>,
    distribute: Option<loopdistribute::Distribute>,
    version: Option<loopversion::Version>,
    fuse: Option<loopfuse::Fuse>,
    interchange: Option<loopinterchange::Interchange>,
    jam: Option<unroll_jam::UnrollJam>,
    only: bool,
    rounds: usize,
    dump: Option<PathBuf>,
    /// Bodies run, naming each one's dump.
    runs: usize,
}

/// `options` less the passes that try a rewrite on a copy and judge it: what
/// a candidate goes through before it is judged.
fn without_candidates(options: &Options) -> Options {
    Options {
        unswitch: false,
        split: false,
        fuse: false,
        distribute: false,
        version: false,
        unroll_jam: false,
        interchange: false,
        ..options.clone()
    }
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
        let (unrollers, passes): (Vec<_>, Vec<_>) = passes.into_iter().partition(|one| one.name() == "unroll");
        let (peelers, passes): (Vec<_>, Vec<_>) = passes.into_iter().partition(|one| one.name() == "peel");
        let (last, passes): (Vec<_>, Vec<_>) =
            passes.into_iter().partition(|one| matches!(one.name(), "merge" | "memcpyopt"));
        // A candidate is judged after the whole pipeline, with the passes that
        // make candidates off.
        let judged = || {
            Box::new(Fixed::new(&Applied {
                options: without_candidates(&applied.options),
                only: None,
                dump: None,
                ..applied.clone()
            }))
        };
        let unswitch = applied.options.unswitch.then_some(simpleloopunswitch::SimpleLoopUnswitch { non_trivial: true });
        let distribute = applied.options.distribute.then(|| loopdistribute::Distribute { passes: vec![judged()] });
        // A loop the copy changes the shape of is judged as it will be run:
        // its header copied and what it loads unchanged moved out.
        let version = applied
            .options
            .version
            .then(
                || loopversion::Version {
                    passes: std::iter::once(judged() as Box<dyn FunctionPass>).chain(header_copied(applied)).collect(),
                },
            );
        let fuse = applied.options.fuse.then(|| loopfuse::Fuse { passes: vec![judged()] });
        Self {
            boundary,
            passes,
            unrollers,
            peelers,
            last,
            unswitch,
            split: (applied.options.split && only.is_none()).then_some(loopsplit::Split),
            distribute: distribute.filter(|_| only.is_none()),
            version: version.filter(|_| only.is_none()),
            fuse: fuse.filter(|_| only.is_none()),
            interchange: (applied.options.interchange && only.is_none()).then_some(loopinterchange::Interchange),
            jam: (applied.options.unroll_jam && only.is_none()).then(unroll_jam::UnrollJam::default),
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
        // A slot is the one pass: -O3's peeler ran in its place.
        one.peelers.clear();
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
        linear.unrollers.clear();
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
            applied.options.rotating(),
            &off,
            &held,
        );
        debug_assert!(checked.is_ok(), "{checked:?}");
        linear.rounds = 1;
        // The slot of the order unswitches, where the pin does.
        linear.unswitch = None;
        if !matches!(phase, Phase::Simplify) {
            linear.peelers.clear();
            linear.split = None;
            linear.distribute = None;
            linear.version = None;
            linear.fuse = None;
            linear.interchange = None;
            linear.jam = None;
        }
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
        if self.only && !self.unrollers.is_empty() {
            run.step(&mut *self.unrollers[0], "r01-unroll", unit, analyses);
            return Ok(run.settled(unit, analyses));
        }
        if self.only && !self.peelers.is_empty() {
            run.step(&mut *self.peelers[0], "r01-peel", unit, analyses);
            return Ok(run.settled(unit, analyses));
        }
        self.fixed(unit, analyses, run, "")?;
        // Again while a peel leaves fewer loops, as LLVM's loop pass manager
        // revisits a parent once its child is gone: a loop whose inner loop
        // was peeled may then be peeled itself.
        let loops = |unit: &Unit| cfg::Shape::of(unit.function).loops.len();
        let mut before = loops(unit);
        while !self.peelers.is_empty() && run.step(&mut *self.peelers[0], "peel", unit, analyses) {
            self.scalarized(unit, analyses, run, "peeled");
            self.fixed(unit, analyses, run, "peeled-")?;
            let after = loops(unit);
            if after >= before {
                break;
            }
            before = after;
        }
        if let Some(unswitch) = &mut self.unswitch {
            run.step(unswitch, "unswitch", unit, analyses);
        }
        // After unswitching, before jamming, as gcc's pass order has it. Each
        // half is cleaned as a peeled loop is.
        if let Some(split) = &mut self.split {
            if run.step(split, "split", unit, analyses) {
                self.scalarized(unit, analyses, run, "split");
                self.fixed(unit, analyses, run, "split-")?;
            }
        }
        // Loops that follow one another and count alike, once unswitching and
        // splitting have made what they are. A fused loop was cleaned by the
        // pipeline that judged it.
        if let Some(fuse) = &mut self.fuse {
            run.step(fuse, "fuse", unit, analyses);
        }
        // After fusing: what is joined is no longer a loop to part, and a
        // part is not the loops a fusion was after. The pipeline that judged a
        // split has cleaned it.
        if let Some(distribute) = &mut self.distribute {
            run.step(distribute, "distribute", unit, analyses);
        }
        // A loop that is a stride from a fill or a unit step, once what runs
        // before has parted and joined what it will.
        if let Some(version) = &mut self.version {
            run.step(version, "version", unit, analyses);
        }
        // Before jamming: the swapped nest is the one that is jammed. Cleaned
        // as a peeled loop is.
        if let Some(interchange) = &mut self.interchange {
            if run.step(interchange, "interchange", unit, analyses) {
                self.scalarized(unit, analyses, run, "interchanged");
                self.fixed(unit, analyses, run, "interchanged-")?;
            }
        }
        // After unswitching, as gcc's pass order has it: an invariant guard is
        // out of the nest by then. The fused copies are cleaned as a peeled
        // loop is.
        if let Some(jam) = &mut self.jam {
            if run.step(jam, "unroll-jam", unit, analyses) {
                self.scalarized(unit, analyses, run, "jammed");
                self.fixed(unit, analyses, run, "jammed-")?;
            }
        }
        for one in &mut self.last {
            let stage = one.name();
            run.step(&mut **one, stage, unit, analyses);
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
            let stage = format!("{stage}-{}", one.name());
            run.step(&mut **one, &stage, unit, analyses);
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
        let mut unroll_settled = None;
        // Separately reject a repeated state, so an oscillator fails at once
        // instead of consuming the limit.
        // Only a list that rounds more than once can come back to a state, so
        // only that one hashes the body (LLVM's order runs its lists once).
        let mut history = (self.rounds > 1).then(|| BTreeSet::from([print::fingerprint(unit.function)]));

        run.fixed += 1;
        // The last round unrolled a loop: the straight line it made is cleaned
        // by the round after the budget, as gcc's cleanup follows its complete
        // unroll.
        let mut unrolled = false;
        for iteration in 0..limit {
            if iteration >= self.rounds && !unrolled {
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
                let name = format!("{prefix}r{:02}-{}", iteration + 1, one.name());
                *settled = (!run.step(&mut **one, &name, unit, analyses)).then(|| (run.version, run.stretches.len()));
            }
            // Ask at the original pipeline boundary: fully converging the
            // scalar passes first destroys matmul's exact counted-loop shape.
            unrolled = false;
            if !self.unrollers.is_empty() && unroll_settled != Some(run.version) {
                let name = format!("{prefix}r{:02}-unroll", iteration + 1);
                if run.step(&mut *self.unrollers[0], &name, unit, analyses) {
                    unrolled = true;
                    // A copy's constant indices are new exact leaves, so it
                    // crosses the structural boundary before the scalar
                    // passes settle it.
                    self.scalarized(unit, analyses, run, &format!("{prefix}unrolled"));
                } else {
                    unroll_settled = Some(run.version);
                }
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

    fn name(&self) -> &'static str {
        "pipeline"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        self.runs += 1;
        let promotes = analyses
            .get::<cfg::Shape>(unit.context, unit.layout, unit.function)
            .dominance
            .irreducible(unit.function)
            .is_empty();
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
    idle_at: Vec<(&'static str, llrm_mir::module::Mark, u8, llrm_mir::footprint::Footprint)>,
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

    /// The passes that found nothing and that no edit since reaches are
    /// recorded as settled on the body as it is: a later run starts none of
    /// them where what they read is as it was (`settled`).
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
        let mut classified: Vec<(usize, llrm_mir::footprint::Footprint)> = Vec::new();
        for &(name, mark, bits, reads) in &self.idle_at {
            let reached = match classified.iter().find(|(at, _)| *at == mark.position()) {
                Some(&(_, found)) => found,
                None => {
                    let found =
                        unit.function.changes_since(mark).map_or(llrm_mir::footprint::Footprint::ALL, |changes| {
                            llrm_mir::footprint::of(unit.function, changes)
                        });
                    classified.push((mark.position(), found));
                    found
                }
            };
            if !reached.meets(reads) {
                self.records.note(name, unit.function, slice, bits);
            }
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
        stage: &str,
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
            self.idle_at.push((name, unit.function.mark(), read, pass.reads()));
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
            self.changed("unreachable", unit, analyses);
        }
    }

    fn changed(
        &mut self,
        stage: &str,
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
            name: Some("pipeline.body".to_owned()),
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
        llrm_support::env_set("LLRM_CHECK_ORDER")
            .then(|| std::env::var("LLRM_CHECK_ORDER").ok().and_then(|value| value.parse().ok()))
            .flatten()
    })
}
