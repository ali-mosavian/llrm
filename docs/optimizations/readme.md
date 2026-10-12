# Optimizations

How the compiler turns MIR into x86 code: the SSA optimizer in `crates/opt`, the backend in `crates/backend/llrm-core`, and one reference per pass family. For contributors.

## Reference

The reference for every pass, analysis and backend phase is the `//!` header at the top of its source file: what it does, how, where it runs, what it needs and keeps. `python3 tools/docsbuild/build.py --sync` extracts the headers into committed pages: [reference](reference/index.md), with one page each for [passes](reference/passes.md), [analyses](reference/analyses.md) and [backend phases](reference/backend.md). The tests fail when those pages are stale, or when a file of a required directory has no header.

| Source | Holds |
|---|---|
| [`crates/opt/llrm-transforms/src`](https://github.com/ali-mosavian/llrm/tree/main/crates/opt/llrm-transforms/src) | The MIR passes and the pipeline that orders them |
| [`crates/opt/llrm-analysis/src`](https://github.com/ali-mosavian/llrm/tree/main/crates/opt/llrm-analysis/src) | The analyses and the pass manager |
| [`crates/backend/llrm-core/src/backend`](https://github.com/ali-mosavian/llrm/tree/main/crates/backend/llrm-core/src/backend) | Lowering for selection, instruction selection and the machine phases |

## Pass index, in pipeline order

Levels and flags: [levels](../reference/levels.md).

| Stage | Passes | Reference |
|---|---|---|
| Module, before bodies | `available-externally`, `globalopt`, `ports`, `stamp` | [passes](passes.md) |
| Body rounds | `sroa`, `fold` (+ `floatfold`), `decide`, `phiopt` | [constant-propagation](constant-propagation.md) |
| | `equalcalls`, `gvn` (+ load and expression PRE) | [gvn](gvn.md) |
| | `tailrec`, `loopsimplify`, `lcssa`, `floatloop`, `trivialunswitch`, `inferspace` | [passes](passes.md) |
| | `hoist`, `loopmotion`, `indvars`, `algebraic` | [lsr](lsr.md), [passes](passes.md) |
| | `dse`, `promote`, `dead`, `fill`, `merge` | [dead-code](dead-code.md) |
| | `unroll`, `peel`, `unswitch`, `unroll-jam` | [unrolling](unrolling.md) |
| Module, interprocedural | `inline`, `ipa-*`, argument promotion, dead arguments, `ipacp` | [passes](passes.md), [module-analyses](module-analyses.md), [early-summaries](early-summaries.md) |
| After the interprocedural step | `globaldce`, `calleepop`, `fixednarrow`, `lsr`, `differences`, `window`, `rotate`, `jumpthread`, `hoist`, `gepoffset`, `addresssink`, `spares`, `homes` | [lsr](lsr.md), [passes](passes.md) |
| Backend | `isel`, spiller, `allocate`, `floatassign`, `floatalloc`, `coalesce`, peephole, `schedule`, `jumps`, frame layout | [backend](backend.md), [floating-pop-reuse](floating-pop-reuse.md), [second-frame](second-frame.md) |

## Per-pass references

| Doc | Subject |
|---|---|
| [constant-propagation](constant-propagation.md) | `fold`, `floatfold`, `floatloop`; `consts`, `ranges`, `floatfacts`, `floatbounds` |
| [gvn](gvn.md) | Value numbering, store-to-load forwarding, PRE |
| [lsr](lsr.md) | Loop strength reduction, exit tests, invariant hoisting, huge pointers |
| [dead-code](dead-code.md) | `dead`, `dse`, `promote`, `loopmotion` |
| [unrolling](unrolling.md) | `unroll`, `peel`, `unroll-jam`, and the size model |
| [raise-arguments](raise-arguments.md) | Word pairs, runtime helpers and numeric arguments of BC objects |
| [floating-pop-reuse](floating-pop-reuse.md) | x87 stack allocation and popping arithmetic |
| [second-frame](second-frame.md) | The second frame layout without a second backend run |
| [module-analyses](module-analyses.md) | Module analyses brought up to date after an edit |
| [early-summaries](early-summaries.md) | Summaries at fixed points; a parked design |

Array bounds checks are in [bounds-checks](../reference/semantics/bounds-checks.md).
