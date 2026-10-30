# Switches and debugging

Audience: an engineer who has a wrong or poor allocation and needs to see what the allocator did. This page lists what changes allocation, what prints, and how to read the lines.

## What changes allocation

| Switch | Effect |
|---|---|
| `-fallocation-search` | no effect on the allocator, which allocates once; |
| `-fallocation-search-all` | try every shape, with and without splitting; -Omax |
| `-Os`, `-Oz` | candidates and routes are priced in bytes, each block counted once |
| `-march=`, `-mtune=` | the CPU's prices: fold discounts, cycle breaking, edge copies |
| `-mstack-is-data` | `ds` is a fourth selector register |
| `LLRM_CANDIDATES=allocator\|spiller` | one route only |

The flags are rows of `PASSES` in [`flags.rs`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/driver/flags.rs#L67), set per level in `pipeline::Options` ([levels](../../reference/levels.md)). A flag may be turned off with `-fno-`: `-fno-allocation-search` keeps the routes.

## What prints

`ISEL_DUMP=1` prints the LIR after each machine phase, `=== PhaseName` then each function ([`lirtext.rs`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/lirtext.rs#L12)). Each route prints its own run, so a function made by both appears twice, the second with `SsaSpill`. `LLRM_VERIFY=1` checks the body after each phase and names the first that breaks an invariant, or loses source bytes.

| Dump text | Means |
|---|---|
| `v12 := add v12, v27` | value `v12` is updated in place (a tie) |
| `AX := mov [[abs+0xc] v36@BX]` | after allocation, a cell addressed through value `v36`, placed in `BX`; before it, `v10@unplaced` |
| `[[bp-0x10]]` | a frame cell: a spill slot, a local or an argument |
| `; requires v13@AX` | the instruction needs `v13` in `AX` |
| `; delivers v5@AX` | the instruction writes `v5` in `AX` and names it nowhere |
| `; clobbers EAX,ECX,...` | registers destroyed (`clobbers_high`: only the upper half) |
| `; parallel-copy 2` | one move of copy group 2 |
| `; spill reload` | a reload the spiller wrote; a store or a remat has no note |
| `; inserted` | the instruction was made by a phase and claims no source bytes |
| `v6 := phi 0x0000:v1 0x0009:v5` | a phi: value from each predecessor block |

`LLRM_DEBUG=channel,channel` (or `all`) writes `[channel] line` to stderr ([`debug.rs`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/support/llrm-support/src/debug.rs#L1)).

| Channel | Line |
|---|---|
| `regalloc` | `NAME: N insns, S spilled, cost C, F forced` after the base allocation; `  trial SHAPE (splitting B): ...` per trial, `  kept the trial` when it won; `last resort for value#V: R evicts [...]`; `out of budget` |
| `events` | one line per allocator decision: `pop vV STAGE weight W`, `vV takes free R`, `vV evicts [(victim, weight)] from R`, `vV loses R to the rewrite` |
| `spill` | `NAME: spill V with {...}`: the value and the siblings sharing its slot |
| `split` | `NAME: split V at {block: [(from,to)]}`: the region carved |
| `ssaspill` | `loop at B peaks at N`: the pressure inside each loop |
| `candidates` | `NAME: spiller X / Y, allocator alone X / Y`: `route_cost` then the other measure |
| `cost` | `NAME executes I instructions, M memory operands; R reloads, S spill stores, T remats; ...` for the kept function |
| `pressure` | the MIR spill model's forecast beside the allocator's reloads, stores and remats |
| `traffic` | frame traffic in loops by cause, before `LoopSlots` |
| `regclass` | points the classes cannot hold, after each early phase and entering `RegAlloc` |
| `size` | instruction count of the kept function |
| `time` | spans by own time; the allocator's are `regalloc *`, `spill *`, `split *`, `ssa *`, `intervals *`, `facts *` |

## Self-checks

Each incremental structure has a switch that recomputes it whole and asserts equality: `LLRM_CHECK_FACTS`, `_RANGES`, `_INTERVALS`, `_LIVE`, `_CLOBBERED`, `_HOLDERS`, `_OVERLAPPING`, `_REQUIRED`, `_POSTINGS`, `_SPLIT`, `_PLACING`, `_OCCUPIED`, `_CLASSES`, `_FOLDS`, `_SIBLINGS`, `_COALESCE`, `_TWOADDR`, `_THRASH`, `_FRAME`, `_COLORS`. `grep -rhn 'LLRM_CHECK_' crates` lists them all. A mismatch is an assertion naming the function and the structure. `LLRM_STRIP_META=1` runs each phase on a body stripped of meta instructions, the invariance check for `-g`.

## Reading a bad allocation

1. `ISEL_DUMP=1`, find the first phase whose output is wrong. `Coalescer` is the last phase before `RegAlloc`.
2. A copy that should be gone: was it refused by the coalescer (compare the `Coalescer` and `RegAlloc` dumps) or not a copy of one width ([coalescing-and-copies](coalescing-and-copies.md))?
3. A spill that should not be: `LLRM_DEBUG=regalloc,spill,split,cost`. Which value, which route won (`candidates`), whether the base allocation or a trial kept it.
4. A register that was not allowed: the class (`regclass`), the pins (`requires`, `delivers`, `fixed`) and the clobbers on the instructions between ([inputs-and-classes](inputs-and-classes.md)).
5. Slow compile: `LLRM_DEBUG=time`; `regalloc trial` spans count the extra allocations of the search. Each timed span costs about 6-7 K instructions of its own, so a step called many times reads high and a change that opens fewer spans reads better than it is (greedy against the old allocator on d_faces -O2: -106 M in the report, -56 M in `perf stat`). The last `[instr]` row prints the span count. Quote deltas from a plain `perf stat -e instructions:u`.

General debugging is in [working/debugging](../../working/debugging.md).

## Same-MIR runs against the pin

`LLRM_MIR_PRINT=fn` prints the LIR the allocator takes as MIR; `llc -start-before=greedy -x mir` allocates it with the pin. The pin parses a MIR with no instruction erased, but ours reaches the allocator through the coalescer, whose erased instructions leave empty entries that `packIndexes` counts (LLVM does the same in a full run). On queens `place_` that made every interval 1.3x longer and the spill weights 3-50% lower than the pin's for the same intervals (top four weights equal to four digits once the entries are left out). Run ours with `LLRM_PACK_DENSE=1` for a same-MIR comparison, and expect the full-pipeline pin to sit between the two (queens `place_`: pin 95 index units, ours 105, dense 74).
