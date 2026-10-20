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

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{CallInfo, Flags, Opcode, Tail};
use llrm_mir::passes::Declared;
use llrm_mir::types::{Type, TypeId};

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
        || switchpow2::folded(options, context, spec, declared, function, block)
        || switchreduce::folded(options, context, spec, declared, function, block)
        || switcharms::folded(options, context, spec, declared, function, block)
}

/// `Builder.CreateIntrinsic(ty, ID, args)` before `at`: a call of
/// `llvm.<base>.i<width of ty>` returning `ty`, declared in the module where
/// it has none.
pub(crate) fn intrinsic_call(
    context: &Context,
    declared: &mut Declared,
    function: &mut Function,
    at: InstId,
    base: &str,
    ty: TypeId,
    args: Vec<Operand>,
) -> Operand {
    let parameters = args.iter().map(|&arg| function.operand_type(context, arg).expect("a value")).collect();
    let function_type = context.types.intern(Type::Function { returns: ty, parameters, variadic: false });
    let width = context.types.int_bits(ty).expect("an integer");
    let callee = declared.declare(&format!("llvm.{base}.i{width}"), function_type);
    let pointer = context.types.ptr(0);
    let callee = Operand::Constant(context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new(); args.len()],
        attrs: Vec::new(),
        tail: Tail::default(),
    };
    let mut operands = args;
    operands.push(callee);
    let call = function.create_instruction(Opcode::Call(Box::new(info)), ty, operands, Flags::default(), None);
    function.insert(call, Position::Before(at)).expect("a placed terminator");
    Operand::Value(function.instruction(call).result.expect("a call's value"))
}
