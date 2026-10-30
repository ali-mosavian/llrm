# MemorySSA clients: references made when asked

Owner w-3. d_faces `-O2`, dist profile of parity after #4704 (28 builds). Follows [mssa-build-plan.md](mssa-build-plan.md).

## Where the cost is

The pin's MemorySSA computes a `MemoryLocation` when a walk asks for one. Ours made a `MemRef` for every load and store of the body before the first question: `BaseAccesses` 8.0 M over 28 builds (`Accesses::plain`: `MemRef::of`, the per-instruction `touched` lists, the `references` map), beside `MemorySsa::run` 6.5 M (the pin: 2.97 M for 25 builds).

How much the clients ask: `AliasQuery::location_of` made 367 `Location`s over 121 query objects whose accesses held 23,630 references: **1.6 %**. So the eager map is nearly all waste for LICM, MemCpyOpt and the walker.

## Who reads `Accesses.references`, and what each needs

| reader | sites | provenance? | route |
|---|---|---|---|
| `AliasQuery::location_of`, `reads`/`writes` lists (walker, LICM, MemCpyOpt) | alias.rs 1, lists 7 | no, since the stack answers a pair with a pointer; `unwritable()` for live-on-entry and `alias`'s no-pointer fallback do | lazy `MemRef::of` |
| `avail.rs` (available loads and stores), `consts.rs` (`_killed`, through-memory) | 3 + 2 | yes: kill sets by object (`may_clobber`) | stay on `PlainAccesses` (annotated) |
| `homes.rs`, `spares.rs`, `promote.rs` | 2 + 1 + 1 | yes: a slot is an object, `restrict` | stay on `PlainAccesses` |
| DSE | 12 | object of the pointer: root, not provenance (#4635) | `PlainAccesses` |
| EarlyCSE | via `Accesses::managed` | yes (11 QCport modules moved without it) | stay |
| `mssa/shadow.rs`, `bin/shadow_mssa.rs` | 3 | instruments | read the accessor |

The only reader of the graph's accesses that needs provenance is `is_use_trivially_optimizable_to_live_on_entry` (`reference.unwritable()`: constant memory) and the footprint overlap for a location with no pointer. Both are asked of one instruction at a time, so they stay lazy; the stack's `pointsToConstantMemory` was the alternative, and cost +2.1 M.

## Steps

1. `Accesses` reads through accessors (`reference(inst)`, `reads(inst)`, `writes(inst)`), the `references` and `touched` fields private; each returns a value made once and kept per instruction. No behaviour change; the 12 reader sites change.
2. `BaseAccesses` makes none up front: `reference(inst)` is `MemRef::of(unit, inst)` on first ask, a call's lists are none. LICM's `add_instruction` keeps working (a new instruction is asked the same way).
3. Gate on each: objsweep and QCport unchanged (the references are the same values, made later), the corpus test that `reference(inst)` equals the eager one, dist pair.

Expected: `BaseAccesses` 8.0 M to about 0.5 M (the forced 1.6 % plus the map). Not touched: the 21.5 M of `PlainAccesses` that the provenance readers pay (`Annotated`, `PointerValues`, `Counted`: a separate row, funcattrs-port's lazy-Annotated plan), and `ExposedFrames` 2.1 M in the build.

## Not viable, measured

Lazy `Annotated` per instruction as `PlainAccesses`'s source cost +1.7 M on dist: the provenance readers walk every load and store of the body, so nothing stays unasked. Provenance has to stay eager for them; laziness pays only where the clients ask 1.6 %.
