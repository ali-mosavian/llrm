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
