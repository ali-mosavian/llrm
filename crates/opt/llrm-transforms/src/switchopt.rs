//! `SimplifyCFGOpt::simplifySwitch`: the rewrites of a `switch`, each its own
//! module.
//!
//! Translation of
//! [`simplifySwitch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)
//! at d1106deb71cc. The steps run in its order and the first that changes the
//! block ends the visit (`requestResimplify`):
//!
//! | # | LLVM | module | option |
//! |---|---|---|---|
//! | 1 | `simplifyEqualityComparisonWithOnlyPredecessor`, `simplifySwitchOnSelect`, `foldValueComparisonIntoPredecessors` | [`switchvalue`] | |
//! | 2 | `turnSwitchRangeIntoICmp` | [`switchrange`] | `convert_switch_range_to_icmp` |
//! | 3 | `eliminateDeadSwitchCases`, `simplifySwitchOfCmpIntrinsic` | [`switchdead`] | |
//! | 4 | `trySwitchToSelect` | [`switchselect`] | |
//! | 5 | `forwardSwitchConditionToPHI` | [`switchphi`] | `forward_switch_cond_to_phi` |
//! | 6 | `simplifySwitchLookup` | [`switchlookup`] | `convert_switch_to_arithmetic` or `convert_switch_to_lookup_table` |
//! | 7 | `simplifySwitchOfPowersOfTwo` | [`switchpow2`] | |
//! | 8 | `reduceSwitchRange` | [`switchreduce`] | |
//! | 9 | `hoistCommonCodeFromSuccessors` | `simplifycfg::branch_hoisted` | `hoist_common_insts` |
//! | 10 | `simplifyDuplicateSwitchArms`, `simplifySwitchWhenUMin`, `simplifySwitchDefaultBranch` | [`switcharms`] | |
//!
//! # How it works
//! `simplified` returns at the first step that changes the block, as
//! `requestResimplify` does; the block is visited again.
//!
//! # Example
//! A `switch` whose cases all go to one block and the default goes elsewhere
//! is rewritten by step 2 (`switchrange`) to a range check and a branch.
//!
//! DEVIATE 1: step 9 runs where the branch's `hoistCommonCodeFromSuccessors`
//! does, before this dispatcher, not between 8 and 10.
//!
//! # Used by
//! `simplifycfg::block_simplified`.

use llrm_mir::context::Context;
use llrm_mir::module::{BlockId, Function};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Declared;

use crate::simplifycfg::Options;
use crate::twoentryphi::Speculation;
use crate::{
    switcharms, switchdead, switchlookup, switchphi, switchpow2, switchrange, switchreduce, switchselect, switchvalue,
};

/// Whether the `switch` ending `block` was rewritten.
pub fn simplified(
    options: &Options,
    context: &Context,
    spec: &Speculation,
    declared: &mut Declared,
    function: &mut Function,
    block: BlockId,
) -> bool {
    if function.terminator(block).is_none_or(|last| function.instruction(last).opcode != Opcode::Switch) {
        return false;
    }
    switchvalue::folded(options, context, spec, function, block)
        || (options.convert_switch_range_to_icmp && switchrange::folded(options, context, spec, function, block))
        || switchdead::folded(options, context, spec, function, block)
        || switchselect::folded(options, context, spec, function, block)
        || (options.forward_switch_cond_to_phi && switchphi::folded(options, context, spec, function, block))
        || ((options.convert_switch_to_arithmetic || options.convert_switch_to_lookup_table)
            && switchlookup::folded(options, context, spec, declared, function, block))
        || switchpow2::folded(options, context, spec, function, block)
        || switchreduce::folded(options, context, spec, declared, function, block)
        || switcharms::folded(options, context, spec, function, block)
}
