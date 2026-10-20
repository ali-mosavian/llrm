> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchphi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchphi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchphi.rs sha256:d179519ea18845b1b249c16ea14c73af842c0f8c82e8d4edfd819498513c72e9 -->

# switchphi

`forwardSwitchConditionToPHI`, a step of `simplifySwitch`
([L6511](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6511)),
with `findPHIForConditionForwarding`
([L6477](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6477)).

A phi input that is a case's constant, on the edge of that case, is the
switched value; so is the input of a phi behind an empty block the case
jumps to, when that makes two inputs alike or the phi already has the value.

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
