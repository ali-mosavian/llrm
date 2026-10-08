# Optimization levels

TL;DR: where each `-O` level stands today, and for each limit its gcc and LLVM counterpart with the source line. `-Omax` is today's `-O3`; PR 1b moves `-O0..-O3` to gcc's definitions.

Sources read:

- gcc: `releases/gcc-13.4.0` of the checkout at `/home/alim/work/personal/gcc` (`git show releases/gcc-13.4.0:gcc/opts.cc`, `gcc/params.opt`; `git describe` of the working tree is `basepoints/gcc-17-4628-g416290b10bb`, not used). The host's `gcc` is 13.4.0, the one measured.
- LLVM: the checkout at `/home/alim/work/personal/llvm-project`, `llvmorg-24-init-10533-gd1106deb71cc` (the object database lacks the release tags). The host's `clang` is 20.1.8, the one measured; a value read at 24-init may differ there.
- `gcc -Q --help=optimizers` reports `-funroll-loops [enabled]` at `-O2`, which the source table does not do (no `unroll_loops` row): the table is taken, the listing is not.

## Levels today

`driver/flags.rs` `Level::options` -> `llrm-transforms/src/pipeline.rs`:

| level | options | pipeline.rs |
|---|---|---|
| `-O0` | `none()`: no pass runs | 96 |
| `-O1`, `-Og` | `basic()`: `default()` less `unroll`, `peel`, `unswitch` | 101 |
| `-O2` | `default()`: every pass on except `unswitch`; `target_percent` 100, inline threshold 225 | 68 |
| `-O3`, `-Omax` | `aggressive()`: `default()` with `target_percent` 200 and inline threshold 250 | 106 |
| `-Os` | `size()`: copies must not grow the code; hint and hot inline bonuses off | 115 |
| `-Oz` | `min_size()`: `size()` less `unroll`, `peel` | 125 |

No level turns `unswitch` on; gcc's `-O3` does.

## Passes: gcc 13.4.0 `default_options_table` (`opts.cc`) against ours

| our option / `-f` name | gcc flag | gcc level (opts.cc line) | ours today |
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
| `unroll` / `unroll-loops` | `-funroll-loops` | no row at any level (a user flag) | -O2 and up |
| `peel` / `peel-loops` | `-fpeel-loops` | -O3 (679) | -O2 and up |
| `unswitch` / `unswitch-loops` | `-funswitch-loops` | -O3 (685) | no level |

## Limits

"Ours" is a file:line in this tree. gcc is `gcc/params.opt` at `releases/gcc-13.4.0` (the line of the `-param=` row), with the -O3 value from `opts.cc` lines 690-694. LLVM is a path under `llvm/lib` at the checkout above.

| limit | ours | gcc | LLVM | verdict for 1b |
|---|---|---|---|---|
| inline threshold, -O2 | 225 `inline.rs:92` | `max-inline-insns-auto` 15 (params.opt:545), `max-inline-insns-single` 70 (557), `early-inlining-insns` 6 (129) | `inline-threshold` 225 `Analysis/InlineCost.cpp:77` | units differ (our MIR cost, not gcc's insns); decided in 1b |
| inline threshold, -O3 | 250 `pipeline.rs:107` | auto 30, single 200, early 14, `inline-min-speedup` 15 (205; 30 below -O3) (`opts.cc` 690-694) | `OptAggressiveThreshold` 250 `include/llvm/Analysis/InlineCost.h:46` | gcc's |
| inline hint | 325 `inline.rs:76` | `inline-heuristics-hint-percent` 200 (201; 600 at -O3) | `inlinehint-threshold` 325 `InlineCost.cpp:81` | LLVM's, no gcc unit |
| locally hot call site | 525 `inline.rs:76` | none | `locally-hot-callsite-threshold` 525 `InlineCost.cpp:125` | same |
| caller growth knee | 250 LIR instructions, 138 MIR operations `inline.rs:129-139` | `large-function-insns` 2700 (373), `large-function-growth` 100 (369) | none | ours is the allocator's measured knee; stays, listed |
| inline frame bytes | 256 `inline.rs:114` | `large-stack-frame` 256 (377), `large-stack-frame-growth` 1000 (381) | none | same value as gcc; growth cap not modelled |
| called-once body | knee `inline.rs:144` | `max-inline-functions-called-once-insns` 4000 (541) | last-call bonus `TargetTransformInfoImpl.h:98` | stays: allocator knee |
| full unroll iterations | 10 `peelsize.rs:90` | `max-completely-peel-times` 16 (465) | `unroll-max-iteration-count-to-analyze` 10 `LoopUnrollPass.cpp:106` | gcc's 16 |
| full unroll operations | 200 `peelsize.rs:90`, replaced by the target's `unroll_budget` (150 on m32, `opcosts.txt:42`) x `target_percent` | `max-completely-peeled-insns` 200 (469) | `unroll-threshold-default` 150 / `-aggressive` 300 `LoopUnrollPass.cpp:168-176` | gcc's 200 against the target's 150: decided in 1b |
| unroll boost | 400 `peelsize.rs:62` | none | `unroll-max-percent-threshold-boost` 400 `LoopUnrollPass.cpp:97` | stays |
| pragma unroll | 16384 `peelsize.rs:59` | none | `pragma-unroll-threshold` 16384 `LoopUnrollPass.cpp:145` | stays |
| peel branches | 16 `peelsize.rs:33` | `max-peel-branches` 32 (617) | none | gcc's 32 |
| unroll times (partial) | none | `max-unroll-times` 8 (729), `max-unrolled-insns` 200 (733) | `unroll-partial-threshold` 150 | no partial unroll here: listed |
| peel loop times / insns | none | `max-peel-times` 16 (621), `max-peeled-insns` 100 (625) | none | no counterpart pass here |
| jump threading path | 100 `jumpthread.rs:34` | `max-fsm-thread-path-insns` 100 (513) | none | same |
| jump threading total | 400 `jumpthread.rs:36` | `max-fsm-thread-paths` (not in 13.4.0's params.opt) | none | stays |
| last chance recoloring | depth 5, interference 8 `allocate.rs:1651` | none | `lcr-max-depth` 5, `lcr-max-interf` 8 `CodeGen/RegAllocGreedy.cpp:95,100` | same |
| tail duplication | 2 `jumps.rs:91` | none | `tail-dup-size` 2 `CodeGen/TailDuplicator.cpp:60` | same |
| memset / memcpy expansion | 16 / 8 `isel.rs:419,423` | none | `MaxStoresPerMemset` 16, `MaxStoresPerMemcpy` 8 `Target/X86/X86ISelLowering.cpp:2936,2938` | same |

## Limits with no counterpart (stay as they are)

Our own numbers, not read from gcc or LLVM: `COUNTED_TRIPS`, `MOST_TERMS`, `MOST_DEGREE`, `MOST_NODES`, `ROUNDS`, `SIZE`, `PHIS` (analysis ranges, induction, difference), `MOST_FIELDS` (argpromotion), `TRIED_SITES` (interprocedural), `UNKNOWN_TRIPS` (profit), `allocate::BUDGET`, `Coloring::BUDGET`, `REMEMBERED` (caches), the pass-order and round counts.
