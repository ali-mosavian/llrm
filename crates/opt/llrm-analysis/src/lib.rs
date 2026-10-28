//! Facts about the MIR, each owned by one module that the passes in
//! llrm-transforms (and the backend) ask.
//!
//! # How it works
//! An analysis here is a function from MIR to a fact. One module owns each fact
//! and its consumers ask it. The few functions that edit the MIR say so:
//! `ssa::pruned_phis`, `ssa::SsaUpdater`, [`noreturn`]'s `after_terminal_calls`
//! and [`interprocedural`]'s rewrites. The modules fall in groups:
//!
//! - Structure: [`cfg`] (blocks, dominance, loops, `Shape`), [`graph`] (the
//!   loop algorithms over any block graph), [`occurrence`] (instruction walks),
//!   [`nearest`] (items in dominating blocks), [`ssa`] (use lists, phi pruning,
//!   `SsaUpdater`), [`liveness`] (live values and register pressure).
//! - Values: [`consts`] (known values; [`constant_cycles`] is its sparse
//!   solver), [`demanded`] (bits read), [`floatfacts`] and [`floatbounds`]
//!   (exact float values), [`ranges`] (intervals), [`difference`] and
//!   [`guards`] (what dominating compares and `assume`s prove),
//!   [`assumptions`].
//! - Loops: [`induction`] (counters, trip counts, recurrences), [`peelsize`]
//!   (is a full copy worth it), [`depanalysis`] (dependences between memory
//!   accesses, for loop interchange).
//! - Memory: [`memory`] (objects, slices, `MemRef`, `Unit`), [`pointerfacts`],
//!   [`regions`], [`alias`] (points-to, summaries), [`effects`],
//!   [`frameescape`], [`globalsaa`], [`sliceset`] and [`cellmap`] (containers),
//!   [`memoryssa`], [`avail`] (the cells loads and stores name).
//! - Calls and control: [`interprocedural`], [`noreturn`], [`branchprob`]
//!   (static frequencies).
//! - [`manager`] caches the facts that are expensive or asked repeatedly.
//!   `testing` and `generated` are test support, behind the `testing` feature.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef data fill:#7c3aed,stroke:#5b21b6,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   mir["<b>llrm-mir</b><br/><small>Function, Context,<br/>DominatorTree, Analyses</small>"]:::src
//!   base["<b>cfg, graph, liveness</b>"]:::opt
//!   val["<b>consts, demanded</b>"]:::mid
//!   rng["<b>ranges, difference,</b><br/><small>guards, floatfacts,<br/>floatbounds</small>"]:::out
//!   ind["<b>induction</b>"]:::mid
//!   loop["<b>peelsize, depanalysis</b>"]:::out
//!   mem["<b>memory, regions, pointerfacts,</b><br/><small>alias, effects, frameescape,<br/>globalsaa, sliceset, cellmap</small>"]:::opt
//!   ssa["<b>memoryssa, avail,</b><br/><small>noreturn,<br/>interprocedural</small>"]:::out
//!   man["<b>manager (cache)</b>"]:::data
//!   use["<b>llrm-transforms passes,</b><br/><small>backend</small>"]:::src
//!   mir --> base
//!   base --> val
//!   val --> rng
//!   val --> ind
//!   base --> ind
//!   ind --> loop
//!   rng --> loop
//!   base --> mem
//!   mem --> ssa
//!   ssa --> man
//!   man --> use
//! ```
//!
//! *Analysis layers, from the MIR inputs to the cache and its consumers*
//!
//! The arrows point from a fact's owner to the modules that ask it. [`consts`]
//! asks [`memoryssa`] when it solves through memory, so the layers are
//! conceptual, not a strict order.
//!
//! # Example
//! None: this is the crate index. Each module's own page has its example; start
//! with [`manager`] for how a pass asks for a fact.
//!
//! # Provides
//! Invalidation and caching, in [`manager`]. A pass asks
//! `analyses.get::<A>(..)` and gets one shared result per function. After each
//! pass the result is kept if the pass named it preserved; or, for an entry
//! that reads only the function, if the function is unchanged; or, for an entry
//! with `depends`, if the change log cannot have altered it; or it is updated
//! from the log (`INCREMENTAL`: `Counted`, `DominatedEdges`,
//! `Accesses`); else it is dropped and recomputed on the next ask. Module
//! entries (`GlobalsAA`, `Summaries`) are dropped unless every run
//! of a pass preserved them. `LLRM_CHECK_REPLAY` asserts that kept and updated
//! results equal a fresh solve. Free-standing analyses (not manager entries)
//! are recomputed per call; a `Unit` (see [`memory`]) carries the cached ones
//! so repeated queries share them.
//!
//! # See also
//! The manager contract is `crates/ir/llrm-mir/src/passes.rs`. LLVM's
//! `AnalysisManager` and `PreservedAnalyses` are its counterpart.

pub mod aa;
pub mod accessanalysis;
pub mod aggregate;
pub mod alias;
pub mod aliasdiff;
pub mod aliassets;
pub mod argumentattrs;
pub mod assumptions;
pub mod attrsummary;
pub mod avail;
pub mod basicaa;
pub mod blockfreq;
pub mod bpi;
pub mod branchprob;
pub mod captureanalysis;
pub mod capturecheck;
pub mod capturetracking;
pub mod cellmap;
pub mod cfg;
pub mod cfgwalk;
pub mod constant_cycles;
pub mod consts;
#[cfg(test)]
mod corpus_tests;
pub mod demanded;
pub mod demandedbits;
pub mod depanalysis;
pub mod difference;
pub mod effects;
pub mod equivalence;
pub mod floatbounds;
pub mod floatfacts;
pub mod forging;
pub mod frameescape;
pub mod functionattrs;
#[cfg(any(test, feature = "testing"))]
pub mod generated;
pub mod globalsaa;
pub mod globalsmodref;
pub mod graph;
pub mod guards;
pub mod induction;
pub mod initializes;
pub mod interprocedural;
pub mod ivdescriptors;
#[cfg(test)]
mod laa_testing;
pub mod liveness;
pub mod loads;
pub mod loadstorepromoter;
pub mod loopaccess;
pub mod loopaccessinfo;
pub mod loopunrollanalyzer;
pub mod manager;
pub mod memdep;
pub mod memdepchecker;
pub mod memory;
pub mod memoryssa;
pub mod mssa;
pub mod mustexecute;
pub mod nearest;
#[cfg(test)]
mod nocapture_agreement_tests;
pub mod noreturn;
pub mod num;
pub mod occurrence;
pub mod peelsize;
pub mod placealias;
pub mod pointerfacts;
pub mod ranger;
pub mod ranges;
pub mod reachability;
pub mod references;
pub mod regions;
pub mod runtimecheck;
pub mod saverestore;
pub mod scev;
pub mod sliceset;
pub mod slotstores;
pub mod ssa;
pub mod ssaupdater;
pub mod ssaupdaterbulk;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod typebasedaa;
