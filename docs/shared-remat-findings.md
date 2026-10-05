# Sharing the remat predicate with Greedy (#441): what was found

`spiller::_copies` states once that a value is a plain same-width copy of another; SsaSpill's `remakable` and Greedy's
`planned` both read it (a copy of a remade value is made again as its source is). The branch has all copies on.

Against main, `tools/bench`: fib nib -O2 memory operands -17%, frames -7..-13%, queens nib -10%, quicksort nib -Os -10%.
Worse: lru bas +1 instruction, queens c -O2/-Os +552 instructions (-1024 memory operands), grep bas +6 code bytes.
Constants and addresses through copies change nothing. Copies of global loads only: bench neutral, deedlines
`INITCROSFADEPICS` +3078 instructions by the cost estimate (not measured executed).

## queens c, `_place`

Greedy-alone wins (SsaSpill's route is 279 instructions, Greedy's 218/219). The spill sequence is the same as main's:
spill 1, 7, 8, 2, 12, 3 in the first round, `split 3 at {0: [(1,4)], 2: [(0,11)]}`. What differs is the listing at entry:

    main:  mov ax,[bp+6] ; cmp ax,[bp+8]
    here:  mov si,[bp+4] ; mov bx,[bp+6] ; mov ax,[bp+8] ; cmp bx,ax

The entry `cmp` reads the copies of arguments 2 and 3, so their defs are not dead; arg 3 stays in `ax` across the entry and
is split later. Main spilled it at the entry, and `materialized` folded the cell into the `cmp`. No pass lacks the fold:
unit probes (spiller tests, `cmp` of two arg-slot loads, spilled one at a time, together, and through copies) all emit
`mov r,[cell] ; cmp r,[cell]`. comparefold's `_plain` rejected a `rematerialized` load (fixed in #485, no bench change).

## lru bas

The arg-slot load `mov v,[bp+6]` stays at function entry and `push ax` is far from it, so `fused_push` (adjacent load and
push) cannot fuse; main reloaded next to the push (`push [bp+6]`).

## Greedy's weight

`analysis/intervals.rs::_weights` is references x block frequency / (size + GRACE), with `_fold_priced` discounting
foldable spills. It has no rematerializable halving (LLVM's `weightCalcHelper` x0.5). Added in `Facts::of` for every value
`_remakable_values` names: 50 more rows worse (bintree c memory operands +4%, nbody_fixed +6%, queens bas, lru c), queens c unchanged.

## Where to pick up

Make the entry copies' first use rebuild too (arg 3 spilled at the entry `cmp`), then the shape matches main's. Measure
executed counts (bench), not sizes.py estimates.
