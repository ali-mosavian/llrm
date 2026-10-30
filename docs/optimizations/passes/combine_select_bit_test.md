> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_bit_test.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_bit_test.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_bit_test.rs sha256:ca828287ca6f65c7606f77329361186b5c1e705b1b68f37dcc26593c7657848d -->

# combine_select_bit_test

`foldSelectBitTest`: `select (icmp eq (and x, 2^k), 0), TC, FC` as the
`and` shifted and combined with a constant.

Translation of
[`foldSelectBitTest`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L4422)
and
[`foldSelectICmpAnd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L153)
at d1106deb71cc.

DEVIATE 1: no vectors.

DEVIATE 2 (temporary): `foldSelectICmpAndBinOp` is not made.

Also `foldSelectICmpAndAnd`
([L752](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L752)):
`select ((x & y) == 0), (x >> z) & 1, 1` as `zext ((x & (y | 1 << z)) !=
0)`.

## Example
```text
%a = and i32 %v, 2                          %a = and i32 %v, 2
%c = icmp eq i32 %a, 0               ==>    %r = xor i32 %a, 6
%r = select i1 %c, i32 6, i32 4
```

## Used by
Named by `combine_visit_select`.
