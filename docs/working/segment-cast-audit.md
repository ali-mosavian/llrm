# Casts through the selector space: audit

The selector-alone space (`[spaces].segment`, `ni:` in the datalayout) drops the offset: near -> selector keeps the segment, selector -> far gives segment:0. A pair of casts through it is not one cast. Who composes or strips addrspacecasts (2026-10-21, `grep AddrSpaceCast`):

| Site | What it does | Verdict |
|---|---|---|
| `constantfolding.rs` `is_eliminable_cast_pair` case 12 (callers: `constant_fold_cast_operand`, `combine_casts.rs`) | composes addrspacecast pairs (pin Instructions.cpp#L3043) | fixed: DEVIATE C1, no fold through an `ni` middle |
| `isel.rs` `global()` | a constant selector cast back to far was lowered as the global's own far address (offset kept) | fixed: selector:0 |
| `constantfolding.rs` offsets/`get_underlying_object`, `valuetracking.rs`, `valuetracking/underlying.rs`, `memory.rs` (MemRef roots), `basicaa.rs`, `globalopt.rs`, `switchlookup.rs` | strip addrspacecasts to name the underlying object | unsafe in principle: a far pointer made from a selector is named by the global it came from; not fixed here, row to follow |
| `alias.rs:3332,3381` | stops at the segment space | safe |
| `frameescape.rs`, `globalsmodref.rs`, `capturetracking.rs`, `capturecheck.rs` | follow uses through casts | safe (they over-approximate escapes) |
| `globals.rs` `target()` (data initializers) | looks through casts for the global an address names | unsafe for a far pointer initialized from a selector; not reachable from QB output today |
| `sroa.rs`, `mem2reg.rs`, `memcpyopt.rs`, `deadstore.rs`, `spill.rs`, `tailcallelim.rs`, `splitcopy.rs`, `combine_loadstore.rs` | look through casts on accesses | not audited one by one: they follow a pointer to its access, not its identity |

How the symptoms became visible: the pair became foldable with the ConstantFolding translation (4a2b1b423a, 2026-10-18) and visitTrunc (bae70570c1). far3's wrong output was masked by a compile failure fixed by cb2ce8f3e; far1's compile failure appeared with fbbe81205 (#3262), which unmasked the folded shape.

## Probes, one per unsafe site (2026-10-21)

Each probe is a MIR test on a selector made of a global (`@pa` -> segment -> far, plus an index) against another global or a constant initializer.

| Site | Probe | Wrong? | Fix |
|---|---|---|---|
| `constantfolding.rs` offsets and `get_underlying_object` | `a_load_through_a_selector_cast_is_not_folded_from_the_global_it_came_from` | yes: a load folded to the initializer's `@k[3]` (100) | stop at an `ni` source |
| `valuetracking.rs` `strip_offsets` (`underlying`) and `valuetracking/underlying.rs` | `underlying_stops_at_a_cast_from_a_selector_alone` | yes: the root was the global | stop at an `ni` source |
| `basicaa.rs` `strip`, `underlying`, `decomposed` | `a_far_pointer_made_of_a_selector_is_may_alias_with_another_global` | yes: NoAlias with another global | stop at an `ni` source |
| `memory.rs` `step`, `alias.rs` provenance | a MemRef/provenance probe with the target's spaces | no: both stop at the selector space (a unit with FLAT spaces sees through; only tests build one) | none |
| `globalopt.rs`, `alias.rs from_integer`, escape and capture followers | read | no: they follow uses through casts, which over-approximates | none |
| `switchlookup.rs`, `globals.rs` `target()` | not probed | unsafe in principle (a table or a data initializer behind a selector cast) | row to follow |

QB programs: three BASIC probes (`VARSEG(a)` then POKE onto `b`; far6 with `VARPTR(z)` taken; `VARSEG(arr(i))` with a run-time index) print right at -O0..-Os on the head with all fixes. The fixes still change 24 m16 QB bench objects (bintree, jpeg_fixed, jpeg_single, queens; bas and bq16, O2/Os/O3), so the BASIC runtime reaches these sites; they are 14 to 75 B smaller at -Os, and bintree, jpeg_fixed, jpeg_single and queens pass.

## Rows above main

| Row | What | Owner | Exit | Review |
|---|---|---|---|---|
| FORGED-SEL-1 | deedlines -Os +201 B (0.6%) above main and above base: a forged selector (DEF SEG = VARSEG, POKE) leaves no global tracked, so loads and stores of the program's globals are kept in memory | funcattrs-port | prove non-aliasing where the selector provably is not DS | 2026-11-30 |

## The remaining sites and passes (2026-10-21)

| Site | What it does with a cast | Verdict |
|---|---|---|
| `switchlookup.rs` `valid_lookup_table_constant` | accepts a constant pointer cast as a table value | safe: the pointer is copied as data, nothing is said of what it points into |
| `globals.rs` `target()` | names the global behind a constant address (data initializers, isel constants) | unsafe in principle for a far pointer initialized from a selector; unreachable: selector values come from `VARSEG` and `DEF SEG`, computed at run time, never in a data initializer or a constant far pointer (isel `global()` lowers a constant selector cast to selector:0) |
| `sroa.rs` | a cast of an alloca aborts the slice build | safe |
| `mem2reg.rs` | a cast of an alloca is allowed only where only lifetime markers use it | safe |
| `memcpyopt.rs` `strip_pointer_casts` (7 identity comparisons) | strips casts, selector casts included, to compare a call argument with the copy's source or destination | green probe, pinned: `test_a_call_through_the_selector_of_a_local_is_not_forwarded_into_its_copy`: a direct argument is forwarded into the copy, one that goes through the local's selector is not, with or without `ni:` (the pass bails on an argument whose pointer chain crosses a space). Two other shapes (a memcpy of `@a` onto its selector, a far argument) never fired in either layout either |
| `memcpyopt.rs` use scan at L1272 | follows the uses of the source through casts | safe (follows uses) |
| `deadstore.rs` | follows the uses of an alloca through casts to see it leave the function | safe (over-approximates) |
| `tailcallelim.rs` | alloca-derived values through casts | safe (follows uses) |
| `splitcopy.rs` | derived pointers whose `underlying` offset is known | safe: `underlying` stops at a selector cast over an object, so the offset is unknown and the copy is not split |
| `combine_loadstore.rs` | users of an alloca through casts, stores only | safe (follows uses; any other user ends it) |
| `spill.rs` (L567, L591, L710, L750, L1115) | whether a value is a frame address, and the cost of rebuilding it | cost heuristics, no identity claim: safe |

No program was found that reaches an unsafe site; the three BASIC probes and the MIR tests are above (memcpyopt: a green probe, pinned). If a pass starts to compare pointers by their stripped root, the rule is `valuetracking::object_below`.
