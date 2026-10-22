# Pin pipeline slots against llrm's pipeline

Output of the slot test (funcattrs-port, 2026-10-22; first table 1442783c8e6d, corrected here): the pin's `-print-pipeline-passes` at -O1, -O2, -O3, -Os
(tests/fixtures/pin-pipeline, tools/pairs/pin_pipeline.py; LLVM d1106deb71cc, opt md5 e8430a02d9bc7c91c31a8fcb5f37d0a1; target m32: i386 -march=i486, no SSE).

## Instrument checks

1. Source of each row. Function-list slots (Early, Cleanup, Simplify, Late) come from the REGISTRY (`parity::REGISTRY`, the slots `llvm_order` builds at that level). Module-level passes are not in the registry, so the first table read them all `none`; they are now matched against the EXECUTED order: `--dump` of d_faces -O1, -O2 and -Os (LLRM_* unset, `llrm-c -m32 -march=i486`), whose stage files are numbered in the order the manager ran them: `globalopt, ports, stamp, pipeline (early list), ipo-early, sccp, pipeline (cleanup list), walk, pipeline (late list), interprocedural, writeonly, globaldce, available-externally, calleepop` and then llrm's own backend-facing passes (rotate, lsr, window, ...); -Os drops the rotate/fold/algebraic/indvars/dead/licm rounds. This is also what pipeline.rs `recorded` adds, in that order. Rows of the Module phase say `EXECUTED` and name the stage.
2. -Os. The pin has no -Os pipeline: `opt -passes='default<Os>'` stops with `The optimization level "Os" is no longer supported. Use O2 in conjunction with the optsize attribute instead.` (run at d1106deb71cc). The other route, `clang -Os -mllvm -print-pipeline-passes`, prints the same list as `clang -O2` (cross-checked in the fixture's header, md5 c28bf2ee...). So the -Os fixture is the -O2 list with optsize on the IR, and -Os counts equal -O2's.

## Counts (pin slots)

| level | match | inverted | none | ours-only | none+inverted: N/A | EXECUTED | MISSING | ORDER |
|---|---|---|---|---|---|---|---|---|
| -O1 | 28 | 9 | 58 | 9 | 20 | 12 | 26 | 9 |
| -O2 | 43 | 4 | 67 | 3 | 25 | 12 | 30 | 4 |
| -O3 | 43 | 4 | 70 | 4 | 26 | 12 | 32 | 4 |
| -Os | 43 | 4 | 67 | 3 | 25 | 12 | 30 | 4 |

First table (registry only): -O1 58 none, -O2 67, -O3 70, -Os 67. Executed order accounts for the Module rows marked EXECUTED: that many of the none were instrument gaps, not gaps in llrm.

## MISSING rows ranked by effect

`tools/pairs/pin_ablate.py`: the pin's -O2 pipeline (fixture) over 12 bench C programs (crc, nbody, bintree, queens, shellsort, matmul, sieve, quicksort, mandel, fib, hanoi, histo; the pin's clang -O0 IR, m32), with one instance of a pass removed at a time; effect = change in instructions of the pin's output. The instrument sees effects (without gvn +50 instructions in 5 of 12 IRs, indvars +100 in 7, loop-unroll-full +77 in 4, early-cse +63 in 2). Result for the pass instances ours has no slot for (the full list is the table below): late `tailcallelim` +1 instruction in 1 of 12; every other MISSING pass 0 of 12. The corpus is small C without libm, memcmp, assumes or SSE: a 0 is not evidence the pass never matters (QB/QCport/Nib corpora were not measured: the pin cannot parse llrm's QB IR), only that these 12 do not need it. Order for owners: (1) late instcombine/tailcallelim/loop-rotate/loop-unroll/sroa/loop-deletion (the late list is where the pin cleans up after the loop passes; ours runs none of them, measured effect small on this corpus), (2) rpo-function-attrs, argpromotion (-O3), constmerge (-Os data bytes), (3) the rest.

Rows with owner `unassigned` go to the coordinator on 10-26; the test `every_pin_slot_is_matched_or_accounted_for` fails on a slot without a row, on a row nothing needs, and on an inversion not in an ORDER row.

## Table

(generated: `LLRM_PIN_SLOT_TABLE=FILE cargo test -p llrm-transforms --lib pin_slot`)

| level | phase | pin slot | nest | state | ours | row | note | owner |
|---|---|---|---|---|---|---|---|---|
| -O1 | Early | ee-instrument<> | function<eager-inv> | none |  | N/A | -pg/-finstrument-functions: no frontend asks for it |  |
| -O1 | Early | lower-expect | function<eager-inv> | none |  | MISSING | llvm.expect reaches MIR (hir/mir.rs) and nothing lowers it; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1178:Partial |  |  |  |
| -O1 | Early | sroa<modify-cfg> | function<eager-inv> | match | sroa@1179:Translated |  |  |  |
| -O1 | Early | early-cse<> | function<eager-inv> | match | earlycse@1180:Translated |  |  |  |
| -O1 | Cleanup | mem2reg | function<eager-inv> | match | promote@1241:Translated |  |  |  |
| -O1 | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | match | combine@1242:Partial + fold@1242:Old + dead@1242:Old |  |  |  |
| -O1 | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1244:Partial |  |  |  |
| -O1 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@646:Translated |  |  |  |
| -O1 | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | earlycse@649:Translated |  |  |  |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@675:Partial |  |  |  |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@677:Partial + fold@677:Old + dead@677:Old |  |  |  |
| -O1 | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | errno-setting libm calls; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@690:Partial |  |  |  |
| -O1 | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | reassociate@696:Translated + algebraic@696:Old |  |  |  |
| -O1 | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  | MISSING | registry Missing entry; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  | MISSING | registry Missing entry; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | floatloop@724:Old + licm@724:Translated |  |  |  |
| -O1 | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | rotate@728:Translated + loopsimplify@728:Old + lcssa@728:Old |  |  |  |
| -O1 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | licm@730:Translated |  |  |  |
| -O1 | Simplify | simple-loop-unswitch<no-nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | trivialunswitch@732:Translated |  |  |  |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@766:Partial |  |  |  |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@768:Partial + fold@768:Old + dead@768:Old |  |  |  |
| -O1 | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopidiom@737:Translated + loopidioms@737:Old |  |  |  |
| -O1 | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | indvars@738:Partial |  |  |  |
| -O1 | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopdeletion@749:Translated |  |  |  |
| -O1 | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | unroll@753:Translated + indvars@753:Translated |  |  |  |
| -O1 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@776:Translated |  |  |  |
| -O1 | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | memcpyopt@818:Translated + merge@818:Partial |  |  |  |
| -O1 | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | inverted | sccp@792:Translated | ORDER | -O1 only, as adce | unassigned (coordinator, 10-26) |
| -O1 | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | inverted | bdce@797:Translated | ORDER | -O1 only, as adce | unassigned (coordinator, 10-26) |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@837:Partial + fold@837:Old + dead@837:Old |  |  |  |
| -O1 | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | coroutines: no frontend |  |
| -O1 | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | inverted | adce@815:Translated | ORDER | -O1 only: ours runs the O2 order minus the absent passes; the pin's O1 list is its own | unassigned (coordinator, 10-26) |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | inverted | decide@832:Partial | ORDER | -O1 only, as adce | unassigned (coordinator, 10-26) |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | inverted | combine@801:Partial + fold@801:Old + dead@801:Old | ORDER | -O1 only, as adce | unassigned (coordinator, 10-26) |
| -O1 | Simplify |  |  | ours-only | 669: cvp@669 |  |  |  |
| -O1 | Simplify |  |  | ours-only | 764: loopsimplify@764 + lcssa@764 |  |  |  |
| -O1 | Simplify |  |  | ours-only | 772: loopsimplify@772 + lcssa@772 |  |  |  |
| -O1 | Simplify |  |  | ours-only | 787: inferspace@787 |  |  |  |
| -O1 | Simplify |  |  | ours-only | 809: decide@809 |  |  |  |
| -O1 | Simplify |  |  | ours-only | 810: cvp@810 |  |  |  |
| -O1 | Simplify |  |  | ours-only | 820: dse@820 |  |  |  |
| -O1 | Simplify |  |  | ours-only | 823: loopsimplify@823 + lcssa@823 + licm@823 |  |  |  |
| -O1 | Late | drop-unnecessary-assumes | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | float2int | function<eager-inv> | match | float2int@1594:Translated |  |  |  |
| -O1 | Late | lower-constant-intrinsics | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | none |  | MISSING | the late loop nest's rotate; ablation 0 of 12 IRs (the early one: +51 instructions, matched) | unassigned (coordinator, 10-26) |
| -O1 | Late | loop-deletion | function<eager-inv>/loop | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | loop-interchange | function<eager-inv>/loop | inverted | loop-interchange@1631:Translated | ORDER | ours after licm@1508, before the pin's loop-rotate/loop-deletion | unassigned (coordinator, 10-26) |
| -O1 | Late | loop-distribute | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | inject-tli-mappings | function<eager-inv> | none |  | N/A | maps scalar libcalls to vector ones; vectors deferred |  |
| -O1 | Late | loop-vectorize<no-interleave-forced-only;vectorize-forced-only;> | function<eager-inv> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -O1 | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | infer-alignment | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | loop-load-elim | function<eager-inv> | inverted | loop-load-elim@1400:Partial | ORDER | stands earlier than the pin's | unassigned (coordinator, 10-26) |
| -O1 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -O1 | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | simplifycfg@1682:Partial |  |  |  |
| -O1 | Late | vector-combine | function<eager-inv> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -O1 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -O1 | Late | loop-unroll<O1> | function<eager-inv> | none |  | MISSING | the late loop-unroll<O2>; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | transform-warning | function<eager-inv> | none |  | N/A | diagnostic for failed pragmas; no output |  |
| -O1 | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | none |  | MISSING | the late sroa<preserve-cfg;aggregate-to-vector>; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | infer-alignment | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -O1 | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | inverted | licm@1508:Translated | ORDER | ours builds licm@1508 first in the Late list; the pin runs it after loop-interchange/loop-distribute/.. | w-6 |
| -O1 | Late | alignment-from-assumptions | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | loop-sink | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | instsimplify | function<eager-inv> | inverted | instsimplify@1662:Translated | ORDER | stands earlier than the pin's (after the late instcombine runs) | unassigned (coordinator, 10-26) |
| -O1 | Late | div-rem-pairs | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | mergeicmps | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | expand-memcmp | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | tailcallelim | function<eager-inv> | none |  | MISSING | the late run after expand-memcmp; ablation +1 instruction in 1 of 12 IRs | unassigned (coordinator, 10-26) |
| -O1 | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | match | simplifycfg@2376:Partial |  |  |  |
| -O1 | Late | annotation-remarks | function | none |  | N/A | remarks off |  |
| -O1 | Late |  |  | ours-only | 1438: simplifycfg@1438 |  |  |  |
| -O1 | Module | memprof-remove-attributes |  | none |  | N/A | memprof: no profile |  |
| -O1 | Module | annotation2metadata |  | none |  | N/A | no source annotations |  |
| -O1 | Module | forceattrs |  | none |  | N/A | command-line forced attributes: none given |  |
| -O1 | Module | inferattrs |  | none |  | EXECUTED (outside the registry) | stamp (interprocedural::Stamp): states what a body does, llrm's own form |  |
| -O1 | Module | coro-early |  | none |  | N/A | coroutines: no frontend |  |
| -O1 | Module | openmp-opt |  | none |  | N/A | OpenMP: no frontend |  |
| -O1 | Module | ipsccp |  | none |  | EXECUTED (outside the registry) | ipo-early (walk::ModuleIpo: ipa-args, ipa-range) + sccp; pipeline.rs DEVIATE: what flows between bodies is ModuleIpo's |  |
| -O1 | Module | called-value-propagation |  | none |  | MISSING | indirect calls only; not measured | unassigned (coordinator, 10-26) |
| -O1 | Module | globalopt |  | none |  | EXECUTED (outside the registry) | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O1 | Module | always-inline |  | none |  | EXECUTED (outside the registry) | walk (the CGSCC walk's inliner) |  |
| -O1 | Module | inline | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | walk, and interprocedural (inlines again after the clones) |  |
| -O1 | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | interprocedural (states what each body does); not compared pass by pass |  |
| -O1 | Module | function-attrs | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | interprocedural (states what each body does); not compared pass by pass |  |
| -O1 | Module | coro-split | cgscc/devirt<4> | none |  | N/A | coroutines: no frontend |  |
| -O1 | Module | coro-annotation-elide | cgscc/devirt<4> | none |  | N/A | coroutines: no frontend |  |
| -O1 | Module | deadargelim |  | none |  | EXECUTED (outside the registry) | ipo-early (walk::ModuleIpo: ipa-deadargs) |  |
| -O1 | Module | coro-cleanup |  | none |  | N/A | coroutines: no frontend |  |
| -O1 | Module | globalopt |  | none |  | EXECUTED (outside the registry) | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O1 | Module | globaldce |  | none |  | EXECUTED (outside the registry) | globaldce (once; the pin runs it twice) |  |
| -O1 | Module | elim-avail-extern |  | none |  | EXECUTED (outside the registry) | available-externally |  |
| -O1 | Module | rpo-function-attrs |  | none |  | MISSING | no stage of ours is known to do it; unverified | unassigned (coordinator, 10-26) |
| -O1 | Module | recompute-globalsaa |  | none |  | N/A | an analysis recompute; ours holds and freezes GlobalsAA (pipeline.rs) |  |
| -O1 | Module | alloc-token |  | none |  | N/A | clang option, off |  |
| -O1 | Module | globaldce |  | none |  | EXECUTED (outside the registry) | globaldce (once; the pin runs it twice) |  |
| -O1 | Module | constmerge |  | none |  | MISSING | merging identical constants: data bytes at -Os; not measured | unassigned (coordinator, 10-26) |
| -O1 | Module | cg-profile |  | none |  | N/A | profile data |  |
| -O1 | Module | rel-lookup-table-converter |  | none |  | N/A | relative lookup tables are a PIC feature; no PIC targets |  |
| -O1 | Module | lower-comment-string |  | none |  | N/A | #pragma comment strings |  |
| -O2 | Early | ee-instrument<> | function<eager-inv> | none |  | N/A | -pg/-finstrument-functions: no frontend asks for it |  |
| -O2 | Early | lower-expect | function<eager-inv> | none |  | MISSING | llvm.expect reaches MIR (hir/mir.rs) and nothing lowers it; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1178:Partial |  |  |  |
| -O2 | Early | sroa<modify-cfg> | function<eager-inv> | match | sroa@1179:Translated |  |  |  |
| -O2 | Early | early-cse<> | function<eager-inv> | match | earlycse@1180:Translated |  |  |  |
| -O2 | Cleanup | mem2reg | function<eager-inv> | match | promote@1241:Translated |  |  |  |
| -O2 | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | match | combine@1242:Partial + fold@1242:Old + dead@1242:Old |  |  |  |
| -O2 | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1244:Partial |  |  |  |
| -O2 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@646:Translated |  |  |  |
| -O2 | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | earlycse@649:Translated |  |  |  |
| -O2 | Simplify | speculative-execution<only-if-divergent-target> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | runs only on a target with divergent branches; x86 has none |  |
| -O2 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@668:Translated |  |  |  |
| -O2 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@669:Translated |  |  |  |
| -O2 | Simplify | jump-table-to-switch | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@675:Partial |  |  |  |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@677:Partial + fold@677:Old + dead@677:Old |  |  |  |
| -O2 | Simplify | aggressive-instcombine | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | aggressive-instcombine@678:Partial |  |  |  |
| -O2 | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | errno-setting libm calls; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Simplify | tailcallelim | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | tailrec@688:Translated |  |  |  |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@690:Partial |  |  |  |
| -O2 | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | reassociate@696:Translated + algebraic@696:Old |  |  |  |
| -O2 | Simplify | constraint-elimination | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | registry Missing entry; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  | MISSING | registry Missing entry; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  | MISSING | registry Missing entry; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | floatloop@724:Old + licm@724:Translated |  |  |  |
| -O2 | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | rotate@728:Translated + loopsimplify@728:Old + lcssa@728:Old |  |  |  |
| -O2 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | licm@730:Translated |  |  |  |
| -O2 | Simplify | simple-loop-unswitch<no-nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | trivialunswitch@732:Translated |  |  |  |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@766:Partial |  |  |  |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@768:Partial + fold@768:Old + dead@768:Old |  |  |  |
| -O2 | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopidiom@737:Translated + loopidioms@737:Old |  |  |  |
| -O2 | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | indvars@738:Partial |  |  |  |
| -O2 | Simplify | extra-simple-loop-unswitch-passes | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopdeletion@749:Translated |  |  |  |
| -O2 | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | unroll@753:Translated + indvars@753:Translated |  |  |  |
| -O2 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@776:Translated |  |  |  |
| -O2 | Simplify | vector-combine | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -O2 | Simplify | mldst-motion<no-split-footer-bb> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Simplify | gvn<> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | inferspace@787:Old + gvn@787:Translated |  |  |  |
| -O2 | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sccp@792:Translated |  |  |  |
| -O2 | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | bdce@797:Translated |  |  |  |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@801:Partial + fold@801:Old + dead@801:Old |  |  |  |
| -O2 | Simplify | dfa-jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dfajumpthreading@806:Translated |  |  |  |
| -O2 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@809:Translated + decide@809:Old |  |  |  |
| -O2 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@810:Translated |  |  |  |
| -O2 | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | adce@815:Translated |  |  |  |
| -O2 | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | memcpyopt@818:Translated + merge@818:Partial |  |  |  |
| -O2 | Simplify | dse | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dse@820:Partial |  |  |  |
| -O2 | Simplify | move-auto-init | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | needs -ftrivial-auto-var-init metadata; no frontend emits it |  |
| -O2 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | loopsimplify@823:Old + lcssa@823:Old + licm@823:Translated |  |  |  |
| -O2 | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | coroutines: no frontend |  |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@832:Partial |  |  |  |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@837:Partial + fold@837:Old + dead@837:Old |  |  |  |
| -O2 | Simplify |  |  | ours-only | 764: loopsimplify@764 + lcssa@764 |  |  |  |
| -O2 | Simplify |  |  | ours-only | 772: loopsimplify@772 + lcssa@772 |  |  |  |
| -O2 | Late | drop-unnecessary-assumes | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | float2int | function<eager-inv> | match | float2int@1594:Translated |  |  |  |
| -O2 | Late | lower-constant-intrinsics | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | none |  | MISSING | the late loop nest's rotate; ablation 0 of 12 IRs (the early one: +51 instructions, matched) | unassigned (coordinator, 10-26) |
| -O2 | Late | loop-deletion | function<eager-inv>/loop | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | loop-interchange | function<eager-inv>/loop | inverted | loop-interchange@1631:Translated | ORDER | ours after licm@1508, before the pin's loop-rotate/loop-deletion | unassigned (coordinator, 10-26) |
| -O2 | Late | loop-distribute | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | inject-tli-mappings | function<eager-inv> | none |  | N/A | maps scalar libcalls to vector ones; vectors deferred |  |
| -O2 | Late | loop-vectorize<no-interleave-forced-only;no-vectorize-forced-only;> | function<eager-inv> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -O2 | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | infer-alignment | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | loop-load-elim | function<eager-inv> | inverted | loop-load-elim@1400:Partial | ORDER | stands earlier than the pin's | unassigned (coordinator, 10-26) |
| -O2 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -O2 | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | simplifycfg@1682:Partial |  |  |  |
| -O2 | Late | slp-vectorizer | function<eager-inv> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -O2 | Late | vector-combine | function<eager-inv> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -O2 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -O2 | Late | loop-unroll<O2> | function<eager-inv> | none |  | MISSING | the late loop-unroll<O2>; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | transform-warning | function<eager-inv> | none |  | N/A | diagnostic for failed pragmas; no output |  |
| -O2 | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | none |  | MISSING | the late sroa<preserve-cfg;aggregate-to-vector>; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | infer-alignment | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -O2 | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | inverted | licm@1508:Translated | ORDER | ours builds licm@1508 first in the Late list; the pin runs it after loop-interchange/loop-distribute/.. | w-6 |
| -O2 | Late | alignment-from-assumptions | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | loop-sink | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | instsimplify | function<eager-inv> | inverted | instsimplify@1662:Translated | ORDER | stands earlier than the pin's (after the late instcombine runs) | unassigned (coordinator, 10-26) |
| -O2 | Late | div-rem-pairs | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | mergeicmps | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | expand-memcmp | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | tailcallelim | function<eager-inv> | none |  | MISSING | the late run after expand-memcmp; ablation +1 instruction in 1 of 12 IRs | unassigned (coordinator, 10-26) |
| -O2 | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | match | simplifycfg@2376:Partial |  |  |  |
| -O2 | Late | annotation-remarks | function | none |  | N/A | remarks off |  |
| -O2 | Late |  |  | ours-only | 1438: simplifycfg@1438 |  |  |  |
| -O2 | Module | memprof-remove-attributes |  | none |  | N/A | memprof: no profile |  |
| -O2 | Module | annotation2metadata |  | none |  | N/A | no source annotations |  |
| -O2 | Module | forceattrs |  | none |  | N/A | command-line forced attributes: none given |  |
| -O2 | Module | inferattrs |  | none |  | EXECUTED (outside the registry) | stamp (interprocedural::Stamp): states what a body does, llrm's own form |  |
| -O2 | Module | coro-early |  | none |  | N/A | coroutines: no frontend |  |
| -O2 | Module | openmp-opt |  | none |  | N/A | OpenMP: no frontend |  |
| -O2 | Module | ipsccp |  | none |  | EXECUTED (outside the registry) | ipo-early (walk::ModuleIpo: ipa-args, ipa-range) + sccp; pipeline.rs DEVIATE: what flows between bodies is ModuleIpo's |  |
| -O2 | Module | called-value-propagation |  | none |  | MISSING | indirect calls only; not measured | unassigned (coordinator, 10-26) |
| -O2 | Module | globalopt |  | none |  | EXECUTED (outside the registry) | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O2 | Module | always-inline |  | none |  | EXECUTED (outside the registry) | walk (the CGSCC walk's inliner) |  |
| -O2 | Module | inline | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | walk, and interprocedural (inlines again after the clones) |  |
| -O2 | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | interprocedural (states what each body does); not compared pass by pass |  |
| -O2 | Module | openmp-opt-cgscc | cgscc/devirt<4> | none |  | N/A | OpenMP: no frontend |  |
| -O2 | Module | function-attrs | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | interprocedural (states what each body does); not compared pass by pass |  |
| -O2 | Module | coro-split | cgscc/devirt<4> | none |  | N/A | coroutines: no frontend |  |
| -O2 | Module | coro-annotation-elide | cgscc/devirt<4> | none |  | N/A | coroutines: no frontend |  |
| -O2 | Module | deadargelim |  | none |  | EXECUTED (outside the registry) | ipo-early (walk::ModuleIpo: ipa-deadargs) |  |
| -O2 | Module | coro-cleanup |  | none |  | N/A | coroutines: no frontend |  |
| -O2 | Module | globalopt |  | none |  | EXECUTED (outside the registry) | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O2 | Module | globaldce |  | none |  | EXECUTED (outside the registry) | globaldce (once; the pin runs it twice) |  |
| -O2 | Module | elim-avail-extern |  | none |  | EXECUTED (outside the registry) | available-externally |  |
| -O2 | Module | rpo-function-attrs |  | none |  | MISSING | no stage of ours is known to do it; unverified | unassigned (coordinator, 10-26) |
| -O2 | Module | recompute-globalsaa |  | none |  | N/A | an analysis recompute; ours holds and freezes GlobalsAA (pipeline.rs) |  |
| -O2 | Module | alloc-token |  | none |  | N/A | clang option, off |  |
| -O2 | Module | globaldce |  | none |  | EXECUTED (outside the registry) | globaldce (once; the pin runs it twice) |  |
| -O2 | Module | constmerge |  | none |  | MISSING | merging identical constants: data bytes at -Os; not measured | unassigned (coordinator, 10-26) |
| -O2 | Module | cg-profile |  | none |  | N/A | profile data |  |
| -O2 | Module | rel-lookup-table-converter |  | none |  | N/A | relative lookup tables are a PIC feature; no PIC targets |  |
| -O2 | Module | lower-comment-string |  | none |  | N/A | #pragma comment strings |  |
| -O3 | Early | ee-instrument<> | function<eager-inv> | none |  | N/A | -pg/-finstrument-functions: no frontend asks for it |  |
| -O3 | Early | lower-expect | function<eager-inv> | none |  | MISSING | llvm.expect reaches MIR (hir/mir.rs) and nothing lowers it; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1178:Partial |  |  |  |
| -O3 | Early | sroa<modify-cfg> | function<eager-inv> | match | sroa@1179:Translated |  |  |  |
| -O3 | Early | early-cse<> | function<eager-inv> | match | earlycse@1180:Translated |  |  |  |
| -O3 | Early | callsite-splitting | function<eager-inv> | none |  | MISSING | -O3 only; not measured | unassigned (coordinator, 10-26) |
| -O3 | Cleanup | mem2reg | function<eager-inv> | match | promote@1241:Translated |  |  |  |
| -O3 | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | match | combine@1242:Partial + fold@1242:Old + dead@1242:Old |  |  |  |
| -O3 | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1244:Partial |  |  |  |
| -O3 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@646:Translated |  |  |  |
| -O3 | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | earlycse@649:Translated |  |  |  |
| -O3 | Simplify | speculative-execution<only-if-divergent-target> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | runs only on a target with divergent branches; x86 has none |  |
| -O3 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@668:Translated |  |  |  |
| -O3 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@669:Translated |  |  |  |
| -O3 | Simplify | jump-table-to-switch | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@675:Partial |  |  |  |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@677:Partial + fold@677:Old + dead@677:Old |  |  |  |
| -O3 | Simplify | aggressive-instcombine | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | aggressive-instcombine@678:Partial |  |  |  |
| -O3 | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | errno-setting libm calls; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Simplify | tailcallelim | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | tailrec@688:Translated |  |  |  |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@690:Partial |  |  |  |
| -O3 | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | reassociate@696:Translated + algebraic@696:Old |  |  |  |
| -O3 | Simplify | constraint-elimination | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | registry Missing entry; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  | MISSING | registry Missing entry; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  | MISSING | registry Missing entry; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | floatloop@724:Old + licm@724:Translated |  |  |  |
| -O3 | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | rotate@728:Translated + loopsimplify@728:Old + lcssa@728:Old |  |  |  |
| -O3 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | licm@730:Translated |  |  |  |
| -O3 | Simplify | simple-loop-unswitch<nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | trivialunswitch@732:Translated |  |  |  |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@766:Partial |  |  |  |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@768:Partial + fold@768:Old + dead@768:Old |  |  |  |
| -O3 | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopidiom@737:Translated + loopidioms@737:Old |  |  |  |
| -O3 | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | indvars@738:Partial |  |  |  |
| -O3 | Simplify | extra-simple-loop-unswitch-passes | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopdeletion@749:Translated |  |  |  |
| -O3 | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | unroll@753:Translated + indvars@753:Translated |  |  |  |
| -O3 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@776:Translated |  |  |  |
| -O3 | Simplify | vector-combine | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -O3 | Simplify | mldst-motion<no-split-footer-bb> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Simplify | gvn<> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | inferspace@787:Old + gvn@787:Translated |  |  |  |
| -O3 | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sccp@792:Translated |  |  |  |
| -O3 | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | bdce@797:Translated |  |  |  |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@801:Partial + fold@801:Old + dead@801:Old |  |  |  |
| -O3 | Simplify | dfa-jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dfajumpthreading@806:Translated |  |  |  |
| -O3 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@809:Translated + decide@809:Old |  |  |  |
| -O3 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@810:Translated |  |  |  |
| -O3 | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | adce@815:Translated |  |  |  |
| -O3 | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | memcpyopt@818:Translated + merge@818:Partial |  |  |  |
| -O3 | Simplify | dse | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dse@820:Partial |  |  |  |
| -O3 | Simplify | move-auto-init | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | needs -ftrivial-auto-var-init metadata; no frontend emits it |  |
| -O3 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | loopsimplify@823:Old + lcssa@823:Old + licm@823:Translated |  |  |  |
| -O3 | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | coroutines: no frontend |  |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@832:Partial |  |  |  |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@837:Partial + fold@837:Old + dead@837:Old |  |  |  |
| -O3 | Simplify |  |  | ours-only | 764: loopsimplify@764 + lcssa@764 |  |  |  |
| -O3 | Simplify |  |  | ours-only | 772: loopsimplify@772 + lcssa@772 |  |  |  |
| -O3 | Late | drop-unnecessary-assumes | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | float2int | function<eager-inv> | match | float2int@1594:Translated |  |  |  |
| -O3 | Late | lower-constant-intrinsics | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | chr | function<eager-inv> | none |  | N/A | control-height-reduction needs profile data |  |
| -O3 | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | none |  | MISSING | the late loop nest's rotate; ablation 0 of 12 IRs (the early one: +51 instructions, matched) | unassigned (coordinator, 10-26) |
| -O3 | Late | loop-deletion | function<eager-inv>/loop | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | loop-interchange | function<eager-inv>/loop | inverted | loop-interchange@1631:Translated | ORDER | ours after licm@1508, before the pin's loop-rotate/loop-deletion | unassigned (coordinator, 10-26) |
| -O3 | Late | loop-distribute | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | inject-tli-mappings | function<eager-inv> | none |  | N/A | maps scalar libcalls to vector ones; vectors deferred |  |
| -O3 | Late | loop-vectorize<no-interleave-forced-only;no-vectorize-forced-only;> | function<eager-inv> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -O3 | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | infer-alignment | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | loop-load-elim | function<eager-inv> | inverted | loop-load-elim@1400:Partial | ORDER | stands earlier than the pin's | unassigned (coordinator, 10-26) |
| -O3 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -O3 | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | simplifycfg@1682:Partial |  |  |  |
| -O3 | Late | slp-vectorizer | function<eager-inv> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -O3 | Late | vector-combine | function<eager-inv> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -O3 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -O3 | Late | loop-unroll<O3> | function<eager-inv> | none |  | MISSING | the late loop-unroll<O2>; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | transform-warning | function<eager-inv> | none |  | N/A | diagnostic for failed pragmas; no output |  |
| -O3 | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | none |  | MISSING | the late sroa<preserve-cfg;aggregate-to-vector>; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | infer-alignment | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -O3 | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | inverted | licm@1508:Translated | ORDER | ours builds licm@1508 first in the Late list; the pin runs it after loop-interchange/loop-distribute/.. | w-6 |
| -O3 | Late | alignment-from-assumptions | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | loop-sink | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | instsimplify | function<eager-inv> | inverted | instsimplify@1662:Translated | ORDER | stands earlier than the pin's (after the late instcombine runs) | unassigned (coordinator, 10-26) |
| -O3 | Late | div-rem-pairs | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | mergeicmps | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | expand-memcmp | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | tailcallelim | function<eager-inv> | none |  | MISSING | the late run after expand-memcmp; ablation +1 instruction in 1 of 12 IRs | unassigned (coordinator, 10-26) |
| -O3 | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | match | simplifycfg@2376:Partial |  |  |  |
| -O3 | Late | annotation-remarks | function | none |  | N/A | remarks off |  |
| -O3 | Late |  |  | ours-only | 1438: simplifycfg@1438 |  |  |  |
| -O3 | Late |  |  | ours-only | 1474: loop-unroll-and-jam@1474 |  |  |  |
| -O3 | Module | memprof-remove-attributes |  | none |  | N/A | memprof: no profile |  |
| -O3 | Module | annotation2metadata |  | none |  | N/A | no source annotations |  |
| -O3 | Module | forceattrs |  | none |  | N/A | command-line forced attributes: none given |  |
| -O3 | Module | inferattrs |  | none |  | EXECUTED (outside the registry) | stamp (interprocedural::Stamp): states what a body does, llrm's own form |  |
| -O3 | Module | coro-early |  | none |  | N/A | coroutines: no frontend |  |
| -O3 | Module | openmp-opt |  | none |  | N/A | OpenMP: no frontend |  |
| -O3 | Module | ipsccp |  | none |  | EXECUTED (outside the registry) | ipo-early (walk::ModuleIpo: ipa-args, ipa-range) + sccp; pipeline.rs DEVIATE: what flows between bodies is ModuleIpo's |  |
| -O3 | Module | called-value-propagation |  | none |  | MISSING | indirect calls only; not measured | unassigned (coordinator, 10-26) |
| -O3 | Module | globalopt |  | none |  | EXECUTED (outside the registry) | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O3 | Module | always-inline |  | none |  | EXECUTED (outside the registry) | walk (the CGSCC walk's inliner) |  |
| -O3 | Module | inline | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | walk, and interprocedural (inlines again after the clones) |  |
| -O3 | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | interprocedural (states what each body does); not compared pass by pass |  |
| -O3 | Module | argpromotion | cgscc/devirt<4> | none |  | MISSING | -O3 only; argpromotion.rs exists, where it runs is not in the registry: unverified | unassigned (coordinator, 10-26) |
| -O3 | Module | openmp-opt-cgscc | cgscc/devirt<4> | none |  | N/A | OpenMP: no frontend |  |
| -O3 | Module | function-attrs | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | interprocedural (states what each body does); not compared pass by pass |  |
| -O3 | Module | coro-split | cgscc/devirt<4> | none |  | N/A | coroutines: no frontend |  |
| -O3 | Module | coro-annotation-elide | cgscc/devirt<4> | none |  | N/A | coroutines: no frontend |  |
| -O3 | Module | deadargelim |  | none |  | EXECUTED (outside the registry) | ipo-early (walk::ModuleIpo: ipa-deadargs) |  |
| -O3 | Module | coro-cleanup |  | none |  | N/A | coroutines: no frontend |  |
| -O3 | Module | globalopt |  | none |  | EXECUTED (outside the registry) | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -O3 | Module | globaldce |  | none |  | EXECUTED (outside the registry) | globaldce (once; the pin runs it twice) |  |
| -O3 | Module | elim-avail-extern |  | none |  | EXECUTED (outside the registry) | available-externally |  |
| -O3 | Module | rpo-function-attrs |  | none |  | MISSING | no stage of ours is known to do it; unverified | unassigned (coordinator, 10-26) |
| -O3 | Module | recompute-globalsaa |  | none |  | N/A | an analysis recompute; ours holds and freezes GlobalsAA (pipeline.rs) |  |
| -O3 | Module | alloc-token |  | none |  | N/A | clang option, off |  |
| -O3 | Module | globaldce |  | none |  | EXECUTED (outside the registry) | globaldce (once; the pin runs it twice) |  |
| -O3 | Module | constmerge |  | none |  | MISSING | merging identical constants: data bytes at -Os; not measured | unassigned (coordinator, 10-26) |
| -O3 | Module | cg-profile |  | none |  | N/A | profile data |  |
| -O3 | Module | rel-lookup-table-converter |  | none |  | N/A | relative lookup tables are a PIC feature; no PIC targets |  |
| -O3 | Module | lower-comment-string |  | none |  | N/A | #pragma comment strings |  |
| -Os | Early | ee-instrument<> | function<eager-inv> | none |  | N/A | -pg/-finstrument-functions: no frontend asks for it |  |
| -Os | Early | lower-expect | function<eager-inv> | none |  | MISSING | llvm.expect reaches MIR (hir/mir.rs) and nothing lowers it; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1178:Partial |  |  |  |
| -Os | Early | sroa<modify-cfg> | function<eager-inv> | match | sroa@1179:Translated |  |  |  |
| -Os | Early | early-cse<> | function<eager-inv> | match | earlycse@1180:Translated |  |  |  |
| -Os | Cleanup | mem2reg | function<eager-inv> | match | promote@1241:Translated |  |  |  |
| -Os | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | match | combine@1242:Partial + fold@1242:Old + dead@1242:Old |  |  |  |
| -Os | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1244:Partial |  |  |  |
| -Os | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@646:Translated |  |  |  |
| -Os | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | earlycse@649:Translated |  |  |  |
| -Os | Simplify | speculative-execution<only-if-divergent-target> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | runs only on a target with divergent branches; x86 has none |  |
| -Os | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@668:Translated |  |  |  |
| -Os | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@669:Translated |  |  |  |
| -Os | Simplify | jump-table-to-switch | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@675:Partial |  |  |  |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@677:Partial + fold@677:Old + dead@677:Old |  |  |  |
| -Os | Simplify | aggressive-instcombine | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | aggressive-instcombine@678:Partial |  |  |  |
| -Os | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | errno-setting libm calls; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Simplify | tailcallelim | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | tailrec@688:Translated |  |  |  |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@690:Partial |  |  |  |
| -Os | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | reassociate@696:Translated + algebraic@696:Old |  |  |  |
| -Os | Simplify | constraint-elimination | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | registry Missing entry; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  | MISSING | registry Missing entry; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  | MISSING | registry Missing entry; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | floatloop@724:Old + licm@724:Translated |  |  |  |
| -Os | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | rotate@728:Translated + loopsimplify@728:Old + lcssa@728:Old |  |  |  |
| -Os | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | licm@730:Translated |  |  |  |
| -Os | Simplify | simple-loop-unswitch<no-nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | trivialunswitch@732:Translated |  |  |  |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@766:Partial |  |  |  |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@768:Partial + fold@768:Old + dead@768:Old |  |  |  |
| -Os | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopidiom@737:Translated + loopidioms@737:Old |  |  |  |
| -Os | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | indvars@738:Partial |  |  |  |
| -Os | Simplify | extra-simple-loop-unswitch-passes | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopdeletion@749:Translated |  |  |  |
| -Os | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | unroll@753:Translated + indvars@753:Translated |  |  |  |
| -Os | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@776:Translated |  |  |  |
| -Os | Simplify | vector-combine | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -Os | Simplify | mldst-motion<no-split-footer-bb> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Simplify | gvn<> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | inferspace@787:Old + gvn@787:Translated |  |  |  |
| -Os | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sccp@792:Translated |  |  |  |
| -Os | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | bdce@797:Translated |  |  |  |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@801:Partial + fold@801:Old + dead@801:Old |  |  |  |
| -Os | Simplify | dfa-jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dfajumpthreading@806:Translated |  |  |  |
| -Os | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@809:Translated + decide@809:Old |  |  |  |
| -Os | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@810:Translated |  |  |  |
| -Os | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | adce@815:Translated |  |  |  |
| -Os | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | memcpyopt@818:Translated + merge@818:Partial |  |  |  |
| -Os | Simplify | dse | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dse@820:Partial |  |  |  |
| -Os | Simplify | move-auto-init | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | needs -ftrivial-auto-var-init metadata; no frontend emits it |  |
| -Os | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | loopsimplify@823:Old + lcssa@823:Old + licm@823:Translated |  |  |  |
| -Os | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  | N/A | coroutines: no frontend |  |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@832:Partial |  |  |  |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@837:Partial + fold@837:Old + dead@837:Old |  |  |  |
| -Os | Simplify |  |  | ours-only | 764: loopsimplify@764 + lcssa@764 |  |  |  |
| -Os | Simplify |  |  | ours-only | 772: loopsimplify@772 + lcssa@772 |  |  |  |
| -Os | Late | drop-unnecessary-assumes | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | float2int | function<eager-inv> | match | float2int@1594:Translated |  |  |  |
| -Os | Late | lower-constant-intrinsics | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | none |  | MISSING | the late loop nest's rotate; ablation 0 of 12 IRs (the early one: +51 instructions, matched) | unassigned (coordinator, 10-26) |
| -Os | Late | loop-deletion | function<eager-inv>/loop | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | loop-interchange | function<eager-inv>/loop | inverted | loop-interchange@1631:Translated | ORDER | ours after licm@1508, before the pin's loop-rotate/loop-deletion | unassigned (coordinator, 10-26) |
| -Os | Late | loop-distribute | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | inject-tli-mappings | function<eager-inv> | none |  | N/A | maps scalar libcalls to vector ones; vectors deferred |  |
| -Os | Late | loop-vectorize<no-interleave-forced-only;no-vectorize-forced-only;> | function<eager-inv> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -Os | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | infer-alignment | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | loop-load-elim | function<eager-inv> | inverted | loop-load-elim@1400:Partial | ORDER | stands earlier than the pin's | unassigned (coordinator, 10-26) |
| -Os | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -Os | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | simplifycfg@1682:Partial |  |  |  |
| -Os | Late | slp-vectorizer | function<eager-inv> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -Os | Late | vector-combine | function<eager-inv> | none |  | N/A | vectors deferred (user ruling 10-20), -sse target |  |
| -Os | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -Os | Late | loop-unroll<O2> | function<eager-inv> | none |  | MISSING | the late loop-unroll<O2>; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | transform-warning | function<eager-inv> | none |  | N/A | diagnostic for failed pragmas; no output |  |
| -Os | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | none |  | MISSING | the late sroa<preserve-cfg;aggregate-to-vector>; ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | infer-alignment | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  | MISSING | the three late runs; ours has no combine slot in the late list | unassigned (coordinator, 10-26) |
| -Os | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | inverted | licm@1508:Translated | ORDER | ours builds licm@1508 first in the Late list; the pin runs it after loop-interchange/loop-distribute/.. | w-6 |
| -Os | Late | alignment-from-assumptions | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | loop-sink | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | instsimplify | function<eager-inv> | inverted | instsimplify@1662:Translated | ORDER | stands earlier than the pin's (after the late instcombine runs) | unassigned (coordinator, 10-26) |
| -Os | Late | div-rem-pairs | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | mergeicmps | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | expand-memcmp | function<eager-inv> | none |  | MISSING | ablation 0 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | tailcallelim | function<eager-inv> | none |  | MISSING | the late run after expand-memcmp; ablation +1 instruction in 1 of 12 IRs | unassigned (coordinator, 10-26) |
| -Os | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | match | simplifycfg@2376:Partial |  |  |  |
| -Os | Late | annotation-remarks | function | none |  | N/A | remarks off |  |
| -Os | Late |  |  | ours-only | 1438: simplifycfg@1438 |  |  |  |
| -Os | Module | memprof-remove-attributes |  | none |  | N/A | memprof: no profile |  |
| -Os | Module | annotation2metadata |  | none |  | N/A | no source annotations |  |
| -Os | Module | forceattrs |  | none |  | N/A | command-line forced attributes: none given |  |
| -Os | Module | inferattrs |  | none |  | EXECUTED (outside the registry) | stamp (interprocedural::Stamp): states what a body does, llrm's own form |  |
| -Os | Module | coro-early |  | none |  | N/A | coroutines: no frontend |  |
| -Os | Module | openmp-opt |  | none |  | N/A | OpenMP: no frontend |  |
| -Os | Module | ipsccp |  | none |  | EXECUTED (outside the registry) | ipo-early (walk::ModuleIpo: ipa-args, ipa-range) + sccp; pipeline.rs DEVIATE: what flows between bodies is ModuleIpo's |  |
| -Os | Module | called-value-propagation |  | none |  | MISSING | indirect calls only; not measured | unassigned (coordinator, 10-26) |
| -Os | Module | globalopt |  | none |  | EXECUTED (outside the registry) | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -Os | Module | always-inline |  | none |  | EXECUTED (outside the registry) | walk (the CGSCC walk's inliner) |  |
| -Os | Module | inline | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | walk, and interprocedural (inlines again after the clones) |  |
| -Os | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | interprocedural (states what each body does); not compared pass by pass |  |
| -Os | Module | openmp-opt-cgscc | cgscc/devirt<4> | none |  | N/A | OpenMP: no frontend |  |
| -Os | Module | function-attrs | cgscc/devirt<4> | none |  | EXECUTED (outside the registry) | interprocedural (states what each body does); not compared pass by pass |  |
| -Os | Module | coro-split | cgscc/devirt<4> | none |  | N/A | coroutines: no frontend |  |
| -Os | Module | coro-annotation-elide | cgscc/devirt<4> | none |  | N/A | coroutines: no frontend |  |
| -Os | Module | deadargelim |  | none |  | EXECUTED (outside the registry) | ipo-early (walk::ModuleIpo: ipa-deadargs) |  |
| -Os | Module | coro-cleanup |  | none |  | N/A | coroutines: no frontend |  |
| -Os | Module | globalopt |  | none |  | EXECUTED (outside the registry) | globalopt (first), writeonly, calleepop: a subset of GlobalOpt |  |
| -Os | Module | globaldce |  | none |  | EXECUTED (outside the registry) | globaldce (once; the pin runs it twice) |  |
| -Os | Module | elim-avail-extern |  | none |  | EXECUTED (outside the registry) | available-externally |  |
| -Os | Module | rpo-function-attrs |  | none |  | MISSING | no stage of ours is known to do it; unverified | unassigned (coordinator, 10-26) |
| -Os | Module | recompute-globalsaa |  | none |  | N/A | an analysis recompute; ours holds and freezes GlobalsAA (pipeline.rs) |  |
| -Os | Module | alloc-token |  | none |  | N/A | clang option, off |  |
| -Os | Module | globaldce |  | none |  | EXECUTED (outside the registry) | globaldce (once; the pin runs it twice) |  |
| -Os | Module | constmerge |  | none |  | MISSING | merging identical constants: data bytes at -Os; not measured | unassigned (coordinator, 10-26) |
| -Os | Module | cg-profile |  | none |  | N/A | profile data |  |
| -Os | Module | rel-lookup-table-converter |  | none |  | N/A | relative lookup tables are a PIC feature; no PIC targets |  |
| -Os | Module | lower-comment-string |  | none |  | N/A | #pragma comment strings |  |
