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
//!   e["<b>E  once, each body, with the</b><br/><small>analyses frozen<br/>[rotate#123;copy#125; loopsimplify<br/>lcssa fold algebraic<br/>indvars#123;exit#125; dead]: only<br/>with -ftree-ch, not at -Os<br/>fixednarrow lsr differences<br/>maskscale window rotate#123;proven#125;<br/>hoist gepoffset<br/>cgpaddr sink</small>"]:::mid
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
//! and no one function shows it. `llrm-cc --dump DIR` writes the module after
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
//! | `strength` | `indvars`, `lsr`, `canonfreeze`, `window`, `gepoffset`, `typepromotion`, `cgpaddr` |
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

use std::path::PathBuf;

use llrm_analysis::cfg;
use llrm_analysis::manager::{GlobalsAA, GlobalsModRefAnalysis, ProgramSummaries, Summaries};
use llrm_analysis::peelsize::Limits;
use llrm_mir::context::GlobalId;
use llrm_mir::module::{GlobalKind, GlobalValue, Linkage, Module, UnnamedAddr};
use llrm_mir::opcode::Attribute;
use llrm_mir::passes::{Analyses, Declared, FunctionPass, ModuleAnalyses, PassManager, PreservedAnalyses, Stage, Unit};
use llrm_mir::program::Program;

use crate::interprocedural::Interprocedural;
use crate::parity::Phase;
use crate::settled::{self, Settled};
use crate::{
    adce, algebraic, availableexternally, bdce, calleepop, cgpaddr, constmerge, dead, deadstore, decide, extload,
    fixednarrow, floatloop, fold, gepbyteindex, gepoffset, globaldce, globalopt, gvn, indvars, inferfunctionattrs, inferspace,
    inline, lcssa, loopdeletion, loopidioms, loopinterchange, looprotate, loopsimplify, loopunrollandjam,
    loopunrollpass, mem2reg, memcpyopt, mldstmotion, partialinline, ports, promote, sccp, simpleloopunswitch,
    simplifycfg, switchlower, tailcallelim, trivialunswitch, typepromotion_pass, window, zeromerge,
};

/// Which passes run, and the copy budgets: the old `Options`. The default
/// is -O2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    /// The library routines the compile states the program links (the pin's
    /// `TargetLibraryInfo`, which it reads from the triple: here the front
    /// end's profile states them, as the ABI chosen by `-mabi` spells and
    /// calls them). DEVIATE D11: llrm-cc states `strlen` for `-m32` under the
    /// System V convention only, llrm-qb and llrm-nib none, so no other
    /// compile forms a call of a library routine. Exit: a `Target::library`
    /// of each target that states its routines. Owner w-6, exit 2026-11-30.
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
    /// The pin's `JumpThreadingPass` slots (`jumpthreading@668`, `@809`) run
    /// where `pre` is off too (-O1): its -O2 pipeline runs them,
    /// `buildO1FunctionSimplificationPipeline` does not. gcc's
    /// `-fthread-jumps`.
    ///
    /// DEVIATE JUMPTHREAD-O1 (owner m68k-lsr-addressing, exit 2026-12-04, or
    /// when the pin's -O1 gains it): a target's driver row may turn it on at
    /// -O1 (`thread_jumps_at_o1` in llrm-cc's targets.toml); no row does but
    /// m68k's. clang's -O1 IR leaves a latch `br` on a compare one block up
    /// (bsearch -O1: a re-test and a loop-carried copy of the result).
    pub thread_jumps: bool,
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
    /// `loopidioms::Merge` also makes adjacent stores wider ones (LLVM's
    /// `DAGCombiner::mergeConsecutiveStores`, DEVIATE `parity.rs`, owner wasm,
    /// exit 2026-11-20); off for a target whose backend merges them, so the
    /// inliner and the passes after it price the stores the pin's IR has.
    pub merge_stores: bool,
    /// After `PromoteMemToReg` the write-through `Promote` forwards a global's
    /// store to the loads after it, across blocks with phis, where there is no
    /// GVN `pre` (-O1, -Og). The pin's -O1 has no GVN and does not; off for a
    /// target compared with it (wasm: jpeg_fixed -O1 was 259 B over).
    pub write_through: bool,
    pub drop_stores: bool,
    /// Work read below a branch moves below it: gcc's `-ftree-sink`.
    pub sink: bool,
    /// `Spares`: a fixed-cell load rematerialized past the writes alias proves
    /// disjoint. llrm only (the pin rematerializes invariant loads alone); off
    /// by default, `-fllrm-spares`. DEVIATE (no number; owner w2, exit
    /// 2026-11-20: delete or justify with numbers), 8.0 M Ir at d_faces.
    pub spare_loads: bool,
    /// A CFG simplification ahead of loop strength reduction, to tidy the
    /// compare-and-branch that follows a `cmpxchg`: the pin's
    /// `AArch64PassConfig` adds it (`EnableAtomicTidy`,
    /// AArch64TargetMachine.cpp#L666, d1106deb71cc), X86's does not. Set by
    /// the target's driver.
    pub atomic_cfg_tidy: bool,
    /// `Homes`: a phi that also sits in a stored cell is spilled to that cell
    /// (gcc `REG_EQUIV`). llrm only; off by default, `-fllrm-homes`. DEVIATE
    /// (no number; owner w2, exit 2026-11-20: delete or justify with numbers),
    /// 8.4 M Ir at d_faces.
    pub stored_homes: bool,
    /// The spiller makes a load of a cell nothing writes again where it is read
    /// (D-I). llrm only; off by default, `-fllrm-stable-loads`. Read by the
    /// backend through the CPU profile.
    pub stable_loads: bool,
    pub promote: bool,
    pub strength: bool,
    /// `cgpaddr`, the address-mode half of CodeGenPrepare. Off lets a pin
    /// `llc`, whose own CodeGenPrepare runs inside it, take the module
    /// before this one (`-fllrm-cgpaddr`).
    pub cgpaddr: bool,
    /// `extload`, DAGCombiner's `ExtendUsesToFormExtLoad` and
    /// `ExtendSetCCUses`: the compares of a loaded value compare its
    /// extension. The pin forms the extending load only where the target
    /// has it (`isLoadExtLegal`); off for a target that has not, where the
    /// wider compare costs more than the extension saved.
    pub extload: bool,
    /// `zeromerge`: zeroed locals that follow each other in the entry block
    /// share one fill. llrm only (DEVIATE D1 of `zeromerge.rs`, owner m16, exit
    /// 2026-11-20); runs where a pointer is 16 bits. `-fllrm-zero-merge`.
    pub zero_merge: bool,
    /// `TypePromotion`, which the pin runs before CodeGenPrepare where the
    /// target asks (`-fllrm-typepromotion`).
    pub typepromotion: bool,
    /// The translated `LoopStrengthReduce` slot (LLVM's `-disable-lsr` turns it
    /// off); `strength` turns it off with the other address passes.
    pub lsr: bool,
    pub unroll: bool,
    /// A second CGSCC walk after the dead globals and loops are gone: the first
    /// walk prices a callee before the folds that empty it (a static callee
    /// whose last caller folds away). llrm only; off by default, the 6502 turns
    /// it on. DEVIATE 6502-SZ1, owner 6502, exit 2026-11-20.
    pub second_inline: bool,
    /// clang's `PTO.LoopUnrolling` (`UnrollLoops`, on above -O1,
    /// [CompilerInvocation.cpp#L1936](https://github.com/llvm/llvm-project/blob/d1106deb71cc/clang/lib/Frontend/CompilerInvocation.cpp#L1936)):
    /// off, the full unroll of the LLVM order only takes a loop that asks
    /// (`OnlyWhenForced`). -Os keeps it on: the unroller's own size threshold
    /// of 0 stops the copies.
    pub loop_unrolling: bool,
    pub loopidioms: bool,
    pub sibcalls: bool,
    pub unswitch: bool,
    /// `-floop-interchange`: LLVM's `LoopInterchange.cpp`, in the pin's
    /// default pipelines (`enable-loopinterchange`) at every level but -O0.
    pub interchange: bool,
    /// `-floop-unroll-and-jam`: LLVM's `LoopUnrollAndJamPass`, in the Late
    /// stage (L1474). DEVIATE (owner m64, exit 2026-11-30): the pin's default
    /// pipelines have it off (`enable-unroll-and-jam`); it is on at -O3 and
    /// -Omax, where the particle row gains from it.
    pub unroll_and_jam: bool,
    /// LPM1's `loop-rotate` between the two LICM runs
    /// (PassBuilderPipelines.cpp L728), the first LICM not speculating; on, as
    /// in the pin.
    pub rotate_slot: bool,
    /// The round's `Promote` is the translated `PromoteMemToReg`
    /// ([`crate::mem2reg`]); on by default, `LLRM_NO_MEM2REG` turns it off.
    pub mem2reg: bool,
    /// The SROA slots run the translated unit (`SROA.cpp`,
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
    /// The `mergeicmps` slot (the pin's `-enable-mergeicmps`, on by default);
    /// `LLRM_NO_MERGEICMPS` turns it off.
    pub merge_icmps: bool,
    /// Loops not proven to run are entered behind a copy of their test, before
    /// the loop passes (gcc's `-ftree-ch`; not at -Os).
    pub copy_headers: bool,
    /// Code size outranks speed where they conflict: -Os and -Oz.
    pub for_size: bool,
    /// The target's price of a code byte at speed counts
    /// (`Tti::speed_milliclocks_per_byte`): -O2 and -O3. -O1 and -Os gate
    /// every metric by itself.
    pub speed_rate: bool,
    /// `Function::hasMinSize`: -Oz. The only level where `LoopRotate` copies no
    /// header
    /// ([LoopRotation.cpp#L80](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LoopRotation.cpp#L80)).
    pub has_min_size: bool,
    /// The allocator tries other shapes of a body and keeps the cheapest
    /// (`-fallocation-search`; -Omax: gcc and LLVM allocate once).
    pub search: bool,
    /// With `search`, every shape rather than the one the spills suggest
    /// (`-fallocation-search-all`; -Omax).
    pub exhaustive: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            library: Vec::new(),
            optimize: true,
            rotate_slot: true,
            limits: Limits { target_percent: 100, ..Limits::default() },
            inline: inline::Threshold::default(),
            lcssa: true,
            floatloop: true,
            fold: true,
            simplifycfg: true,
            decide: true,
            dead: true,
            hoist: true,
            forward: true,
            drop_loads: true,
            pre: true,
            thread_jumps: false,
            drop_stores: true,
            sink: true,
            spare_loads: false,
            atomic_cfg_tidy: false,
            stored_homes: false,
            stable_loads: false,
            promote: true,
            strength: true,
            cgpaddr: true,
            extload: true,
            zero_merge: true,
            typepromotion: true,
            lsr: true,
            unroll: true,
            second_inline: false,
            loop_unrolling: true,
            loopidioms: true,
            sibcalls: true,
            unswitch: false,
            interchange: true,
            unroll_and_jam: false,
            mem2reg: experiment_mem2reg(),
            sroa: !llrm_support::env_flag!("LLRM_NO_SROA"),
            combine: !llrm_support::env_flag!("LLRM_NO_COMBINE"),
            licm: !llrm_support::env_flag!("LLRM_NO_LICM"),
            merge_icmps: !llrm_support::env_flag!("LLRM_NO_MERGEICMPS"),
            copy_headers: true,
            for_size: false,
            speed_rate: true,
            has_min_size: false,
            switch_conversion: true,
            jump_tables: true,
            lower_switches: true,
            merge_stores: true,
            write_through: true,
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
    /// no sibling calls or unswitching. Pattern fill is
    /// on (`LoopIdiomRecognize` is in LLVM's O1 pipeline,
    /// PassBuilderPipelines.cpp L562), the user's decision of 2026-10-10
    /// (docs/reference/levels.md).
    pub fn basic() -> Self {
        Self {
            inline: inline::Threshold::default(),
            // gcc runs each pass once: the third round changed five passes of
            // d_faces and the fourth none, 253 of 3,084 Minstr at -O1.
            pre: false,
            thread_jumps: false,
            loop_unrolling: false,
            unswitch: false,
            switch_conversion: false,
            speed_rate: false,
            ..Self::default()
        }
    }

    /// -O2: gcc's: -O1 with inlining, gcse, sibling calls and pattern fill.
    /// Same as [`Options::default`]: whether a complete copy may grow the code
    /// is the unroller's own size threshold, not an option here.
    pub fn standard() -> Self {
        Self::default()
    }

    /// -O3: gcc's: -O2 with unswitching and the larger inline threshold; the
    /// copy budget is LLVM's -O3 (`UnrollThresholdAggressive`, twice the
    /// target's), not gcc's. No ipa-cp clones by default: the pin makes
    /// none. A target that states `Tti::ipa_cp_clones_at_speed` takes gcc's
    /// `-fipa-cp-clone` here (DEVIATE IPACP-O3, resolved in `recorded`).
    pub fn speed() -> Self {
        Self {
            inline: inline::Threshold::new(250),
            unswitch: true,
            unroll_and_jam: true,
            limits: Limits { target_percent: 200, ..Limits::default() },
            ..Self::default()
        }
    }

    /// -Omax: every pass the default has on, LLVM's -O3 budgets and a 250
    /// inline threshold.
    pub fn aggressive() -> Self {
        Self {
            limits: Limits { target_percent: 200, ..Limits::default() },
            inline: inline::Threshold { cp_clone: Some(true), ..inline::Threshold::new(250) },
            unswitch: true,
            unroll_and_jam: true,
            search: true,
            exhaustive: true,
            ..Self::default()
        }
    }

    /// -Os: no copy grows the code. The limit stays -O2's 225, but every
    /// caller is `optsize` (D8), so a call is held to 50, 487 with an
    /// `inlinehint`, as the pin holds it (audit row 3: mirrored).
    pub fn size() -> Self {
        Self {
            limits: Limits { target_percent: 100, ..Limits::default() },
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

    /// Whether this is -O1: optimizing, with the unroller's -O1 level
    /// (`unroll_level`) and size not preferred.
    pub fn at_o1(&self) -> bool {
        self.optimize && !self.for_size && unroll_level(self) == 1
    }

    /// Whether the target's byte price at speed counts: -O2 and -O3.
    pub fn prices_bytes(&self) -> bool {
        self.speed_rate && !self.for_size
    }

    /// Whether the code generator may grow code for speed: -O3 and -Omax
    /// (`CodeGenOptLevel::Aggressive`).
    pub fn aggressive_codegen(&self) -> bool {
        self.limits.target_percent >= 200
    }

    /// -Oz: no loop is copied.
    pub fn min_size() -> Self {
        Self { unroll: false, has_min_size: true, inline: inline::Threshold::default().for_min_size(), ..Self::size() }
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
            "lsr" | "canonfreeze" | "maskscale" | "looptermfold" | "hardwareloops" | "constanthoisting" => {
                self.strength && self.lsr
            }
            "indvars" | "window" | "gepoffset" | "gepbyteindex" | "narrowrange" => self.strength,
            "cgpaddr" | "cgpselectarms" => self.strength && self.cgpaddr,
            "extload" => self.extload,
            "zeromerge" => self.zero_merge,
            "typepromotion" => self.strength && self.typepromotion,
            "unroll" => self.unroll,
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
        Box::new(loopidioms::Merge { size: applied.options.prefers_size(), widen: applied.options.merge_stores }),
        Box::new(memcpyopt::MemCpyOpt),
    ];
    every.into_iter().filter(|one| applied.options.wanted(one.name())).collect()
}

/// Under LLVM's order the translated LICM does the moving of code out of and
/// into loops, in its three slots: the passes after the fixed point that
/// stood in for MachineLICM and gcc's `pass_sink_code` (`hoist` again) have no
/// counterpart there, and run only without the order or the LICM.
pub fn llvm_motion(applied: &Applied) -> bool {
    applied.options.licm
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

    fn is_required(&self) -> bool {
        self.inner.is_required()
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let count = |unit: &Unit| -> i64 {
            unit.function.layout().iter().map(|&b| unit.function.iter_block(b).count() as i64).sum()
        };
        if !llrm_support::env_flag!("W2_SLOT_COUNT") {
            return self.inner.run(unit, analyses);
        }
        let before = count(unit);
        let result = self.inner.run(unit, analyses);
        eprintln!("W2SLOT {} {} {}", self.name, before, count(unit) - before);
        result
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
        let mut cfg = true;
        for part in &mut self.parts {
            let preserved = llrm_mir::passes::run_checked(part.as_mut(), unit, analyses);
            if !preserved.are_all_preserved() {
                changed = true;
                cfg &= preserved.cfg_preserved();
                analyses.invalidate(unit.function, &preserved);
            }
        }
        match (changed, cfg) {
            (false, _) => PreservedAnalyses::all(),
            (true, true) => PreservedAnalyses::none().preserve_cfg(),
            (true, false) => PreservedAnalyses::none(),
        }
    }
}

/// What a [`PerLoop`] left behind: the function as the group ended, so that the
/// slots after the head can tell their work is done.
type Fused = std::rc::Rc<std::cell::Cell<Option<llrm_mir::module::Mark>>>;

/// LPM2's passes (indvars, loop-deletion, loop-unroll-full, indvars on the
/// loops the unroll made) as the loop pass manager runs them: loop by loop,
/// each loop through every pass before the next loop is looked at, so an
/// unrolled loop is gone, and an earlier loop's exit test already rewritten,
/// when the next loop's `indvars` asks what dominates it. The slot is the
/// first of them (`indvars@738`) and runs all of `parts`, the loops being
/// those the function has when it starts, in the order the unroll visits
/// them. The slots after it, [`AfterGroup`], find the function as the group
/// left it and do nothing.
struct PerLoop {
    head: Box<dyn FunctionPass>,
    parts: Vec<Box<dyn FunctionPass>>,
    fused: Fused,
}

impl FunctionPass for PerLoop {
    fn name(&self) -> &'static str {
        self.head.name()
    }

    fn translations(&self) -> Vec<&'static str> {
        self.head.translations()
    }

    fn adds_memory_operations(&self) -> bool {
        self.head.adds_memory_operations() || self.parts.iter().any(|part| part.adds_memory_operations())
    }

    fn reads(&self) -> llrm_mir::footprint::Footprint {
        self.parts.iter().fold(self.head.reads(), |all, part| all.with(part.reads()))
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let headers: Vec<llrm_mir::module::BlockId> = analyses
            .get::<llrm_mir::passes::Loops>(unit.context, unit.layout, unit.function)
            .get_loops_in_reverse_sibling_preorder()
            .iter()
            .rev()
            .map(|one| one.get_header())
            .collect();
        let mut changed = false;
        for header in headers {
            let found = analyses.get::<llrm_mir::passes::Loops>(unit.context, unit.layout, unit.function);
            if !unit.function.is_placed(header) || !found.is_loop_header(header) {
                continue;
            }
            let _scope = crate::loopfilter::only(&[header]);
            for part in std::iter::once(&mut self.head).chain(self.parts.iter_mut()) {
                // Each part is its own step: charged to the head's slot, the
                // unroll's cost read as the indvars' (`LLRM_DEBUG=time`).
                let name = part.name();
                let preserved =
                    llrm_mir::passes::spanned(name, || llrm_mir::passes::run_checked(part.as_mut(), unit, analyses));
                if !preserved.are_all_preserved() {
                    changed = true;
                    analyses.invalidate(unit.function, &preserved);
                }
            }
        }
        self.fused.set(Some(unit.function.mark()));
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// The unswitch slot of LPM1 at -O3, where the pin's `SimpleLoopUnswitchPass`
/// is non-trivial. A non-trivial unswitch hands the loop pass manager the
/// sibling loops it cloned (`LPMUpdater::addSiblingLoops`) and asks for the
/// loop again (`revisitCurrentLoop`); the manager then runs the whole of LPM1
/// on each of them: instsimplify, simplifycfg, licm, rotate, licm, unswitch.
/// The first visit of each loop is the flat slots before this one (`preamble`
/// ran on every loop already), so only the revisits and the clones run the
/// preamble again, then the unswitch.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopPassManager.cpp#L270>
struct UnswitchRevisit {
    preamble: Vec<Box<dyn FunctionPass>>,
    unswitch: Box<dyn FunctionPass>,
}

impl FunctionPass for UnswitchRevisit {
    fn name(&self) -> &'static str {
        self.unswitch.name()
    }

    fn translations(&self) -> Vec<&'static str> {
        self.unswitch.translations()
    }

    fn adds_memory_operations(&self) -> bool {
        self.unswitch.adds_memory_operations() || self.preamble.iter().any(|part| part.adds_memory_operations())
    }

    fn reads(&self) -> llrm_mir::footprint::Footprint {
        self.preamble.iter().fold(self.unswitch.reads(), |all, part| all.with(part.reads()))
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let headers: Vec<llrm_mir::module::BlockId> = analyses
            .get::<llrm_mir::passes::Loops>(unit.context, unit.layout, unit.function)
            .get_loops_in_reverse_sibling_preorder()
            .iter()
            .rev()
            .map(|one| one.get_header())
            .collect();
        // (loop, whether LPM1's preamble has run on it since it was made or
        // revisited)
        let mut work: std::collections::VecDeque<(llrm_mir::module::BlockId, bool)> =
            headers.into_iter().map(|header| (header, true)).collect();
        let mut changed = false;
        let _ = crate::loopfilter::take_unswitched();
        while let Some((header, preamble_ran)) = work.pop_front() {
            let found = analyses.get::<llrm_mir::passes::Loops>(unit.context, unit.layout, unit.function);
            if !unit.function.is_placed(header) || !found.is_loop_header(header) {
                continue;
            }
            {
                let _scope = crate::loopfilter::only(&[header]);
                let preamble = if preamble_ran { &mut self.preamble[..0] } else { &mut self.preamble[..] };
                for part in preamble.iter_mut().chain(std::iter::once(&mut self.unswitch)) {
                    let preserved = llrm_mir::passes::run_checked(part.as_mut(), unit, analyses);
                    if !preserved.are_all_preserved() {
                        changed = true;
                        analyses.invalidate(unit.function, &preserved);
                    }
                }
            }
            // The loop itself is popped first, then its siblings in order.
            let (revisit, mut siblings) = crate::loopfilter::take_unswitched();
            siblings.reverse();
            for one in siblings {
                work.push_front((one, false));
            }
            if revisit {
                work.push_front((header, false));
            }
        }
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// A slot after the head of a [`PerLoop`]: run by the group already, it is
/// skipped while the function is as the group left it, and runs as itself
/// otherwise (alone, or after something else changed the function).
struct AfterGroup {
    inner: Box<dyn FunctionPass>,
    fused: Fused,
}

impl FunctionPass for AfterGroup {
    fn name(&self) -> &'static str {
        self.inner.name()
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
        if self.fused.get() == Some(unit.function.mark()) {
            return PreservedAnalyses::all();
        }
        self.inner.run(unit, analyses)
    }
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
                    Box::new(crate::combine_pass::Combine {
                        enabled: applied.options.combine,
                        phi_constant_users: applied.options.prices_bytes(),
                    }),
                    Box::new(crate::simplifylibcalls::SimplifyLibCalls::with_routines(applied.options.library.clone())),
                ],
            }),
        )
    };
    let dead = |name| slot(name, Box::new(dead::Dead));
    // SROA runs mem2reg on what it splits: one slot, nothing between
    // (SROA.cpp `promoteAllocas`).
    let translated = applied.options.sroa;
    // DEVIATE (owner wasm, exit 2026-11-20), -O1 and -Og only (no GVN `pre`),
    // and where `write_through` is on: the write-through `Promote`
    // after PromoteMemToReg forwards a store of a global to the load after it,
    // which LLVM's InstCombine does in `visitLoadInst`
    // (FindAvailableLoadedValue, not translated); GVN does it at the other
    // levels. Exit: that translation.
    let write_through = || -> Vec<Box<dyn FunctionPass>> {
        if applied.options.pre || !applied.options.write_through {
            Vec::new()
        } else {
            vec![Box::new(promote::Promote), Box::new(promote::DeadSlots)]
        }
    };
    // `SROAPass(ModifyCFG)` everywhere but the late slot, which the pin builds
    // with `PreserveCFG` (PassBuilderPipelines.cpp L1393, L1494).
    let sroa = |name: &'static str| -> Box<dyn FunctionPass> {
        let parts: Vec<Box<dyn FunctionPass>> = if translated {
            let pass: Box<dyn FunctionPass> =
                if name == "sroa@1494" { Box::new(crate::sroa::Sroa) } else { Box::new(crate::sroa::SroaModifyCfg) };
            let mut parts: Vec<Box<dyn FunctionPass>> = vec![pass, Box::new(crate::mem2reg::Mem2Reg)];
            parts.extend(write_through());
            parts
        } else {
            vec![Box::new(promote::Sroa), Box::new(promote::Promote), Box::new(promote::DeadSlots)]
        };
        Box::new(Together { name, parts })
    };
    let earlycse = |name, memory_ssa| slot(name, Box::new(crate::earlycse::EarlyCse { memory_ssa }));
    // `licm<allowspeculation>` or `licm<no-allowspeculation>`; under
    // `LLRM_NO_LICM` the slot stays and holds nothing. DEVIATE FMULADD-SPLIT:
    // each one splits the products it could hoist first, loops at -O1 having
    // no other slot where the factors stand outside.
    let licm = |name, speculate| -> Box<dyn FunctionPass> {
        if applied.options.licm {
            let split = Box::new(crate::fmuladdsplit::FmuladdSplit { size: applied.options.prefers_size() });
            Box::new(Together { name, parts: vec![split, Box::new(crate::licm::Licm { apply: true, speculate })] })
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
                // `SimplifyCFGPass()` has `ConvertSwitchRangeToICmp` off.
                slot(
                    "decide@1178",
                    Box::new(simplifycfg::SimplifyCfg {
                        options: simplifycfg::Options {
                            loop_form_kept: false,
                            convert_switch_range_to_icmp: false,
                            ..Default::default()
                        },
                    }),
                ),
                // DEVIATE VASTART-PROMOTE (owner x86-m32, exit 2026-11-27): a
                // one-pointer `va_list` is a slot SROA can promote.
                slot("vastart@1178", Box::new(crate::vastart::VaStart)),
                sroa("sroa@1179"),
                earlycse("earlycse@1180", false),
            ]
        }
        Phase::Cleanup => vec![
            Box::new(Together {
                name: "promote@1241",
                // DEVIATE as in the SROA slot (owner w2, exit 2026-11-20).
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
            // DropUnnecessaryAssumesPass (l.1577), the first of `OptimizePM`.
            slot(
                "drop-unnecessary-assumes@1577",
                Box::new(crate::dropunnecessaryassumes::DropUnnecessaryAssumes { drop_dereferenceable: false }),
            ),
            slot("float2int@1594", Box::new(crate::float2int::Float2Int)),
            // `LowerConstantIntrinsicsPass` (l.1597), right after Float2Int.
            slot("lower-constant-intrinsics@1597", Box::new(crate::lowerconstantintrinsics::LowerConstantIntrinsics)),
            // LoopRotatePass (l.1621): the loop pass manager that holds
            // LoopInterchange runs before `addVectorPasses` and its unroll
            // (l.1476), so a loop the unroll leaves with its exit in the
            // middle is not rotated after it.
            slot(
                "rotate@1621",
                Box::new(looprotate::LoopRotate {
                    duplicate_headers: applied.options.copy_headers && !applied.options.has_min_size,
                    size: applied.options.prefers_size(),
                    check_exit_count: true,
                    free_guard: false,
                }),
            ),
            // LoopInterchangePass (l.1631), in the loop pass manager after
            // LoopRotate (l.1621) and LoopDeletion (l.1628).
            slot("loop-interchange@1631", Box::new(loopinterchange::Interchange::default())),
            // LoopVectorizePass (l.1354): a no-op where the target takes no
            // vector values.
            slot(
                "loop-vectorize@1354",
                Box::new(crate::loopvectorize::LoopVectorize {
                    // clang vectorizes loops at -O2 and up
                    // (`PTO.LoopVectorization`).
                    only_when_forced: !applied.options.pre,
                    ..Default::default()
                }),
            ),
            // DropUnnecessaryAssumesPass (l.1360), after the vectorizer.
            slot(
                "drop-unnecessary-assumes@1360",
                Box::new(crate::dropunnecessaryassumes::DropUnnecessaryAssumes { drop_dereferenceable: true }),
            ),
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
                    options: loopunrollandjam::JamOptions {
                        on: true,
                        clean: true,
                        pre: applied.options.pre,
                        ..Default::default()
                    },
                }),
            ),
            // LoopUnrollPass (l.1476) with the full unroll the target allows
            // (X86 sets no partial or runtime unrolling), then SROA (l.1494)
            // on the allocas the unroll made constant-indexed. The loops the
            // early unroll refused are looked at again after the combines.
            slot(
                "unroll@1476",
                Box::new(loopunrollpass::LoopFullUnroll {
                    opt_level: unroll_level(&applied.options),
                    opt_size: size,
                    only_when_forced: !applied.options.loop_unrolling,
                    target_unrolling: true,
                    inliner_follows: true,
                    speed_rate: applied.options.prices_bytes(),
                    created: Default::default(),
                    decided: Default::default(),
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
            if applied.options.merge_icmps {
                slot(
                    "mergeicmps@1672",
                    Box::new(crate::mergeicmps::MergeICmps::with_routines(applied.options.library.clone())),
                )
            } else {
                Box::new(Together { name: "mergeicmps@1672", parts: Vec::new() })
            },
            slot("expandmemcmp@1673", Box::new(crate::expandmemcmp::ExpandMemCmp)),
            // TailCallElim (l.1677, the module optimization pipeline's
            // "calls created during optimization"), at every level: at -O1 it
            // is the only one, so a self-recursive tail call is a loop there
            // (recmany -O1 -m32 ran 1500 frames deep, 60 KB, where the pin's
            // runs in one).
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
                slot("aggressive-instcombine@678", Box::new(crate::aggressive_instcombine::AggressiveInstCombine)),
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
            // Non-trivial unswitching (-O3) revisits the loops it made and the
            // loop it unswitched (`UnswitchRevisit`); the trivial one, at every
            // other level, never does.
            let unswitch_slot = || -> Box<dyn FunctionPass> {
                let unswitch =
                    Box::new(simpleloopunswitch::SimpleLoopUnswitch { non_trivial: applied.options.unswitch });
                if !applied.options.unswitch {
                    return unswitch;
                }
                let mut preamble: Vec<Box<dyn FunctionPass>> = vec![
                    Box::new(crate::loopinstsimplify::LoopInstSimplify),
                    Box::new(crate::loopsimplifycfg::LoopSimplifyCfg),
                    Box::new(floatloop::FloatLoop),
                ];
                if applied.options.licm {
                    preamble.push(Box::new(crate::licm::Licm { apply: true, speculate: !rotating }));
                }
                if rotating {
                    preamble.push(Box::new(looprotate::LoopRotate {
                        duplicate_headers: applied.options.copy_headers && !applied.options.has_min_size,
                        size: applied.options.prefers_size(),
                        check_exit_count: false,
                        free_guard: false,
                    }));
                }
                if applied.options.licm {
                    preamble.push(Box::new(crate::licm::Licm { apply: true, speculate: true }));
                }
                Box::new(UnswitchRevisit { preamble, unswitch })
            };
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
                        check_exit_count: false,
                        free_guard: false,
                    }),
                ));
            }
            list.extend([
                licm("licm@730", true),
                // The pin's `SimpleLoopUnswitchPass(NonTrivial = Level == O3)`:
                // the options say -O3 and -Omax.
                slot("trivialunswitch@732", unswitch_slot()),
                // SimplifyCFGOptions().convertSwitchRangeToICmp(true), L766:
                // the option is read by `simplifySwitch`, untranslated.
                simplify_cfg("decide@766"),
                combine("combine@768"),
            ]);
            list.extend(canonical("loopsimplify@772", "lcssa@772"));
            // What the unroll makes, for the slot after it.
            let created: std::rc::Rc<std::cell::RefCell<Vec<llrm_mir::module::BlockId>>> = Default::default();
            let decided: std::rc::Rc<std::cell::RefCell<Vec<llrm_mir::module::BlockId>>> = Default::default();
            // LPM2 runs loop by loop (`PerLoop`): the slot of indvars runs the
            // passes of the slots after it too, each of which then finds its
            // work done.
            let fused: Fused = Default::default();
            let deletion = || -> Box<dyn FunctionPass> {
                Box::new(crate::loopidiom::pass::AfterDeletion {
                    deletion: loopdeletion::LoopDeletion,
                    idiom: crate::loopidiom::pass::LoopIdiom {
                        for_size: size,
                        ..crate::loopidiom::pass::LoopIdiom::with_routines(applied.options.library.clone())
                    },
                })
            };
            let unroll = || -> Box<dyn FunctionPass> {
                Box::new(loopunrollpass::LoopFullUnroll {
                    opt_level: unroll_level(&applied.options),
                    opt_size: size,
                    only_when_forced: !applied.options.loop_unrolling,
                    target_unrolling: false,
                    inliner_follows: applied.options.inline.limit > 0 || applied.options.inline.last,
                    speed_rate: applied.options.prices_bytes(),
                    created: std::rc::Rc::clone(&created),
                    decided: std::rc::Rc::clone(&decided),
                })
            };
            let on_created = || -> Box<dyn FunctionPass> {
                Box::new(indvars::IndVarsOnCreated { created: std::rc::Rc::clone(&created) })
            };
            let tails: Vec<Box<dyn FunctionPass>> =
                [("loopdeletion@749", deletion()), ("unroll@753", unroll()), ("indvars@753", on_created())]
                    .into_iter()
                    .filter(|(name, _)| applied.options.wanted(name))
                    .map(|(_, pass)| pass)
                    .collect();
            list.extend([
                slot(
                    "loopidiom@737",
                    Box::new(crate::loopidiom::pass::LoopIdiom {
                        for_size: size,
                        speed_rate: applied.options.prices_bytes(),
                        ..crate::loopidiom::pass::LoopIdiom::with_routines(applied.options.library.clone())
                    }),
                ),
                slot("loopidioms@737", Box::new(loopidioms::LoopIdioms { size })),
                slot(
                    "indvars@738",
                    Box::new(PerLoop {
                        head: Box::new(indvars::IndVars { lftr: indvars::lftr_default(), size }),
                        parts: tails,
                        fused: std::rc::Rc::clone(&fused),
                    }),
                ),
                slot("loopdeletion@749", Box::new(AfterGroup { inner: deletion(), fused: std::rc::Rc::clone(&fused) })),
                slot("unroll@753", Box::new(AfterGroup { inner: unroll(), fused: std::rc::Rc::clone(&fused) })),
                // The loops the unroll made go on to LPM2's passes: indvars on
                // them.
                slot("indvars@753", Box::new(AfterGroup { inner: on_created(), fused })),
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
                slot("merge@818", Box::new(loopidioms::Merge { size, widen: applied.options.merge_stores })),
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
        .filter(|one| {
            applied.options.pre
                || (applied.options.thread_jumps && is_jump_threading(one.name()))
                || !crate::parity::ABSENT_AT_O1.contains(&one.name())
        })
        .collect()
}

/// The slots of `JumpThreadingPass` (`Options::thread_jumps`).
fn is_jump_threading(name: &str) -> bool {
    matches!(name, "jumpthreading@668" | "jumpthreading@809")
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
    let options = applied.options.clone();
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

/// The level `LoopFullUnroll` is told: 1 at -O1 (`pre` off, as `ABSENT_AT_O1`
/// reads it), else 3 or 2 by the target percent. It was always 2 or 3, so the
/// -O1 rule `unroll_never_grows_at_o1` never ran.
/// Whether ipa-cp copies without `-fipa-cp-clone`: at -O2 and -O3 where the
/// target says so, never for size.
fn cp_clones_by_default(
    options: &Options,
    target_clones_at_speed: bool,
) -> bool {
    target_clones_at_speed && unroll_level(options) >= 2 && !options.prefers_size()
}

fn unroll_level(options: &Options) -> u32 {
    if !options.pre {
        1
    } else if options.limits.target_percent >= 200 {
        3
    } else {
        2
    }
}

/// The `-O<text>` words every driver takes. A driver maps each to its own
/// mid-end `Options`; `Omax` is `Options::aggressive()` there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LevelName {
    O0,
    O1,
    O2,
    O3,
    Omax,
    Os,
    Oz,
}

impl LevelName {
    /// The level `-O<text>` names; a bare `-O` is -O1, as in gcc.
    pub fn parse(text: &str) -> Result<Self, String> {
        Ok(match text {
            "0" => Self::O0,
            "" | "1" => Self::O1,
            "2" => Self::O2,
            "3" => Self::O3,
            "max" => Self::Omax,
            "s" => Self::Os,
            "z" => Self::Oz,
            _ => {
                return Err(format!(
                    "unknown optimization level -O{text}; choose -O0, -O1, -O2, -O3, -Omax, -Os or -Oz"
                ));
            }
        })
    }
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
/// once each.
pub(crate) fn function_stages(applied: &Applied) -> Vec<Fixed> {
    vec![
        Fixed::linear(applied, Phase::Early),
        Fixed::linear(applied, Phase::Cleanup),
        Fixed::linear(applied, Phase::Simplify),
        // Placed after the interprocedural passes, as LLVM runs it
        // (`recorded`).
        Fixed::linear(applied, Phase::Late),
    ]
}

/// The first value of a defined function that is a vector, where the target
/// has no vector MIR: "a <4 x i32> value", as isel refuses a type it has no
/// registers for. The mid-end assumes scalars (`combine_shared::width`).
pub fn refuse_vectors(program: &Program) -> Result<(), String> {
    if program.target.has_vector_mir() {
        return Ok(());
    }
    for module in &program.modules {
        for (_, global, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
            for at in 0..function.value_count() {
                let ty = function.value(llrm_mir::module::ValueId(at as u32)).ty;
                if matches!(
                    module.context.types.get(ty),
                    llrm_mir::types::Type::Vector { .. }
                ) {
                    let name = global.name.as_deref().unwrap_or("?");
                    return Err(format!("@{name}: a {} value", module.context.types.display(ty)));
                }
            }
        }
    }
    Ok(())
}

/// An `alloca` asking more than the target's frames keep and no more than it
/// accepts (`Machine::kept_stack_align`, `max_alloca_align`) is clamped to what
/// they keep; one asking more than it accepts is refused by name. A dynamic one
/// is left as asked where the target masks the address
/// (`honors_dynamic_alloca_align`). Alignment is
/// the explicit one, else the type's, as known-bits reads it.
pub fn constrain_allocas(program: &mut Program) -> Result<(), String> {
    let (kept, max) = (program.target.kept_stack_align(), program.target.max_alloca_align());
    let realigned_above = program.target.realigned_above();
    if kept.is_none() && max.is_none() {
        return Ok(());
    }
    let honors_dynamic = program.target.honors_dynamic_alloca_align();
    // The passes that raise an alloca's alignment stop at the datalayout's
    // stack alignment (`tryEnforceAlignment`); no frame here keeps more.
    if let Some(kept) = kept {
        let held = program.layout.stack_align;
        program.layout.stack_align = Some(held.map_or(kept, |held| held.min(kept)));
    }
    let layout = program.layout.clone();
    for module in &mut program.modules {
        let names: Vec<String> = module
            .functions()
            .filter(|(_, _, function)| !function.is_declaration())
            .filter_map(|(_, global, _)| global.name.as_deref().map(str::to_owned))
            .collect();
        for name in names {
            let (context, function) = module.function_mut(&name).expect("a defined function");
            for block in function.layout().to_vec() {
                for inst in function.iter_block(block).collect::<Vec<_>>() {
                    let llrm_mir::opcode::Opcode::Alloca { allocated, align, .. } = function.instruction(inst).opcode
                    else {
                        continue;
                    };
                    if honors_dynamic && !llrm_mir::instructioncost::is_static_alloca(function, inst) {
                        continue;
                    }
                    let wanted = align.unwrap_or_else(|| layout.align(&context.types, allocated));
                    if let Some(max) = max.filter(|max| wanted > *max) {
                        return Err(format!(
                            "@{name}: an object aligned to {wanted} needs stack realignment, not supported yet (accepts {max})"
                        ));
                    }
                    let realigned = realigned_above.is_some_and(|above| wanted > above);
                    if let Some(kept) = kept.filter(|kept| wanted > *kept && !realigned) {
                        function.set_alloca_align(inst, kept);
                        // An access that states more than its object keeps
                        // claims an alignment nothing has, and
                        // `isSafeToSpeculateLoad` refuses it
                        // (`isDereferenceableAndAlignedPointer`): the second
                        // load of `c >= '0' && c <=
                        // '9'` stayed behind its branch and the range check
                        // never formed.
                        let Some(result) = function.instruction(inst).result else { continue };
                        for user in function.users(result).into_iter().copied().collect::<Vec<_>>() {
                            let access = function.instruction(user.user);
                            let pointer = match access.opcode {
                                llrm_mir::opcode::Opcode::Load { align, .. } => (user.index == 0).then_some(align),
                                llrm_mir::opcode::Opcode::Store { align, .. } => (user.index == 1).then_some(align),
                                _ => None,
                            };
                            if pointer.is_some_and(|stated| stated.is_none_or(|stated| stated > kept)) {
                                function.set_access_align(user.user, kept);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// `applied`, with what the pipeline did to each function.
pub fn recorded(
    program: &mut Program,
    applied: &Applied,
) -> Result<Vec<Stage>, String> {
    refuse_vectors(program)?;
    constrain_allocas(program)?;
    timed();
    let mut manager = PassManager::default();
    manager.verify_each = llrm_support::debug::verifying();
    manager.dump = applied.dump.clone();
    manager.dump_inputs = llrm_support::env_flag!("LLRM_DUMP_INPUTS");
    if applied.options.optimize && applied.options.prefers_size() {
        mark_for_size(program, applied.options.has_min_size);
    }
    if !applied.options.optimize {
        manager.add_program(availableexternally::EliminateAvailableExternally);
        return manager.run(program);
    }
    // DEVIATE IPACP-O3 (owner x86-m32-c, exit 2026-12-20): gcc's
    // `-fipa-cp-clone` is on at -O3, the pin has no such pass; the target
    // states whether it copies at -O2 and -O3 (DEVIATE IPACP-O2 for -O2). A
    // flag given wins.
    let resolved = Applied {
        options: Options {
            inline: inline::Threshold {
                cp_clone: Some(applied.options.inline.cp_clone.unwrap_or_else(|| {
                    cp_clones_by_default(
                        &applied.options,
                        program.target.tti().is_some_and(|tti| tti.ipa_cp_clones_at_speed()),
                    )
                })),
                cp_hint_clones: applied.options.inline.cp_clone.is_some() || unroll_level(&applied.options) >= 3,
                cp_peel_limited: applied.options.inline.cp_clone.is_none(),
                ..applied.options.inline
            },
            ..applied.options.clone()
        },
        ..applied.clone()
    };
    let applied = &resolved;
    // The clones of the step before the walk and of the one after it share a
    // bound.
    let cloning = crate::ipacp::Shared::default();
    // LLVM's GlobalsAA is made at `RequireAnalysisPass<GlobalsAA>` and
    // `RecomputeGlobalsAAPass` alone, read cached in between.
    manager.require_cached::<GlobalsAA>();
    manager.require_cached::<GlobalsModRefAnalysis>();
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
    let walked = |dump: &str| Applied { dump: applied.dump.as_ref().map(|one| one.join(dump)), ..applied.clone() };
    let mut late_stage = None;
    for (at, stage) in function_stages(applied).into_iter().enumerate() {
        // The walk simplifies each body: not a module-wide stage. The late
        // list follows the interprocedural passes.
        if at == 3 {
            late_stage = Some(stage);
        } else if at != 2 {
            manager.add(stage);
        }
        // LLVM's CGSCC walk: each body inlined into, then simplified, callees
        // first. `Phase::Simplify` runs there and not module-wide.
        if at == 1 {
            // What each body does to memory is stated here, as the pin's
            // PostOrderFunctionAttrs inside the CGSCC walk: the early
            // simplification (SROA, EarlyCSE, PromotePass, InstCombine,
            // SimplifyCFG) ran before any callee had an attribute, so
            // InstCombine could not sink a call of a `memory(none)` helper into
            // one arm of an `&&`.
            manager.add_program(crate::interprocedural::Stamp);
            // `buildInlinerPipeline` (PassBuilderPipelines.cpp:997,
            // d1106deb71cc).
            manager.compute::<GlobalsAA>();
            manager.compute::<GlobalsModRefAnalysis>();
            let mut simplify = Fixed::linear(&walked("walk"), Phase::Simplify);
            // `Tti::inline_trial_light_simplify`: the SZ2 trial's copy goes
            // through the cleanup list, not the whole one.
            let mut light = Fixed::linear(&walked("walk"), Phase::Cleanup);
            manager.add_program(crate::walk::Walk {
                inline: applied.options.inline,
                speed_rate: applied.options.prices_bytes(),
                rerun_unchanged: true,
                promote_rate: Some(if applied.options.prefers_size() {
                    0
                } else {
                    applied.options.limits.milliclocks_per_byte
                }),
                simplify: Box::new(move |module, analyses, id, stage| {
                    TRIGGER.with(|trigger| *trigger.borrow_mut() = stage.to_owned());
                    let list = if stage == crate::walk::TRIAL_LIGHT { &mut light } else { &mut simplify };
                    rerun(module, analyses, id, list, true).unwrap_or_else(|error| panic!("pipeline: {error}"));
                    TRIGGER.with(|trigger| trigger.borrow_mut().clear());
                }),
            });
        }
        // LLVM's IPSCCP and DeadArgumentElimination run after the early
        // function pipeline, before the Simplify list.
        if at == 0 {
            manager.add_program(crate::walk::ModuleIpo { inline: applied.options.inline, cloning: cloning.clone() });
        }
    }
    // Once every body has reached its own fixed point, as the old Nib
    // driver's whole-module step: a body it changes goes back through.
    let again_applied =
        Applied { dump: applied.dump.as_ref().map(|one| one.join("interprocedural")), ..applied.clone() };
    let mut again = Fixed::linear(&again_applied, Phase::Simplify);
    // The pin follows IPSCCP, ArgumentPromotion and DeadArgumentElimination
    // with `GlobalCleanup` (PromotePass, InstCombine, SimplifyCFG,
    // PassBuilderPipelines.cpp#L1241) and runs the function simplification
    // pipeline once per body, in the CGSCC walk: these stages leave a body to
    // the walk's run, not a second full one (GVN ran five times on `main` of
    // memory_expansion, the pin's twice). `promote.` keeps the full list: the
    // minimal one left the far sort arguments of Nib quicksort unfolded.
    let mut cleaned = Fixed::linear(&again_applied, Phase::Cleanup);
    // The copies of a tree recursion in itself and then its tail-call
    // elimination, after the clones ipa-cp made (D10).
    let late = Some({
        let mut simplify = Fixed::linear(&again_applied, Phase::Simplify);
        crate::walk::SelfInline {
            simplify: Box::new(move |module, analyses, id, stage| {
                TRIGGER.with(|trigger| *trigger.borrow_mut() = stage.to_owned());
                rerun(module, analyses, id, &mut simplify, true).unwrap_or_else(|error| panic!("pipeline: {error}"));
                TRIGGER.with(|trigger| trigger.borrow_mut().clear());
            }),
            for_size: applied.options.prefers_size(),
            speed_rate: applied.options.prices_bytes(),
            inlining: applied.options.inline.limit != 0,
            flag: applied.options.inline.recursive,
            o3: applied.options.inline.limit >= 250,
        }
    });
    manager.add_program(Interprocedural {
        late,
        cloning,
        pipeline: Box::new(move |module, analyses, id, stage| {
            // The pin runs nothing over a body `DeadArgumentEliminationPass`
            // changed: the late function pipeline, which every body goes
            // through, finds what the lost argument leaves
            // (PassBuilderPipelines.cpp,
            // `buildModuleOptimizationPipeline` follows the module pass).
            if stage == "ipa-deadargs." {
                return;
            }
            TRIGGER.with(|trigger| *trigger.borrow_mut() = stage.to_owned());
            let done =
                // The stage that only retargets loads and stores of globals
                // keeps GlobalsAA past its bodies, as the walk's runs do.
                rerun(
                    module,
                    analyses,
                    id,
                    if stage == "ipsccp." { &mut cleaned } else { &mut again },
                    stage == "globalsra.",
                )
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
    let target_wants_round = !applied.options.prices_bytes()
        && !applied.options.prefers_size()
        && program.target.tti().is_some_and(|tti| tti.inline_second_round_at_o1());
    if (applied.options.second_inline || target_wants_round) && applied.options.inline.limit != 0 {
        manager.compute::<GlobalsAA>();
        manager.compute::<GlobalsModRefAnalysis>();
        let mut simplify = Fixed::linear(&walked("walk2"), Phase::Simplify);
        manager.add_program(crate::walk::Walk {
            inline: applied.options.inline,
            speed_rate: applied.options.prices_bytes(),
            rerun_unchanged: false,
            promote_rate: Some(if applied.options.prefers_size() {
                0
            } else {
                applied.options.limits.milliclocks_per_byte
            }),
            simplify: Box::new(move |module, analyses, id, stage| {
                TRIGGER.with(|trigger| *trigger.borrow_mut() = stage.to_owned());
                rerun(module, analyses, id, &mut simplify, true).unwrap_or_else(|error| panic!("pipeline: {error}"));
                TRIGGER.with(|trigger| trigger.borrow_mut().clear());
            }),
        });
    }
    // `ReversePostOrderFunctionAttrsPass` (PassBuilderPipelines.cpp:1542).
    manager.add_module(crate::rpoattrs::ReversePostOrderFunctionAttrs);
    // Once the callers that remain are the ones that stay: an internal function
    // they all call directly pops its own arguments.
    if applied.options.wanted("calleepop") {
        manager.add_module(calleepop::CalleePop { size: applied.options.prefers_size() });
    }
    // The end of `buildModuleOptimizationPipeline`'s function list (l.1662),
    // after the interprocedural passes: a body that took the late unroll is
    // not run through the simplification pipeline again.
    if let Some(stage) = late_stage {
        manager.add(stage);
    }
    // `buildModuleOptimizationPipeline` (PassBuilderPipelines.cpp:1567,
    // d1106deb71cc). DEVIATE (owner w9, exit 2026-11-20): made even where not
    // held, as GlobalDCE, which LLVM runs after this point, abandoned it
    // here.
    manager.recompute::<GlobalsAA>();
    manager.recompute::<GlobalsModRefAnalysis>();
    // The summaries are made. The passes below move and rewrite what a function
    // does and add nothing to it, so they read these and do not make them
    // again after each edit, as gcc's passes after IPA read the modref
    // summaries it made there.
    manager.freeze::<Summaries>();
    // `AArch64PassConfig::addIRPasses`, after AtomicExpand and before LSR.
    if applied.options.atomic_cfg_tidy && applied.options.simplifycfg {
        manager.add(simplifycfg::SimplifyCfg { options: simplifycfg::Options::ATOMIC_TIDY });
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
    // `addIRPasses`: PartiallyInlineLibCalls follows LSR (the pin's
    // `-disable-partial-libcall-inlining` is `LLRM_NO_PARTIAL_LIBCALLS`).
    if applied.options.wanted("partialinline") && !llrm_support::env_flag!("LLRM_NO_PARTIAL_LIBCALLS") {
        manager.add(partialinline::PartiallyInlineLibCalls);
    }
    // On the pointers LSR chose: a huge one a loop keeps in one window is far
    // there.
    if applied.options.wanted("window") {
        manager.add(window::Window { size: applied.options.prefers_size() });
    }
    // DEVIATE Z80-ROT: after every loop pass, as the gcc tail rotate was, a
    // counted loop proven to run is entered at its body at -Oz (target
    // opt-in). Owner z80, exit 2026-11-30.
    if applied.options.has_min_size {
        manager.add(looprotate::LoopRotate {
            duplicate_headers: false,
            size: true,
            check_exit_count: false,
            free_guard: true,
        });
    }
    // A loop entered at its body runs it at least once: what it loads
    // unchanged may now leave it, as MachineLICM follows LLVM's LSR.
    if applied.options.wanted("hoist") && !llvm_motion(applied) {
        manager.add(crate::licm::Licm { apply: true, speculate: true });
    }
    // After hoist, which would move a constant `gep` out of its loop.
    if applied.options.wanted("gepoffset") {
        manager.add(gepoffset::GepOffset);
    }
    // After gepoffset, which leaves the constant part of an address in its own `gep`.
    if applied.options.wanted("gepbyteindex") {
        manager.add(gepbyteindex::GepByteIndex);
    }
    // After gvn's last partial redundancy elimination, which makes the phis it
    // sinks.
    // The pin's `addIRPasses` slot for SelectOptimize, before CodeGenPrepare;
    // DEVIATE SELOPT-C at the speed rate.
    if applied.options.prices_bytes() && applied.options.wanted("selectoptimize") {
        manager.add(crate::selectoptimize::SelectOptimizePass);
    }
    // The pin's `addCodeGenPrepare` runs TypePromotion, then CodeGenPrepare.
    if applied.options.wanted("typepromotion") {
        manager.add(typepromotion_pass::TypePromotion);
    }
    // CodeGenPrepare's sinking: compares, masks, address phis and the address
    // arithmetic of a block's accesses, made in the block that uses them.
    if applied.options.wanted("cgpaddr") {
        if applied.options.prefers_size() || applied.options.at_o1() {
            manager.add(cgpaddr::CgpAddrBytesStrict);
        } else {
            manager.add(cgpaddr::CgpAddr);
        }
    }
    // DEVIATE CS2 (owner m32, exit 2026-11-20), at the speed rate.
    if (applied.options.prices_bytes() || applied.options.for_size) && applied.options.wanted("cgpselectarms") {
        manager.add(crate::cgpselect::SelectArms { size: applied.options.for_size });
    }
    // After cgpaddr, with the other passes that make the forms isel sees:
    // the pin's DAGCombiner makes one extending load for a load and its
    // extension (`ExtendUsesToFormExtLoad`), which isel cannot.
    if applied.options.wanted("extload") {
        manager.add(extload::ExtLoad);
    }
    // DEVIATE D1 of `zeromerge` (owner m16, exit 2026-11-20): after SROA and
    // loop idioms made the fills.
    if applied.options.wanted("zeromerge") {
        manager.add(zeromerge::ZeroMerge);
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
    // `MergeFunctions`: off in the pin's pipeline; a target that wants it at
    // -Os says so (DEVIATE MERGEFUNC-OS).
    if applied.options.optimize
        && applied.options.prefers_size()
        && program.target.tti().is_some_and(|tti| tti.merge_functions_at_size())
    {
        manager.add_program(crate::mergefunc::MergeFunctions);
    }
    if applied.options.optimize
        && !applied.options.prefers_size()
        && program.target.tti().is_some_and(|tti| tti.merge_local_functions_at_speed())
    {
        manager.add_program(crate::mergefunc::MergeLocalFunctions);
    }
    if applied.options.optimize {
        // `GlobalDCEPass` then `ConstantMergePass` after the function passes:
        // what a late unroll or fold left unnamed goes before merging.
        manager.add_program(globaldce::GlobalDce);
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

/// What clang does at -Os and -Oz: every defined function carries `optsize`,
/// and `minsize` at -Oz (`Function::hasOptSize` / `hasMinSize` read them).
fn mark_for_size(
    program: &mut Program,
    min_size: bool,
) {
    for module in &mut program.modules {
        for global in &mut module.globals {
            let GlobalKind::Function(function) = &mut global.kind else { continue };
            if function.is_declaration() {
                continue;
            }
            for name in ["optsize"].into_iter().chain(min_size.then_some("minsize")) {
                if !function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(flag) if flag == name)) {
                    function.attrs.push(Attribute::Flag(name.into()));
                }
            }
        }
    }
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
        passes.push(Box::new(crate::lsr_pass::LoopStrengthReduce {
            size: applied.options.prefers_size(),
            bytes_strict: applied.options.prefers_size() || applied.options.at_o1(),
        }));
        // `TargetPassConfig::addIRPasses` (pin l.875): LoopTermFold follows
        // LSR; it does nothing for a target whose TTI does not enable it.
        if applied.options.wanted("looptermfold") {
            passes.push(Box::new(crate::looptermfold::LoopTermFold { size: applied.options.prefers_size() }));
        }
        // What the counters it chose leave behind (a bound subtracted from a
        // counter rebased by it), as LLVM's LSR cleans with
        // SimplifyInstructions.
        passes.push(Box::new(algebraic::Differences));
        // A masked counter read as an index is the byte offset, where the
        // target prices a scaled index.
        if applied.options.wanted("maskscale") {
            passes.push(Box::new(crate::maskscale::MaskScale));
        }
        // The remainder LSR wrote out as a quotient and a product, back to one
        // where the divide stays.
        passes.push(Box::new(crate::remidiom::RemainderIdiom { size: applied.options.prefers_size() }));
        // A divide whose operands ScalarEvolution proves narrow, in the narrow
        // type, where the target has one (DEVIATE NRW-1); at speed
        // only: the call it replaces is the smaller code.
        if applied.options.wanted("narrowrange") && !applied.options.prefers_size() && !applied.options.at_o1() {
            passes.push(Box::new(crate::narrowrange::NarrowByRange));
        }
        // The pin's llc runs it in the pre-ISel passes, after LSR; it does
        // nothing for a target that counts no loop in hardware.
        if applied.options.wanted("hardwareloops") {
            passes.push(Box::new(crate::hardwareloops::HardwareLoops));
        }
        // `TargetPassConfig::addIRPasses` (pin l.874): ConstantHoisting
        // follows LSR; it hoists nothing for a target whose constants all
        // cost the basic one.
        if applied.options.wanted("constanthoisting") {
            passes.push(Box::new(crate::constanthoisting::ConstantHoisting));
        }
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
    // It does not go through the manager's loop, which skips an `optnone` body.
    if let GlobalKind::Function(function) = &module.global(id).kind
        && llrm_mir::passes::skipped_for_optnone(&*fixed, function)
    {
        return Ok(());
    }
    let layout = analyses.program().layout.clone();
    let outer = analyses.outer(module);
    let mut declared = Declared::over(std::rc::Rc::clone(&outer.globals), module.metadata.len())
        .with_assume_declared(outer.assume_declared())
        .with_guard_declared(outer.guard_declared());
    if fixed.wants_sizes() {
        declared.count_referenced(module);
    }
    llrm_mir::passes::snapshot_for_audit(module);
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
        let mut one = Self::linear(&quiet, phase);
        one.passes.retain(|pass| pass.name() == slot);
        one.only = true;
        one
    }

    /// `passes` once each in order, no fixed point: LLVM's function pipelines
    /// run a pass where it stands.
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
        run.fixed += 1;
        // A named point: what was kept as sound is worked out afresh.
        analyses.refresh_soft();
        run.rounds += 1;
        for one in &mut self.passes {
            let pass = one.name();
            let name = format_args!("{prefix}r01-{pass}");
            run.step(&mut **one, &name, unit, analyses);
        }
        run.settled(unit, analyses);
        Ok(())
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
            fixed: 0,
            billing: runs_billed(),
            idle: 0,
            useful: 0,
            idle_with_analyses: 0,
            useful_with_analyses: 0,
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
            "body {} trigger {:?}: {} fixed points, {} rounds, {} pass runs, {} skipped as settled in an earlier run, {} changes, work idle {} useful {} (with the analyses the passes computed first: idle {} useful {}) name {}",
            unit.id.map_or(-1, |id| i64::from(id.0)),
            TRIGGER.with(|trigger| trigger.borrow().clone()),
            run.fixed,
            run.rounds,
            run.steps,
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
    /// For `LLRM_DEBUG=runs`: fixed points reached, rounds in them, passes run.
    fixed: usize,
    rounds: usize,
    steps: usize,
    /// The work of passes that changed nothing, and of those that did (only
    /// where `runs` is on).
    billing: bool,
    idle: u64,
    useful: u64,
    /// The same two with the analyses the passes computed first: what skipping
    /// a pass would not save, since the next pass to ask would pay it.
    idle_with_analyses: u64,
    useful_with_analyses: u64,
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
        if llrm_mir::passes::skipped_for_optnone(&*pass, unit.function) {
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
            visibility: llrm_mir::Visibility::Default,
            unnamed_addr: UnnamedAddr::default(),
            address_space: 0,
            kind: GlobalKind::Function(Box::new(unit.function.clone())),
        });
        let module = Module {
            context: unit.context.clone(),
            datalayout: None,
            globals,
            aliases: Vec::new(),
            metadata: unit.metadata.to_vec(),
            named_metadata: Vec::new(),
            sections: Default::default(),
            special: Default::default(),
            global_metadata: Default::default(),
            ..Module::default()
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
