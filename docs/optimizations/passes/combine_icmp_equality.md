> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_equality.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_equality.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_equality.rs sha256:b4c6bcc1adb6dbce5562c2dfcc2646ba3ac157aae5d64db35b216a84bce45d8b -->

# combine_icmp_equality

InstCombine's `foldICmpEquality`, from
[`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L6222)
at d1106deb71cc.

`fold_icmp_equality` is the pin's function: the same folds in the same
order, each arm commented with the C++ it translates. An arm that matched
and did not return falls through, as there. `visit_icmp` calls it where
`visitICmpInst` does.

MIRROR: the order of the arms and the conditions of each.

DEVIATE: no vectors. `foldICmpEqualityWithOffset` simplifies without the
instruction as context (no dominating conditions or assumptions: the same
folds or fewer), and the select it builds does not copy the old select's
metadata.

## Example
`(b & 255) == zext a` is `a == trunc b`:

```text
%m = and i16 %b, 255   ; %t = trunc i16 %b to i8
%z = zext i8 %a to i16 ; %r = icmp eq i8 %a, %t
%r = icmp eq i16 %m, %z
```

## Used by
Named by `combine_compares`.
