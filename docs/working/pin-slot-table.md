# Pin pipeline slots against llrm's registry

First output of the slot test (funcattrs-port, 2026-10-22): the pin's `-print-pipeline-passes` at -O1, -O2, -O3, -Os
(tests/fixtures/pin-pipeline, tools/pairs/pin_pipeline.py, pin opt md5 e8430a02d9bc7c91c31a8fcb5f37d0a1, LLVM d1106deb71cc, target m32: i386 -march=i486 no SSE).
The pin has no -Os pipeline (`opt` rejects it: O2 with `optsize`), so the -Os fixture is the -O2 list.

Rule: a pin pass gets the phase of the nest it stands in (first top-level function group Early, second Cleanup, the CGSCC's function groups Simplify, later function groups Late, the rest Module), then the first not yet matched group of registry slots of that phase, built at that level by `llvm_order`, whose `llvm` names it, searching forward from the previous match. `inverted` = the only match is before the previous one (ours runs it earlier than the pin). `none` = no registry slot names it. `ours-only` = a built slot group no pin pass was matched to.

| level | match | inverted | none | ours-only |
|---|---|---|---|---|
| -O1 | 28 | 9 | 58 | 9 |
| -O2 | 43 | 4 | 67 | 3 |
| -O3 | 43 | 4 | 70 | 4 |
| -Os | 43 | 4 | 67 | 3 |

Findings, -O2 (the other levels differ by the absent passes at -O1 and the -O3 unswitch):
- The registry names no slot for 11 pin passes of the CGSCC function pipeline (2 more in the early function group: ee-instrument, lower-expect): speculative-execution, jump-table-to-switch, libcalls-shrinkwrap, constraint-elimination and loop-instsimplify/loop-simplifycfg (constraint-elimination, loop-instsimplify, loop-simplifycfg and vector-combine are `Missing` entries in the registry, which the matcher cannot reach: they sit in the Simplify phase but their llvm names are not built, so they read `none` here, as they should), extra-simple-loop-unswitch-passes, vector-combine, mldst-motion, move-auto-init, coro-elide.
- Late (`buildModuleOptimizationPipeline`'s function list): the registry names 7 of its 32 passes (3 match, 4 stand earlier than the pin's). The late `tailcallelim` (after expand-memcmp, before the final simplifycfg), `loop-rotate`/`loop-deletion` of the Late loop nest, `loop-distribute`, `loop-vectorize`, `slp-vectorizer`, `loop-unroll<O2>`, `sroa<preserve-cfg;aggregate-to-vector>`, `infer-alignment`, `alignment-from-assumptions`, `loop-sink`, `div-rem-pairs`, `mergeicmps`, `expand-memcmp`, three late `instcombine` and `vector-combine` have no slot.
- Order: ours runs `licm@1508` first in Late, the pin after loop-interchange; `loop-load-elim`, `instsimplify@1662` likewise stand earlier than the pin's.
- Module phase: the registry knows only `lsr`; the 29 module/CGSCC passes (ipsccp, globalopt x2, inline, function-attrs x2, rpo-function-attrs, deadargelim, globaldce x2, ...) have no registry slot at all; ours runs them in `walk::ModuleIpo`/interprocedural.
- docs/architecture/parity-pipeline-slots.md lists the pin's passes per level but not which of ours stands in each slot, so none of the above shows there; pass-parity.md compares costs against clang 20, not slots.

Table (generated: `LLRM_PIN_SLOT_TABLE=FILE cargo test -p llrm-transforms --lib pin_slot`):

| level | phase | pin slot | nest | state | ours |
|---|---|---|---|---|---|
| -O1 | Early | ee-instrument<> | function<eager-inv> | none |  |
| -O1 | Early | lower-expect | function<eager-inv> | none |  |
| -O1 | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1178:Partial |
| -O1 | Early | sroa<modify-cfg> | function<eager-inv> | match | sroa@1179:Translated |
| -O1 | Early | early-cse<> | function<eager-inv> | match | earlycse@1180:Translated |
| -O1 | Cleanup | mem2reg | function<eager-inv> | match | promote@1241:Translated |
| -O1 | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | match | combine@1242:Partial + fold@1242:Old + dead@1242:Old |
| -O1 | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1244:Partial |
| -O1 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@646:Translated |
| -O1 | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | earlycse@649:Translated |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@675:Partial |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@677:Partial + fold@677:Old + dead@677:Old |
| -O1 | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@690:Partial |
| -O1 | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | reassociate@696:Translated + algebraic@696:Old |
| -O1 | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  |
| -O1 | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  |
| -O1 | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | floatloop@724:Old + licm@724:Translated |
| -O1 | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | rotate@728:Translated + loopsimplify@728:Old + lcssa@728:Old |
| -O1 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | licm@730:Translated |
| -O1 | Simplify | simple-loop-unswitch<no-nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | trivialunswitch@732:Translated |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@766:Partial |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@768:Partial + fold@768:Old + dead@768:Old |
| -O1 | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopidiom@737:Translated + loopidioms@737:Old |
| -O1 | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | indvars@738:Partial |
| -O1 | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopdeletion@749:Translated |
| -O1 | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | unroll@753:Translated + indvars@753:Translated |
| -O1 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@776:Translated |
| -O1 | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | memcpyopt@818:Translated + merge@818:Partial |
| -O1 | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | inverted | sccp@792:Translated |
| -O1 | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | inverted | bdce@797:Translated |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@837:Partial + fold@837:Old + dead@837:Old |
| -O1 | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O1 | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | inverted | adce@815:Translated |
| -O1 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | inverted | decide@832:Partial |
| -O1 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | inverted | combine@801:Partial + fold@801:Old + dead@801:Old |
| -O1 | Simplify |  |  | ours-only | 669: cvp@669 |
| -O1 | Simplify |  |  | ours-only | 764: loopsimplify@764 + lcssa@764 |
| -O1 | Simplify |  |  | ours-only | 772: loopsimplify@772 + lcssa@772 |
| -O1 | Simplify |  |  | ours-only | 787: inferspace@787 |
| -O1 | Simplify |  |  | ours-only | 809: decide@809 |
| -O1 | Simplify |  |  | ours-only | 810: cvp@810 |
| -O1 | Simplify |  |  | ours-only | 820: dse@820 |
| -O1 | Simplify |  |  | ours-only | 823: loopsimplify@823 + lcssa@823 + licm@823 |
| -O1 | Late | drop-unnecessary-assumes | function<eager-inv> | none |  |
| -O1 | Late | float2int | function<eager-inv> | match | float2int@1594:Translated |
| -O1 | Late | lower-constant-intrinsics | function<eager-inv> | none |  |
| -O1 | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | none |  |
| -O1 | Late | loop-deletion | function<eager-inv>/loop | none |  |
| -O1 | Late | loop-interchange | function<eager-inv>/loop | inverted | loop-interchange@1631:Translated |
| -O1 | Late | loop-distribute | function<eager-inv> | none |  |
| -O1 | Late | inject-tli-mappings | function<eager-inv> | none |  |
| -O1 | Late | loop-vectorize<no-interleave-forced-only;vectorize-forced-only;> | function<eager-inv> | none |  |
| -O1 | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | none |  |
| -O1 | Late | infer-alignment | function<eager-inv> | none |  |
| -O1 | Late | loop-load-elim | function<eager-inv> | inverted | loop-load-elim@1400:Partial |
| -O1 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -O1 | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | simplifycfg@1682:Partial |
| -O1 | Late | vector-combine | function<eager-inv> | none |  |
| -O1 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -O1 | Late | loop-unroll<O1> | function<eager-inv> | none |  |
| -O1 | Late | transform-warning | function<eager-inv> | none |  |
| -O1 | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | none |  |
| -O1 | Late | infer-alignment | function<eager-inv> | none |  |
| -O1 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -O1 | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | inverted | licm@1508:Translated |
| -O1 | Late | alignment-from-assumptions | function<eager-inv> | none |  |
| -O1 | Late | loop-sink | function<eager-inv> | none |  |
| -O1 | Late | instsimplify | function<eager-inv> | inverted | instsimplify@1662:Translated |
| -O1 | Late | div-rem-pairs | function<eager-inv> | none |  |
| -O1 | Late | mergeicmps | function<eager-inv> | none |  |
| -O1 | Late | expand-memcmp | function<eager-inv> | none |  |
| -O1 | Late | tailcallelim | function<eager-inv> | none |  |
| -O1 | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | match | simplifycfg@2376:Partial |
| -O1 | Late | annotation-remarks | function | none |  |
| -O1 | Late |  |  | ours-only | 1438: simplifycfg@1438 |
| -O1 | Module | memprof-remove-attributes |  | none |  |
| -O1 | Module | annotation2metadata |  | none |  |
| -O1 | Module | forceattrs |  | none |  |
| -O1 | Module | inferattrs |  | none |  |
| -O1 | Module | coro-early |  | none |  |
| -O1 | Module | openmp-opt |  | none |  |
| -O1 | Module | ipsccp |  | none |  |
| -O1 | Module | called-value-propagation |  | none |  |
| -O1 | Module | globalopt |  | none |  |
| -O1 | Module | always-inline |  | none |  |
| -O1 | Module | inline | cgscc/devirt<4> | none |  |
| -O1 | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | none |  |
| -O1 | Module | function-attrs | cgscc/devirt<4> | none |  |
| -O1 | Module | coro-split | cgscc/devirt<4> | none |  |
| -O1 | Module | coro-annotation-elide | cgscc/devirt<4> | none |  |
| -O1 | Module | deadargelim |  | none |  |
| -O1 | Module | coro-cleanup |  | none |  |
| -O1 | Module | globalopt |  | none |  |
| -O1 | Module | globaldce |  | none |  |
| -O1 | Module | elim-avail-extern |  | none |  |
| -O1 | Module | rpo-function-attrs |  | none |  |
| -O1 | Module | recompute-globalsaa |  | none |  |
| -O1 | Module | alloc-token |  | none |  |
| -O1 | Module | globaldce |  | none |  |
| -O1 | Module | constmerge |  | none |  |
| -O1 | Module | cg-profile |  | none |  |
| -O1 | Module | rel-lookup-table-converter |  | none |  |
| -O1 | Module | lower-comment-string |  | none |  |
| -O2 | Early | ee-instrument<> | function<eager-inv> | none |  |
| -O2 | Early | lower-expect | function<eager-inv> | none |  |
| -O2 | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1178:Partial |
| -O2 | Early | sroa<modify-cfg> | function<eager-inv> | match | sroa@1179:Translated |
| -O2 | Early | early-cse<> | function<eager-inv> | match | earlycse@1180:Translated |
| -O2 | Cleanup | mem2reg | function<eager-inv> | match | promote@1241:Translated |
| -O2 | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | match | combine@1242:Partial + fold@1242:Old + dead@1242:Old |
| -O2 | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1244:Partial |
| -O2 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@646:Translated |
| -O2 | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | earlycse@649:Translated |
| -O2 | Simplify | speculative-execution<only-if-divergent-target> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O2 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@668:Translated |
| -O2 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@669:Translated |
| -O2 | Simplify | jump-table-to-switch | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@675:Partial |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@677:Partial + fold@677:Old + dead@677:Old |
| -O2 | Simplify | aggressive-instcombine | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | aggressive-instcombine@678:Partial |
| -O2 | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O2 | Simplify | tailcallelim | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | tailrec@688:Translated |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@690:Partial |
| -O2 | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | reassociate@696:Translated + algebraic@696:Old |
| -O2 | Simplify | constraint-elimination | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O2 | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  |
| -O2 | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  |
| -O2 | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | floatloop@724:Old + licm@724:Translated |
| -O2 | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | rotate@728:Translated + loopsimplify@728:Old + lcssa@728:Old |
| -O2 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | licm@730:Translated |
| -O2 | Simplify | simple-loop-unswitch<no-nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | trivialunswitch@732:Translated |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@766:Partial |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@768:Partial + fold@768:Old + dead@768:Old |
| -O2 | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopidiom@737:Translated + loopidioms@737:Old |
| -O2 | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | indvars@738:Partial |
| -O2 | Simplify | extra-simple-loop-unswitch-passes | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | none |  |
| -O2 | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopdeletion@749:Translated |
| -O2 | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | unroll@753:Translated + indvars@753:Translated |
| -O2 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@776:Translated |
| -O2 | Simplify | vector-combine | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O2 | Simplify | mldst-motion<no-split-footer-bb> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O2 | Simplify | gvn<> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | inferspace@787:Old + gvn@787:Translated |
| -O2 | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sccp@792:Translated |
| -O2 | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | bdce@797:Translated |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@801:Partial + fold@801:Old + dead@801:Old |
| -O2 | Simplify | dfa-jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dfajumpthreading@806:Translated |
| -O2 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@809:Translated + decide@809:Old |
| -O2 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@810:Translated |
| -O2 | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | adce@815:Translated |
| -O2 | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | memcpyopt@818:Translated + merge@818:Partial |
| -O2 | Simplify | dse | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dse@820:Partial |
| -O2 | Simplify | move-auto-init | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O2 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | loopsimplify@823:Old + lcssa@823:Old + licm@823:Translated |
| -O2 | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O2 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@832:Partial |
| -O2 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@837:Partial + fold@837:Old + dead@837:Old |
| -O2 | Simplify |  |  | ours-only | 764: loopsimplify@764 + lcssa@764 |
| -O2 | Simplify |  |  | ours-only | 772: loopsimplify@772 + lcssa@772 |
| -O2 | Late | drop-unnecessary-assumes | function<eager-inv> | none |  |
| -O2 | Late | float2int | function<eager-inv> | match | float2int@1594:Translated |
| -O2 | Late | lower-constant-intrinsics | function<eager-inv> | none |  |
| -O2 | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | none |  |
| -O2 | Late | loop-deletion | function<eager-inv>/loop | none |  |
| -O2 | Late | loop-interchange | function<eager-inv>/loop | inverted | loop-interchange@1631:Translated |
| -O2 | Late | loop-distribute | function<eager-inv> | none |  |
| -O2 | Late | inject-tli-mappings | function<eager-inv> | none |  |
| -O2 | Late | loop-vectorize<no-interleave-forced-only;no-vectorize-forced-only;> | function<eager-inv> | none |  |
| -O2 | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | none |  |
| -O2 | Late | infer-alignment | function<eager-inv> | none |  |
| -O2 | Late | loop-load-elim | function<eager-inv> | inverted | loop-load-elim@1400:Partial |
| -O2 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -O2 | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | simplifycfg@1682:Partial |
| -O2 | Late | slp-vectorizer | function<eager-inv> | none |  |
| -O2 | Late | vector-combine | function<eager-inv> | none |  |
| -O2 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -O2 | Late | loop-unroll<O2> | function<eager-inv> | none |  |
| -O2 | Late | transform-warning | function<eager-inv> | none |  |
| -O2 | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | none |  |
| -O2 | Late | infer-alignment | function<eager-inv> | none |  |
| -O2 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -O2 | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | inverted | licm@1508:Translated |
| -O2 | Late | alignment-from-assumptions | function<eager-inv> | none |  |
| -O2 | Late | loop-sink | function<eager-inv> | none |  |
| -O2 | Late | instsimplify | function<eager-inv> | inverted | instsimplify@1662:Translated |
| -O2 | Late | div-rem-pairs | function<eager-inv> | none |  |
| -O2 | Late | mergeicmps | function<eager-inv> | none |  |
| -O2 | Late | expand-memcmp | function<eager-inv> | none |  |
| -O2 | Late | tailcallelim | function<eager-inv> | none |  |
| -O2 | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | match | simplifycfg@2376:Partial |
| -O2 | Late | annotation-remarks | function | none |  |
| -O2 | Late |  |  | ours-only | 1438: simplifycfg@1438 |
| -O2 | Module | memprof-remove-attributes |  | none |  |
| -O2 | Module | annotation2metadata |  | none |  |
| -O2 | Module | forceattrs |  | none |  |
| -O2 | Module | inferattrs |  | none |  |
| -O2 | Module | coro-early |  | none |  |
| -O2 | Module | openmp-opt |  | none |  |
| -O2 | Module | ipsccp |  | none |  |
| -O2 | Module | called-value-propagation |  | none |  |
| -O2 | Module | globalopt |  | none |  |
| -O2 | Module | always-inline |  | none |  |
| -O2 | Module | inline | cgscc/devirt<4> | none |  |
| -O2 | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | none |  |
| -O2 | Module | openmp-opt-cgscc | cgscc/devirt<4> | none |  |
| -O2 | Module | function-attrs | cgscc/devirt<4> | none |  |
| -O2 | Module | coro-split | cgscc/devirt<4> | none |  |
| -O2 | Module | coro-annotation-elide | cgscc/devirt<4> | none |  |
| -O2 | Module | deadargelim |  | none |  |
| -O2 | Module | coro-cleanup |  | none |  |
| -O2 | Module | globalopt |  | none |  |
| -O2 | Module | globaldce |  | none |  |
| -O2 | Module | elim-avail-extern |  | none |  |
| -O2 | Module | rpo-function-attrs |  | none |  |
| -O2 | Module | recompute-globalsaa |  | none |  |
| -O2 | Module | alloc-token |  | none |  |
| -O2 | Module | globaldce |  | none |  |
| -O2 | Module | constmerge |  | none |  |
| -O2 | Module | cg-profile |  | none |  |
| -O2 | Module | rel-lookup-table-converter |  | none |  |
| -O2 | Module | lower-comment-string |  | none |  |
| -O3 | Early | ee-instrument<> | function<eager-inv> | none |  |
| -O3 | Early | lower-expect | function<eager-inv> | none |  |
| -O3 | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1178:Partial |
| -O3 | Early | sroa<modify-cfg> | function<eager-inv> | match | sroa@1179:Translated |
| -O3 | Early | early-cse<> | function<eager-inv> | match | earlycse@1180:Translated |
| -O3 | Early | callsite-splitting | function<eager-inv> | none |  |
| -O3 | Cleanup | mem2reg | function<eager-inv> | match | promote@1241:Translated |
| -O3 | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | match | combine@1242:Partial + fold@1242:Old + dead@1242:Old |
| -O3 | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1244:Partial |
| -O3 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@646:Translated |
| -O3 | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | earlycse@649:Translated |
| -O3 | Simplify | speculative-execution<only-if-divergent-target> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O3 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@668:Translated |
| -O3 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@669:Translated |
| -O3 | Simplify | jump-table-to-switch | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@675:Partial |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@677:Partial + fold@677:Old + dead@677:Old |
| -O3 | Simplify | aggressive-instcombine | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | aggressive-instcombine@678:Partial |
| -O3 | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O3 | Simplify | tailcallelim | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | tailrec@688:Translated |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@690:Partial |
| -O3 | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | reassociate@696:Translated + algebraic@696:Old |
| -O3 | Simplify | constraint-elimination | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O3 | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  |
| -O3 | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  |
| -O3 | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | floatloop@724:Old + licm@724:Translated |
| -O3 | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | rotate@728:Translated + loopsimplify@728:Old + lcssa@728:Old |
| -O3 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | licm@730:Translated |
| -O3 | Simplify | simple-loop-unswitch<nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | trivialunswitch@732:Translated |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@766:Partial |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@768:Partial + fold@768:Old + dead@768:Old |
| -O3 | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopidiom@737:Translated + loopidioms@737:Old |
| -O3 | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | indvars@738:Partial |
| -O3 | Simplify | extra-simple-loop-unswitch-passes | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | none |  |
| -O3 | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopdeletion@749:Translated |
| -O3 | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | unroll@753:Translated + indvars@753:Translated |
| -O3 | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@776:Translated |
| -O3 | Simplify | vector-combine | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O3 | Simplify | mldst-motion<no-split-footer-bb> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O3 | Simplify | gvn<> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | inferspace@787:Old + gvn@787:Translated |
| -O3 | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sccp@792:Translated |
| -O3 | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | bdce@797:Translated |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@801:Partial + fold@801:Old + dead@801:Old |
| -O3 | Simplify | dfa-jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dfajumpthreading@806:Translated |
| -O3 | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@809:Translated + decide@809:Old |
| -O3 | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@810:Translated |
| -O3 | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | adce@815:Translated |
| -O3 | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | memcpyopt@818:Translated + merge@818:Partial |
| -O3 | Simplify | dse | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dse@820:Partial |
| -O3 | Simplify | move-auto-init | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O3 | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | loopsimplify@823:Old + lcssa@823:Old + licm@823:Translated |
| -O3 | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -O3 | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@832:Partial |
| -O3 | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@837:Partial + fold@837:Old + dead@837:Old |
| -O3 | Simplify |  |  | ours-only | 764: loopsimplify@764 + lcssa@764 |
| -O3 | Simplify |  |  | ours-only | 772: loopsimplify@772 + lcssa@772 |
| -O3 | Late | drop-unnecessary-assumes | function<eager-inv> | none |  |
| -O3 | Late | float2int | function<eager-inv> | match | float2int@1594:Translated |
| -O3 | Late | lower-constant-intrinsics | function<eager-inv> | none |  |
| -O3 | Late | chr | function<eager-inv> | none |  |
| -O3 | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | none |  |
| -O3 | Late | loop-deletion | function<eager-inv>/loop | none |  |
| -O3 | Late | loop-interchange | function<eager-inv>/loop | inverted | loop-interchange@1631:Translated |
| -O3 | Late | loop-distribute | function<eager-inv> | none |  |
| -O3 | Late | inject-tli-mappings | function<eager-inv> | none |  |
| -O3 | Late | loop-vectorize<no-interleave-forced-only;no-vectorize-forced-only;> | function<eager-inv> | none |  |
| -O3 | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | none |  |
| -O3 | Late | infer-alignment | function<eager-inv> | none |  |
| -O3 | Late | loop-load-elim | function<eager-inv> | inverted | loop-load-elim@1400:Partial |
| -O3 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -O3 | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | simplifycfg@1682:Partial |
| -O3 | Late | slp-vectorizer | function<eager-inv> | none |  |
| -O3 | Late | vector-combine | function<eager-inv> | none |  |
| -O3 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -O3 | Late | loop-unroll<O3> | function<eager-inv> | none |  |
| -O3 | Late | transform-warning | function<eager-inv> | none |  |
| -O3 | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | none |  |
| -O3 | Late | infer-alignment | function<eager-inv> | none |  |
| -O3 | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -O3 | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | inverted | licm@1508:Translated |
| -O3 | Late | alignment-from-assumptions | function<eager-inv> | none |  |
| -O3 | Late | loop-sink | function<eager-inv> | none |  |
| -O3 | Late | instsimplify | function<eager-inv> | inverted | instsimplify@1662:Translated |
| -O3 | Late | div-rem-pairs | function<eager-inv> | none |  |
| -O3 | Late | mergeicmps | function<eager-inv> | none |  |
| -O3 | Late | expand-memcmp | function<eager-inv> | none |  |
| -O3 | Late | tailcallelim | function<eager-inv> | none |  |
| -O3 | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | match | simplifycfg@2376:Partial |
| -O3 | Late | annotation-remarks | function | none |  |
| -O3 | Late |  |  | ours-only | 1438: simplifycfg@1438 |
| -O3 | Late |  |  | ours-only | 1474: loop-unroll-and-jam@1474 |
| -O3 | Module | memprof-remove-attributes |  | none |  |
| -O3 | Module | annotation2metadata |  | none |  |
| -O3 | Module | forceattrs |  | none |  |
| -O3 | Module | inferattrs |  | none |  |
| -O3 | Module | coro-early |  | none |  |
| -O3 | Module | openmp-opt |  | none |  |
| -O3 | Module | ipsccp |  | none |  |
| -O3 | Module | called-value-propagation |  | none |  |
| -O3 | Module | globalopt |  | none |  |
| -O3 | Module | always-inline |  | none |  |
| -O3 | Module | inline | cgscc/devirt<4> | none |  |
| -O3 | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | none |  |
| -O3 | Module | argpromotion | cgscc/devirt<4> | none |  |
| -O3 | Module | openmp-opt-cgscc | cgscc/devirt<4> | none |  |
| -O3 | Module | function-attrs | cgscc/devirt<4> | none |  |
| -O3 | Module | coro-split | cgscc/devirt<4> | none |  |
| -O3 | Module | coro-annotation-elide | cgscc/devirt<4> | none |  |
| -O3 | Module | deadargelim |  | none |  |
| -O3 | Module | coro-cleanup |  | none |  |
| -O3 | Module | globalopt |  | none |  |
| -O3 | Module | globaldce |  | none |  |
| -O3 | Module | elim-avail-extern |  | none |  |
| -O3 | Module | rpo-function-attrs |  | none |  |
| -O3 | Module | recompute-globalsaa |  | none |  |
| -O3 | Module | alloc-token |  | none |  |
| -O3 | Module | globaldce |  | none |  |
| -O3 | Module | constmerge |  | none |  |
| -O3 | Module | cg-profile |  | none |  |
| -O3 | Module | rel-lookup-table-converter |  | none |  |
| -O3 | Module | lower-comment-string |  | none |  |
| -Os | Early | ee-instrument<> | function<eager-inv> | none |  |
| -Os | Early | lower-expect | function<eager-inv> | none |  |
| -Os | Early | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;no-switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1178:Partial |
| -Os | Early | sroa<modify-cfg> | function<eager-inv> | match | sroa@1179:Translated |
| -Os | Early | early-cse<> | function<eager-inv> | match | earlycse@1180:Translated |
| -Os | Cleanup | mem2reg | function<eager-inv> | match | promote@1241:Translated |
| -Os | Cleanup | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | match | combine@1242:Partial + fold@1242:Old + dead@1242:Old |
| -Os | Cleanup | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | decide@1244:Partial |
| -Os | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@646:Translated |
| -Os | Simplify | early-cse<memssa> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | earlycse@649:Translated |
| -Os | Simplify | speculative-execution<only-if-divergent-target> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -Os | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@668:Translated |
| -Os | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@669:Translated |
| -Os | Simplify | jump-table-to-switch | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@675:Partial |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@677:Partial + fold@677:Old + dead@677:Old |
| -Os | Simplify | aggressive-instcombine | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | aggressive-instcombine@678:Partial |
| -Os | Simplify | libcalls-shrinkwrap | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -Os | Simplify | tailcallelim | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | tailrec@688:Translated |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@690:Partial |
| -Os | Simplify | reassociate | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | reassociate@696:Translated + algebraic@696:Old |
| -Os | Simplify | constraint-elimination | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -Os | Simplify | loop-instsimplify | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  |
| -Os | Simplify | loop-simplifycfg | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | none |  |
| -Os | Simplify | licm<no-allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | floatloop@724:Old + licm@724:Translated |
| -Os | Simplify | loop-rotate<header-duplication;no-prepare-for-lto;no-check-exit-count> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | rotate@728:Translated + loopsimplify@728:Old + lcssa@728:Old |
| -Os | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | licm@730:Translated |
| -Os | Simplify | simple-loop-unswitch<no-nontrivial;trivial> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | trivialunswitch@732:Translated |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;no-switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@766:Partial |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@768:Partial + fold@768:Old + dead@768:Old |
| -Os | Simplify | loop-idiom | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopidiom@737:Translated + loopidioms@737:Old |
| -Os | Simplify | indvars | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | indvars@738:Partial |
| -Os | Simplify | extra-simple-loop-unswitch-passes | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | none |  |
| -Os | Simplify | loop-deletion | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | loopdeletion@749:Translated |
| -Os | Simplify | loop-unroll-full | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop | match | unroll@753:Translated + indvars@753:Translated |
| -Os | Simplify | sroa<modify-cfg> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sroa@776:Translated |
| -Os | Simplify | vector-combine | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -Os | Simplify | mldst-motion<no-split-footer-bb> | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -Os | Simplify | gvn<> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | inferspace@787:Old + gvn@787:Translated |
| -Os | Simplify | sccp | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | sccp@792:Translated |
| -Os | Simplify | bdce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | bdce@797:Translated |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@801:Partial + fold@801:Old + dead@801:Old |
| -Os | Simplify | dfa-jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dfajumpthreading@806:Translated |
| -Os | Simplify | jump-threading | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | jumpthreading@809:Translated + decide@809:Old |
| -Os | Simplify | correlated-propagation | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | cvp@810:Translated |
| -Os | Simplify | adce | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | adce@815:Translated |
| -Os | Simplify | memcpyopt | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | memcpyopt@818:Translated + merge@818:Partial |
| -Os | Simplify | dse | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | dse@820:Partial |
| -Os | Simplify | move-auto-init | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -Os | Simplify | licm<allowspeculation> | cgscc/devirt<4>/function<eager-inv;no-rerun>/loop-mssa | match | loopsimplify@823:Old + lcssa@823:Old + licm@823:Translated |
| -Os | Simplify | coro-elide | cgscc/devirt<4>/function<eager-inv;no-rerun> | none |  |
| -Os | Simplify | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | decide@832:Partial |
| -Os | Simplify | instcombine<max-iterations=1;no-verify-fixpoint> | cgscc/devirt<4>/function<eager-inv;no-rerun> | match | combine@837:Partial + fold@837:Old + dead@837:Old |
| -Os | Simplify |  |  | ours-only | 764: loopsimplify@764 + lcssa@764 |
| -Os | Simplify |  |  | ours-only | 772: loopsimplify@772 + lcssa@772 |
| -Os | Late | drop-unnecessary-assumes | function<eager-inv> | none |  |
| -Os | Late | float2int | function<eager-inv> | match | float2int@1594:Translated |
| -Os | Late | lower-constant-intrinsics | function<eager-inv> | none |  |
| -Os | Late | loop-rotate<header-duplication;no-prepare-for-lto;check-exit-count> | function<eager-inv>/loop | none |  |
| -Os | Late | loop-deletion | function<eager-inv>/loop | none |  |
| -Os | Late | loop-interchange | function<eager-inv>/loop | inverted | loop-interchange@1631:Translated |
| -Os | Late | loop-distribute | function<eager-inv> | none |  |
| -Os | Late | inject-tli-mappings | function<eager-inv> | none |  |
| -Os | Late | loop-vectorize<no-interleave-forced-only;no-vectorize-forced-only;> | function<eager-inv> | none |  |
| -Os | Late | drop-unnecessary-assumes<drop-deref> | function<eager-inv> | none |  |
| -Os | Late | infer-alignment | function<eager-inv> | none |  |
| -Os | Late | loop-load-elim | function<eager-inv> | inverted | loop-load-elim@1400:Partial |
| -Os | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -Os | Late | simplifycfg<bonus-inst-threshold=1;forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;switch-to-lookup;no-keep-loops;hoist-common-insts;no-hoist-loads-stores-with-cond-faulting;sink-common-insts;speculate-blocks;simplify-cond-branch;no-speculate-unpredictables> | function<eager-inv> | match | simplifycfg@1682:Partial |
| -Os | Late | slp-vectorizer | function<eager-inv> | none |  |
| -Os | Late | vector-combine | function<eager-inv> | none |  |
| -Os | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -Os | Late | loop-unroll<O2> | function<eager-inv> | none |  |
| -Os | Late | transform-warning | function<eager-inv> | none |  |
| -Os | Late | sroa<preserve-cfg;aggregate-to-vector> | function<eager-inv> | none |  |
| -Os | Late | infer-alignment | function<eager-inv> | none |  |
| -Os | Late | instcombine<max-iterations=1;no-verify-fixpoint> | function<eager-inv> | none |  |
| -Os | Late | licm<allowspeculation> | function<eager-inv>/loop-mssa | inverted | licm@1508:Translated |
| -Os | Late | alignment-from-assumptions | function<eager-inv> | none |  |
| -Os | Late | loop-sink | function<eager-inv> | none |  |
| -Os | Late | instsimplify | function<eager-inv> | inverted | instsimplify@1662:Translated |
| -Os | Late | div-rem-pairs | function<eager-inv> | none |  |
| -Os | Late | mergeicmps | function<eager-inv> | none |  |
| -Os | Late | expand-memcmp | function<eager-inv> | none |  |
| -Os | Late | tailcallelim | function<eager-inv> | none |  |
| -Os | Late | simplifycfg<bonus-inst-threshold=1;no-forward-switch-cond;switch-range-to-icmp;switch-to-arithmetic;no-switch-to-lookup;keep-loops;no-hoist-common-insts;hoist-loads-stores-with-cond-faulting;no-sink-common-insts;speculate-blocks;simplify-cond-branch;speculate-unpredictables> | function<eager-inv> | match | simplifycfg@2376:Partial |
| -Os | Late | annotation-remarks | function | none |  |
| -Os | Late |  |  | ours-only | 1438: simplifycfg@1438 |
| -Os | Module | memprof-remove-attributes |  | none |  |
| -Os | Module | annotation2metadata |  | none |  |
| -Os | Module | forceattrs |  | none |  |
| -Os | Module | inferattrs |  | none |  |
| -Os | Module | coro-early |  | none |  |
| -Os | Module | openmp-opt |  | none |  |
| -Os | Module | ipsccp |  | none |  |
| -Os | Module | called-value-propagation |  | none |  |
| -Os | Module | globalopt |  | none |  |
| -Os | Module | always-inline |  | none |  |
| -Os | Module | inline | cgscc/devirt<4> | none |  |
| -Os | Module | function-attrs<skip-non-recursive-function-attrs> | cgscc/devirt<4> | none |  |
| -Os | Module | openmp-opt-cgscc | cgscc/devirt<4> | none |  |
| -Os | Module | function-attrs | cgscc/devirt<4> | none |  |
| -Os | Module | coro-split | cgscc/devirt<4> | none |  |
| -Os | Module | coro-annotation-elide | cgscc/devirt<4> | none |  |
| -Os | Module | deadargelim |  | none |  |
| -Os | Module | coro-cleanup |  | none |  |
| -Os | Module | globalopt |  | none |  |
| -Os | Module | globaldce |  | none |  |
| -Os | Module | elim-avail-extern |  | none |  |
| -Os | Module | rpo-function-attrs |  | none |  |
| -Os | Module | recompute-globalsaa |  | none |  |
| -Os | Module | alloc-token |  | none |  |
| -Os | Module | globaldce |  | none |  |
| -Os | Module | constmerge |  | none |  |
| -Os | Module | cg-profile |  | none |  |
| -Os | Module | rel-lookup-table-converter |  | none |  |
| -Os | Module | lower-comment-string |  | none |  |
