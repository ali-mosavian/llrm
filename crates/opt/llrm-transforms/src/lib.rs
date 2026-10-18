//! MIR to MIR transforms: the passes that rewrite a function or a module, the
//! driver that orders them, and the models they price with.
//!
//! # How it works
//! A pass asks `llrm_analysis` for its facts and changes the MIR; one that
//! needs a machine fact (an operation's cost, the registers, a huge-pointer
//! window) reads it from the program's target through [`crate::profit`] and
//! [`crate::spill`]. [`crate::pipeline`] holds the order and the switches
//! (`Options`, one per -O level); everything else is named by its role.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef note fill:transparent,stroke:transparent,color:#64748b
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   a["<b>globalopt, ports, Stamp</b><br/><small>(module / program)</small>"]:::mid
//!   subgraph g_body["PER BODY, REPEATED TO A FIXED POINT"]
//!     s1["<b>sroa (+ splitcopy)</b><br/><small>at boundaries</small>"]:::mid
//!     s2["<b>cleanup: fold decide dead</b><br/><small>algebraic phiopt dse gvn<br/>promote inferspace<br/>calls: tailrec<br/>loops: loopsimplify lcssa<br/>hoist loopmotion<br/>trivialunswitch indvars<br/>floatloop fill</small>"]:::mid
//!     s3["<b>after each round: unroll</b>"]:::mid
//!     s4["<b>after it settles: peel,</b><br/><small>unswitch, split, fuse, distribute, unroll-jam, merge</small>"]:::mid
//!   end
//!   ipa["<b>interprocedural: inline,</b><br/><small>ipacp, argpromotion,<br/>deadargs, narrowspace</small>"]:::mid
//!   dce["<b>globaldce</b><br/><small>availableexternally<br/>calleepop</small>"]:::mid
//!   ch["<b>copy headers (rotate,</b><br/><small>loopsimplify, lcssa, fold,<br/>algebraic, indvars, dead)<br/>fixednarrow lsr window<br/>rotate jumpthread hoist<br/>gepoffset addresssink<br/>spares homes</small>"]:::mid
//!   sel["selection"]:::note
//!   a -->|"Fixed (per body)"| s1
//!   s1 --> s2
//!   s2 --> s3
//!   s3 --> s4
//!   s4 --> ipa
//!   ipa -.->|"reruns Fixed on a<br/>changed body"| s1
//!   ipa --> dce
//!   dce --> ch
//!   ch --> sel
//!   style g_body fill:#47556912,stroke:#475569,stroke-width:1.6px,stroke-dasharray:7 5,color:#475569
//! ```
//!
//! *Pass order across a module*
//!
//! The modules fall into five groups.
//!
//! Driver: [`crate::pipeline`] (`Options`, `Fixed`, the order above) and
//! [`crate::interprocedural`] (the whole-program step); [`crate::candidate`]
//! judges a rewrite tried on a copy of the body.
//!
//! Scalar and memory: `fold` (with `canonical` and `floatfold`), `decide`,
//! `dead`, `algebraic`, `phiopt`, `cfg`, `dse`, `gvn` (with `loadjoins`),
//! `promote` (`Sroa`, `Promote`, with `splitcopy`), `inferspace`, `jumpthread`,
//! `gepoffset`, `addresssink`, `fixednarrow`, `ports`.
//!
//! Loops: `loopsimplify`, `lcssa` (with `lcssamerges`), `hoist`, `loopmotion`,
//! `trivialunswitch`, `unswitch`, `loopsplit`, `loopfuse`, `loopdistribute`
//! (with `loopshape`), `indvars` (with `loopexit`, `exitfold`, `exitsink`),
//! `lsr` (with `mirror`), `window`, `rotate`, `unroll`, `peel`, `unroll_jam`,
//! `loopidioms`, `floatloop`, `tailrec`.
//!
//! Whole program: `globalopt`, `globaldce`, `availableexternally`, `inline`,
//! `ipacp`, `argpromotion`, `deadargs`, `narrowspace`, `calleepop`; and
//! `spares` and `homes`, which record for selection what the optimizer proved.
//!
//! Shared by the rest: `profit` and `spill` (what a change costs and what
//! register pressure costs), `loopclone` (loop iterations as CFG), `edges`,
//! `counting` and `expand` (what a counted-loop rewrite places before a loop),
//! `transform` (value reuse and phi cleanup).
//!
//! # Example
//! None: this is the crate's index, with no transformation of its own. Read a
//! module's own header, or `docs/optimizations/passes/readme.md`.
//!
//! # See also
//! `docs/reference/levels.md` for which pass runs at which level, and each
//! module's own header.

pub mod addresssink;
pub mod algebraic;
pub mod argpromotion;
#[cfg(test)]
mod argpromotion_tests;
pub mod availableexternally;
#[cfg(test)]
mod availableexternally_tests;
pub mod calleepop;
#[cfg(test)]
mod calleepop_tests;
pub mod candidate;
pub mod canonical;
pub mod cfg;
pub mod constraintelim;
#[cfg(test)]
mod constraintelim_tests;
#[cfg(test)]
mod corpus_tests;
pub mod counting;
pub mod cvp;
#[cfg(test)]
mod cvp_tests;
pub mod dead;
pub mod deadargs;
#[cfg(test)]
mod deadargs_tests;
pub mod decide;
pub mod dse;
pub mod earlycse;
pub mod edges;
pub mod exitfold;
pub mod exitsink;
pub mod expand;
pub mod fixednarrow;
pub mod floatfold;
pub mod floatloop;
pub mod fold;
pub mod gepoffset;
pub mod globaldce;
#[cfg(test)]
mod globaldce_tests;
pub mod globalopt;
pub mod gvn;
#[cfg(test)]
mod gvn_scalar_tests;
pub mod hoist;
pub mod homes;
pub mod indvars;
pub mod inferspace;
#[cfg(test)]
mod inferspace_tests;
pub mod inline;
pub mod inlinecost;
#[cfg(test)]
mod inlinecost_tests;
pub mod interprocedural;
pub mod ipacp;
pub mod jumpthread;
pub mod lcssa;
pub mod lcssamerges;
pub mod loadjoins;
pub mod local;
#[cfg(test)]
mod local_tests;
pub mod loopclone;
pub mod loopdistribute;
pub mod loopexit;
pub mod loopfuse;
pub mod loopidiom;
pub mod loopidioms;
pub mod loopinterchange;
pub mod loopmotion;
pub mod loopnest;
pub mod looprotate;
pub mod loopshape;
pub mod loopsimplify;
pub mod loopsplit;
pub mod loopunroll;
pub mod loopversion;
pub mod lsr;
pub mod mem2reg;
pub mod memcpyopt;
pub mod mirror;
pub mod narrowspace;
#[cfg(test)]
mod narrowspace_tests;
pub mod parity;
#[cfg(test)]
mod parity_tests;
pub mod peel;
pub mod phiopt;
pub mod pipeline;
#[cfg(test)]
mod pipeline_ported_tests;
pub mod ports;
pub mod profit;
pub mod promote;
pub mod rotate;
pub mod settled;
pub mod simplifycfg;
pub mod simplifyindvar;
pub mod sink;
pub mod spares;
pub mod spill;
pub mod splitcopy;
pub mod switchcg;
pub mod switchlower;
pub mod tailrec;
#[cfg(test)]
pub mod testing;
pub mod transform;
pub mod trivialunswitch;
pub mod unroll;
pub mod unroll_jam;
pub mod unswitch;
pub mod versionalias;
pub mod versionstride;
pub mod vncoercion;
#[cfg(test)]
mod vncoercion_tests;
pub mod window;
// Not ported, meaning nothing where a value is whole: `wholephis` and
// `wholestores` joined word halves of the old MIR's split values (called
// from `algebraic`).
// Not ported, meaning nothing where an address is an operand: `pointeraccess`
// split a packed far pointer's memory references into offset and selector
// words for the old register allocator; a pointer here is its type, and
// isel splits it. So `transform`'s `SplitPointers`, which ran it, is not
// ported either.
// Not ported, an analysis here: `transform`'s `PointerProvenance` wrote
// `alias::annotated`'s references into the body; alias's `Annotated`
// manager entry answers them on demand.
// Not ported, meaning nothing where a call's arguments are its operands:
// `transform`'s `Place` (`placed`, `_argument_run`, `_may_pass`, `_meets`)
// moved what stood among a call's argument pushes ahead of them.
