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

## Review findings

A review of the first version against the code found:

- Slice "Pointers takes the call arguments and captures from the held Summaries" is circular: the summaries fixed point takes its captures from its own partial result, and GlobalsAA feeds Summaries. It also moves `escaped` and `escaped_before` both ways, so dse and `observers` would lose transforms as well as gain them. Dropped.
- Enumeration is needed by more consumers than the first census named: `Writes` copies every `stores` list into `Calls` for through-memory, float-facts, fold and floatloop; `Accesses::resolved` and `Accesses::plain` build further lists. `may_write` is not one predicate (`regions::overlapping` with the program, `may_clobber` with pointer-fact offsets, `queries.may_overlap`). The per-call summary API is expected under the stop rule.
- `_callbacks` depends on the GlobalsAA entries and the held summaries only; it is made per body, not per call. `_tracked` is module level and made per call. `_summarized` dedupes by (callee, bits escaped before) and `calls_annotated` does not.
- `Calls` equality in fold and floatfold is by content: keep it, with a pointer fast path, not a version number.
- The call arguments and captures are read only by the escape phase of `points_to`: the lever below.

## Measured since the review

Every `points_to` solve of the QCport -O1 compile, 34,762 of them, is 16.6% of the compile (spans included): 11.0% in the value solve and 5.7% in the escape phase. `arguments` and `captures` are read only by the escape phase (alias.rs lines 2122-2125); the value solve (1851-2013) never reads them, so the value solve of the plain, arguments-only and arguments-plus-captures configurations is the same computation. The solves are asked of 6,209 distinct body states and 17,144 distinct (state, unit configuration) pairs: a value solve made once per pair would be 56% fewer, once per state 84% fewer.

## Proposal (after the review)

Slices in the order the review gave, each deleting what it replaces.

1. **The value solve once per body state.** Split `points_to` into the value solve and the escape phase `(arguments, captures)`. The value solve's result (the values, and what the escape phase takes from it: the cells in and out of each block, the fields, the unbounded objects) is held per body state, keyed by the function's mark and the identity of the declarations the unit reads (a callee's attributes enter it); the escape phase reads it. Every configuration, `Pointers`, GlobalsAA's contribution, `_direct_summary`, `initialized`'s two and `calls_annotated`'s, asks the held solve. Exact by construction, since the value solve does not see what differs; a check mode solves again and compares. Upper bound 6-9% of the compile (56-84% of 11.0%); the memo and the hits that miss because a key differs lower it.
2. **`initialized` from `calls_annotated`'s solve.** Its `facts` and `actuals` are read only through `values` and `reference()`, so they come from the one solve `calls_annotated` makes for the same body: two of its three solves gone, exact. About 1%.
3. **The callbacks once, and what goes with them.** `_callbacks` is a function of the GlobalsAA entries and the held summaries only (the body does not enter), made for every body: once per `Outer` (it is replaced when either result is), not as a module analysis (module analyses have no dependency between them). `_tracked(unit)` is module level and made per call: hoist it. `_summarized` dedupes by (callee, bits escaped before); `calls_annotated` does not: port it. About 0.7-1%.
4. **`stamped` caches the shared solve by declarations as well as state**, since it writes callees' `memory`, `nocapture` and `initializes` attributes bottom-up mid-loop, which changes callers' `_allowed`, `unmodeled_write` and fills.
5. **A `CallSummary` queried per reference** only if the measurement justifies it: count the `CallEffects` computations in a body state that no `Writes` reader follows, and the lists no enumerating consumer walks. The census above says enumeration is needed by more than four (`Writes` copies every `stores` list into `Calls` for through-memory, float-facts, fold and floatloop, and `Accesses::resolved` and `Accesses::plain` are further builders), and the overlap predicates differ per caller (`regions::overlapping` with the program, `may_clobber` with pointer-fact offsets, `queries.may_overlap`), so a single `may_write` is not one predicate. Expected under the 0.5% stop rule; likely cut.

Dropped from the first version: making `Pointers` take captures from the held `Summaries`. The summaries fixed point takes its captures from its own partial result, and GlobalsAA feeds Summaries, so neither can read a `Pointers` that depends on `Summaries`; and an argument-aware solve moves `escaped` and `escaped_before` both ways (a non-capturing actual's pointees escape as lent, and `passing` adds objects), so the plain consumers (dse, `observers`) would lose transforms as well as gain them.

Total prediction: 6-8% of the -O1 compile, mostly slice 1. Stop rule: a slice that buys under 0.5% is not merged.

## Invalidation

A summary is keyed by function name and parameter index; the call summary holds an `Rc` of it and the actuals of one body state, so it is dropped with the body's `CallEffects`-equivalent result by the same declaration. The passes that change a signature or add a function (`argpromotion::promoted`, `narrowspace::narrowed`, `deadargs::removed`, `ipacp::cloned`) run inside the interprocedural step, before `stamped_all` is rerun and before the freeze of #1153; they make `Summaries` again as they do today. Nothing after the freeze changes a signature. The held `Summary` is not edited in place.

## Oracle

- Objects byte-identical on the 272 programs (bench, vsgcc kernels, QCport at -O1/-O2/-Os) and the 66 vsgcc programs, for every slice (none is meant to change what a consumer sees), with the `LLRM_CHECK_*` switch of each recomputing the old path and comparing.
- Each slice deletes the path it replaces; the deletion in the diff is the evidence that no consumer was left on the old one.

## Risks

- The memo key must name everything the value solve reads: the function's mark, the declarations (callee attributes), the context (object numbering) and the layout. The check mode that solves again and compares is how a missed input is found, on the 272 programs and the 66.
- Held solves are memory: one per body state kept while the manager keeps the body; they are dropped with it.
- `Calls` equality in fold, floatfold and floatloop is by content; keep content equality with a pointer fast path, not a version number (a per-manager counter collides after a manager is dropped).
