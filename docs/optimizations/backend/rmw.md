> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/rmw.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/rmw.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/rmw.rs sha256:2f14839dd7ea483e4f488c4f18e9b8873a1266ba7594e8cc5d186c73dcad5819 -->

# rmw

Selects a private load, update and store of one cell as a single
memory-destination instruction, before register allocation.

## How it works
`selected` makes two scans over a block's instructions, with the use count
of every value. Both match backwards from a store and replace the chain by
one instruction on the cell; the chain's other instructions become anchors
that keep their source ownership but no values.

The integer fold (`_fold_integer`) finds `mov [cell], r` where `r` is the
one-use result of `add`, `sub`, `and`, `or` or `xor`, one operand of which
is the one-use load of the same cell, of width 1, 2 or 4. A commutative
operation may have the load on either side. The result is `op [cell], src`.
The byte fold (`_fold_byte`) finds C's `byte |= mask` after integer
promotion and truncation, and gives `or byte [cell], mask`:

```text
 integer:  a = mov [m]          byte:  a = movzx [m]    (byte load)
           r = op a, b                 w = movzx mask   (byte to word)
           mov [m], r                  j = or a, w
              =>                       n = movzx j      (word to byte)
           op [m], b                   mov [m], n
                                          =>
                                       or [m], mask     (byte)
```

A chain is taken only when every instruction is plain (no fixed registers,
no clobbers, no group, not a spill or frame adjustment, not volatile), each
intermediate value has exactly one use, and nothing between the load and the
store can observe or change memory: only anchors, or computations whose
operands and results are values, no memory. The load is moved down to the
store, so an intervening read, write, call or branch stops the fold.

## Example
`*p += x;` is a load of `[p]`, an `add` and a store back. The store became
`add [t1], t2`; the load and the `add` stay as `elided` anchors and
their values no longer exist. No dump shows the body before selection, so
only the result is shown:

```lir
fn _f@3(t1@ax, t2@dx)

B0  entry  → exit                                                 ×1.0
     #2               elided
     #3               elided
     #4               add.i16     [t1], t2
     #5               ret
```

From `ISEL_DUMP=1 llrm-c -m16 -O2` of `void f(int *p, int x) { *p += x; }`.

## Runs
During instruction selection, in the combining step of `isel/combined.rs`,
after `comparefold` and `stepflags` and before the argument selections of
`peephole.peep`. Its input is SSA over virtual registers.

## Keeps
The result has the same blocks. A folded store defines no value, so the
values of the load and operation are gone, not just unused.
