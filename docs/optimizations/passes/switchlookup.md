> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchlookup.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchlookup.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchlookup.rs sha256:1d3abae12377e6125f95836f7ed57708a2e3eb197078b980c71a6079fe8cc046 -->

# switchlookup

`simplifySwitchLookup`: a switch whose only job is to give constants to the
phis of one common block becomes arithmetic on its condition, behind a range
check where the default can run. A step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).

## LLVM structure
| LLVM | here |
|---|---|
| [`simplifySwitchLookup`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7567) | `folded`, `Plan::of`, `Plan::apply` |
| [`getCaseResults`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6641), `constantFold`, `lookupConstant` | `case_results` |
| [`SwitchReplacement`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7007) | `Replacement` |
| [`shouldBuildLookupTable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7429), `isTypeLegalForLookupTable`, `isSwitchDense`, `wouldFitInRegister` | `should_build_lookup_table` and friends |
| [`shouldUseSwitchConditionAsTableIndex`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7466) | `condition_is_index` |
| [`reuseTableCompare`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7502) | `table_compare_reused` |

The legality questions (`isTypeLegal`, `fitsInLegalInteger`) are asked of
the target ([`Tti::is_type_legal`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/tti.rs)) and
the data layout, not answered here.

## How it works
Each case is walked to the common block, folding what the case block
computes from the condition (`case_results`). A table of the results is
sized (`condition_is_index`, the offset of the lowest case), priced
(`should_build_lookup_table`) and, per phi, found to be one value, linear
in the index, a bit map in a register or cells of an array global. The phis
then take the arithmetic, behind a range check or, for holes with a default
that is not a constant, a bit mask.

## Example
```text
  switch i16 %x, label %d [ 0, a  1, b  2, c  3, e ]    %0 = icmp ult i16 %x, 4
  phi [10, a] [13, b] [16, c] [19, e] [0, d]        =>  %1 = mul nsw i16 %x, 3
                                                        %2 = add nsw i16 %1, 10
                                                        br i1 %0, label %lookup, label %d
```

| # | DEVIATE |
|---|---|
| D1 | A table is a private constant from `Declared::table`, named `switch.table.N`, deduplicated, without the function's name or the preferred alignment of its cells; `shouldBuildLookupTablesForConstant` is not asked and a constant MIR cannot hold is not offered. |
| D2 | No branch weights on the range check (`setFittedBranchWeights`). |
| D3 | No `DomTreeUpdater`: callers recompute the tree. |
| D4 | A hole at index 0 of a table whose holes are poison takes the first result as the linear offset; the pin `cast<ConstantInt>`s the poison. |

## Used by
[`crate::switchopt`](switchopt.md), step 6 of `simplifySwitch`, under
`convert_switch_to_arithmetic` or `convert_switch_to_lookup_table`.
