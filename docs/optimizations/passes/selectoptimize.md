> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/selectoptimize.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/selectoptimize.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/selectoptimize.rs sha256:b87d57f74169f2e18c2da6015b14913a72cc08f771004eb8f6cfa9cb4b6d6244 -->

# selectoptimize

`SelectOptimize`: a group of selects (or select-like `or`/`add`/`sub` of an
extended condition) becomes a branch where the branch is the cheaper form
([SelectOptimize.cpp#L336](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L336)).

## How it works
The target gates the pass (`Target`): it must enable it and support scalar
selects, and the function must not be `optsize`/`minsize`. Consecutive
select-likes of one condition in a block form a group. Outside innermost
loops a group converts when its branch is highly predictable and the target
finds predictable selects expensive, when its cold operand's dependence
slice costs at least `TCC_Expensive`, or when it is three or more selects
in a loop's latch. In an innermost loop the loop's critical path over two
iterations is priced with the selects as selects and as branches (latency
plus the dependence on the operands, mispredictions at the default rate);
the loop converts when the branches cut it by 4 cycles and 12.5%, with a
gradient of 25% between the iterations, and each group whose branch cost is
below its select cost converts.

A converted group splits its block: the instructions only its operands need
sink into `select.true.sink` / `select.false.sink` (interleaved across the
group's selects), the condition is frozen, and phis replace the selects.

```text
 start: %s = select i1 %c, %a, %b       start: %c.frozen = freeze i1 %c
        ...                                    br i1 %c.frozen, end, select.false
                                        select.false: br end
                                        end:   %s = phi [%a, start], [%b, select.false]
```

DEVIATE SO1 (exit 2026-12-04, pin permalink above): the MIR carries no
`ProfileSummaryInfo`, so no block is cold, `shouldOptimizeForSize` is the
`optsize`/`minsize` attribute alone, and `setExplicitlyUnknownBranchWeights`
is a no-op.

DEVIATE SO2 (exit 2026-12-04, pin permalink above): no optimization
remarks, statistics or debug locations; `shouldTreatInstructionLikeSelect`
is `TargetTransformInfoImplBase`'s, as no target of ours overrides it.

## Runs
Not wired: the pin's `llc` leaves it off (`-disable-select-optimize`
defaults to true, `TargetPassConfig.cpp` L278), so no pipeline slot holds
it. Where the pin runs it, it is after `ExpandReductions` in
`addIRPasses`.
