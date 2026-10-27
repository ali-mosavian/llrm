> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/expand.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/expand.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/expand.rs sha256:d3b4ec1592f3fcc2e3e8a01aaa525423dae29ad4bd5f09c3247f9ddd2aefcaeb -->

# expand

Values placed from their sums (LLVM's `SCEVExpander`): a `Scev` is built
from its terms, each scaled and added once, before one instruction.

## How it works
An `Expander` places everything before the instruction it was made for,
normally the branch ending a loop's preheader, and remembers what it made,
by sum and type, so the same sum or monomial is built once.

`int` builds a sum of terms `factor * (v1 * v2 * ...)` plus a constant. Each
monomial's values are multiplied (a value wider than the sum is truncated
first, one narrower is zero-extended: only an extended recurrence's start
names one, after its proof); the product is scaled by the factor's magnitude
(`scaled`: nothing for 1, `shl` for a power of two, else `mul`); positive
terms are added, negative ones subtracted afterwards, then the constant is
added. Where no term is variable the result is the constant.

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

## Example
None: `expand` is infrastructure and has no pass-level before and after of
its own. Its `test_generated_recurrences_expand_to_what_the_loop_computes`
in `expand_tests.rs` checks expanded values against the interpreter; the
passes that use it show it at work, see [`crate::lsr_pass`](lsr_pass.md).

## Used by
[`crate::lsr_pass`](lsr_pass.md), while it rewrites a loop, and
[`crate::counting`](counting.md), which builds its seeds with it. Not a pass: it runs
only inside those.

## See also
[`crate::counting`](counting.md).
