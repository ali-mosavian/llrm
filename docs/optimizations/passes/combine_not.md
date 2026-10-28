> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_not.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_not.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_not.rs sha256:560ae17f3902294574737e867145ac8e93a5594a0fff9c3b487886f4ddb5f727 -->

# combine_not

`InstCombinerImpl::foldNot`: a `not` (`xor X, -1`) pushed into its operand.

Translation of
[`foldNot`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L5237)
at d1106deb71cc, called by `visit_xor` where `visitXor` calls it.

MIRROR: the order of the arms, and the one-use condition of each.

DEVIATE 1: no vectors, and only integer constants are inverted.

DEVIATE 2: owed, each where it stood: the `bitcast` arms (a `not` of a
`bitcast` of a compare, of a `sext` of a bool), `sinkNotIntoLogicalOp`,
min/max and `is_fpclass` intrinsics, `foldNotXor`, `getFreelyInverted`, a
`not` of a compare with several uses (`canFreelyInvertAllUsersOf`, now
inline in `combine_compares`), and `fcmp`.

## Example
`~(X + 5)` is `-6 - X`:

```text
%a = add i32 %x, 5
%r = xor i32 %a, -1   ==>   %r = sub i32 -6, %x
```

## Used by
Named by `combine_xor`.
