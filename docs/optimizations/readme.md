# Optimizations

How the compiler turns MIR into x86 code: the SSA optimizer in `crates/opt`, the backend in `crates/codegen/llrm-codegen`, and one reference per pass family. For contributors.

## Reference

The reference for every pass, analysis and backend phase is the `//!` header at the top of its source file: what it does, how, where it runs, what it needs and keeps. `python3 tools/docs/docsbuild/build.py --sync` writes one page per header, with an index for each directory: [passes](passes/readme.md), [analyses](analyses/readme.md) and [backend phases](backend/readme.md). The tests fail when a page is stale, or when a file of a required directory has no header.

| Source | Holds |
|---|---|
| [`crates/opt/llrm-transforms/src`](https://github.com/ali-mosavian/llrm/tree/main/crates/opt/llrm-transforms/src) | The MIR passes and the pipeline that orders them |
| [`crates/opt/llrm-analysis/src`](https://github.com/ali-mosavian/llrm/tree/main/crates/opt/llrm-analysis/src) | The analyses and the pass manager |
| [`crates/codegen/llrm-codegen/src/backend`](https://github.com/ali-mosavian/llrm/tree/main/crates/codegen/llrm-codegen/src/backend) | Lowering for selection, instruction selection and the machine phases |

## Pass index, in pipeline order

Levels and flags: [levels](../reference/levels.md).

| Stage | Passes | Notes |
|---|---|---|
| Module, before bodies | `available-externally`, [`globalopt`](passes/globalopt.md), [`ports`](passes/ports.md), `stamp` |  |
| Body rounds | `sroa`, [`fold`](passes/fold.md), [`floatfold`](passes/floatfold.md), [`decide`](passes/decide.md) |  |
|  | `equalcalls`, [`gvn`](passes/gvn.md) |  |
|  | [`tailrec`](passes/tailcallelim.md), [`loopsimplify`](passes/loopsimplify.md), [`lcssa`](passes/lcssa.md), [`floatloop`](passes/floatloop.md), [`trivialunswitch`](passes/trivialunswitch.md), [`inferspace`](passes/inferspace.md) |  |
|  | `licm`, [`indvars`](passes/indvars.md), [`algebraic`](passes/algebraic.md) |  |
|  | [`deadstore`](passes/deadstore.md), [`promote`](passes/promote.md), [`dead`](passes/dead.md), [`loopidioms`](passes/loopidioms.md) |  |
|  | [`loopunroll`](passes/loopunroll.md), [`looppeel`](passes/looppeel.md), [`simpleloopunswitch`](passes/simpleloopunswitch.md), `loopsplit`, `loopfuse`, `loopdistribute`, `unroll-jam` |  |
| Module, interprocedural | [`inline`](passes/inline.md), [`argpromotion`](passes/argpromotion.md), [`deadargs`](passes/deadargs.md), [`ipacp`](passes/ipacp.md), [`interprocedural`](passes/interprocedural.md) | [module-analyses](module-analyses.md), [early-summaries](early-summaries.md) |
| After the interprocedural step | [`globaldce`](passes/globaldce.md), [`calleepop`](passes/calleepop.md), [`fixednarrow`](passes/fixednarrow.md), [`lsr`](passes/lsr_pass.md), [`window`](passes/window.md), [`rotate`](passes/rotate.md), [`jumpthreading`](passes/jumpthreading.md), [`gepoffset`](passes/gepoffset.md), [`cgpaddr`](passes/cgpaddr.md), [`spares`](passes/spares.md), [`homes`](passes/homes.md) |  |
| Backend | see the [backend phases](backend/readme.md) | [floating-pop-reuse](floating-pop-reuse.md), [second-frame](second-frame.md), [allocation-algorithm](allocation-algorithm.md) |

## Reference pages

Each pass, analysis and backend phase has one page, generated from the `//!` header of its source file: [passes](passes/readme.md), [analyses](analyses/readme.md), [backend phases](backend/readme.md). Edit the header, then run `python3 tools/docs/docsbuild/build.py --sync` and commit what it writes.

## Notes

| Doc | Subject |
|---|---|
| [raise-arguments](raise-arguments.md) | Word pairs, runtime helpers and numeric arguments of BC objects |
| [floating-pop-reuse](floating-pop-reuse.md) | x87 stack allocation and popping arithmetic |
| [second-frame](second-frame.md) | The second frame layout without a second backend run |
| [allocation-algorithm](allocation-algorithm.md) | Where allocation time goes, measured, and the single-allocation design |
| [module-analyses](module-analyses.md) | Module analyses brought up to date after an edit |
| [early-summaries](early-summaries.md) | Summaries at fixed points; a parked design |
| [single-allocation-decisions](single-allocation-decisions.md) | What best-of-six allocation decides, and the in-run step for each |

Array bounds checks are in [bounds-checks](../reference/semantics/bounds-checks.md).
