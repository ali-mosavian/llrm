> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/expand.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/expand.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/expand.rs sha256:e8153cb2718fae5f95f00caf594ba9debd3b2dfc3675387144f6b1ce7b095d59 -->

# expand

Values placed from their sums (LLVM's `SCEVExpander`): a `Scev` is built
from its terms, each scaled and added once, before one instruction.

## How it works
An `Expander` places everything before the instruction it was made for,
normally the branch ending a loop's preheader, and remembers what it made,
by sum and type, so the same sum or monomial is built once.

`int` builds a sum of terms `factor * (v1 * v2 * ...)` plus a constant. Each
monomial's values are multiplied (a value wider than the sum is truncated
first); the product is scaled by the factor's magnitude (`scaled`: nothing
for 1, `shl` for a power of two, else `mul`); positive terms are added,
negative ones subtracted afterwards, then the constant is added. Where no
term is variable the result is the constant.

```text
 sum = 4*a*b + 3*c - 7
 %ab  = mul a, b
 %t1  = shl %ab, 2
 %t2  = mul c, 3
 %s   = add %t1, %t2
 %r   = sub %s, 7        (one build; a repeat of the sum reads %r)
```

`value` is `pointer + sum` as an `i8` GEP (or the integer sum where there is
no pointer). `least` is the least of several sums as unsigned numbers, an
`icmp ult` and a `select` per further sum.

## Used by
[`crate::lsr`](lsr.md) and [`crate::exitfold`](exitfold.md), while they rewrite a loop, and
[`crate::counting`](counting.md), which builds its seeds with it. Not a pass: it runs
only inside those.

## See also
[`crate::counting`](counting.md).
