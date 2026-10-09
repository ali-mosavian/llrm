# Call effects as a summary queried per call (gcc's modref shape)

Status: proposal, for review.

## What costs

The alias family is 29.8% of the QCport -O1 compile (223.1 G instructions, 906 bodies, 65 modules). Two rows in it are bookkeeping: `call-effects` 5.2% and `stamp` 2.7%.

`call-effects` is [`calls_annotated`](https://github.com/ali-mosavian/llrm/blob/0a28dac455e4a7e87cf862838b4dfaee05981de7/crates/opt/llrm-analysis/src/alias.rs#L1330) (98.7% of the row), 9.35 times per body. Timing its three parts by spans (the spans add overhead, so the split is of the row, not of the compile):

| part | share of the row |
|---|---:|
| `points_to(unit, Some(arguments), Some(captures))`: a whole-body points-to solve that knows each call's actual arguments and what each callee captures | ~51% |
| per call: `Summary::instantiated` against the actuals, the unknown-write expansion (escaped, non-local and every tracked global), and building the call's loads, stores and fills as `Rc<[MemRef]>` lists | ~34% |
| `_callbacks`: the effects of the GlobalsAA entries, a union of summaries | ~13% |

`stamp` is [`stamped_all`](https://github.com/ali-mosavian/llrm/blob/0a28dac455e4a7e87cf862838b4dfaee05981de7/crates/opt/llrm-transforms/src/interprocedural.rs#L1191), 93% of it [`alias::initialized`](https://github.com/ali-mosavian/llrm/blob/0a28dac455e4a7e87cf862838b4dfaee05981de7/crates/opt/llrm-analysis/src/alias.rs#L1517): for every function, to state which bytes of a pointer parameter it writes before reading, two points-to solves and a `calls_annotated` (a third) over every call.

## What gcc does

GCC's modref keeps, per function, a bounded tree of the accesses it makes (alias sets, a parameter index and an offset range, or "global" or "unknown"), plus flags for side effects. It is made twice per function by a walk of the statements ([`pass_modref`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L103) early and [late](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L369)) and propagated once per unit ([`pass_ipa_modref`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L164)). A call is never given a footprint: the alias oracle asks the callee's summary about the one reference in question (`modref_may_alias`). The points-to of the actuals is the function's one solution (`pass_build_alias`, [:213](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L213)), made once and kept.

## What ours does

`Summary` ([alias.rs](https://github.com/ali-mosavian/llrm/blob/0a28dac455e4a7e87cf862838b4dfaee05981de7/crates/opt/llrm-analysis/src/alias.rs#L320)) is already the modref summary: slices of objects (`Parameter(i)`, globals, the unknown object) read and written, captures, flags. `instantiated` is already the "apply at the call site" step. What is not modref-shaped is that the application is done eagerly for every call of every body, into lists, behind an `Effect` ([:1319](https://github.com/ali-mosavian/llrm/blob/0a28dac455e4a7e87cf862838b4dfaee05981de7/crates/opt/llrm-analysis/src/alias.rs#L1319)), with a points-to solve of its own to find the actuals. `Writes`, `Accesses` and the consumers below hold or copy those lists.

## Census of consumers

What each consumer asks of a call (read from the code):

| consumer | asks | needs a list? |
|---|---|---|
| [`memoryssa::built`](https://github.com/ali-mosavian/llrm/blob/0a28dac455e4a7e87cf862838b4dfaee05981de7/crates/opt/llrm-analysis/src/memoryssa.rs#L834) | is the call a def (writes anything), a use (reads anything) | no, a flag |
| memoryssa clobber walk, `changes`/`clobbered` | may the call write this cell | no, per reference |
| `memoryssa::spares` (hoist, gvn via `_undisturbed`, the `spares` pass, `homes`) | does the call leave this load's bytes alone | no, per reference |
| `promote::through` | any write overlapping each available cell; `writes.is_empty()` | no, per cell and a flag |
| `loopmotion`, `hoist` | does anything in the loop write the reference | no, per reference |
| `memory::unmodeled_write` | does the call write at all (attributes only) | no, one bit |
| [`consts::_killed`](https://github.com/ali-mosavian/llrm/blob/0a28dac455e4a7e87cf862838b4dfaee05981de7/crates/opt/llrm-analysis/src/consts.rs#L535) (every memory solve: through-memory, float-facts, fold, floatloop) | kill from a cell map every cell any write overlaps | yes, walks the writes with the overlap index |
| [`avail::after`](https://github.com/ali-mosavian/llrm/blob/0a28dac455e4a7e87cf862838b4dfaee05981de7/crates/opt/llrm-analysis/src/avail.rs#L191) (gvn's holders) | the same, for the holders map | yes |
| [`avail::dead_stores`](https://github.com/ali-mosavian/llrm/blob/0a28dac455e4a7e87cf862838b4dfaee05981de7/crates/opt/llrm-analysis/src/avail.rs#L677), `Stored::new` (dse) | which cells the call reads, writes, and fills | yes: reads, writes, fills |
| `memoryssa` `Reach::of` | the objects the call may write (a reject filter) | the object set |
| `alias::initialized` | which parameters the call may read; which bytes it initializes | loads' slices; the attribute |
| `fold`, `floatfold`, `floatloop` | whether two `Calls` maps are equal, to share a solve | an identity, not a list |
| `loopmotion`, `floatfacts`, `floatbounds`, `peelsize` | call `consts::cells` with an empty `Calls`: a call writes everything if `unmodeled_write`, else nothing | nothing |

`fills` (from `initializes`) is read in only two places, [`_fills`](https://github.com/ali-mosavian/llrm/blob/0a28dac455e4a7e87cf862838b4dfaee05981de7/crates/opt/llrm-analysis/src/alias.rs#L1419) and `initialized`'s transfer, per call from the call-site and callee attributes, and used by the dead-store walk.

Most consumers ask a yes/no about one reference or one flag. Enumeration is needed by four (`_killed`, `avail::after`, `dead_stores`, `Stored::new`) and each of them walks every call of every block, so a lazy API saves only the calls nothing enumerates.

## Proposal

Three slices, each deleting what it replaces.

1. **The callbacks once.** `_callbacks` is a function of the module's GlobalsAA entries and the module's summaries; the unit only supplies `globals_aa`. It is computed for every body of every `calls_annotated`. Compute it once per (GlobalsAA, Summaries) result, held beside them. Exact; the per-call copy of `_callbacks` goes. Prediction: ~0.7% of the compile.
2. **One points-to with call knowledge per body state.** `calls_annotated` solves points-to with the actuals and the callees' captures; `initialized` solves it twice more (plain, and with actuals), and `Pointers`, the manager's analysis, once more without. Once `Summaries` is held, the captures are known and the actuals are the body's: the manager's `Pointers` can be that one solve (arguments and captures supplied from the held `Summaries` through `Outer`), read by call-effects, by `initialized` and by the plain consumers. Its invalidation is the `Depends` declaration of #1128 plus the `Outer` identity, as for every analysis that reads `Summaries`. Prediction: 2-2.7% (the argument-aware solve of call-effects) and ~1% (two of `initialized`'s three), together ~3% of the compile. This slice changes what the plain consumers see (a call to a callee known not to capture no longer lets its actuals escape), so objects can change; that is the slice that needs the explained-diffs oracle.
3. **No lists for the callers that ask yes/no.** `CallSummary` per call site: the callee's `Summary` (an `Rc`), the actuals' provenance, the call-site attributes. Methods: `writes_anything()`, `reads_anything()`, `may_write(&MemRef)`, `may_read(&MemRef)`, `objects_written()`, `for_each_write(f)`, `for_each_read(f)`, `fills()`. `may_write` instantiates only the slices that can meet the reference's object. `for_each_*` instantiates once and keeps the list in the call summary (today's `made` sharing by slice set), so the enumerating consumers pay what they pay now and the others pay nothing. `Calls` equality becomes a version of the `Writes` result. Prediction: ~1% (the part of the 34% for calls nothing enumerates).
4. **`initialized` from the shared solve and the call summaries**: slice 2 and 3 applied to `stamped_all`. Prediction: ~0.5% beyond what slice 2 gives.

Total prediction 4-5% of the -O1 compile. Stop rule: a slice that buys under 0.5% is not merged.

## Invalidation

A summary is keyed by function name and parameter index; the call summary holds an `Rc` of it and the actuals of one body state, so it is dropped with the body's `CallEffects`-equivalent result by the same declaration. The passes that change a signature or add a function (`argpromotion::promoted`, `narrowspace::narrowed`, `deadargs::removed`, `ipacp::cloned`) run inside the interprocedural step, before `stamped_all` is rerun and before the freeze of #1153; they make `Summaries` again as they do today. Nothing after the freeze changes a signature. The held `Summary` is not edited in place.

## Oracle

- Objects byte-identical on the 272 programs (bench, vsgcc kernels, QCport at -O1/-O2/-Os) and the 66 vsgcc programs, for slices 1, 3 and 4, with the `LLRM_CHECK_*` switch of each slice recomputing the old path and comparing (lists against queries, for every reference a consumer asked).
- Slice 2 may differ, by construction, wherever a call to a non-capturing callee kept an actual from escaping. Each differing file is explained (which call, which fact), and the quality is measured (clocks, instructions, bytes) before it merges.
- Each slice deletes the path it replaces; the deletion in the diff is the evidence that no consumer was left on the old one.

## Risks

- Slice 2 moves the plain `Pointers` consumers to a solve that depends on `Summaries`; before the first `Summaries` exists (the early stretches) they need the plain solve, so `Pointers` has two configurations, and the invalidation must follow which was used.
- `may_write` re-instantiates per question; a loop of questions about one call (promote asks per cell) needs the cache the list gave for free. The call summary keeps the instantiated slices after the first question.
- The `Calls` equality comparisons in fold, floatfold and floatloop need an identity: a version number of the `Writes` result.
