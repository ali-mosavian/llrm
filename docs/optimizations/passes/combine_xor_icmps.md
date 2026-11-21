> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_xor_icmps.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_xor_icmps.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_xor_icmps.rs sha256:db180eb00da94296fcc28d81814470179c36a8b967f3f014f63960bbb708e359 -->

# combine_xor_icmps

`foldXorOfICmps`: an `xor` of two `i1` compares, folded to one compare.

Translation of
[`foldXorOfICmps`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L4864)
at d1106deb71cc.

## How it works
In order: two compares of the same operands merge by their compare codes;
two sign-bit tests are one sign-bit test of the `xor` of the values; two
compares of one value with constants are one range test where the range
type holds `(a | b) & !(a & b)` exactly; two single-bit tests under one
power of two are one test of the `xor`. Last, when `or` and `and` of the
pair fold (`simplifyBinOp`) to one side and the other, `a ^ b` is
`a & !b`, which inverts `b`'s predicate in place.

## Example
Whether two values differ in sign:

```text
%a = icmp sgt i32 %x, -1                    %v = xor i32 %x, %y
%b = icmp sgt i32 %y, -1          ==>       %r = icmp slt i32 %v, 0
%r = xor i1 %a, %b
```

MIRROR: the C++ order and the one-use conditions.

DEVIATE 1: scalars only; the pin matches splat vector constants
(`m_APInt`). Exit: the vector `ConstantRange`.

## Used by
`visit_xor`.
