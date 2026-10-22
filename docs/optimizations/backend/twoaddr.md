> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/twoaddr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/twoaddr.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/twoaddr.rs sha256:d33061bf64c584db96365de450c486704c2ce19b7315c2f31e8a32f7fc6d42bf -->

# twoaddr

Rewrites three-address arithmetic into x86's two-address form before
register allocation.

## How it works
An instruction that writes the register of its first source (`Binary`,
`Unary`, `Funnel`, and a one-destination `Multiply`) is split into a copy
and the instruction on the copy:

```text
 c := a + b          ==>      c := a
                              c := c + b
```

Three things happen first so the copy is less often needed.

- Update in place: `c := a + b` followed by the copy `a := c`, with `a` dead
  between, and `c` defined once and read only by that copy, becomes `a := a
  + b`. No interval grows and the copy the coalescer might refuse is gone.
- Commuting: for `add`, `and`, `or`, `xor` and `imul` the sources are
  swapped when that lets the destination reuse a source that dies. A swap is
  chosen when the second source is the destination already, when only the
  second source dies, or when neither lives on and the second is less
  blocked (by interference with the destination's copy neighbours) and
  closer by copies. Interference is computed only among the values a swap
  asks about. Before those, a sum that ends in a register (`requires`) keeps
  the operand delivered in it as the tied one (LLVM's physical-register rule
  in `isProfitableToCommute`), so `a + b` returned in EAX is `add eax, edx`.
- A memory destination whose cell is not the first source (`[m] := a + b`)
  is computed in a fresh register: load, operate, store.

A source that is not a value (an immediate) is copied the same way. The
inserted copies own no bytes of the input.

## Example
The body of a counted loop, before and after. Update in place made `v4 :=
add v5, v7; v5 := mov v4` into `v5 := add v5, v7`, leaving self-moves for
the coalescer:

```lir
; before (stage FloatAlloc)          ; after (stage TwoAddress)
block 0x0005                         block 0x0005
  v4 := add v5, v7                     v5 := add v5, v7
  v6 := add v7, 0x1                    v7 := add v7, 0x1
  v5 := mov v4   ; parallel-copy 2     v5 := mov v5   ; parallel-copy 2
  v7 := mov v6   ; parallel-copy 2     v7 := mov v7   ; parallel-copy 2
```

From `ISEL_DUMP=1` of unit test `test_a_counted_loop` in `isel_tests.rs`.

## Runs
A machine phase after `floatalloc` and before `coalesce`
(`flow::machine_with`), at every level. The coalescer then removes the
copies the allocator can share a register across.

## See also
LLVM's
[`TwoAddressInstructionPass`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp).
