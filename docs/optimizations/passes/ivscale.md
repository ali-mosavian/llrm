> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/ivscale.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/ivscale.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/ivscale.rs sha256:5fc8bdf97bcd56eb11365373540dfc4fe998ef07fe77f06646d9408aeaca575a -->

# ivscale

A counter read only as `i << k` counts in bytes (DEVIATE LSR4).

Real mode has no scale-4 address without the address-size prefix, so LSR
keeps `shl i, k` for the addresses and the counter beside it: the shift runs
every trip and holds a register. `4 * i` steps by 4 as well as `i` does, so
it can be the counter, and the bound is shifted once, ahead of the loop.
LSR's own formulas keep the scaled use (`scale 4` is legal for the
description and the pin chooses it); the cost is in the code the scaled
form becomes, which LSR prices once per scaled register.

## How it works
An innermost-or-not loop in simplify form whose counter `i` is a phi
`{start,+,1}` of one width `w`, read only by its latch add, shifts `shl i,
k` (one `k`) and one `icmp eq/ne` of `i` or `i + 1` against a loop-invariant
bound `B`, which the latch branches on. The counter becomes `i << k`: start
and step shifted, each shift replaced by the counter, `B << k` made in the
preheader.

The rewrite is exact when `i << k` hits the bound on the trip `i` does: `B`
is at least `start` (plus one for the test of `i + 1`) and below `2^(w-k)`,
so the counter reaches `B` before either wraps. Both come from `B`'s
unsigned range under the loop's guards (SCEV), so a bound the analysis
cannot limit is left alone. The latch add loses its wrap flags. Only a
target that prices a scaled index (`Tti::scaled_index_insns`) runs it.
Cost: O(uses) per loop.

DEVIATE LSR4 (the pin's LSR chooses the scaled form and its code prices it
once, not per trip; exit: the scaled form is priced and emitted alike, then
this is deleted; owner llrm-w12-lsr).

## Example
The butterfly loop of c's `stft_single` at m16 (schematic); `half` is
`m >> 1` with `m` at most 256:

```text
 %j = phi i16 [ 0 ], [ %next ]            %j = phi i16 [ 0 ], [ %next ]
 %b = shl i16 %j, 2                       %next = add i16 %j, 4
 ...load [%a + %b]                  ==>   ...load [%a + %j]
 %next = add i16 %j, 1                    %c = icmp eq i16 %next, %half4
 %c = icmp eq i16 %next, %half            (%half4 = shl %half, 2 in the preheader)
```

## Runs
After `lsr`, `differences` and `maskscale`, in the strength slot
([`crate::pipeline::strength_slot`](pipeline.md)); option `ivscale`.
