> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/mirror.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/mirror.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/mirror.rs sha256:9b54e13433baec32dc2cd42bc6143768e7d0b4ef02c7d26f76dd42ee38292f1f -->

# mirror

Turns an index that only some trips step into a pointer that steps with it,
so a target whose addresses take no scale `k` does not shift the index on
every access of `base + k * i`.

## How it works
In `if (a[j] < p) { ...; ++i; }` the counter `i` is stepped on some paths
of the loop and joined, so it is no counter for `crate::lsr`, which
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

## Example
Before:
```mir
define ... @"_keep@3"(ptr %0, ptr %1, i16 %2, i16 %3) ... {
  ; ...
b2:
  %4 = phi i16 [ 0, %b1 ], [ %13, %b5 ]
  %5 = phi i16 [ 0, %b1 ], [ %12, %b5 ]
  ; ...
b5:
  %12 = phi i16 [ %5, %b4 ], [ %16, %b6 ]
  %13 = add nsw i16 %4, 1
  br label %b2

b6:
  %14 = mul i16 %5, 2
  %15 = getelementptr inbounds i8, ptr %1, i16 %14
  store i16 %10, ptr %15, !tbaa !9
  %16 = add nsw i16 %5, 1
  br label %b5
}
```
After:
```mir
define ... @"_keep@3"(ptr %0, ptr %1, i16 %2, i16 %3) ... {
b1:
  %4 = getelementptr i8, ptr %1, i16 0
  ; ...
b4:
  %10 = phi i16 [ %16, %b5 ], [ 0, %20 ]
  %11 = phi ptr [ %15, %b5 ], [ %4, %20 ]
  %lsr.iv1 = phi i16 [ %lsr.iv.next, %b5 ], [ %6, %20 ]
  ; ...
b5:
  %15 = phi ptr [ %11, %b4 ], [ %19, %b6 ]
  %16 = phi i16 [ %10, %b4 ], [ %18, %b6 ]
  ; ...
b6:
  store i16 %13, ptr %11, !tbaa !9
  %18 = add nsw i16 %10, 1
  %19 = getelementptr i8, ptr %11, i16 2
  br label %b5
  ; ...
}
```
`i` is stepped only on the trips that store, so `lsr` could not reduce
`out[i]`. `%11` is the same cycle over pointers: it starts at `out` and
steps by 2 where `i` steps by 1. `i` stays for the return value.

From `--dump` of `keep` (`if (a[j] < p) { out[i] = a[j]; ++i; }` in a loop
over `j`) at -O2 -m16, stages `fixednarrow` and `lsr` (the other changes of
`lsr` are left out).

## Runs
Not a pass: `crate::lsr` calls `found` and `applied` on each loop before
its counters are chosen, then chooses again. Not at `-Os`, and with
`-fstrength-reduce` off nothing runs. Where the target's addresses take the
scale (`AddressForm::scales`) there is nothing to save and the loop is left.

## See also
gcc's ivopts and LLVM's LSR reduce an affine counter only. The effect on
quicksort at -m16 is recorded in `docs/reference/levels.md`.
