> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/rmw.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/rmw.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/rmw.rs sha256:7accb7dcdf3fbca013ef6309f4054837ef452f6543d73ce9faac537297f0871d -->

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
A load of a cell, an `add` of a value, a store back (instruction 0x0003 is
an anchor that carries no operation; `[None v1@unplaced]` is the cell
`[v1]`, `[None]` the second load's cell):

```lir
=== before
    block 0x0000
      0x0000  v2 := mov [None v1@unplaced]  ; inserted
      0x0001  v3 := mov [None]  ; inserted
      0x0002  v4 := add v2, v3  ; inserted
      0x0003     ; inserted
      0x0004  [None v1@unplaced] := mov v4  ; inserted
=== after
    block 0x0000
      0x0000     ; inserted
      0x0001  v3 := mov [None]  ; inserted
      0x0002     ; inserted
      0x0003     ; inserted
      0x0004  [None v1@unplaced] := add [None v1@unplaced], v3  ; inserted
```

The load, the `add` and the store became one `add [v1], v3`, and `v2` and
`v4` no longer exist.

From unit test `test_private_integer_update_selects_a_memory_destination` in
`rmw.rs`.

## Runs
During instruction selection, in the combining step of `isel/combined.rs`,
after `comparefold` and `stepflags` and before the argument selections of
`peephole.peep`. Its input is SSA over virtual registers.

## Keeps
The result has the same blocks. A folded store defines no value, so the
values of the load and operation are gone, not just unused.
