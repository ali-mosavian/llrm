> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_binop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_binop.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_binop.rs sha256:b150a77bebfb474077f2b40dc71dead801f077d93fa05a25c59154e36843acec -->

# combine_icmp_binop

InstCombine's `foldICmpBinOp`, the add and subtract arms
([`InstCombineCompares.cpp:5301`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L5301)
at d1106deb71cc).

MIRROR: the order of the steps and each condition, up to the `Xor`/`Or`
folds.

OWED, in place as `// OWED:` comments: the equality forms of
`foldICmpOrXX`, and every arm after `srem` (the shared second operand,
`udiv`, `shl`, `lshr`, `and`...).

DEVIATE 1: `ShareCommonDivisor` takes only its fast path (the offset is 1,
or -1 for the negative forms); the general case needs `isMultipleOf`, which
MIR's value tracking does not have. Exit: `isMultipleOf`.

DEVIATE 2: no vectors; the constants of the equality fold are integers.

DEVIATE 3: the shared multiplier asks `simplifyICmpInst` without the
instruction as its context.

## How it works
With `Op0 = A + B` and `Op1 = C + D` (an `or disjoint` is an add), a
relational compare that sees one side offset by one steps the strictness
instead and drops the offset: `A + 1 s> Y` is `A s>= Y`, `X s> C + -1` is
`X s>= C`. The offset must not wrap in the compare's signedness.

## Example
The sift loop of x_heapsort kept `or disjoint %x, 1` live for its guard:

```text
%l = or disjoint i32 %x, 1   ; %c = icmp sge i32 %x, %n
%c = icmp sgt i32 %l, %n
```

## Used by
Named by `combine_compares`.
