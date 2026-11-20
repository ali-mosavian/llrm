> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_not.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_not.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_not.rs sha256:bda2d10949698cd3d3446c8f75c560f4da30410c128647c198f5322ac21cb132 -->

# combine_not

`InstCombinerImpl::foldNot`: a `not` (`xor X, -1`) pushed into its operand.

Translation of
[`foldNot`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L5237)
at d1106deb71cc, called by `visit_xor` where `visitXor` calls it.

MIRROR: the order of the arms, and the one-use condition of each.

DEVIATE 1: no vectors, and only integer constants are inverted.

DEVIATE 2: owed, each where it stood: the `bitcast` arms (a `not` of a
`bitcast` of a compare, of a `sext` of a bool), min/max and `is_fpclass`
intrinsics, `foldNotXor` and `fcmp`. `sinkNotIntoLogicalOp` is in, with
`canFreelyInvertAllUsersOf` and `freelyInvertAllUsersOf` of
`combine_invert`; a phi is never freely inverted there (no insertion point
after its definition).

## Example
`~(X + 5)` is `-6 - X`:

```text
%a = add i32 %x, 5
%r = xor i32 %a, -1   ==>   %r = sub i32 -6, %x
```

## Used by
Named by `combine_and`, `combine_or`, `combine_visit_select`, `combine_xor`.
