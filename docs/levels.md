# Optimization levels

TL;DR: what each `-O` level is, and for each limit its gcc and LLVM counterpart with the source line. `-Omax` is the old `-O3`; `-O0..-O3` are gcc's definitions of the passes this compiler has.

## Passes by level

`x`: the pass does not run. `on`: it runs, with no level-dependent budget. Else its budget at that level. Names are those of `LLRM_DEBUG=time` (`mir`, `lir`) and of `LLRM_DEBUG=runs` (the interprocedural stages). `Level::options` (`driver/flags.rs`) maps -Og to -O1's. Read from `pipeline.rs` and the backend profile (`size`, `search`, `exhaustive`, `routes`), and checked against `LLRM_DEBUG=time` and `runs` over `bench/*.c` at each level, -m32 and -m16.

| pass | -O0 | -O1 | -O2 | -O3 | -Omax | -Og |
|---|---|---|---|---|---|---|
| *MIR, whole module, before the body loop* | | | | | | |
| `mir available-externally` | on | on | on | on | on | on |
| `mir globalopt` | x | on | on | on | on | on |
| `mir ports` (module, then in the body loop) | x | on | on | on | on | on |
| `mir stamp` | x | on | on | on | on | on |
| *MIR, each body to a fixed point (`mir pipeline`)* | | | | | | |
| `mir sroa` | x | on | on | on | on | on |
| `mir fold` | x | on | on | on | on | on |
| `mir decide` | x | on | on | on | on | on |
| `mir tailrec` | x | x | on | on | on | x |
| `mir loopsimplify` | x | on | on | on | on | on |
| `mir lcssa` | x | on | on | on | on | on |
| `mir floatloop` | x | on | on | on | on | on |
| `mir hoist` | x | on | on | on | on | on |
| `mir loopmotion` | x | on | on | on | on | on |
| `mir trivialunswitch` | x | on | on | on | on | on |
| `mir inferspace` | x | on | on | on | on | on |
| `mir dse` | x | on | on | on | on | on |
| `mir gvn` | x | x | on | on | on | x |
| `mir promote` | x | on | on | on | on | on |
| `mir indvars` | x | on | on | on | on | on |
| `mir algebraic` | x | on | on | on | on | on |
| `mir dead` | x | on | on | on | on | on |
| `mir unroll`: operations, at most 16 iterations | x | 150, no growth | 150, no growth | 150, may grow | 300, may grow | 150, no growth |
| `mir peel`: operations | x | x | x | 150, may grow | 300, may grow | x |
| `mir fill` | x | x | on | on | on | x |
| `mir merge` | x | x | on | on | on | x |
| `mir unswitch` | x | x | x | on | x | x |
| *MIR, whole module (`mir interprocedural`; stages as `LLRM_DEBUG=runs` names them)* | | | | | | |
| `mir interprocedural` | x | on | on | on | on | on |
| `inline`, `ipa-inline`: threshold | x | 90 | 225 | 250 | 250 | 90 |
| `inline`: body budget, operations (m32 / m16) | x | 2 / 3 | 6 / 9 | 6 / 10 | 6 / 10 | 2 / 3 |
| `inline`: hint / hot-call threshold | x | 325 / 525 | 325 / 525 | 325 / 525 | 325 / 525 | 325 / 525 |
| `inline`: function called once | x | on | on | on | on | on |
| `inline-trial`: clocks an inline must save per byte it adds | x | 16 | 16 | 16 | 16 | 16 |
| `promote.` (argpromotion) | x | on | on | on | on | on |
| `narrow.` (narrowspace) | x | on | on | on | on | on |
| `ipa<N>.` (constant returns) | x | on | on | on | on | on |
| `ipa-args` (constant parameters) | x | on | on | on | on | on |
| `ipa-cp.` (clone for constants, growth allowed) | x | x | x | on | on | x |
| `ipa-range` | x | x | on | on | on | x |
| `ipa-deadargs` | x | on | on | on | on | on |
| `ipa-recursive`: body budget, operations (m32 / m16); depth 8, size 450 | x | 2 / 3 | 6 / 9 | 6 / 10 | 6 / 10 | 2 / 3 |
| `ipa-pure` | x | on | on | on | on | on |
| `ipa-noreturn` | x | on | on | on | on | on |
| *MIR, after the interprocedural step* | | | | | | |
| `mir globaldce` | x | on | on | on | on | on |
| `mir calleepop` | x | on | on | on | on | on |
| `mir fixednarrow` | x | on | on | on | on | on |
| `mir lsr`: ivopts groups / all-candidates / always-prune bounds | x | 250 / 40 / 10 | 250 / 40 / 10 | 250 / 40 / 10 | none | 250 / 40 / 10 |
| `mir differences` | x | on | on | on | on | on |
| `mir window` | x | on | on | on | on | on |
| `mir rotate` | x | on | on | on | on | on |
| `mir jumpthread` | x | on | on | on | on | on |
| `mir gepoffset` | x | on | on | on | on | on |
| `mir addresssink` | x | on | on | on | on | on |
| `mir spares` | x | on | on | on | on | on |
| `mir homes` | x | on | on | on | on | on |
| *MIR, lowering for selection* | | | | | | |
| `mir assumptions` | on | on | on | on | on | on |
| `mir ehprepare` | on | on | on | on | on | on |
| `mir fp to unsigned` | on | on | on | on | on | on |
| `mir selects` | on | on | on | on | on | on |
| `mir near code` | on | on | on | on | on | on |
| *Backend (LIR), in order* | | | | | | |
| `isel` | on | on | on | on | on | on |
| Second selection and machine run without callee facts, cheaper kept | x | x | x | x | on | x |
| `lir frame` | on | on | on | on | on | on |
| `lir far-indirect-calls` | on | on | on | on | on | on |
| `lir ssaspill`: the spiller's route, kept if cheaper (`-fallocation-routes`) | x | spill traffic >= 0.5% | spill traffic >= 0.5% | spill traffic >= 0.5% | on | spill traffic >= 0.5% |
| `candidate spiller` | x | spill traffic >= 0.5% | spill traffic >= 0.5% | spill traffic >= 0.5% | on | spill traffic >= 0.5% |
| `candidate cost` | x | spill traffic >= 0.5% | spill traffic >= 0.5% | spill traffic >= 0.5% | on | spill traffic >= 0.5% |
| `candidate allocator alone` | on | on | on | on | on | on |
| `lir phielim` | on | on | on | on | on | on |
| `lir pressuresink` | on | on | on | on | on | on |
| `lir floatfold` | on | on | on | on | on | on |
| `lir floatassign` | on | on | on | on | on | on |
| `lir floatalloc` | on | on | on | on | on | on |
| `lir twoaddr` | on | on | on | on | on | on |
| `lir coalesce` | on | on | on | on | on | on |
| `lir regalloc` | on | on | on | on | on | on |
| `regalloc candidates`, `regalloc trial`: allocations beyond the first (`-fallocation-search`) | x | <= 2 | <= 2 | <= 2 | <= 12 | <= 2 |
| `lir parcopy` | on | on | on | on | on | on |
| `lir peephole` | on | on | on | on | on | on |
| `lir loopslots` | on | on | on | on | on | on |
| `lir schedule` | on | on | on | on | on | on |
| `lir jumps` | on | on | on | on | on | on |
| `lir duplicated returns` | on | on | on | on | on | on |
| `masm return overhead` | on | on | on | on | on | on |
| `masm cleaned returns` | on | on | on | on | on | on |
| `stack checks` | on | on | on | on | on | on |

Unroll and peel budgets are the target's `unroll_budget` (m32 150, m16 200) times `target_percent` (100, -Omax 200). -Os and -Oz are not in the table: -Os is -O2's passes, `peel` on, no copy grows the code, `inline` hint and hot bonuses off, 0 clocks per byte; -Oz is -Os less `unroll` and `peel`. -Omax does not turn `unswitch` on; gcc's -O3 does, and so does ours now.

Not rows: the frontend and link steps (`mir runtime`, `mir link`, `mir verify frontend`), the wrappers `mir pipeline` and `candidate first frame`, the pass manager's own (`mir declared`, `interface`, `invalidate`, `outer analyses`), the analyses, and the allocator's inner steps (`regalloc *`, `spill *`, `split *`, `ssa *`, `siblings *`, `intervals *`, `facts *`); each runs wherever its pass does.

Sources read:

- gcc: `releases/gcc-13.4.0` of the checkout at `/home/alim/work/personal/gcc` (`git show releases/gcc-13.4.0:gcc/opts.cc`, `gcc/params.opt`; `git describe` of the working tree is `basepoints/gcc-17-4628-g416290b10bb`, not used). The host's `gcc` is 13.4.0, the one measured.
- LLVM: the checkout at `/home/alim/work/personal/llvm-project`, `llvmorg-24-init-10533-gd1106deb71cc` (the object database lacks the release tags). The host's `clang` is 20.1.8, the one measured; a value read at 24-init may differ there.
- `gcc -Q --help=optimizers` reports `-funroll-loops [enabled]` at `-O2`, which the source table does not do (no `unroll_loops` row): the table is taken, the listing is not.

## Passes: gcc 13.4.0 `default_options_table` (`opts.cc`) against ours

| our option / `-f` name | gcc flag | gcc level (opts.cc line) | ours |
|---|---|---|---|
| `dead` / `tree-dce` | `-ftree-dce` | -O1 (590) | every level above -O0 |
| `promote` / `tree-sra` | `-ftree-sra` | -O1, not -Og (614) | every level above -O0, -Og too |
| `drop_stores` / `tree-dse` | `-ftree-dse` | -O1, not -Og (612) | every level above -O0, -Og too |
| `hoist` / `move-loop-invariants` | `-fmove-loop-invariants` | -O1, not -Og (607) | every level above -O0, -Og too |
| `strength` / `strength-reduce` | `-ftree-slsr` | -O1 (594) | same |
| `inline.last` / `inline-functions-called-once` | `-finline-functions-called-once` | -O1, not -Og (606) | every level above -O0, -Og too |
| `inline` / `inline-functions` | `-finline-small-functions`, `-finline-functions` | -O2 (627, 652) | every level above -O0 |
| `forward`, `drop_loads` / `gcse` | `-fgcse` | -O2 (624) | every level above -O0 |
| `sibcalls` / `optimize-sibling-calls` | `-foptimize-sibling-calls` | -O2 (636) | same |
| (`jumpthread`) | `-fthread-jumps` | -O1 and up (`OPT_LEVELS_1_PLUS`, 584); `-ftree-dominator-opts` (591) threads in DOM at -O1 too | every level above -O0. Off at -O1 it costs x_switch x1.87 the clocks, queens x1.10, geomean of the 66 +1.1% clocks, code -2.2% (2026-10-09); gcc has it on there. (This row said -O2 until 2026-10-10: `opts.cc` 584 is `OPT_LEVELS_1_PLUS`.) |
| `fill` / `tree-loop-distribute-patterns` | `-ftree-loop-distribute-patterns` | -O2 (653) | same |
| `unroll` / `unroll-loops` | complete unrolling (`cunroll`) is in the loop passes at every level with loop optimisation; it may *grow* the code only with `-O3`, `-funroll-loops` or `-fpeel-loops` (`opts.cc` 1311-1316, `flag_cunroll_grow_size`) | -O1 and up; may grow at -O3 | -O1 and up, may grow at -O3 (`limits.grows`) |
| `peel` / `peel-loops` | `-fpeel-loops` | -O3 (679) | -O3 |
| `unswitch` / `unswitch-loops` | `-funswitch-loops` | -O3 (685) | -O3 |
| `inline.cp_clone` / `ipa-cp-clone` | `-fipa-cp-clone` | -O3 (676) | -O3: `ipacp` copies a function for the constants a hot call passes |
| (`program_parameters`) | `-fipa-cp` | -O2 (629) | every level above -O0: a constant every call passes is the parameter's value |
| `ipa_ranges` / `ipa-vrp` | `-fipa-vrp` | -O2 (633) | -O2 and up; off at -O1 since 2026-10-10 (compile -1.0% on bench + QCport, code +0.00% geomean) |
| (`bounded`, the range analysis) | `-ftree-vrp` | -O2 (649) | every level above -O0, -O1 too: 447 M of d_faces' 34 G at -O1. Not an on/off gate: lsr, indvars, decide and hoist read it, so the plan is to make it demand-driven |
| (`gvn`) | `-ftree-fre` | -O1 (592) | not at -O1: `gvn` is `forward` and `drop_loads` (`-fgcse`), off there, and there is no value-numbering-only mode (#1106). `-ftree-pre` is -O2 (646), `-fcode-hoisting` -O2 (618) |
| (`deadargs`, `interprocedural`) | `-fipa-sra` | -O2 (632) | every level above -O0, -O1 too. Kept: off at -O1, hanoi is +27% code, console.c +1.2% |
| (`lir peephole`) | `-fpeephole2` | -O2 (638) | every level above -O0: the same rounds at -O1 (507 M of d_faces' 34 G) |
| (`lir scheduler`) | `-fschedule-insns2` | -O2 (642) | every level above -O0: `Scheduler` is a phase of every machine (`flow.rs:122`) |
| (`lir jumps`, common tails) | `-fcrossjumping` | -O2 (619) | every level above -O0: 438 M at -O1 |
| (the spiller making a constant or address again where it is read) | `-flra-remat` | -O2 (635) | every level above -O0 (`spiller.rs` `planned_with`) |

## Limits

"Ours" is a file:line in this tree. gcc is `gcc/params.opt` at `releases/gcc-13.4.0` (the line of the `-param=` row), with the -O3 value from `opts.cc` lines 690-694. LLVM is a path under `llvm/lib` at the checkout above.

| limit | ours | gcc | LLVM | verdict for 1b |
|---|---|---|---|---|
| inline threshold, -O2 | 225 `inline.rs:92` (ours: see below) | `max-inline-insns-auto` 15 (params.opt:545), `max-inline-insns-single` 70 (557), `early-inlining-insns` 6 (129) | `inline-threshold` 225 `Analysis/InlineCost.cpp:77` | kept: a ratio, not gcc's unit |
| inline threshold, -O3 | 250 `pipeline.rs:107` | auto 30, single 200, early 14, `inline-min-speedup` 15 (205; 30 below -O3) (`opts.cc` 690-694) | `OptAggressiveThreshold` 250 `include/llvm/Analysis/InlineCost.h:46` | 250, a ratio |
| inline hint | 325 `inline.rs:76` | `inline-heuristics-hint-percent` 200 (201; 600 at -O3) | `inlinehint-threshold` 325 `InlineCost.cpp:81` | LLVM's, no gcc unit |
| locally hot call site | 525 `inline.rs:76` | none | `locally-hot-callsite-threshold` 525 `InlineCost.cpp:125` | same |
| caller growth knee | 250 LIR instructions, 138 MIR operations `inline.rs:129-139` | `large-function-insns` 2700 (373), `large-function-growth` 100 (369) | none | ours is the allocator's measured knee; stays, listed |
| inline frame bytes | 256 `inline.rs:114` | `large-stack-frame` 256 (377), `large-stack-frame-growth` 1000 (381) | none | same value as gcc; growth cap not modelled |
| called-once body | knee `inline.rs:144` | `max-inline-functions-called-once-insns` 4000 (541) | last-call bonus `TargetTransformInfoImpl.h:98` | stays: allocator knee |
| full unroll iterations | 10 `peelsize.rs:90` | `max-completely-peel-times` 16 (465) | `unroll-max-iteration-count-to-analyze` 10 `LoopUnrollPass.cpp:106` | 16 (gcc's), done |
| full unroll operations | 200 `peelsize.rs:90`, replaced by the target's `unroll_budget` (150 on m32, `opcosts.txt:42`) x `target_percent` | `max-completely-peeled-insns` 200 (469) | `unroll-threshold-default` 150 / `-aggressive` 300 `LoopUnrollPass.cpp:168-176` | unchanged: the target's 150 (m32) x `target_percent` |
| full unroll, growth test | copies x 2/3 <= loop `peelsize.rs:147` | `estimated_unrolled_size` takes `unr_insns * 2 / 3` before every comparison (tree-ssa-loop-ivcanon.cc:411) | none | gcc's |
| unroll boost | 400 `peelsize.rs:62` | none | `unroll-max-percent-threshold-boost` 400 `LoopUnrollPass.cpp:97` | stays |
| pragma unroll | 16384 `peelsize.rs:59` | none | `pragma-unroll-threshold` 16384 `LoopUnrollPass.cpp:145` | stays |
| peel branches | 16 `peelsize.rs:33` | `max-peel-branches` 32 (617) | none | 32 (gcc's), done |
| unroll times (partial) | none | `max-unroll-times` 8 (729), `max-unrolled-insns` 200 (733) | `unroll-partial-threshold` 150 | no partial unroll here: listed |
| peel loop times / insns | none | `max-peel-times` 16 (621), `max-peeled-insns` 100 (625) | none | no counterpart pass here |
| jump threading path | 100 `jumpthread.rs:34` | `max-fsm-thread-path-insns` 100 (513) | none | same |
| jump threading, branch path | 7 instructions `jumpthread.rs:122` (15 `:37`, scale 2 `:39`) | `max-jump-thread-duplication-stmts` 15 (params.opt:589) with `fsm-scale-path-stmts` 2 (165): `profitable_path_p` rejects `n * 2 >= 15` (tree-ssa-threadbackward.cc) | `jump-threading-threshold` 6 `Scalar/JumpThreading.cpp:88` | gcc's; phis other than the state's count 1 each, as gcc counts them |
| jump threading, branch at -Os | 0 copies `jumpthread.rs` | none copied unless every statement of the block dies (tree-ssa-threadupdate.cc:2077) | 3 at minsize (`JumpThreading.cpp:310`) | gcc's: 0 |
| jump threading into a loop through its header | not done | `thread_through_loop_header` (tree-ssa-threadupdate.cc:1712) only for two idioms | none | left to `Rotate` |
| jump threading total | 400 `jumpthread.rs:36` | `max-fsm-thread-paths` (not in 13.4.0's params.opt) | none | stays |
| recursive inlining depth | 8 `inline.rs` (`RECURSIVE_DEPTH`) | `max-inline-recursive-depth-auto` 8 (params.opt:573) | none | gcc's |
| recursive inlining size | 450 operations `inline.rs` (`RECURSIVE_SIZE`) | `max-inline-insns-recursive-auto` 450 (:553) | none | gcc's number in our operations |
| recursive inlining probability | 10% `inline.rs` (`RECURSIVE_PROBABILITY`) | `min-inline-recursive-probability` 10 (:769) | none | gcc's |
| ipa-cp evaluation | 500 `ipacp.rs:32` | `ipa-cp-eval-threshold` 500 (params.opt:217) | none | gcc's; benefit x frequency x 1000 / size, benefit from our clocks |
| ipa-cp clones of a function | 8 `ipacp.rs:34` | `ipa-cp-max-recursive-depth` 8 (225), `ipa-cp-value-list-size` 8 (253) | none | gcc's, one cap for both |
| ipa-cp recursion penalty | 40% `ipacp.rs:36` | `ipa-cp-recursion-penalty` 40 (237) | none | gcc's |
| ipa-cp unit growth | 10% of the unit, 16000 large `ipacp.rs:38` | `ipa-cp-unit-growth` 10 (245), `ipa-cp-large-unit-insns` 16000 (249) | none | gcc's |
| ipa-cp hot call | a call in `main` only in a loop (frequency 1.5) `ipacp.rs` | `cgraph_edge::maybe_hot_p`, `ipcp_cloning_candidate_p` ("no hot calls") | none | gcc's |
| last chance recoloring | depth 5, interference 8 `allocate.rs:1651` | none | `lcr-max-depth` 5, `lcr-max-interf` 8 `CodeGen/RegAllocGreedy.cpp:95,100` | same |
| tail duplication | 2 `jumps.rs:91` | none | `tail-dup-size` 2 `CodeGen/TailDuplicator.cpp:60` | same |
| memset / memcpy expansion | 16 / 8 `isel.rs:419,423` | none | `MaxStoresPerMemset` 16, `MaxStoresPerMemcpy` 8 `Target/X86/X86ISelLowering.cpp:2936,2938` | same |

## Limits with no counterpart (stay as they are)

Our own numbers, not read from gcc or LLVM: `COUNTED_TRIPS`, `MOST_TERMS`, `MOST_DEGREE`, `MOST_NODES`, `ROUNDS`, `SIZE`, `PHIS` (analysis ranges, induction, difference), `MOST_FIELDS` (argpromotion), `TRIED_SITES` (interprocedural), `UNKNOWN_TRIPS` (profit), `allocate::BUDGET`, `Coloring::BUDGET`, `REMEMBERED` (caches), the pass-order and round counts.

## The inline threshold is ours

`Threshold::budget` is `clamp(call_reach / 2, 6, 24) * limit / 225`, compared with the callee's MIR operations (`semantic_count`). So 225 and 250 are ratios (250/225 is LLVM's -O3 over -O2) and the budget is 6 to 24 operations at -O2, 26 at -O3 where `call_reach` allows (m32's is 3, m16's 18: 6 and 9 at -O2, 6 and 10 at -O3, 2 and 3 at -O1); LLVM's 225 is cost units of about 5 per instruction plus a call penalty of 25. Calibration, `clang -O2 -Rpass=inline` against `LLRM_DEBUG=inline`, on `x*3+1`, a small loop and a loop over an array: ours 2, 8 and 10 operations; LLVM `cost=-25` (threshold 337), `10` and `10` (threshold 225), each after its bonuses, so no per-instruction scale can be read off them. -O1's 90 is 225 x `early-inlining-insns` 6 / `max-inline-insns-auto` 15 (params.opt:129, 545).

## Register allocation by level: gcc's IRA against ours

gcc's settings (`toplev.cc`, `ira.cc`):

| level | `ira_conflicts_p` | `-fira-region` | `-fcaller-saves` |
|---|---|---|---|
| -O0 | off (`fast_allocation`) | one | off |
| -O1 | on | mixed | off |
| -O2, -O3 | on | mixed | on |
| -Os | on | one | on |

We match -O0 only: `Options::none()` has no allocation search, so a function is allocated once, through the allocator alone (compile -51% on QCport `d_faces`, -48% on `d_alias`, bytes +0.02%).

-O1 equals -O2 in gcc's allocator, so there is nothing to match: our -O1 costs more than gcc's because the allocator runs on a larger body, not because of a policy.

-Os does not match. gcc's one region is the allocator without loop-tree regions; the counterpart here is no live-range splitting. Measured on 131 files: compile -35% (`d_faces`) and -20% (`d_alias`), but bytes +0.49% geomean, worst `pl_trace` +5.8%. Splitting earns its bytes in this allocator, so -Os splits.

The allocator tries other shapes of a body (`-fallocation-search`) at every level but -O0, and every level but -O0 makes each function by the allocator alone and by the spiller's route and keeps the cheaper (`-fallocation-routes`). The two were one switch, and turning both off at -O2 cost x_dct +15% clocks, x_ll_arith +17.6% bytes and recmany +13%: what those rows needed was the route (dct8: spiller 170, allocator alone 214), not a shape. `-fno-allocation-search` now keeps the route: x_dct, x_ll_arith and recmany are byte-identical to the search's, QCport bytes +0.12% (-O1), -0.01% (-O2), +0.10% (-Os), the 66 m32 programs' clocks 0.9987 (-O2) and 0.9994 (-Os) of the search's, and the compile 26-29% shorter (d_faces). It is not the default: the 16-bit bench kernels lose 5-20% of their instructions without it (quicksort -O2 +20%: the `Scoped` shape keeps a loop's address base in a register), 185 of 230 bench measurements worse. gcc runs IRA once at -O1, -O2 and -Os.
