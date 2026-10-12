# MIR passes

For contributors to `crates/opt/llrm-transforms`: the order the MIR optimizer runs its passes in, and what each file there does. Which analysis a pass reads is named in its entry and described in [analyses](https://github.com/ali-mosavian/llrm/tree/main/crates/opt/llrm-analysis/src); what runs after MIR is in [backend](backend.md); what each `-O` level turns on is in [levels](../reference/levels.md).

## How a body is optimized

`pipeline::recorded` ([`pipeline.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-transforms/src/pipeline.rs#L367)) drives llrm-mir's pass manager over a module. At -O0 only `available-externally` runs. Above it the order is:

```
 module     available-externally, globalopt, ports, stamp
 each body  Fixed  (below), to a fixed point
 module     interprocedural   (inline, ipa-*; reruns Fixed on bodies it changed)
 module     globaldce, available-externally, calleepop
 each body  [rotate+cleanup, header copy: -O1 and up, not -Os], fixednarrow, lsr, differences,
            window, rotate, jumpthread, hoist, gepoffset, addresssink
 module     spares, homes
 then       lowering for selection and the backend
```

`Fixed` ([`pipeline.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-transforms/src/pipeline.rs#L549)) is one function pass, `mir pipeline`:

```
 sroa                              structural boundary, outside the rounds
 +-- round ------------------------------------------------------------+
 | fold  ports  decide  phiopt  equalcalls  tailrec  loopsimplify       |
 | lcssa  floatloop  hoist  loopmotion  trivialunswitch  inferspace     |
 | dse  gvn  promote  indvars  algebraic  dead  fill                    |
 +-- then unroll; if it expanded a loop: sroa, and another round -------+
 peel (while loops disappear: sroa, rounds again)
 unswitch (-O3)   unroll-jam (-O3, then sroa and rounds)   merge
```

A pass is skipped in a round when nothing it reads has changed since it last made no change. The fixed point ends when a round changes nothing; a body that does not settle in a size-scaled number of rounds is a compiler error.

Every pass in the pipeline can be removed with `-fno-NAME` where the table below gives one; the `-f` names are `PASSES` in [`driver/flags.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/backend/llrm-core/src/driver/flags.rs#L67), the per-level options are `Level::options` ([`flags.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/backend/llrm-core/src/driver/flags.rs#L51)) over `pipeline::Options` ([`pipeline.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-transforms/src/pipeline.rs#L47)).

## Seeing a pass

- `llrm-c --dump DIR` (or `LLRM_MIR_STAGES=DIR`) writes the module after each changed step, `NN-pass.ll`; per body, `N/NNN-stage.ll`.
- `LLRM_DEBUG=time` costs each pass; `LLRM_DEBUG=runs` counts rounds and idle runs ([compile-time](../working/measurement/compile-time.md)).
- Pass channels of `LLRM_DEBUG`: `unroll`, `peel`, `unroll-jam`, `lsr`, `hoist`, `inline`, `ipa-cp`, `jumpthread`, `tailrec`; each prints why a candidate was taken or refused.
- `llrm-mir FILE` reads, verifies and prints MIR text; `--run` interprets it.

## Whole-module passes before the bodies

| File | Pass | What it does | Gate |
|---|---|---|---|
| `availableexternally.rs` | `available-externally` | A body kept only to inline from is a declaration again after inlining, so none is emitted. Runs before and after the interprocedural step. | every level |
| `globalopt.rs` | `globalopt` | An internal global only ever loaded is marked `constant`, so loads of it fold. | above -O0 |
| `ports.rs` | `ports` | An I/O port call is narrowed to the memory its device reaches (`inaccessiblemem` when none), as the target describes the port; a non-constant port is narrowed where `ranges` bounds it. Runs over the module, then in each body's round. | above -O0 |
| `interprocedural.rs` (`Stamp`) | `stamp` | States on each body what it does to memory before any body's pipeline runs, so a loop that calls it is judged on that. | above -O0 |

## The body rounds

| File | Pass | What it does | Gate | Reads |
|---|---|---|---|---|
| `promote.rs` (`Sroa`) | `sroa` | Splits aggregates into scalar leaves and promotes them. Runs at structural boundaries only, not inside rounds. | `-ftree-sra` | `memoryssa`, `alias` |
| `fold.rs` | `fold` | Replaces every value consts proves a number by that number, folds an operation on a join per incoming edge, then runs `floatfold` and the canonical forms. See [constant-propagation](constant-propagation.md). | `fold` option | `consts`, `constant_cycles`, `floatfacts`, call writes |
| `floatfold.rs` | `floatfold` | Replaces float values `floatfacts` proves exact by constants. Also called from `fold`. | with `fold` | `floatfacts`, `floatbounds` |
| `canonical.rs` | (in `fold`) | Compares with the constant on the right and the predicate mirrored, no neutral terms, a zero test at the width tested. | with `fold` | |
| `decide.rs` | `decide` | A branch whose way is known is decided and the dead edge removed; unreachable blocks go and single-entry chains merge (`cfg.rs` `merged`). | `decide` option | `consts`, `constant_cycles`, `ranges` |
| `phiopt.rs` | `phiopt` | A phi of two constants chosen by a branch becomes arithmetic of the condition, where the target prices it no more than the branch; priced in bytes at -Os. | above -O0 | target `OperationCosts` |
| `gvn.rs` | `equalcalls`, `gvn` | Value numbering, store-to-load forwarding, load and expression PRE. `equalcalls` merges two equal calls that touch no memory, ahead of `tailrec`. See [gvn](gvn.md). | `-fgcse` for both; `-ftree-pre` for the insertions | `memoryssa`, `avail`, `alias`, `Counted`, spill model |
| `loadjoins.rs` | (in `gvn`) | A load whose bytes every predecessor already holds becomes a phi; with PRE a predecessor holding none gets a load on its edge where that speculates nothing. | with `gvn` | `avail`, `memoryssa` |
| `tailrec.rs` | `tailrec` | A self call in tail position becomes a branch to the entry with the arguments rewritten; an associative result (`return f(n-1) + x`) uses an accumulator. | `-foptimize-sibling-calls` | profit prices |
| `loopsimplify.rs` | `loopsimplify` | Gives each loop a preheader, one latch and dedicated exits. | above -O0 | `cfg::Shape` |
| `lcssa.rs`, `lcssamerges.rs` | `lcssa` | Loop-closed SSA: a value used outside its loop is read through a phi at each exit, merged by phis where exits join. | `lcssa` option | loops, dominators |
| `floatloop.rs` | `floatloop` | A counted loop of exact float work in memory is run once: its last trip, from the memory earlier trips leave. | `floatloop` option | `floatfacts` |
| `hoist.rs` | `hoist` | Loop-invariant work, including loads the loop does not write, moves to the preheader unless the function then prices higher because values held across the loop spill. | `-fmove-loop-invariants` | `memoryssa`, `induction`, `noreturn`, spill model |
| `loopmotion.rs` | `loopmotion` | A store nothing inside the loop observes is sunk to the loop's single exit. | with `hoist` | `memoryssa`, `regions` |
| `trivialunswitch.rs` | `trivialunswitch` | A branch on a loop-invariant condition in the header that leaves the loop on one side is decided before the loop; nothing is copied. | above -O0 | |
| `inferspace.rs` | `inferspace` | A far pointer made from a near one by `addrspacecast` is replaced by the near one in what only reads, writes or steps it, so no selector is loaded. | above -O0 | |
| `dse.rs` | `dse` | Removes a store every path overwrites before reading, or that nothing after it can read. See [dead-code](dead-code.md). | `-ftree-dse` | `avail`, `observers`, `Pointers` |
| `promote.rs` (`Promote`) | `promote` | A load of a cell a store left its value in on every path becomes that value, a phi where paths join; stores stay for `dse`. | `-ftree-sra` | `memoryssa`, `regions` |
| `splitcopy.rs` | (in `promote`) | A `memcpy` into a local nothing else reaches becomes the loads and stores of the bytes the local is read as. | with `promote` | |
| `indvars.rs` | `indvars` | Exit values of counters, final updates sunk to exits, an inner recurrence's exit value carried round the outer loop, widening of counters. Which counters a loop keeps is `lsr`'s. | `-fstrength-reduce` | `induction`, `Registers` |
| `loopexit.rs` | (in `indvars`) | Affine exit values evaluated at the backedge count; finite counted loops with no effect deleted. | with `indvars` | `induction` |
| `exitfold.rs` | (in `indvars`) | Exits a count decides are folded: one never taken is removed, one on an invariant is tested once before the loop, exits that leave to the same place merge into one test on the least count. | with `indvars` | `induction::exits` |
| `exitsink.rs` | (in `indvars`) | An add, subtract or extension in a loop read only through exit phis moves to those exits. | with `indvars` | `liveness` |
| `algebraic.rs` | `algebraic` | Integer identities modulo the width (InstCombine and reassociation of a loop-carried operand). A rewritten instruction drops `nsw`, `nuw`, `exact`. | `algebraic` | `ranges` |
| `dead.rs` | `dead` | Removes instructions nothing that stays reads. See [dead-code](dead-code.md). | `-ftree-dce` | `llrm_mir::memory` effects |
| `unroll.rs` | `unroll` | Full unroll of a loop with an exact trip count within the budget. See [unrolling](unrolling.md). | `-funroll-loops`; may grow only at -O3 | `peelsize`, `induction`, `consts` |
| `peel.rs` | `peel` | Clones every block of an exact loop once per trip, keeps the loop as a residual, and lets `fold` and `decide` prove it dead. | `-fpeel-loops`, -O3 | `peelsize` |
| `fill.rs` | `fill`, `merge` | A counted loop storing one byte value into consecutive bytes becomes `llvm.memset`; `merge` joins adjacent fills and copies. | `-ftree-loop-distribute-patterns`, not at -Og | `induction`, `consts` |
| `unswitch.rs` | `unswitch` | Specialises a loop around a pure invariant condition, accepted when the re-optimised copy removes a loop and prices lower at the target's costs. | `-funswitch-loops`, -O3 | `induction`, profit |
| `unroll_jam.rs` | `unroll-jam` | Unrolls a perfect two-loop nest's outer loop and fuses the inner copies; the factor is the largest the dependences allow, at most 4. | `-floop-unroll-and-jam`, -O3 | `dependence`, `induction` |

```
 tailrec                                     phiopt
 define i32 @f(i32 %n, i32 %acc)             br c, T, F ; J: phi [1,T] [0,F]
   ... %r = call @f(%n-1, %acc*%n)     =>        %p = zext i1 c to i32
   ret %r                                    (no branch, no phi)
 becomes a branch to the entry
```

## Interprocedural stage

`Interprocedural` (`interprocedural.rs`, name `mir interprocedural`) runs once every body has settled. It repeats until no body changes: inline, then constants across direct calls, then the clean-up of each changed body by `Fixed`. Its stages are named in `LLRM_DEBUG=runs` and listed with their per-level limits in [levels](../reference/levels.md). It re-derives module analyses after each edit; see [module-analyses](module-analyses.md), and for why summaries are not re-made at every splice, [early-summaries](early-summaries.md).

| File | Stage | What it does | Gate |
|---|---|---|---|
| `inline.rs` | `inline`, `ipa-inline`, `ipa-recursive` | Splits the caller at the call, clones the callee's blocks, binds parameters, joins the returns. Admission is cost: priced work copied against calls removed, in clocks (bytes at -Os), under `Threshold`. A body called once moves in. | `-finline-functions`, `-finline-functions-called-once` |
| `argpromotion.rs` | `promote.` | A pointer parameter only read at fixed offsets is replaced by the fields; callers load them. | above -O0 |
| `narrowspace.rs` | `narrow.` | A far pointer parameter every call fills from one near space becomes a near pointer; `inferspace` then folds the casts. | above -O0 |
| `interprocedural.rs` | `ipa<N>.`, `ipa-args` | A constant every call returns or passes becomes the function's value or parameter. | above -O0 |
| `ipacp.rs` | `ipa-cp.` | A function called with a constant is cloned for it; the clone may grow the unit only at -O3. Limits in [levels](../reference/levels.md). | `-fipa-cp-clone` |
| `interprocedural.rs` | `ipa-range` | States the callers' argument ranges on a body's parameters. | `-fipa-vrp`, -O2 and up |
| `deadargs.rs` | `ipa-deadargs` | A parameter nothing reads goes, and the argument at every call. | above -O0 |
| `interprocedural.rs` | `ipa-pure`, `ipa-noreturn` | Drops dead pure calls and the tails of calls that cannot return. | above -O0 |

## After the interprocedural stage

| File | Pass | What it does | Gate | Reads |
|---|---|---|---|---|
| `globaldce.rs` | `globaldce` | Deletes a module body or variable nothing live names; a declaration stays. | above -O0 | call graph |
| `calleepop.rs` | `calleepop` | An internal function whose callers are all seen pops its own arguments (`ret N`), and no call cleans up. Priced in clocks or bytes. | above -O0 | target calling description |
| `rotate.rs` | `rotate` | With `proven`, a loop proven to run once is entered at its body, its test merging into the latch. With `copy` (`-ftree-ch`, the default above -Os) an unproven loop is entered behind a copy of its test. `unroll` and `peel` refuse a rotated loop, so the proven form runs last. | header copy on at -O1 and up, not at -Os (`-fno-tree-ch` off; user decision 2026-10-12: compile +9.3% -O1, +14.0% -O2, slopes #1559); `proven` always | `induction::guards`, ranges |
| `fixednarrow.rs` | `fixednarrow` | `llvm.smul.fix(a, b, s)` whose 64-bit product `ranges` proves fits 32 bits becomes `mul` and `ashr`, avoiding the EAX:EDX form. | above -O0 | `ranges` |
| `lsr.rs`, `expand.rs`, `counting.rs`, `mirror.rs` | `lsr`, `differences` | Chooses each loop's induction variables once, by cost. See [lsr](lsr.md). `mirror.rs` gives a pointer to a counter that only some trips step, on targets whose addresses take no scale (not at -Os). `expand.rs` places a computed sum; `counting.rs` builds what a counted-loop rewrite puts before the loop. `algebraic::Differences` cleans bounds subtracted from rebased counters. | `-fstrength-reduce`; unbounded search at -Omax | `induction::Scev`, `guards`, spill model |
| `window.rs` | `window` | Splits a counted loop's huge pointers into windows with an outer loop, normalising each recurrence so no carry into the selector happens inside one. | `-fstrength-reduce` | `Machine::huge_window`, costs |
| `jumpthread.rs` | `jumpthread` | Threads a branch; from -O2 also a switch on a loop's state: each path into the join gets its own copy of the blocks down to the switch, jumping to the case its constant picks. | above -O0 | `consts`, dominators |
| `hoist.rs` | `hoist` (again) | A loop entered at its body runs it at least once, so what it loads unchanged may now leave. | `-fmove-loop-invariants` | |
| `gepoffset.rs` | `gepoffset` | `gep T, p, (i + 8) * 2` becomes a `gep` of the constant bytes under one of the rest, so selection takes a displacement. | `-fstrength-reduce` | |
| `addresssink.rs` | `addresssink` | A comparison read by a branch in another block is made again beside it, and a phi read only as an address whose arms compute the same thing becomes one computation in the join. | `-fstrength-reduce` | |
| `spares.rs` | `spares` | Keeps for selection, as `!llrm.spares` on a fixed-cell load, each write that leaves its bytes as they were. | above -O0 | `memoryssa` |
| `homes.rs` | `homes` | Marks a phi whose value is also stored (`!llrm.home`) so a spill reads the cell and needs no store of its own. | above -O0 | |

```
 gepoffset                                  inferspace
 gep T, p, (i + 8) * 2                      %w = addrspacecast p to addrspace(1)
   => gep T, (gep i8, p, 32), i * 2         %g = gep i8, %w, 4 ; load %g
                                              => %q = gep i8, %p, 4 ; load %q
```

The passes that follow, `assumptions`, `ehprepare`, floating-to-unsigned lowering, `selects` and `near code`, prepare MIR for instruction selection; they belong to the backend and are listed in [backend](backend.md).

## Support, not passes

| File | Holds |
|---|---|
| `lib.rs` | Module list; a pass asks `llrm-analysis` for facts and names nothing about the machine. |
| `transform.rs` | Shared helpers: liveness of values (`live`), the value-numbering key (`_computation`), `subexpressions`, `equal_calls`, `forwarded`, trivial-phi cleanup. |
| `edges.rs` | CFG edge surgery: splitting an edge, phis of a block, redirecting. |
| `cfg.rs` | `merged`: a block jumping unconditionally to a block it alone reaches takes its instructions. Called from `decide`; also a pass of its own name `merged`. |
| `loopclone.rs` | Clones a loop's blocks for peeling (`peeled`), chained partial unroll (`chained`) and unswitching. |
| `profit.rs` | Prices semantic work at the target's `OperationCosts`, weights blocks by frequency, and decides motions (`motion_price`). A kind without a price makes the answer unknown, not cheap. |
| `spill.rs` | The one spill model every pricing of register pressure shares: a value past the target's registers costs its stores and loads; `gvn`, `hoist`, `lsr` and `tailrec` use it. |
| `decide.rs` / `canonical.rs` | See the rounds above. |
| `testing.rs` | Test harness shared by the `*_tests.rs` files: parse MIR text, run a pass, print. |
| `interprocedural_stamped.txt` | Golden record of what the stamp states of each corpus body, read by the stamp test. |
