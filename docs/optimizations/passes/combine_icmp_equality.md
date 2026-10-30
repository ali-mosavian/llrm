> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_equality.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_equality.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_equality.rs sha256:68017cbdcb7d959d5a1e15a47c775a6aa7ab3a8f37bf9409f82d3f034c646c92 -->

# combine_icmp_equality

InstCombine's `foldICmpEquality`
([`InstCombineCompares.cpp:6222`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L6222))
at d1106deb71cc, up to the `ashr`/`trunc` range check.

MIRROR: the order of the arms and each condition. Like the pin, the shift
pair returns `None` for the whole function when its amounts differ.

OWED, in place as `// OWED:` comments: `foldICmpIntrinsicWithIntrinsic`,
the power-of-two canonicalizations, the `fshr` rotate, the `xor` constant
move, the add/sub/xor power-of-two mask and `foldICmpEqualityWithOffset`.

DEVIATE: the constants are integers (MIR has no vectors), and a constant of
the `or` arm that is a constant expression does not match.

## How it works
An equality compare whose operands share structure drops the shared part:
`(A^B) == A` is `B == 0`, `(X&Z) == (Y&Z)` is `((X^Y)&Z) == 0`, and a
masked value against a zero extension `(B & 255) == zext A` is the narrow
compare `A == trunc B`.

## Example
The test of a byte against a wider value, as `strchr` writes it:

```text
%a = and i16 %c, 255          ; %t = trunc i16 %c to i8
%z = zext i8 %y to i16        ; %r = icmp eq i8 %y, %t
%r = icmp eq i16 %a, %z
```

## Used by
Named by `combine_compares`.
