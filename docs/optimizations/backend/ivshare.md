> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/ivshare.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/ivshare.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/ivshare.rs sha256:71f73e0e5428a170a7009edfd07ec499fd55daa06d14761e01e724fe95cc5343 -->

# ivshare

Streams that walk the same index share one pointer (DEVIATE LSR2).

A loop that reads `[b0 + i]`, `[b1 + i]`, `[b2 + i]` holds three bases
and the index; the target has three registers that can make an address.
`getMinusSCEV` of two unrelated pointers is `CouldNotCompute` and a pointer
made from another object breaks provenance, so LSR cannot make the
difference; LIR holds integers, which have none. The sum `p = i + b0`
steps with `i`, and each other stream is `[p + (bk - b0)]` with the
difference made once, in the preheader: `k` registers instead of `k + 1`.
Each operand keeps its own selector, so the streams may lie in different
segments: the offsets wrap alike. The precedent outside the pin is gcc's
`get_computation_aff_1` (tree-ssa-loop-ivopts.cc#L4028).

## How it works
An innermost loop with a preheader and one latch. Its index is a phi `i`
stepped by one `add i, imm` or `inc i`. The streams are the memory operands
indexed by `i` (scale 1) on a base defined outside the loop. The rewrite
applies where the distinct bases and the index are one more than the target
has registers to address with: then the pointer and the differences fit
them. The index has no other reader than the streams and its step; or it
is doubled for two-byte elements, and the double has no other reader.

## Example
A loop reading `[b0 + i]`, `[b1 + i]`, `[b2 + i]`; the difference is made
once, ahead of the loop, and the streams share one pointer:

```text
  pre:                              pre:
    jmp loop                          d1 := sub b1, b0
  loop:                       =>      p := add i, b0
    x0 := [b0 + i]                    jmp loop
    x1 := [b1 + i]                  loop:
    x2 := [b2 + i]                    x0 := [p]
    i := add i, 1                     x1 := [p + d1]
                                      p := add p, 1
```

O(instructions) per loop.

## Used by
Named by `flow`.
