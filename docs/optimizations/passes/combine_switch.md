> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_switch.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_switch.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_switch.rs sha256:66befa4a5166aec73a479cb38000ff75d84c3816f401e310be5d815d3b05b9cb -->

# combine_switch

InstCombine's `switch`: `InstCombinerImpl::visitSwitchInst`
([InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L4470)).

## How it works
A condition computed by an invertible operation on a value (`x + c`,
`c - x`, `x ^ c`, `x << c`, an extension) is replaced by the value, the
case values mapped through the inverse. A condition whose high bits are
known, and are the same in every case, is truncated to the bits that vary.

## Example
```text
switch i32 %n, ...  [i32 5, ...]   ==>   switch i32 %x, ...  [i32 3, ...]
; %n = add i32 %x, 2
```

MIRROR: the order of the folds and each guard.

DEVIATE 1: `simplifySwitchOnSelectUsingRanges` is not translated. Exit
2026-11-12.

DEVIATE 2: `handlePotentiallyDeadSuccessors` for a constant or undef
condition is not here; the CFG simplifiers fold a switch on a constant.

DEVIATE 3: conditions wider than 128 bits are left alone (case values are
`u128`).

## Used by
`combine` (its dispatch on `Opcode::Switch`).
