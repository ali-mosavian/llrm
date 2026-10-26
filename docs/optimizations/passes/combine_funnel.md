> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_funnel.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_funnel.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_funnel.rs sha256:a6670ea1065074aa6ab62b2263c0560634949c20e1f7d31a9199730d8ae36bd1 -->

# combine_funnel

`convertOrOfShiftsToFunnelShift` and `matchFunnelShift`: an `or` of two
opposite shifts, or of a pair of concatenations, is a funnel shift.

Translation of
[`InstCombinerImpl::convertOrOfShiftsToFunnelShift`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2992)
and `matchFunnelShift` (:3189) at d1106deb71cc.

MIRROR: the shift pair (constant amounts summing to the width, `Width - x`
with `x` known below the width, and for a rotate the masked-negation forms),
and the `zext` concatenation pair with its dominance and poison-flag
conditions.

DEVIATE 1: no vectors; only integer constants (`undef` and `poison` lanes of
a constant amount are not matched).

## Used by
`visitOr` ([`crate::combine_or`](combine_or.md)), and by `SimplifyDemandedBits`
([`crate::combine_demanded`](combine_demanded.md)) through
`Combiner::convert_or_of_shifts_to_funnel_shift`.

## Example
```text
%a = shl i32 %x, 5                      %r = call i32 @llvm.fshl.i32(i32 %x, i32 %y, i32 5)
%b = lshr i32 %y, 27           ==>
%r = or i32 %a, %b
```
