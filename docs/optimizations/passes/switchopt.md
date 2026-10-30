> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchopt.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchopt.rs sha256:a4b050ef5b8c3089b14adcaded4aaa3dd1d5a6dcb4e382d7430c2e005e41b44c -->

# switchopt

`SimplifyCFGOpt::simplifySwitch`: the rewrites of a `switch`, each its own
module.

Translation of
[`simplifySwitch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)
at d1106deb71cc. The steps run in its order and the first that changes the
block ends the visit (`requestResimplify`):

| # | LLVM | module | option |
|---|---|---|---|
| 1 | `simplifyEqualityComparisonWithOnlyPredecessor`, `simplifySwitchOnSelect`, `foldValueComparisonIntoPredecessors` | [`switchvalue`](switchvalue.md) | |
| 2 | `turnSwitchRangeIntoICmp` | [`switchrange`](switchrange.md) | `convert_switch_range_to_icmp` |
| 3 | `eliminateDeadSwitchCases`, `simplifySwitchOfCmpIntrinsic` | [`switchdead`](switchdead.md) | |
| 4 | `trySwitchToSelect` | [`switchselect`](switchselect.md) | |
| 5 | `forwardSwitchConditionToPHI` | [`switchphi`](switchphi.md) | `forward_switch_cond_to_phi` |
| 6 | `simplifySwitchLookup` | [`switchlookup`](switchlookup.md) | `convert_switch_to_arithmetic` or `convert_switch_to_lookup_table` |
| 7 | `simplifySwitchOfPowersOfTwo` | [`switchpow2`](switchpow2.md) | |
| 8 | `reduceSwitchRange` | [`switchreduce`](switchreduce.md) | |
| 9 | `hoistCommonCodeFromSuccessors` | `simplifycfg::branch_hoisted` | `hoist_common_insts` |
| 10 | `simplifyDuplicateSwitchArms`, `simplifySwitchWhenUMin`, `simplifySwitchDefaultBranch` | [`switcharms`](switcharms.md) | |

## How it works
`simplified` returns at the first step that changes the block, as
`requestResimplify` does; the block is visited again.

## Example
A `switch` whose cases all go to one block and the default goes elsewhere
is rewritten by step 2 (`switchrange`) to a range check and a branch.

DEVIATE 1: step 9 runs where the branch's `hoistCommonCodeFromSuccessors`
does, before this dispatcher, not between 8 and 10.

## Used by
`simplifycfg::block_simplified`.
