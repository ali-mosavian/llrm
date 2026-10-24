//! A branch on an or/and chain of compares of one value with constants,
//! turned into a switch (or a range check): LLVM's
//! [`SimplifyCFGOpt::simplifyBranchOnICmpChain`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L5469)
//! with `ConstantComparesGatherer`
//! ([L580](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L580)).
//!
//! # Example
//! ```text
//!   %a = icmp eq i16 %x, 1                 switch i16 %x, label %f [
//!   %b = icmp eq i16 %x, 5                   i16 5, label %t
//!   %c = or i1 %a, %b                        i16 1, label %t ]
//!   br i1 %c, label %t, label %f
//! ```
//!
//! One clause that is not a compare of the value is allowed: it is tested by
//! a branch of its own first (`switch.early.test`), frozen unless it is known
//! not to be undef or poison.
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | A pointer constant (`null`, `inttoptr`) is not a compared integer (`getConstantInt`'s pointer arm), so no value is a pointer and `ptrtoint` is never made. |
//! | D2 | No branch weights; `!unpredictable` is kept. |
//! | D3 | `!llvm.loop` of the old branch is not copied, as `IRBuilder::CreateCondBr` does not. |

use llrm_analysis::occurrence::{arms, from_arms};
use llrm_mir::constantrange::ConstantRange;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison;
use llrm_mir::valuetracking::{Query, pattern};
use llrm_support::hash::HashSet;

use crate::edges;
use crate::simplifycfg::condition_deleted;
use crate::twoentryphi::Speculation;

/// `ConstantComparesGatherer`.
struct Gatherer<'q, 'a> {
    q: &'q Query<'a>,
    comp_value: Option<Operand>,
    extra: Option<Operand>,
    values: Vec<u128>,
    used: u32,
    is_eq: bool,
    ignore_first_match: bool,
    multiple_matches: bool,
}

fn mask(width: u32) -> u128 {
    llrm_mir::context::mask(width)
}

impl<'q, 'a> Gatherer<'q, 'a> {
    fn of(
        q: &'q Query<'a>,
        cond: Operand,
    ) -> Self {
        let mut one = Self {
            q,
            comp_value: None,
            extra: None,
            values: Vec::new(),
            used: 0,
            is_eq: false,
            ignore_first_match: false,
            multiple_matches: false,
        };
        one.gather(cond);
        if one.comp_value.is_some() || !one.multiple_matches {
            return one;
        }
        one.extra = None;
        one.values.clear();
        one.used = 0;
        one.ignore_first_match = true;
        one.gather(cond);
        one
    }

    /// `setValueOnce`.
    fn set_value_once(
        &mut self,
        new: Operand,
    ) -> bool {
        if self.ignore_first_match {
            self.ignore_first_match = false;
            return false;
        }
        if self.comp_value.is_some_and(|old| old != new) {
            self.multiple_matches = true;
            return false;
        }
        self.comp_value = Some(new);
        true
    }

    /// `matchInstruction`: `v` as a compare of a value with constants.
    fn match_instruction(
        &mut self,
        mut v: Operand,
        mut is_eq: bool,
    ) -> bool {
        if let Some(inner) = pattern::not(self.q, v)
            && self.q.instruction_of(inner).is_some()
        {
            v = inner;
            is_eq = !is_eq;
        }
        if let Some((_, instruction, value)) = pattern::cast(self.q, v, CastOp::Trunc)
            && instruction.flags.contains(Flags::NUW)
        {
            if !self.set_value_once(value) {
                return false;
            }
            self.used += 1;
            self.values.push(u128::from(is_eq));
            return true;
        }
        let Some((_, _, predicate, left, right)) = pattern::icmp(self.q, v) else { return false };
        let Some((c, width)) = self.q.int_constant(right) else { return false };
        let pred = predicate.pred;
        let constant = |operand| self.q.int_constant(operand).map(|(bits, _)| bits);

        if pred == if is_eq { IntPredicate::Eq } else { IntPredicate::Ne } {
            // (x & ~2^z) == y --> x == y || x == y|2^z, undoing instcombine's
            // fuse.
            if let Some((_, _, value, mask_operand)) = pattern::binary(self.q, left, BinaryOp::And)
                && let Some(rhs) = constant(mask_operand)
            {
                let mask = !rhs & mask(width);
                if mask.is_power_of_two() && c & !mask == c {
                    if !self.set_value_once(value) {
                        return false;
                    }
                    self.values.extend([c, c | mask]);
                    self.used += 1;
                    return true;
                }
            }
            // (x | 2^z) == y --> x == y || x == y&~2^z.
            if let Some((_, _, value, mask_operand)) = pattern::binary(self.q, left, BinaryOp::Or)
                && let Some(rhs) = constant(mask_operand)
            {
                let mask = rhs;
                if mask.is_power_of_two() && c | mask == c {
                    if !self.set_value_once(value) {
                        return false;
                    }
                    self.values.extend([c, c & !mask]);
                    self.used += 1;
                    return true;
                }
            }
            if !self.set_value_once(left) {
                return false;
            }
            self.used += 1;
            self.values.push(c);
            return true;
        }

        // `x ult 3` adds 0, 1, 2.
        let mut span = ConstantRange::make_exact_icmp_region(pred, width, c);
        // The range compare idiom instcombine emits: `add x, C` first.
        let mut candidate = left;
        if let Some((_, _, value, addend)) = pattern::binary(self.q, left, BinaryOp::Add)
            && let Some(rhs) = constant(addend)
        {
            span = span.subtract(rhs);
            candidate = value;
        }
        // An and/!= chain builds the values that do not pass it.
        if !is_eq {
            span = span.inverse();
        }
        if span.is_size_larger_than(8) || span.is_empty_set() {
            return false;
        }
        if !self.set_value_once(candidate) {
            return false;
        }
        let mut at = span.lower;
        loop {
            self.values.push(at);
            at = at.wrapping_add(1) & mask(width);
            if at == span.upper {
                break;
            }
        }
        self.used += 1;
        true
    }

    /// `gather`.
    fn gather(
        &mut self,
        v: Operand,
    ) {
        let (op0, op1);
        if let Some(pair) = pattern::logical_or(self.q, v) {
            self.is_eq = true;
            (op0, op1) = pair;
        } else if let Some(pair) = pattern::logical_and(self.q, v) {
            self.is_eq = false;
            (op0, op1) = pair;
        } else {
            return;
        }
        let mut stack = vec![op0, op1];
        let mut visited: HashSet<Operand> = [v, op0, op1].into_iter().collect();
        while let Some(v) = stack.pop() {
            if self.q.instruction_of(v).is_some() {
                let pair = if self.is_eq { pattern::logical_or(self.q, v) } else { pattern::logical_and(self.q, v) };
                if let Some((op0, op1)) = pair {
                    if visited.insert(op1) {
                        stack.push(op1);
                    }
                    if visited.insert(op0) {
                        stack.push(op0);
                    }
                    continue;
                }
                if self.match_instruction(v, self.is_eq) {
                    continue;
                }
            }
            // One clause may differ from the others.
            if self.extra.is_none() {
                self.extra = Some(v);
                continue;
            }
            self.comp_value = None;
            break;
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The branches `folded` gathered a chain from.
    pub(crate) static GATHERED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `block`'s `br` on a chain of compares turned into a switch: whether it was.
pub fn folded(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    let &[cond, Operand::Block(if_true), Operand::Block(if_false)] = &function.instruction(last).operands[..] else {
        return false;
    };
    // `gather` starts from a logical or/and (`or`, `and`, `select`): anything
    // else gathers nothing, and a lone compare is refused (`used <= 1`).
    let chain_root = match cond {
        Operand::Value(v) => match function.value(v).def {
            ValueDef::Instruction(def) => matches!(
                function.instruction(def).opcode,
                Opcode::Binary(BinaryOp::Or | BinaryOp::And) | Opcode::Select
            ),
            _ => false,
        },
        _ => false,
    };
    if !chain_root {
        return false;
    }
    #[cfg(test)]
    GATHERED.with(|one| one.set(one.get() + 1));
    let (gathered, extra_is_safe, ty, width) = {
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
            cxt: Some(last),
        };
        if q.instruction_of(cond).is_none() {
            return false;
        }
        let gatherer = Gatherer::of(&q, cond);
        let Some(comp_value) = gatherer.comp_value else { return false };
        // Avoid turning single compares into a switch.
        if gatherer.used <= 1 {
            return false;
        }
        let safe = gatherer.extra.is_none_or(|extra| is_guaranteed_not_to_be_undef_or_poison(&q, extra, 0));
        let ty = q.type_of(comp_value).expect("a compared value has a type");
        let width = q.scalar_bits(comp_value);
        ((comp_value, gatherer.extra, gatherer.values, gatherer.is_eq), safe, ty, width)
    };
    let (comp_value, extra, mut values, true_when_equal) = gathered;

    // The switch cannot hold a value twice: largest first, as LLVM sorts.
    values.sort_unstable_by(|a, b| b.cmp(a));
    values.dedup();
    // A switch on one value is a conditional branch.
    if extra.is_some() && values.len() < 2 {
        return false;
    }

    let (mut default, mut edge) = (if_false, if_true);
    if !true_when_equal {
        std::mem::swap(&mut default, &mut edge);
    }

    let void = function.instruction(last).ty;
    let i1 = context.types.int(1);
    let int = |value: u128| Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(value) }));
    let mut at = block;
    if let Some(extra) = extra {
        let new = split_before(function, block, last);
        let old_jump = function.terminator(block).expect("the jump the split made");
        let mut tested = extra;
        if !extra_is_safe {
            let freeze = function.create_instruction(Opcode::Freeze, i1, vec![extra], Flags::default(), None);
            function.insert(freeze, Position::Before(old_jump)).expect("a placed terminator");
            tested = Operand::Value(function.instruction(freeze).result.expect("a freeze's value"));
        }
        let operands = if true_when_equal {
            vec![tested, Operand::Block(edge), Operand::Block(new)]
        } else {
            vec![tested, Operand::Block(new), Operand::Block(edge)]
        };
        let branch = function.create_instruction(Opcode::Br, void, operands, Flags::default(), None);
        function.insert(branch, Position::Before(old_jump)).expect("a placed terminator");
        function.erase(old_jump).expect("a terminator defines nothing");
        // `addPredecessorToBlock(EdgeBB, BB, NewBB)`.
        for phi in edges::phis(function, edge) {
            let incoming = arms(function, phi);
            let value = incoming.iter().find(|&&(_, from)| from == new).expect("an input from the split block").0;
            let mut now = incoming;
            now.push((value, block));
            function.set_operands(phi, from_arms(&now));
        }
        at = new;
    }

    let contiguous = values[0].wrapping_sub(values[values.len() - 1]) & mask(width) == values.len() as u128 - 1;
    let new = if contiguous {
        // A range check, not a switch; no new edge.
        let range = ConstantRange::non_empty(width, values[values.len() - 1], values[0].wrapping_add(1));
        let (pred, rhs, offset) = range.get_equivalent_icmp_offset();
        let mut x = comp_value;
        if offset != 0 {
            let add = function.create_instruction(
                Opcode::Binary(BinaryOp::Add),
                ty,
                vec![x, int(offset)],
                Flags::default(),
                None,
            );
            function.insert(add, Position::Before(last)).expect("a placed terminator");
            x = Operand::Value(function.instruction(add).result.expect("an add's value"));
        }
        let compare = function.create_instruction(Opcode::ICmp(pred), i1, vec![x, int(rhs)], Flags::default(), None);
        function.insert(compare, Position::Before(last)).expect("a placed terminator");
        let result = Operand::Value(function.instruction(compare).result.expect("a compare's value"));
        function.create_instruction(
            Opcode::Br,
            void,
            vec![result, Operand::Block(edge), Operand::Block(default)],
            Flags::default(),
            None,
        )
    } else {
        let mut operands = vec![comp_value, Operand::Block(default)];
        operands.extend(values.iter().flat_map(|&value| [int(value), Operand::Block(edge)]));
        // The edges added: each phi of the target gets one input per new case.
        for phi in edges::phis(function, edge) {
            let mut now = arms(function, phi);
            let value = now.iter().find(|&&(_, from)| from == at).expect("an input from the branch's block").0;
            now.extend(std::iter::repeat_n((value, at), values.len() - 1));
            function.set_operands(phi, from_arms(&now));
        }
        function.create_instruction(Opcode::Switch, void, operands, Flags::default(), None)
    };
    function.insert(new, Position::Before(last)).expect("a placed terminator");
    for (kind, node) in function.instruction(last).metadata.clone() {
        if kind == "unpredictable" {
            function.annotate(new, &kind, node);
        }
    }
    // `eraseTerminatorAndDCECond`.
    function.erase(last).expect("a terminator defines nothing");
    condition_deleted(context, spec.outer.callees(), function, cond, true);
    true
}

/// `SplitBlock(block, last)`: `last` and what follows it move to a new block,
/// placed after `block`, which jumps to it; the successors' phis name the new
/// block.
fn split_before(
    function: &mut Function,
    block: BlockId,
    last: InstId,
) -> BlockId {
    let void = function.instruction(last).ty;
    let new = function.create_block(Some("switch.early.test"));
    function.insert_block(new, Some(block)).expect("a new block");
    function.splice(&[last], Position::End(new)).expect("a placed terminator");
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(new)], Flags::default(), None);
    function.insert(jump, Position::End(block)).expect("a placed block");
    for successor in function.successors(new).into_iter().collect::<Vec<_>>() {
        for phi in edges::phis(function, successor) {
            let incoming: Vec<_> = arms(function, phi)
                .into_iter()
                .map(|(value, from)| (value, if from == block { new } else { from }))
                .collect();
            function.set_operands(phi, from_arms(&incoming));
        }
    }
    new
}

#[cfg(test)]
#[path = "icmpchain_tests.rs"]
mod tests;
