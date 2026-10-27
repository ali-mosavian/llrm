> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchphi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchphi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchphi.rs sha256:ff1a02300f47c3113e8369add3eff2b627842b73bce5cfb0736efbfbcb165257 -->

# switchphi

`forwardSwitchConditionToPHI`, a step of `simplifySwitch`
([L6511](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6511)),
with `findPHIForConditionForwarding`
([L6477](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6477)).

A phi input that is a case's constant, on the edge of that case, is the
switched value; so is the input of a phi behind an empty block the case
jumps to, when that makes two inputs alike or the phi already has the value.

## How it works
For each case, a phi of its destination whose one input from the switch's
block is the case's constant takes the switched value instead. A phi behind
an empty block that only the switch enters, whose input from that block is
the constant, is collected; once all cases are seen, such a phi is changed
if two of its inputs are collected or it already has the switched value.

## Example
```text
  switch i16 %x, label %d [ i16 1, label %m ]        m:
  m:                                                   %r = phi i16 [ %x, %b0 ], [ 0, %d ]
    %r = phi i16 [ 1, %b0 ], [ 0, %d ]
```

| # | DEVIATE |
|---|---|
| D1 | The forwarding phis are visited in the order they were found; LLVM keys a `DenseMap` by phi, and the phis are independent. |

## Used by
`switchopt::simplified`.
