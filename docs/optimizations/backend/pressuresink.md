> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/pressuresink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/pressuresink.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/pressuresink.rs sha256:adb4b07e08c1cc7ecec86238dab24221cc55260668a5f547e093549488aa540f -->

# pressuresink

Moves a load down to just before its one reader when that raises the
pressure of no register class and lowers it for some.

## How it works
A value defined late is live a short while; LLVM's scheduler works bottom-up
for the same reason. The sink runs before allocation, when each value's
class is what [`crate::backend::regclass`](regclass.md) says confines it (a byte
register, an address base or index, a selector) or the x87 stack, and loads
of different classes differ in nothing but those sets.

A call is no float-region boundary when x87 values live in registers the
allocator spills (`rfp_on`): a float argument is read after the call
instead of spilled across it.

Within a block, from the last instruction up, a load `d = load [a]` is a
candidate when it is plain (a move or float load of a cell, not volatile, no
group, no fixed registers or clobbers, not a call), `d` is defined once and
is not live out of the block, and exactly one later instruction `u` reads
`d`, not at once after the load and not part of a group. Everything between
the load and `u` must be able to stay ahead of it: no float-region boundary,
no volatile access, no barrier, nothing that may write the loaded cell
(`may_write`), nothing that redefines `d` or an address operand of the load.
Where all that lies between are loads feeding `u` itself, the order is
immaterial and nothing moves.

Moving the load takes `d` out of its class over the stretch, and keeps each
address operand that dies at the load live until `u`. For a class `S` the
count of live values confined within `S` changes by `-[d in S] + #{address
operands in S that die at the load}`:

```text
 before                      after
 d = load [a]    a dies      x = ...
 x = ...                     y = ...
 y = ...                     d = load [a]   (at u's position)
 u = op d, x                 u = op d, x
```

The load moves where no class rises and at least one falls; the sets tried
are the general set, the x87 stack and the classes of `d` and of those dying
address operands. The moved load takes `u`'s `at`, so a block's instructions
stay in order by it.

DEVIATE P1 (#2903): no pin counterpart. LLVM orders loads by the scheduler;
exit: the pre-RA machine scheduler translated (target 2026-11-20), then this
file is deleted.

## Example
None: no test fixture exercises `sunk` on a body whose LIR it prints, and
the sketch above is not program output. Read `sunk` for the candidate test.

## Runs
A machine phase after `phielim` and before `floatfold`
(`flow::machine_with`), in every machine, before values are placed.

## Needs
Classes from [`crate::backend::regclass`](regclass.md), liveness from
[`crate::backend::allocate`](allocate.md) and the store check `may_write` of
[`crate::backend::storedhomes`](storedhomes.md).
