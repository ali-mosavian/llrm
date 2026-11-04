//! A `switch` that only picks the constant of one phi, turned into selects:
//! LLVM's
//! [`trySwitchToSelect`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6972),
//! a step of `simplifySwitch`
//! ([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)),
//! with `initializeUniqueCases`
//! ([L6724](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6724)),
//! `getCaseResults`
//! ([L6644](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6644)),
//! `foldSwitchToSelect`
//! ([L6795](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6795))
//! and `removeSwitchAfterSelectFold`
//! ([L6925](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6925)).
//!
//! # How it works
//! Each case (and the default) is followed through blocks that only fold to
//! constants, to one common destination with one phi. With at most two
//! distinct results, at most 16 cases each, the switch becomes selects on
//! compares of the value, or on a mask when a result's cases are a bit group.
//!
//! # Example
//! ```text
//!   switch i32 %a, label %d [ 10, %x  20, %y ]   %c1 = icmp eq i32 %a, 20
//!   x: br %end   y: br %end   d: br %end       ==> %s1 = select i1 %c1, i32 2, i32 4
//!   end: %p = phi [ 42, %x ], [ 2, %y ], [ 4, %d ] %c0 = icmp eq i32 %a, 10
//!                                                  %s = select i1 %c0, i32 42, i32 %s1
//! ```
//!
//! Each case block may hold constant-foldable instructions on its way to the
//! common destination; at most two distinct results (and a default) are
//! taken.
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | No branch weights on the selects. |
//! | D2 | No names on the new values. |
//!
//! # Used by
//! `switchopt::simplified`.

use llrm_analysis::occurrence::{arms, from_arms};
use llrm_mir::constantfolding::{Env, constant_fold_inst_operands_at};
use llrm_mir::context::{Constant, ConstantId, ConstantKind, Context, mask, signed};
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::known::compute_known_bits;

use crate::simplifycfg::Options;
use crate::twoentryphi::Speculation;

/// `max-switch-cases-per-result`.
const MAX_SWITCH_CASES_PER_RESULT: usize = 16;
/// `MaxUniqueResults` of the call in `trySwitchToSelect`.
const MAX_UNIQUE_RESULTS: usize = 2;

/// `SwitchCaseResultVectorTy`: each result and the case values that give it.
type UniqueResults = Vec<(ConstantId, Vec<ConstantId>)>;

fn int_value(
    context: &Context,
    constant: ConstantId,
) -> Option<u128> {
    match context.get(constant).kind {
        ConstantKind::Int(value) => Some(value),
        ConstantKind::Zero | ConstantKind::Null => Some(0),
        _ => None,
    }
}

/// `ConstantFoldInstruction` over the pool: the constant `inst` is when the
/// values the pool knows are substituted (`constantFold`).
fn constant_folded(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    inst: InstId,
    pool: &IdMap<ValueId, ConstantId>,
) -> Option<ConstantId> {
    let lookup = |operand| match operand {
        Operand::Constant(constant) => Some(constant),
        Operand::Value(value) => pool.get(&value).copied(),
        Operand::Block(_) => None,
    };
    let instruction = function.instruction(inst);
    if instruction.opcode == Opcode::Select {
        let condition = int_value(context, lookup(instruction.operands[0])?)?;
        // An all-ones i1 is true, a null one false.
        return lookup(instruction.operands[if condition != 0 { 1 } else { 2 }]);
    }
    if instruction.opcode == Opcode::Phi {
        return None;
    }
    let ops = instruction.operands.iter().map(|&operand| lookup(operand)).collect::<Option<Vec<_>>>()?;
    constant_fold_inst_operands_at(
        context,
        Env { globals: &spec.outer.globals, layout: spec.layout },
        function,
        inst,
        &ops,
    )
}

/// `getCaseResults`: the constants the phis of the common destination take
/// when the switch goes to `dest` (on `case`; the default if none). Whether
/// they were found, and the ones found: LLVM's partial result is kept, as
/// `initializeUniqueCases` reads it of the default.
fn case_results(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    switch: InstId,
    case: Option<ConstantId>,
    mut dest: BlockId,
    common: &mut Option<BlockId>,
) -> (bool, Vec<(InstId, ConstantId)>) {
    // The block from which the common destination is entered.
    let mut pred = function.parent(switch).expect("a placed switch");
    let mut pool: IdMap<ValueId, ConstantId> = IdMap::new();
    if let (Some(case), Operand::Value(cond)) = (case, function.instruction(switch).operands[0]) {
        pool.insert(cond, case);
    }
    let first = dest;
    for inst in function.iter_block(first).collect::<Vec<_>>() {
        let instruction = function.instruction(inst);
        if instruction.opcode.is_terminator() {
            // A simple branch continues to the next block.
            let mut successors = instruction
                .operands
                .iter()
                .filter_map(
                    |operand| match operand {
                        Operand::Block(block) => Some(*block),
                        _ => None,
                    },
                );
            let (Some(next), None) = (successors.next(), successors.next()) else { return (false, Vec::new()) };
            if !matches!(instruction.opcode, Opcode::Br | Opcode::Switch) {
                return (false, Vec::new());
            }
            pred = first;
            dest = next;
        } else if let Some(constant) = constant_folded(context, spec, function, inst, &pool) {
            // Side-effect free and constant. Used outside this block or by a
            // phi slot for another block, it could not be bypassed: it would
            // no longer dominate its uses.
            let result = instruction.result.expect("a folded instruction has a value");
            for user in function.users(result) {
                let user_inst = function.instruction(user.user);
                if function.parent(user.user) == Some(first) {
                    continue;
                }
                if user_inst.opcode == Opcode::Phi
                    && user_inst.operands[user.index as usize + 1] == Operand::Block(first)
                {
                    continue;
                }
                return (false, Vec::new());
            }
            pool.insert(result, constant);
        } else {
            break;
        }
    }

    // Without a common destination yet, this is it; any other aborts.
    let common = *common.get_or_insert(dest);
    if dest != common {
        return (false, Vec::new());
    }

    // The values for this case from the phis of the destination.
    let mut res = Vec::new();
    for phi in crate::edges::phis(function, common) {
        let Some(&(value, _)) = arms(function, phi).iter().find(|&&(_, from)| from == pred) else { continue };
        let constant = match value {
            Operand::Constant(constant) => Some(constant),
            Operand::Value(value) => pool.get(&value).copied(),
            Operand::Block(_) => None,
        };
        let Some(constant) = constant else { return (false, res) };
        res.push((phi, constant));
    }
    (!res.is_empty(), res)
}

/// `initializeUniqueCases`: the phi, the common destination, the cases by
/// result and the default's result, with `MAX_UNIQUE_RESULTS` bounding the
/// results.
fn unique_cases(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    switch: InstId,
    default: BlockId,
) -> Option<(InstId, BlockId, UniqueResults, Option<ConstantId>)> {
    let operands = &function.instruction(switch).operands;
    let mut common = None;
    let mut phi = None;
    let mut unique: UniqueResults = Vec::new();
    for pair in operands[2..].chunks_exact(2) {
        let (Operand::Constant(value), Operand::Block(dest)) = (pair[0], pair[1]) else { unreachable!("a case") };
        let (found, results) = case_results(context, spec, function, switch, Some(value), dest, &mut common);
        // Only one value per case is permitted.
        if !found || results.len() != 1 {
            return None;
        }
        let (result_phi, result) = results[0];
        let count = match unique.iter_mut().find(|(known, _)| *known == result) {
            Some((_, values)) => {
                values.push(value);
                values.len()
            }
            None => {
                unique.push((result, vec![value]));
                1
            }
        };
        if count > MAX_SWITCH_CASES_PER_RESULT || unique.len() > MAX_UNIQUE_RESULTS {
            return None;
        }
        // The phi must be the same for every case.
        if *phi.get_or_insert(result_phi) != result_phi {
            return None;
        }
    }
    // The default's result: abort unless it is there or the default is
    // unreachable.
    let (_, defaults) = case_results(context, spec, function, switch, None, default, &mut common);
    let default_result = (defaults.len() == 1).then(|| defaults[0].1);
    if default_result.is_none() && !crate::simplifycfg::only_unreachable(function, default) {
        return None;
    }
    Some((phi?, common?, unique, default_result))
}

/// The instructions `foldSwitchToSelect` builds, before the switch.
struct Builder<'a> {
    context: &'a Context,
    function: &'a mut Function,
    at: InstId,
}

impl Builder<'_> {
    fn emit(
        &mut self,
        opcode: Opcode,
        ty: llrm_mir::types::TypeId,
        operands: Vec<Operand>,
    ) -> Operand {
        let new = self.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        self.function.insert(new, Position::Before(self.at)).expect("a placed terminator");
        Operand::Value(self.function.instruction(new).result.expect("a value"))
    }

    fn constant(
        &self,
        ty: llrm_mir::types::TypeId,
        value: u128,
    ) -> Operand {
        Operand::Constant(self.context.constant(Constant { ty, kind: ConstantKind::Int(value) }))
    }

    /// `CreateICmpEQ(a, b)`.
    fn icmp_eq(
        &mut self,
        a: Operand,
        b: Operand,
    ) -> Operand {
        let i1 = self.context.types.int(1);
        self.emit(Opcode::ICmp(IntPredicate::Eq), i1, vec![a, b])
    }

    /// `CreateSelect(cond, t, f)`.
    fn select(
        &mut self,
        cond: Operand,
        t: ConstantId,
        f: Operand,
    ) -> Operand {
        let ty = self.context.get(t).ty;
        self.emit(Opcode::Select, ty, vec![cond, Operand::Constant(t), f])
    }

    /// `CreateAnd(a, mask)`: `a & -1` is `a`.
    fn and(
        &mut self,
        a: Operand,
        ty: llrm_mir::types::TypeId,
        bits: u128,
        width: u32,
    ) -> Operand {
        if bits == mask(width) {
            return a;
        }
        let rhs = self.constant(ty, bits);
        self.emit(Opcode::Binary(BinaryOp::And), ty, vec![a, rhs])
    }
}

/// `foldSwitchToSelect`: the value that replaces the switch, built before
/// `switch`.
fn folded_to_select(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    switch: InstId,
    unique: &UniqueResults,
    default: Option<ConstantId>,
) -> Option<Operand> {
    let condition = function.instruction(switch).operands[0];
    let ty = function.operand_type(context, condition).expect("a switch value");
    let width = context.types.int_bits(ty).expect("an integer switch");
    let case = |value: ConstantId| Operand::Constant(value);

    // Two results of one case each: one select, or two with a default.
    if unique.len() == 2 && unique[0].1.len() == 1 && unique[1].1.len() == 1 {
        let (first, second) = (unique[0].1[0], unique[1].1[0]);
        let mut b = Builder { context, function, at: switch };
        let mut value = Operand::Constant(unique[1].0);
        if let Some(default) = default {
            let compare = b.icmp_eq(condition, case(second));
            value = b.select(compare, unique[1].0, Operand::Constant(default));
        }
        let compare = b.icmp_eq(condition, case(first));
        return Some(b.select(compare, unique[0].0, value));
    }

    // One result with a default: the cases are a group the default is not.
    let default = default?;
    if unique.len() != 1 {
        return None;
    }
    let values = &unique[0].1;
    let count = values.len();
    let bits_of = |value: ConstantId| int_value(context, value).expect("a case value");
    if count.is_power_of_two() {
        // n bits group cases that map to the same result:
        //   case 0,4      -> Cond & 0b1..1011 == 0 ? result : default
        //   case 0,2,4,6  -> Cond & 0b1..1001 == 0 ? result : default
        //   case 0,2,8,10 -> Cond & 0b1..0101 == 0 ? result : default
        let mut min = bits_of(values[0]);
        let mut and_mask = mask(width);
        for &value in values {
            let bits = bits_of(value);
            if signed(bits, width) < signed(min, width) {
                min = bits;
            }
            and_mask &= bits;
        }
        let known = {
            let q = Query {
                context,
                globals: &[],
                metadata: &[],
                function: &*function,
                layout: spec.layout,
                dt: None,
                assumptions: None,
                dom_conditions: None,
                cond_context: None,
                cxt: None,
            };
            compute_known_bits(&q, condition, 0)
        };
        let log2 = count.trailing_zeros();
        let mut b = Builder { context, function, at: switch };
        // Bits only the case values set, and none free to vary but the ones
        // the cases cover.
        if and_mask != 0 && known.max_value() >= and_mask {
            let free_bits = known.count_max_active_bits().wrapping_sub(and_mask.count_ones());
            if free_bits == log2 {
                let masked = b.and(condition, ty, and_mask, width);
                let rhs = b.constant(ty, and_mask);
                let compare = b.icmp_eq(masked, rhs);
                return Some(b.select(compare, unique[0].0, Operand::Constant(default)));
            }
        }

        // The bits the case numbers touch.
        let mut bit_mask = 0u128;
        for &value in values {
            bit_mask |= bits_of(value).wrapping_sub(min) & mask(width);
        }
        // Cases of one result cover all the numbers of the touched bits.
        if bit_mask.count_ones() == log2 {
            let mut reduced = condition;
            if min != 0 {
                let rhs = b.constant(ty, min);
                reduced = b.emit(Opcode::Binary(BinaryOp::Sub), ty, vec![condition, rhs]);
            }
            let masked = b.and(reduced, ty, !bit_mask & mask(width), width);
            let zero = b.constant(ty, 0);
            let compare = b.icmp_eq(masked, zero);
            return Some(b.select(compare, unique[0].0, Operand::Constant(default)));
        }
    }

    // The degenerate case: two cases of one result.
    if count == 2 {
        let mut b = Builder { context, function, at: switch };
        let one = b.icmp_eq(condition, case(values[0]));
        let two = b.icmp_eq(condition, case(values[1]));
        let i1 = context.types.int(1);
        let either = b.emit(Opcode::Binary(BinaryOp::Or), i1, vec![one, two]);
        return Some(b.select(either, unique[0].0, Operand::Constant(default)));
    }
    None
}

/// Whether the `switch` ending `block` was rewritten.
pub fn folded(
    _options: &Options,
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(switch) = function.terminator(block) else { return false };
    let Operand::Block(default) = function.instruction(switch).operands[1] else { return false };
    let Some((phi, dest, unique, default_result)) = unique_cases(context, spec, function, switch, default) else {
        return false;
    };
    let Some(value) = folded_to_select(context, spec, function, switch, &unique, default_result) else {
        return false;
    };

    // `removeSwitchAfterSelectFold`.
    let successors: Vec<BlockId> = function
        .instruction(switch)
        .operands
        .iter()
        .enumerate()
        .filter(|&(at, _)| at == 1 || (at >= 2 && at % 2 == 1))
        .filter_map(|(_, operand)| match operand {
            Operand::Block(block) => Some(*block),
            _ => None,
        })
        .collect();
    let void = function.instruction(switch).ty;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(dest)], Flags::default(), None);
    function.insert(jump, Position::Before(switch)).expect("a placed terminator");
    let mut incoming = arms(function, phi);
    incoming.retain(|&(_, from)| from != block);
    incoming.push((value, block));
    function.set_operands(phi, from_arms(&incoming));
    for successor in successors {
        if successor != dest {
            function.predecessor_removed(context, successor, block, false);
        }
    }
    function.erase(switch).expect("a terminator defines nothing");
    true
}

#[cfg(test)]
#[path = "switchselect_tests.rs"]
mod tests;
