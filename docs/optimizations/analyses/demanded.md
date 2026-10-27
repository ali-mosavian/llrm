> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/demanded.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/demanded.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/demanded.rs sha256:3893468f3558e0d8176a79dcc7bdc73ac72e5020d84c94bc1bfebd7cfcd04779 -->

# demanded

Which bits of each integer value anything reads: LLVM's `DemandedBits`.

Exit: deleted, with `algebraic::_unread_bits` (its only reader), by the
InstCombine `SimplifyDemandedBits` translation (instcombine-port); BDCE
(`demandedbits`) does not replace it.

## How it works
A backward fixed point over the use-def graph, the dual of
`valuetracking::known_zero`. A bit no user reads can be anything, so a mask
that clears only such bits, or an operand all of whose bits are such,
can go.

Every instruction that is not a pure integer operation reads its integer
operands in full: stores, calls, compares, divides and pointer
computations. Those seed the map. A pure integer instruction (a phi, or one
`valuetracking::demanded_operands` has a rule for) passes on instead: given
the bits read of its result, the rule gives the bits read of each operand,
and those join into the operand's entry.

```text
  %a = load i32          read: 0x000000ff   (only the low byte)
  %m = and i32 %a, 255   read: 0xffffffff   <- stored in full
  store i32 %m, ptr %p

  and passes on 0xff & 0xffffffff = 0xff to %a
```

Work goes onto a stack whenever an entry grows, so a loop's phis reach a
fixed point: demand only grows and is bounded by the width.

Only integers of at most 128 bits are tracked (`u128` masks). A value that
is absent from the map is not read at all.

## Example
No honest example: no test asks `demanded` for its map. The listing above
shows the rule by hand; its only consumer is `algebraic` in
`llrm-transforms`.

## Provides
`demanded(context, function)`: a `ValueId -> u128` map. Cached as
[`crate::manager::DemandedBits`](manager.md). Consumer: `algebraic`'s removal of masks
and operands over unread bits (`algebraic` in `llrm-transforms`).

## Needs
`llrm_mir::valuetracking::demanded_operands`, the per-opcode table.

## See also
LLVM `DemandedBits` and `SimplifyDemandedBits`.
