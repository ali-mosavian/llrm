> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopnest.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopnest.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopnest.rs sha256:beb46cedd6a044a250c7f5aba7387145c36016c242e34fc6cb3bb1156ff37246 -->

# loopnest

Where a loop holds exactly one other loop: the nest's blocks, and what may
be asked of it (an inner loop that counts the same in every outer trip, the
accesses of its body).

## How it works
A nest is a loop that holds exactly one other loop and nothing else besides
pure work. The outer header tests and has one way into the loop. A chain of
single-successor blocks `pre` leads from it to the inner header, and a
chain `post` from the inner exit to the outer latch; no other block is in
the outer loop. Work in `pre`, `post` and the outer header is pure (no
memory, no division), and `pre`, `post` have no phis. The inner loop has one
latch and leaves only from its header, into a block only it enters.

The inner loop runs the same in every outer iteration
(`inner_is_invariant`): its test and the starts and steps of its counters
read only values the nest does not change, and nothing it computes is read
outside it. Its memory accesses (`accesses`) are each an affine function of
the two counters over an identified object.

```text
  outer header --> pre... --> inner header <--+
       ^                        |  body ------+
       |                        v
       +------ outer latch <-- post...
```

## Example
The blocks of the nest of `a_column_walk_is_swapped` in
`loopinterchange_tests.rs`:

```mir
b1:  %j = phi ..; br %run, b2, b6        ; the outer header
b2:  %c = mul %j, 2; br b3               ; pre: pure work, one block
b3:  %i = phi ..; br %more, b4, b5       ; the inner header
b4:  .. store; %i.next = add %i, 1; br b3 ; the inner body and latch
b5:  %j.next = add %j, 1; br b1          ; post, the outer latch
```

`pre` is `[b2]`, `post` `[b5]`; the inner count is the same each outer trip.

## Provides
`Nest`, `nest_of`, `inner_is_invariant`, `accesses`. Consumer:
`loopinterchange`.

## See also
[`crate::loopinterchange`](loopinterchange.md).
