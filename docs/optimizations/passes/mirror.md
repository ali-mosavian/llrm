> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/mirror.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/mirror.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/mirror.rs sha256:84d08457cdbb23ea0a5824e2494dd3860dd263feb49cb1a7226f9bd084ed2319 -->

# mirror

A pointer that follows a counter which only some trips step: the address
`base + k * i` on a target whose addresses take no scale `k`.

## How it works
In `if (a[j] < p) { ...; ++i; }` the counter `i` is stepped on some paths
of the loop and joined, so it is no counter for [`crate::lsr`](lsr.md), which
reduces `i * k` only for counters every trip steps. Without a scale every
`a[i]` is a shift (`lea bx,[i+i]`) and a base register, where the loop's
other accesses want that register too.

`induction::step_cycle` finds the cycle: a header phi whose latch value is
a join of adds of constants back to the phi. `found` accepts a loop with
no loop inside it and a preheader that only enters it, and a cycle of the
native index width whose values `y` are scaled (`mul y, k` or `shl y, s`,
with `k > 1` and no scale of `AddressForm::scales` for it) into byte
`getelementptr`s from one invariant, flat pointer (not a far or huge one,
which carries). The family with most readers is taken.

`applied` then builds the same cycle over pointers: a phi where the cycle
has a phi, and `gep p, step * k` where it has `add y, step`. The pointer
phi starts at `base + start * k`, built in the preheader. Each reading
`gep` is replaced by the pointer of the node it read.

```text
  before                              after
  i = phi [0, pre] [i2, latch]        p = phi [base, pre] [p2, latch]
  ...                                 ...
  o = i * 2 ; q = gep base, o         (q is p)
  if (c) { i1 = i + 1 }               if (c) { p1 = gep p, 2 }
  i2 = phi [i, ..] [i1, ..]           p2 = phi [p, ..] [p1, ..]
```

The new geps are not `inbounds`: the pointer moves with every step of `i`,
one of which may leave the object where no access follows. The counter
stays for whatever else reads it; a loop that reads it nowhere loses it to
`dead`.

## Runs
Not a pass: [`crate::lsr`](lsr.md) calls `found` and `applied` on each loop before
its counters are chosen, then chooses again. Not at `-Os`, and with
`-fstrength-reduce` off nothing runs. Where the target's addresses take the
scale (`AddressForm::scales`) there is nothing to save and the loop is left.

## See also
gcc's ivopts and LLVM's LSR reduce an affine counter only. The effect on
quicksort at -m16 is recorded in `docs/reference/levels.md`.
