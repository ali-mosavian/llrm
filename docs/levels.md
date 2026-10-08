# Optimization levels

TL;DR: what each `-O` level is, and for each limit its gcc and LLVM counterpart with the source line. `-Omax` is the old `-O3`; `-O0..-O3` are gcc's definitions of the passes this compiler has.

Sources read:

- gcc: `releases/gcc-13.4.0` of the checkout at `/home/alim/work/personal/gcc` (`git show releases/gcc-13.4.0:gcc/opts.cc`, `gcc/params.opt`; `git describe` of the working tree is `basepoints/gcc-17-4628-g416290b10bb`, not used). The host's `gcc` is 13.4.0, the one measured.
- LLVM: the checkout at `/home/alim/work/personal/llvm-project`, `llvmorg-24-init-10533-gd1106deb71cc` (the object database lacks the release tags). The host's `clang` is 20.1.8, the one measured; a value read at 24-init may differ there.
- `gcc -Q --help=optimizers` reports `-funroll-loops [enabled]` at `-O2`, which the source table does not do (no `unroll_loops` row): the table is taken, the listing is not.

## Levels

`driver/flags.rs` `Level::options` -> `llrm-transforms/src/pipeline.rs`:

| level | options | pipeline.rs |
|---|---|---|
| `-O0` | `none()`: no pass runs | 96 |
| `-O1`, `-Og` | `basic()`: the scalar passes and the last call inlined; inline threshold 90; complete copies of a loop must not grow the code; no gcse, sibling calls, fill, peel, unswitch | 101 |
| `-O2` | `standard()`: `-O1` with inlining (threshold 225), gcse, sibling calls, fill | 123 |
| `-O3` | `speed()`: `-O2` with peeling, unswitching, complete copies that may grow the code, inline threshold 250 | 128 |
| `-Omax` | `aggressive()`: every pass on but unswitching, `target_percent` 200, inline threshold 250: the old `-O3` | 133 |
| `-Os` | `size()`: copies must not grow the code; hint and hot inline bonuses off | 115 |
| `-Oz` | `min_size()`: `size()` less `unroll`, `peel` | 125 |

`-Omax` does not turn `unswitch` on; gcc's `-O3` does, and so does ours now.

## Passes: gcc 13.4.0 `default_options_table` (`opts.cc`) against ours

| our option / `-f` name | gcc flag | gcc level (opts.cc line) | ours |
|---|---|---|---|
| `dead` / `tree-dce` | `-ftree-dce` | -O1 (590) | every level above -O0 |
| `promote` / `tree-sra` | `-ftree-sra` | -O1, not -Og (614) | same |
| `drop_stores` / `tree-dse` | `-ftree-dse` | -O1, not -Og (612) | same |
| `hoist` / `move-loop-invariants` | `-fmove-loop-invariants` | -O1, not -Og (607) | same |
| `strength` / `strength-reduce` | `-ftree-slsr` | -O1 (594) | same |
| `inline.last` / `inline-functions-called-once` | `-finline-functions-called-once` | -O1, not -Og (606) | same |
| `inline` / `inline-functions` | `-finline-small-functions`, `-finline-functions` | -O2 (627, 652) | every level above -O0 |
| `forward`, `drop_loads` / `gcse` | `-fgcse` | -O2 (624) | every level above -O0 |
| `sibcalls` / `optimize-sibling-calls` | `-foptimize-sibling-calls` | -O2 (636) | same |
| `fill` / `tree-loop-distribute-patterns` | `-ftree-loop-distribute-patterns` | -O2 (653) | same |
| `unroll` / `unroll-loops` | complete unrolling (`cunroll`) is in the loop passes at every level with loop optimisation; it may *grow* the code only with `-O3`, `-funroll-loops` or `-fpeel-loops` (`opts.cc` 1311-1316, `flag_cunroll_grow_size`) | -O1 and up; may grow at -O3 | -O1 and up, may grow at -O3 (`limits.grows`) |
| `peel` / `peel-loops` | `-fpeel-loops` | -O3 (679) | -O3 |
| `unswitch` / `unswitch-loops` | `-funswitch-loops` | -O3 (685) | -O3 |
| `inline.cp_clone` / `ipa-cp-clone` | `-fipa-cp-clone` | -O3 (676) | -O3: `ipacp` copies a function for the constants a hot call passes |
| (`program_parameters`) | `-fipa-cp` | -O2 (629) | every level above -O0: a constant every call passes is the parameter's value |

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

`Threshold::budget` is `clamp(call_reach / 2, 6, 24) * limit / 225`, compared with the callee's MIR operations (`semantic_count`). So 225 and 250 are ratios (250/225 is LLVM's -O3 over -O2) and the budget is 6 to 24 operations at -O2, 26 at -O3; LLVM's 225 is cost units of about 5 per instruction plus a call penalty of 25. Calibration, `clang -O2 -Rpass=inline` against `LLRM_DEBUG=inline`, on `x*3+1`, a small loop and a loop over an array: ours 2, 8 and 10 operations; LLVM `cost=-25` (threshold 337), `10` and `10` (threshold 225), each after its bonuses, so no per-instruction scale can be read off them. -O1's 90 is 225 x `early-inlining-insns` 6 / `max-inline-insns-auto` 15 (params.opt:129, 545).
