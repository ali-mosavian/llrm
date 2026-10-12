> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/twoaddr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/twoaddr.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/twoaddr.rs sha256:b38baa3e3aec52c2a68a61dc31a3c994443063ad72679abdf023967d8eae4cb3 -->

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
  asks about.
- A memory destination whose cell is not the first source (`[m] := a + b`)
  is computed in a fresh register: load, operate, store.

A source that is not a value (an immediate) is copied the same way. The
inserted copies own no bytes of the input.

## Runs
A machine phase after `floatalloc` and before `coalesce`
(`flow::machine_with`), at every level. The coalescer then removes the
copies the allocator can share a register across.

## See also
LLVM's
[`TwoAddressInstructionPass`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp).
