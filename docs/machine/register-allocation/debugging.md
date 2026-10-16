# Switches and debugging

Audience: an engineer who has a wrong or poor allocation and needs to see what the allocator did. This page lists what changes allocation, what prints, and how to read the lines.

## What changes allocation

| Switch | Effect |
|---|---|
| `-fallocation-search` | try other shapes of a body after the first allocation; on at every level but -O0 ([assignment](assignment.md#search-over-shapes-of-the-body)) |
| `-fallocation-search-all` | try every shape, with and without splitting; -Omax |
| `-fallocation-routes` | make each function by the allocator alone and by the SSA spiller's route and keep the cheaper; on at every level but -O0 ([ssa-spill-route](ssa-spill-route.md)) |
| `-Os`, `-Oz` | candidates and routes are priced in bytes, each block counted once |
| `-march=`, `-mtune=` | the CPU's prices: fold discounts, cycle breaking, edge copies |
| `-mstack-is-data` | `ds` is a fourth selector register |
| `LLRM_CANDIDATES=allocator\|spiller` | one route only |

The flags are rows of `PASSES` in [`flags.rs`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/driver/flags.rs#L67), set per level in `pipeline::Options` ([levels](../../reference/levels.md)). A flag may be turned off with `-fno-`: `-fno-allocation-search` keeps the routes.

## What prints

`ISEL_DUMP=1` prints the LIR after each machine phase, a banner `═══ fn · Stage · m16 ═══` then the function ([`lirtext`](../../optimizations/backend/lirtext.md)). Each route prints its own run, so a function made by both appears twice, the second with `SsaSpill`. `LLRM_VERIFY=1` checks the body after each phase and names the first that breaks an invariant, or loses source bytes. Every line is explained in [LIR text](../../reference/lir-text.md); what matters here:

- An instruction is `#12  dest  = op.i16  operands  ; note`. The label `#12` is kept by every phase, so two stages are compared by label.
- A value is its source name or `t12`; after allocation a register (`ax`, `eax`) stands in its place. In an address, `t4@bx` names the value and the register it was placed in.
- A frame slot is `[slot2]`, listed on the `frame` line. Blocks are `B3`, every target names its block, and a loop is indented.
- `call f(t12@ax)` needs `t12` in `ax`; `fn` and `; also clobbers esi` state what a call destroys.
- Notes: `spill` (a store), `reload`, `remat`, `copy` (one move of a phi's parallel copy), `elided` (an operation selection removed that still carries a value).

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
5. Slow compile: `LLRM_DEBUG=time`; `regalloc trial` spans count the extra allocations of the search.

General debugging is in [working/debugging](../../working/debugging.md).
