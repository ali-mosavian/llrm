//! A division and the remainder of the same operands are brought together, or
//! one made of the other, as LLVM's `DivRemPairs`.
//!
//! # How it works
//! A translation of LLVM
//! [`DivRemPairs.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/DivRemPairs.cpp)
//! at d1106deb71cc. A `sdiv` or `udiv` and a `srem`, `urem` or expanded
//! remainder `x - (x / y) * y` with the same dividend, divisor and signedness
//! are a pair. Where the target has a divide that makes both
//! (`TargetTransformInfo::hasDivRemOp`: x86 on every register type, or what a
//! target's `Tti` says), the expanded remainder is first written as the real
//! `srem`/`urem`, and the pair is put side by side so the backend finds one
//! divide: the one in a block the other's block follows is moved after it, and
//! a pair in two arms of one branch goes to the block before the branch when
//! nothing before either can fail to reach it. Where the target has none, a
//! `srem`/`urem` becomes `x - (x / y) * y` of the quotient already made, and
//! the quotient moves up to the remainder when it stood below it.
//!
//! # Example
//! ```text
//!  before                          after (no divide-remainder instruction)
//!  %d = sdiv i32 %x, %y            %d = sdiv i32 %x, %y
//!  ...                             ...
//!  %r = srem i32 %x, %y            %m = mul i32 %d, %y
//!                                  %r = sub i32 %x, %m
//! ```
//!
//! An operand that may be `undef` is frozen first, one value for the
//! quotient and the remainder, and the quotient loses its `exact` flag.
//!
//! DEVIATE 1: `CatchSwitchInst` terminators have no MIR spelling, so the check
//! that refuses to hoist into one has nothing to refuse.
//!
//! DEVIATE 2: `hasDivRemOp` is read at two sites, `Tti::has_div_rem_op`
//! (default false) and `Lowering::has_div_rem_op`, and either one true counts.
//! The pin has one, `TTI.hasDivRemOp(Type, IsSigned)`
//! ([DivRemPairs.cpp#L194](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DivRemPairs.cpp#L194)),
//! which `X86TTIImpl` answers
//! ([X86TargetTransformInfo.cpp#L7421](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7421)).
//! Reason: x86 states its answer on its `Lowering`; the 6502 states its on its
//! `Tti`. With the default false, x86 and arm64 are as they were. Exit: x86
//! states it on its `Tti`, then the `Lowering` site goes. Owner: the 6502
//! goal, in its own small PR with an x86 m16, m32 and m64 sweep and a control.
//!
//! # Parity slot
//! `divrempairs@1667`, in the late function pipeline between `instsimplify`
//! and `tailrec` (PassBuilderPipelines.cpp#L1667).
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.

use llrm_mir::context::Context;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::hash::HashMap;
use llrm_mir::lowering::Lowering;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::tti::Tti;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef;

/// `DivRemMapKey`: signedness and operands.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Key {
    signed: bool,
    dividend: Operand,
    divisor: Operand,
}

/// A matched pair: the `udiv`/`sdiv` and the instruction taken as the
/// remainder (a `urem`/`srem`, or the expanded form).
#[derive(Clone, Copy, Debug)]
struct Entry {
    div: InstId,
    rem: InstId,
}

impl Entry {
    fn signed(
        &self,
        function: &Function,
    ) -> bool {
        function.instruction(self.div).opcode == Opcode::Binary(BinaryOp::SDiv)
    }

    fn dividend(
        &self,
        function: &Function,
    ) -> Operand {
        function.instruction(self.div).operands[0]
    }

    fn divisor(
        &self,
        function: &Function,
    ) -> Operand {
        function.instruction(self.div).operands[1]
    }

    /// `isRemExpanded`: anything but a `srem` or `urem`.
    fn rem_expanded(
        &self,
        function: &Function,
    ) -> bool {
        !matches!(
            function.instruction(self.rem).opcode,
            Opcode::Binary(BinaryOp::SRem | BinaryOp::URem)
        )
    }
}

/// `matchExpandedRem`: `X - ((X ?/ Y) * Y)`, the form a remainder is expanded
/// into, with the multiply either way round.
fn match_expanded_rem(
    function: &Function,
    inst: InstId,
) -> Option<(Key, InstId)> {
    let sub = function.instruction(inst);
    if sub.opcode != Opcode::Binary(BinaryOp::Sub) {
        return None;
    }
    let (dividend, rounded) = (sub.operands[0], sub.operands[1]);
    let defined = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            llrm_mir::module::ValueDef::Instruction(def) => Some((def, function.instruction(def))),
            _ => None,
        },
        _ => None,
    };
    let (_, mul) = defined(rounded)?;
    if mul.opcode != Opcode::Binary(BinaryOp::Mul) {
        return None;
    }
    // `m_c_Mul(m_CombineAnd(m_IDiv(m_Specific(Dividend), m_Value(Divisor)),
    // m_Instruction(Div)), m_Deferred(Divisor))`: either operand the divide.
    for (div_side, other) in [(mul.operands[0], mul.operands[1]), (mul.operands[1], mul.operands[0])] {
        let Some((_, div)) = defined(div_side) else { continue };
        let signed = match div.opcode {
            Opcode::Binary(BinaryOp::SDiv) => true,
            Opcode::Binary(BinaryOp::UDiv) => false,
            _ => continue,
        };
        if div.operands[0] == dividend && div.operands[1] == other {
            return Some((Key { signed, dividend, divisor: other }, inst));
        }
    }
    None
}

/// `getWorklist`: the pairs, in the order of the remainders'
/// first appearance (LLVM's `MapVector`).
fn worklist(function: &Function) -> Vec<Entry> {
    let mut divs: HashMap<Key, InstId> = HashMap::default();
    let mut rems: Vec<(Key, InstId)> = Vec::new();
    let mut rem_at: HashMap<Key, usize> = HashMap::default();
    let mut remember = |rems: &mut Vec<(Key, InstId)>, key: Key, inst: InstId| match rem_at.get(&key) {
        Some(&at) => rems[at].1 = inst,
        None => {
            rem_at.insert(key, rems.len());
            rems.push((key, inst));
        }
    };
    for &block in function.layout() {
        for inst in function.iter_block(block) {
            let instruction = function.instruction(inst);
            let key = |signed| Key { signed, dividend: instruction.operands[0], divisor: instruction.operands[1] };
            match instruction.opcode {
                Opcode::Binary(BinaryOp::SDiv) => {
                    divs.insert(key(true), inst);
                }
                Opcode::Binary(BinaryOp::UDiv) => {
                    divs.insert(key(false), inst);
                }
                Opcode::Binary(BinaryOp::SRem) => remember(&mut rems, key(true), inst),
                Opcode::Binary(BinaryOp::URem) => remember(&mut rems, key(false), inst),
                _ => {
                    if let Some((expanded, rem)) = match_expanded_rem(function, inst) {
                        remember(&mut rems, expanded, rem);
                    }
                }
            }
        }
    }
    rems.into_iter().filter_map(|(key, rem)| divs.get(&key).map(|&div| Entry { div, rem })).collect()
}

struct DivRem<'a> {
    context: &'a Context,
    globals: &'a [GlobalValue],
    lowering: Option<&'a dyn Lowering>,
    tti: Option<&'a dyn Tti>,
}

impl DivRem<'_> {
    /// `TTI.hasDivRemOp(Type, IsSigned)`.
    ///
    /// DEVIATE 2 (see the header): asks the target's `Tti` and its `Lowering`.
    /// Reason: x86 states the answer on its lowering
    /// (`isOperationLegal(SDIVREM)`), the pin's `TTI` asks that. Exit: x86
    /// answers on its `Tti`.
    fn has_div_rem_op(
        &self,
        bits: u32,
        signed: bool,
    ) -> bool {
        self.tti.is_some_and(|tti| tti.has_div_rem_op(bits, signed))
            || self.lowering.is_some_and(|lowering| lowering.has_div_rem_op(bits, signed))
    }

    /// `isGuaranteedToTransferExecutionToSuccessor` of every instruction
    /// before `inst` in its block.
    fn safe_to_hoist(
        &self,
        function: &Function,
        inst: InstId,
    ) -> bool {
        let block = function.parent(inst).expect("placed");
        for before in function.iter_block(block) {
            if before == inst {
                return true;
            }
            if !crate::local::is_guaranteed_to_transfer_execution_to_successor(
                self.context,
                self.globals,
                function,
                before,
            ) {
                return false;
            }
        }
        true
    }

    /// `optimizeDivRem`. Whether anything changed.
    fn run(
        &self,
        function: &mut Function,
        tree: &DomTree,
        layout: &llrm_mir::datalayout::DataLayout,
    ) -> bool {
        let mut changed = false;
        for mut entry in worklist(function) {
            let ty = function.instruction(entry.div).ty;
            let Some(bits) = self.context.types.int_bits(ty) else { continue };
            let signed = entry.signed(function);
            let has_div_rem = self.has_div_rem_op(bits, signed);
            let expanded_at_first = entry.rem_expanded(function);

            if has_div_rem && entry.rem_expanded(function) {
                // The target has a divide-remainder but the remainder is
                // expanded: write it as the real one, next to the old, and
                // leave `((X / Y) * Y)` where it is.
                let (x, y) = (entry.dividend(function), entry.divisor(function));
                let op = Opcode::Binary(if signed { BinaryOp::SRem } else { BinaryOp::URem });
                let name = name_of(function, entry.rem).map(|one| format!("{one}.recomposed"));
                let real = function.create_instruction(op, ty, vec![x, y], Flags::default(), name.as_deref());
                function.insert(real, Position::After(entry.rem)).expect("placed after the old");
                copy_location(function, entry.rem, real);
                let old = entry.rem;
                entry.rem = real;
                replace_with(function, old, real);
                changed = true;
            }

            // The target has the divide-remainder and the two are in one
            // block: the backend sees the pair as it is.
            if has_div_rem && function.parent(entry.rem) == function.parent(entry.div) {
                continue;
            }

            let mut div_dominates = tree.instruction_dominates(function, entry.div, entry.rem);
            if !div_dominates && !tree.instruction_dominates(function, entry.rem, entry.div) {
                // A pair in two blocks neither of which dominates the other.
                let div_block = function.parent(entry.div).expect("placed");
                let rem_block = function.parent(entry.rem).expect("placed");
                let mut pred: Option<BlockId> = None;
                let successors = function.successors(rem_block);
                if successors.len() == 1 && successors[0] == div_block {
                    // pred -> rem -> div, pred -> div.
                    pred = unique_predecessor(function, rem_block);
                } else if let Some(rem_pred) = unique_predecessor(function, rem_block) {
                    // pred -> rem, pred -> div: only worth it when the target
                    // makes both from one divide.
                    if has_div_rem && Some(rem_pred) == unique_predecessor(function, div_block) {
                        pred = Some(rem_pred);
                    }
                }
                let hoist_into = pred.filter(|&pred| {
                    let terminator = function.terminator(pred).expect("a block ends in a terminator");
                    crate::local::is_guaranteed_to_transfer_execution_to_successor(
                        self.context,
                        self.globals,
                        function,
                        terminator,
                    ) && self.safe_to_hoist(function, entry.rem)
                        && self.safe_to_hoist(function, entry.div)
                        && function.successors(pred).iter().all(|&block| block == div_block || block == rem_block)
                        && function.predecessors(div_block).iter().all(|&block| block == rem_block || block == pred)
                });
                match hoist_into {
                    Some(pred) => {
                        div_dominates = true;
                        let terminator = function.terminator(pred).expect("a block ends in a terminator");
                        function.move_to(entry.div, Position::Before(terminator)).expect("placed");
                        changed = true;
                        if has_div_rem {
                            function.move_to(entry.rem, Position::Before(terminator)).expect("placed");
                            continue;
                        }
                    }
                    None => continue,
                }
            }

            // No divide-remainder instruction and the remainder was already
            // expanded: nothing to do.
            if !has_div_rem && expanded_at_first {
                continue;
            }

            if has_div_rem {
                // Put the lower of the pair next to the other, so the
                // backend sees them together.
                if div_dominates {
                    function.move_to(entry.rem, Position::After(entry.div)).expect("placed");
                } else {
                    function.move_to(entry.div, Position::After(entry.rem)).expect("placed");
                }
            } else {
                // X % Y --> X - ((X / Y) * Y), on the quotient already made.
                let (x, y) = (entry.dividend(function), entry.divisor(function));
                let div_value = Operand::Value(function.instruction(entry.div).result.expect("a value"));
                if !div_dominates {
                    function.move_to(entry.div, Position::Before(entry.rem)).expect("placed");
                }
                let mul = function.create_instruction(
                    Opcode::Binary(BinaryOp::Mul),
                    ty,
                    vec![div_value, y],
                    Flags::default(),
                    None,
                );
                let mul_value = Operand::Value(function.instruction(mul).result.expect("a value"));
                let sub = function.create_instruction(
                    Opcode::Binary(BinaryOp::Sub),
                    ty,
                    vec![x, mul_value],
                    Flags::default(),
                    None,
                );
                function.insert(mul, Position::After(entry.rem)).expect("placed");
                copy_location(function, entry.rem, mul);
                function.insert(sub, Position::After(mul)).expect("placed");
                copy_location(function, entry.rem, sub);

                // An `exact` divide would make `X % Y` poison where it was
                // defined.
                let flags = function.instruction(entry.div).flags;
                function.set_flags(entry.div, flags.without(Flags::POISON_GENERATING));

                // An operand that may be undef is frozen first, as one value
                // for the quotient and the remainder.
                for (index, value) in [(0, x), (1, y)] {
                    let query = Query {
                        globals: self.globals,
                        dt: Some(tree),
                        cxt: Some(entry.div),
                        ..Query::bare(self.context, function, layout)
                    };
                    if is_guaranteed_not_to_be_undef(&query, value, 0) {
                        continue;
                    }
                    let name = match value {
                        Operand::Value(one) => function.value(one).name.clone().map(|name| format!("{name}.frozen")),
                        _ => None,
                    };
                    let freeze =
                        function.create_instruction(Opcode::Freeze, ty, vec![value], Flags::default(), name.as_deref());
                    function.insert(freeze, Position::Before(entry.div)).expect("placed");
                    copy_location(function, entry.div, freeze);
                    let frozen = Operand::Value(function.instruction(freeze).result.expect("a value"));
                    function.set_operand(entry.div, index, frozen);
                    function.set_operand(if index == 0 { sub } else { mul }, index, frozen);
                }

                let name = name_of(function, entry.rem).map(|one| format!("{one}.decomposed"));
                if let (Some(name), Some(result)) = (name, function.instruction(sub).result) {
                    function.set_name(result, &name);
                }
                replace_with(function, entry.rem, sub);
            }
            changed = true;
        }
        changed
    }
}

/// The name of the value `inst` makes, if it has one.
fn name_of(
    function: &Function,
    inst: InstId,
) -> Option<String> {
    function.instruction(inst).result.and_then(|result| function.value(result).name.clone())
}

/// `BasicBlock::getUniquePredecessor`.
fn unique_predecessor(
    function: &Function,
    block: BlockId,
) -> Option<BlockId> {
    let predecessors = function.predecessors(block);
    (predecessors.len() == 1).then(|| predecessors[0])
}

/// `setDebugLoc`: `to` stands where `from` did.
fn copy_location(
    function: &mut Function,
    from: InstId,
    to: InstId,
) {
    let line: Vec<_> = function.instruction(from).metadata.iter().filter(|(kind, _)| kind == "dbg").cloned().collect();
    for (kind, node) in line {
        function.set_metadata(to, &kind, Some(node));
    }
}

/// `replaceAllUsesWith` and `eraseFromParent`.
fn replace_with(
    function: &mut Function,
    old: InstId,
    new: InstId,
) {
    let from = function.instruction(old).result.expect("a value");
    let to = Operand::Value(function.instruction(new).result.expect("a value"));
    function.replace_value(from, to);
    function.erase(old).expect("nothing reads it now");
}

pub struct DivRemPairs;

impl FunctionPass for DivRemPairs {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "divrempairs"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("div-rem-pairs")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        let lowering = outer.target().lowering();
        let tti = outer.target().tti();
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let pass = DivRem {
            context: unit.context,
            globals: &outer.globals,
            lowering: lowering.as_deref(),
            tti: tti.as_deref(),
        };
        if pass.run(unit.function, &tree, unit.layout) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "divrempairs_tests.rs"]
mod tests;
