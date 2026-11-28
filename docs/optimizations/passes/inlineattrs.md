> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/inlineattrs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/inlineattrs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/inlineattrs.rs sha256:6facc24889107ad93bfcab077a79368270883a23940bdd6c701216d118908da2 -->

# inlineattrs

What `InlineFunction` carries from the call site onto the copy of the body:
`AddParamAndFnBasicAttributes`
([L1473](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp#L1473))
and `AddReturnAttributes`
([L1652](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp#L1652)).

## How it works
A call in the body that passes one of the callee's own parameters on gets
the attributes the site states for that parameter (`nonnull`,
`dereferenceable`, `dereferenceable_or_null`, `align`, and `range` where the
call has none); a pointer into a parameter's object gets its `readonly` and
`readnone`. A call whose value the callee returns, in the return's block
with nothing that may leave between, gets the return attributes the site
states.

## Deviations
- IA1 (owner w9, exit 2026-11-30): `nofpclass` is not an attribute here, and
  two `range`s are not intersected (the existing one stays). A call that may
  throw or not return between the value and the return is any call or
  invoke, not `isGuaranteedToTransferExecutionToSuccessor` (no callee
  attributes at hand).
- A copy is never simplified while it is made (`isSimplified`): the check is
  always false.

## Example
A call in the inlined body that passes on one of the callee's parameters
gets the attributes the call site states for that argument.

```text
call site:  call void @f(ptr nonnull dereferenceable(8) %p)
callee:     define void @f(ptr %a) { call void @g(ptr %a) }
copy:       call void @g(ptr nonnull dereferenceable(8) %p)
```

## Used by
Named by `inline`.
