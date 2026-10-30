> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/remidiom.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/remidiom.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/remidiom.rs sha256:cb7fa1cd0b3d79d4d179e999eb4259defe7666869380cddfa7e0dce2b5c03144 -->

# remidiom

The remainder idiom `x - (x / c) * c` read back as `x % c`, where the target
keeps the divide.

## How it works
Loop strength reduction expands a remainder by a constant, as the pin's
does: ScalarEvolution models `urem x, c` as `x - (x /u c) * c`, and LSR
writes that out, narrowed to the counter it chose (`sub i16 %n, mul (trunc
(udiv i32 %x, 251)), 251`). A target that lowers the division to a
multiply-high is served; one that keeps the `div` (a word, or any size at
-Os) pays a divide, a multiply and a subtract where the divide's own
remainder was one instruction. Here the idiom is found after LSR and
replaced by `urem`/`srem` (and a `trunc`), whose selection reads the
divide's remainder.

The quotient, its `trunc` and the product must have no other user (a second
user is `DivRemPairs`' case, and a remainder beside a live quotient
is a second divide). The subtrahend's left operand is `x` itself, or a
counter ScalarEvolution proves is `trunc x + k` for a constant `k` (`x % c +
1` becomes `(x + 1) - (x / c) * c` once LSR picks the counter `x + 1`); the
fold then adds `k` to the remainder.

DEVIATE REM1: the pin has this fold in the DAG (`foldRemainderIdiom`,
[DAGCombiner.cpp#L4215](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4215)),
on a `UDIVREM` node, and not through a `trunc`. Ours is an IR fold so both
selectors take it, and it looks through the narrowing LSR makes.
Exit: the DAG selector's `foldRemainderIdiom`, and the old selector gone.

## Example
What LSR leaves of `trunc (urem i32 %x, 251)` at -Os, and what this pass
makes of it (schematic: names are the printer's):

```text
 before                                  after
 %q = udiv i32 %x, 251                   %w = urem i32 %x, 251
 %t = trunc i32 %q to i16                %r = trunc i32 %w to i16
 %m = mul nuw i16 %t, 251
 %r = sub i16 %n, %m
```

From unit test
`test_the_narrowed_idiom_is_one_remainder_where_the_divide_is_kept`
in `remidiom_tests.rs`, which asserts the `urem` and that no `udiv` or
`mul` is left.

## Runs
After `lsr` and `differences`, in the strength slot
([`crate::pipeline::strength_slot`](pipeline.md)).
