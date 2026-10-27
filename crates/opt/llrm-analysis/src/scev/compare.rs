//! The ordering of operands: `CompareValueComplexity`, `CompareSCEVComplexity`
//! and `GroupByComplexity`, which put `(a + b)` and `(b + a)` in one form.

use llrm_mir::module::{InstId, Linkage, Operand, ValueDef};

use super::expr::{Kind, Scev, Type, Use};
use super::{Cx, ScalarEvolution, value_id};

/// `scalar-evolution-max-value-compare-depth`.
const MAX_VALUE_COMPARE_DEPTH: u32 = 2;
/// `scalar-evolution-max-scev-compare-depth`.
const MAX_SCEV_COMPARE_DEPTH: u32 = 32;

/// `llvm::GlobalValue::LinkageTypes`' numbering.
fn linkage_rank(linkage: Linkage) -> i32 {
    match linkage {
        Linkage::External => 0,
        Linkage::AvailableExternally => 1,
        Linkage::LinkOnce => 2,
        Linkage::LinkOnceOdr => 3,
        Linkage::Weak => 4,
        Linkage::WeakOdr => 5,
        Linkage::Appending => 6,
        Linkage::Internal => 7,
        Linkage::Private => 8,
        Linkage::ExternWeak => 9,
        Linkage::Common => 10,
    }
}

/// An instruction's operands as `Instruction::getOperand` numbers them: a
/// phi's are its values (the blocks are kept apart), and an `alloca` always
/// has its array size.
fn llvm_operands(
    cx: &Cx,
    inst: InstId,
) -> Vec<Operand> {
    use llrm_mir::opcode::Opcode as O;
    let instruction = cx.function.instruction(inst);
    match instruction.opcode {
        O::Phi => instruction.operands.chunks(2).map(|pair| pair[0]).collect(),
        O::Alloca { .. } if instruction.operands.is_empty() => {
            // The constant 1 the C++ operand list holds is the same constant
            // in both operands being compared: represent it by a block-free
            // placeholder that compares equal to itself.
            vec![Operand::Block(llrm_mir::module::BlockId(u32::MAX))]
        }
        _ => instruction.operands.clone(),
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L592
    /// `CompareValueComplexity`.
    pub(super) fn compare_value_complexity(
        &self,
        cx: &Cx,
        lv: Operand,
        rv: Operand,
        depth: u32,
    ) -> i32 {
        if depth > MAX_VALUE_COMPARE_DEPTH {
            return 0;
        }

        // Order pointer values after integer values. This helps SCEVExpander
        // form GEPs.
        let is_pointer = |v: Operand| -> bool {
            match v {
                Operand::Value(id) => self.ty_of(cx, cx.function.value(id).ty).is_some_and(|ty| ty.is_pointer()),
                Operand::Constant(id) => self.ty_of(cx, cx.context.get(id).ty).is_some_and(|ty| ty.is_pointer()),
                Operand::Block(_) => false,
            }
        };
        let (l_pointer, r_pointer) = (is_pointer(lv), is_pointer(rv));
        if l_pointer != r_pointer {
            return i32::from(l_pointer) - i32::from(r_pointer);
        }

        // Compare getValueID values.
        let (l_id, r_id) = (value_id(cx, lv), value_id(cx, rv));
        if l_id != r_id {
            return l_id as i32 - r_id as i32;
        }

        // Sort arguments by their position.
        if let (Operand::Value(l), Operand::Value(r)) = (lv, rv)
            && let (ValueDef::Argument(l), ValueDef::Argument(r)) = (cx.function.value(l).def, cx.function.value(r).def)
        {
            return l as i32 - r as i32;
        }

        if let (Operand::Constant(l), Operand::Constant(r)) = (lv, rv)
            && let (llrm_mir::context::ConstantKind::Global(l), llrm_mir::context::ConstantKind::Global(r)) =
                (&cx.context.get(l).kind, &cx.context.get(r).kind)
        {
            let (lg, rg) = (cx.global(*l), cx.global(*r));
            let difference = linkage_rank(lg.linkage) - linkage_rank(rg.linkage);
            if difference != 0 {
                return difference;
            }
            let semantic = |linkage: Linkage| !matches!(linkage, Linkage::Private | Linkage::Internal);
            // Use the names to distinguish the two values, but only if the
            // names are semantically important.
            if semantic(lg.linkage) && semantic(rg.linkage) {
                return match lg.name.cmp(&rg.name) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
            }
        }

        // For instructions, compare their loop depth, and their operand
        // count. This is pretty loose.
        if let (Operand::Value(l), Operand::Value(r)) = (lv, rv)
            && let (ValueDef::Instruction(l), ValueDef::Instruction(r)) =
                (cx.function.value(l).def, cx.function.value(r).def)
        {
            // Compare loop depths.
            let (l_parent, r_parent) = (cx.function.parent(l), cx.function.parent(r));
            if l_parent != r_parent
                && let (Some(l_parent), Some(r_parent)) = (l_parent, r_parent)
            {
                let (l_depth, r_depth) = (cx.loops.get_loop_depth(l_parent), cx.loops.get_loop_depth(r_parent));
                if l_depth != r_depth {
                    return l_depth as i32 - r_depth as i32;
                }
            }

            // Compare the number of operands.
            let (l_ops, r_ops) = (llvm_operands(cx, l), llvm_operands(cx, r));
            if l_ops.len() != r_ops.len() {
                return l_ops.len() as i32 - r_ops.len() as i32;
            }
            for (l, r) in l_ops.into_iter().zip(r_ops) {
                let result = self.compare_value_complexity(cx, l, r, depth + 1);
                if result != 0 {
                    return result;
                }
            }
        }

        0
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L675
    /// `CompareSCEVComplexity`: negative, zero or positive as `lhs` is less
    /// complex than, as complex as, or more complex than `rhs`; `None` when
    /// the depth limit was reached.
    pub(super) fn compare_scev_complexity(
        &self,
        cx: &Cx,
        lhs: Scev,
        rhs: Scev,
        depth: u32,
    ) -> Option<i32> {
        // Fast-path: SCEVs are uniqued so we can do a quick equality check.
        if lhs == rhs {
            return Some(0);
        }

        // Primarily, sort the SCEVs by their getSCEVType().
        let (l_type, r_type) = (self.scev_type(lhs), self.scev_type(rhs));
        if l_type != r_type {
            return Some(l_type as i32 - r_type as i32);
        }

        if depth > MAX_SCEV_COMPARE_DEPTH {
            return None;
        }

        // Aside from the getSCEVType() ordering, the particular ordering isn't
        // very important except that it's beneficial to be consistent, so that
        // (a + b) and (b + a) don't end up as different expressions.
        match l_type {
            Type::Unknown => {
                let (Kind::Unknown(l), Kind::Unknown(r)) = (&self.node(lhs).kind, &self.node(rhs).kind) else {
                    unreachable!()
                };
                return Some(self.compare_value_complexity(cx, *l, *r, depth + 1));
            }
            Type::Constant => {
                let (Kind::Constant(l), Kind::Constant(r)) = (&self.node(lhs).kind, &self.node(rhs).kind) else {
                    unreachable!()
                };
                // Compare constant values.
                if l.width != r.width {
                    return Some(l.width as i32 - r.width as i32);
                }
                return Some(if l.ult(*r) { -1 } else { 1 });
            }
            Type::VScale => unreachable!("DEVIATE 2: no vscale"),
            Type::AddRec => {
                let (Kind::AddRec(_, l_head), Kind::AddRec(_, r_head)) = (&self.node(lhs).kind, &self.node(rhs).kind)
                else {
                    unreachable!()
                };
                // There is always a dominance between two recs that are used
                // by one SCEV, so we can safely sort recs by loop header
                // dominance. We require such order in getAddExpr.
                if l_head != r_head {
                    if cx.tree.dominates(*l_head, *r_head) {
                        return Some(1);
                    }
                    debug_assert!(
                        cx.tree.dominates(*r_head, *l_head),
                        "No dominance between recurrences used by one SCEV?"
                    );
                    return Some(-1);
                }
            }
            Type::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
            _ => {}
        }

        // Lexicographically compare n-ary-like expressions.
        let (l_ops, r_ops) = (self.operands_of(lhs), self.operands_of(rhs));
        if l_ops.len() != r_ops.len() {
            return Some(l_ops.len() as i32 - r_ops.len() as i32);
        }
        for (l, r) in l_ops.iter().zip(&r_ops) {
            let x = self.compare_scev_complexity(cx, l.scev, r.scev, depth + 1);
            if x != Some(0) {
                return x;
            }
        }
        Some(0)
    }

    /// `SCEV::operands` for every kind, `UDiv`'s two included.
    pub fn operands_of(
        &self,
        s: Scev,
    ) -> Vec<Use> {
        match &self.node(s).kind {
            Kind::UDiv(l, r) => vec![*l, *r],
            _ => self.operands(s).to_vec(),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L783
    /// `GroupByComplexity`: order `ops` by complexity and group equal ones
    /// together, deterministically (not by where the nodes landed in memory).
    pub(super) fn group_by_complexity(
        &self,
        cx: &Cx,
        ops: &mut [Use],
    ) {
        if ops.len() < 2 {
            return;
        }

        // Whether LHS has provably less complexity than RHS.
        let is_less_complex = |lhs: Use, rhs: Use| -> bool {
            self.compare_scev_complexity(cx, lhs.scev, rhs.scev, 0).is_some_and(|complexity| complexity < 0)
        };
        if ops.len() == 2 {
            // This is the common case, which also happens to be trivially
            // simple. Special case it.
            if is_less_complex(ops[1], ops[0]) {
                ops.swap(0, 1);
            }
            return;
        }

        // Do the rough sort by complexity. `stable_sort` with a comparator that
        // may report "not less" both ways (the depth limit): a merge sort keeps
        // the equivalent elements in order, as `std::stable_sort` does.
        merge_sort(ops, &is_less_complex);

        // Now that we are sorted by complexity, group elements of the same
        // complexity. Note that this is, at worst, N^2, but the vector is
        // likely to be extremely short in practice.
        let e = ops.len();
        let mut i = 0;
        while i + 2 < e {
            let s = ops[i];
            let complexity = self.scev_type(s.scev);
            // If there are any objects of the same complexity and same value
            // as this one, group them.
            let mut j = i + 1;
            while j != e && self.scev_type(ops[j].scev) == complexity {
                if ops[j].scev == s.scev {
                    // Found a duplicate. Move it to immediately after i'th
                    // element.
                    ops.swap(i + 1, j);
                    i += 1; // no need to rescan it.
                    if i == e - 2 {
                        return; // Done!
                    }
                }
                j += 1;
            }
            i += 1;
        }
    }
}

/// A stable merge sort by `less`, which need not be a strict weak order.
fn merge_sort(
    items: &mut [Use],
    less: &dyn Fn(Use, Use) -> bool,
) {
    if items.len() < 2 {
        return;
    }
    let middle = items.len() / 2;
    merge_sort(&mut items[..middle], less);
    merge_sort(&mut items[middle..], less);
    let mut merged: Vec<Use> = Vec::with_capacity(items.len());
    let (mut left, mut right) = (0, middle);
    while left < middle && right < items.len() {
        // Take from the right only if it is strictly less: ties keep order.
        if less(items[right], items[left]) {
            merged.push(items[right]);
            right += 1;
        } else {
            merged.push(items[left]);
            left += 1;
        }
    }
    merged.extend_from_slice(&items[left..middle]);
    merged.extend_from_slice(&items[right..]);
    items.copy_from_slice(&merged);
}
