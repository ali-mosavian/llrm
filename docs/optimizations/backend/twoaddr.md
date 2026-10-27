> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/twoaddr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/twoaddr.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/twoaddr.rs sha256:630c5b3e870b020bae60dabaa34140ea145e4da8990b73b46c206099910a74cf -->

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
- Commuting (`twoaddress::Commuter`, `tryInstructionCommute`): for `add`,
  `and`, `or`, `xor` and `imul` the sources are swapped when the second dies
  here and the first does not, or when `isProfitableToCommute` says so: a
  register the sum is to end in, a use of the first between its definition
  and here, a copy chain, or the second defined later. A sum that ends in a
  register (`requires`) keeps the operand delivered in it as the tied one,
  so `a + b` returned in EAX is `add eax, edx`.
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
[`TwoAddressInstructionPass`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp),
translated in `twoaddress.rs` for the commuting; the rest is this file's
own.
