> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_phi_args.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_phi_args.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_phi_args.rs sha256:cfdd41e23f630bf04f0d42e3c094980c3b3044d91a932548e840d5a1045cad8b -->

# combine_phi_args

`visitPHINode`'s argument folds: a phi of the same operation applied to
different values is the operation applied to a phi of those values.

Translation of `foldPHIArgZextsIntoPHI` and `foldPHIArgOpIntoPHI`
([InstCombinePHI.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombinePHI.cpp#L779))
at d1106deb71cc.

## How it works
- Three or more incoming values that are `zext`s of one type (each with one
  user) and at least one constant that fits that type become a phi of the
  narrow type and one `zext` of it.
- Two or more incoming values that are all the same cast, or all the same
  binary operator or compare with one constant right operand, each with one
  user, become the operation applied to a phi of the left operands (or to
  the one left operand all of them share). The new operation goes at the top
  of the phi's block, after the phis.

## Example
```text
%a = zext i1 %x to i32 ; %b = zext i1 %y to i32
%p = phi i32 [ %a, %l ], [ %b, %r ]    ==>  %p.in = phi i1 [ %x, %l ], [ %y, %r ]
                                            %p = zext i1 %p.in to i32
```

MIRROR: the opcode and one-user tests, `isSameOperationAs` (opcode, type,
operand types, predicate), `shouldChangeType` for integer casts, the
constant right operand, the intersection of the binary operators' flags.

DEVIATE 1: the rest of `visitPHINode` is not translated: the GEP, load,
`insertvalue`/`extractvalue` and binary-operator-of-two-variables forms of
`foldPHIArgOpIntoPHI`, `foldPHIArgIntToPtrToPHI`, the pointer-cast phi,
`foldDeadPhiWeb`, `foldIntegerTypedPHI`, `SliceUpIllegalIntegerPHI`,
and `foldDependentIVs`. Exit: later slices.

DEVIATE 2: no check that the block's terminator is an EH pad, which MIR
cannot have a phi before; no debug locations.

## Used by
`combine` (its `visit`, the `Phi` opcode).
