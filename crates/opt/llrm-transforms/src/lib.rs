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
//!     s2["<b>cleanup: fold decide dead</b><br/><small>algebraic dse gvn<br/>promote inferspace<br/>calls: tailrec<br/>loops: loopsimplify lcssa<br/>licm<br/>trivialunswitch indvars<br/>floatloop fill</small>"]:::mid
//!     s3["<b>after each round: unroll</b>"]:::mid
//!     s4["<b>after it settles: peel,</b><br/><small>unswitch, merge</small>"]:::mid
//!   end
//!   ipa["<b>interprocedural: inline,</b><br/><small>ipacp, argpromotion,<br/>deadargs, narrowspace</small>"]:::mid
//!   dce["<b>globaldce</b><br/><small>availableexternally<br/>calleepop</small>"]:::mid
//!   ch["<b>copy headers (rotate,</b><br/><small>loopsimplify, lcssa, fold,<br/>algebraic, indvars, dead)<br/>fixednarrow lsr window<br/>rotate hoist<br/>gepoffset cgpaddr<br/>spares homes</small>"]:::mid
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
//! [`crate::interprocedural`] (the whole-program step).
//!
//! Scalar and memory: `fold` (with `canonical` and `floatfold`), `decide`,
//! `dead`, `algebraic`, `cfg`, `dse`, `gvn`,
//! `promote` (`Sroa`, `Promote`, with `splitcopy`), `inferspace`,
//! `gepoffset`, `cgpaddr`, `fixednarrow`, `ports`.
//!
//! Loops: `loopsimplify`, `lcssa` (with `lcssamerges`), `hoist`, `loopmotion`,
//! `trivialunswitch`, `unswitch`,
//! `indvars` (with `exitvalues`, `exitsink`),
//! `lsr` (with `mirror`), `window`, `rotate`, `unroll`, `peel`,
//! `loopidioms`, `floatloop`, `tailcallelim`, `loopversioning` (with
//! `runtimechecks`, the memory checks it branches on).
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

pub mod adce;
pub mod algebraic;
pub mod alwaysundef;
pub mod argpromotion;
#[cfg(test)]
mod argpromotion_tests;
pub mod availableexternally;
#[cfg(test)]
mod availableexternally_tests;
pub mod bdce;
pub mod branchtocommon;
pub mod calledvaluepropagation;
pub mod calleepop;
#[cfg(test)]
mod calleepop_tests;
pub mod canonexit;
pub mod canonfreeze;
pub mod canonical;
pub mod cfg;
pub mod cgpaddr;
pub mod cgpbranch;
pub mod cgpmemalign;
#[cfg(test)]
mod cgpmemalign_tests;
pub mod cgpretdup;
pub mod cgpswitch;
pub mod cmpopt;
#[cfg(test)]
mod codegen_slots_tests;
pub mod combine;
pub mod combine_add_idioms;
pub mod combine_addsub;
pub mod combine_and;
pub mod combine_and_select;
pub mod combine_bitorder;
pub mod combine_branch;
pub mod combine_build;
pub mod combine_castcall;
pub mod combine_casts;
#[cfg(test)]
mod combine_casts_tests;
pub mod combine_compares;
pub mod combine_demanded;
#[cfg(test)]
mod combine_dispatch_tests;
pub mod combine_distribute;
#[cfg(test)]
mod combine_flags_result_tests;
pub mod combine_fp;
pub mod combine_funnel;
pub mod combine_gep;
pub mod combine_gep_base;
pub mod combine_gep_index;
pub mod combine_gep_merge;
pub mod combine_gep_phi;
pub mod combine_icmp_binop;
pub mod combine_icmp_const;
pub mod combine_icmp_logic;
pub mod combine_icmp_ranges;
pub mod combine_invert;
pub mod combine_loadstore;
pub mod combine_memintrinsics;
pub mod combine_minmax;
pub mod combine_muldivrem;
#[cfg(test)]
mod combine_muldivrem_tests;
pub mod combine_narrow;
pub mod combine_negator;
pub mod combine_not;
pub mod combine_or;
pub mod combine_pass;
pub mod combine_phi;
pub mod combine_phi_args;
pub mod combine_select;
pub mod combine_select_bit_test;
pub mod combine_select_bools;
pub mod combine_select_ext_const;
pub mod combine_select_into_op;
pub mod combine_select_op_op;
pub mod combine_select_spf;
#[cfg(test)]
mod combine_semantics_tests;
pub mod combine_shared;
pub mod combine_shift_binop;
pub mod combine_shifts;
pub mod combine_sink;
#[cfg(test)]
mod combine_sink_tests;
pub mod combine_sub;
pub mod combine_visit_select;
pub mod combine_xor;
pub mod condstores;
pub mod condtocond;
pub mod congruentiv;
pub mod constmerge;
pub mod constraintelim;
pub mod constraintelim_induction;
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
pub mod deadstore;
pub mod decide;
pub mod dfajumpthreading;
#[cfg(test)]
mod dfajumpthreading_tests;
pub mod divrempairs;
pub mod earlycse;
pub mod edges;
pub mod emptyblocks;
pub mod exitne;
pub mod exitopt;
pub mod exitpredicate;
pub mod exitsink;
pub mod exitvalues;
pub mod expand;
pub mod expansion;
pub mod extload;
pub mod extpromotion;
pub mod fixednarrow;
pub mod float2int;
pub mod floatfold;
pub mod floatloop;
pub mod fold;
pub mod gepoffset;
pub mod globaldce;
#[cfg(test)]
mod globaldce_tests;
pub mod globalopt;
pub mod globalsra;
pub mod gvn;
#[cfg(test)]
mod gvn_load_tests;
#[cfg(test)]
mod gvn_scalar_tests;
pub mod hoistcommon;
pub mod homes;
pub mod icmpchain;
pub mod indvars;
pub mod inferalignment;
pub mod inferfunctionattrs;
pub mod inferspace;
#[cfg(test)]
mod inferspace_tests;
pub mod inline;
pub mod inlinecost;
#[cfg(test)]
mod inlinecost_tests;
pub mod instsimplify;
pub mod instworklist;
#[cfg(test)]
mod instworklist_tests;
pub mod interprocedural;
pub mod ipacp;
pub mod ipsccp;
pub mod ivusers;
#[cfg(test)]
mod ivusers_tests;
pub mod jumptableswitch;
pub mod jumpthreading;
#[cfg(test)]
mod jumpthreading_tests;
pub mod knownpred;
pub mod lcssa;
pub mod lcssamerges;
pub mod lftr;
pub mod licm;
pub mod local;
#[cfg(test)]
mod local_tests;
pub mod loopclone;
pub mod loopdeletion;
pub mod loopfilter;
pub mod loopidiom;
pub mod loopidioms;
pub mod loopinstsimplify;
pub mod loopinterchange;
pub mod loopinterchange_transform;
pub mod looploadelim;
pub mod looppeel;
pub mod looprotate;
pub mod loopsimplify;
pub mod loopsimplifycfg;
pub mod loopunroll;
pub mod loopunrollandjam;
pub mod loopunrollpass;
pub mod loopunrollruntime;
pub mod loopversioning;
pub mod lowerconstantintrinsics;
pub mod lowerexpect;
pub mod lsr_chain;
pub mod lsr_collect;
#[cfg(test)]
mod lsr_collect_tests;
pub mod lsr_formula;
#[cfg(test)]
mod lsr_formula_tests;
pub mod lsr_generate;
#[cfg(test)]
mod lsr_generate_tests;
pub mod lsr_implement;
pub mod lsr_narrow;
pub mod lsr_pass;
#[cfg(test)]
mod lsr_pass_tests;
pub mod lsr_prepare;
pub mod lsr_solve;
pub mod lsr_use;
#[cfg(test)]
mod lsr_use_tests;
pub mod mem2reg;
pub mod memcpyopt;
pub mod memcpyoptimizer;
pub mod memsetpattern;
#[cfg(test)]
mod memsetpattern_tests;
pub mod mergestores;
pub mod mldstmotion;
pub mod narrowspace;
#[cfg(test)]
mod narrowspace_tests;
pub mod parity;
#[cfg(test)]
mod parity_tests;
#[cfg(test)]
mod pin_slots_tests;
pub mod pipeline;
#[cfg(test)]
mod pipeline_ported_tests;
pub mod ports;
pub mod predicateinfo;
pub mod profit;
pub mod promote;
pub mod reassociate;
pub mod reassociate_fneg;
pub mod reassociate_forms;
pub mod reassociate_fp;
pub mod reassociate_inst;
pub mod reassociate_negate;
pub mod reassociate_optimize;
pub mod reassociate_pairs;
pub mod reassociate_rank;
pub mod reassociate_tree;
pub mod remidiom;
pub mod rotate;
pub mod rpoattrs;
pub mod runtimechecks;
pub mod sccp;
pub mod sccpsolver;
pub mod settled;
pub mod simpleloopunswitch;
pub mod simplifycfg;
pub mod simplifyindvar;
pub mod simplifylibcalls;
pub mod sinkcast;
pub mod sinkcommon;
pub mod slpcompares;
pub mod spares;
pub mod speculate;
pub mod spill;
pub mod splitcopy;
pub mod splitstore;
pub mod sroa;
pub mod switcharms;
pub mod switchcg;
pub mod switchdead;
pub mod switchlookup;
pub mod switchlower;
pub mod switchopt;
pub mod switchphi;
pub mod switchpow2;
pub mod switchrange;
pub mod switchreduce;
pub mod switchselect;
pub mod switchvalue;
pub mod tailcallelim;
pub mod tailmerge;
#[cfg(test)]
pub mod testing;
pub mod transform;
pub mod trivialunswitch;
pub mod trunc_inst_combine;
pub mod twoentryphi;
pub mod typepromotion;
pub mod uncondicmp;
pub mod undefpred;
pub mod uremloop;
pub mod vncoercion;
#[cfg(test)]
mod vncoercion_tests;
pub mod walk;
pub mod widen;
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
