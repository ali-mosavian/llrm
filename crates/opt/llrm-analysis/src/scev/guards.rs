//! `ScalarEvolution::LoopGuards` and `applyLoopGuards`: the facts the guards
//! of a loop give about the expressions in it.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ScalarEvolution.cpp`. The conditions
//! that dominate a loop (the branches on the way up from the preheader, the
//! `llvm.assume`s) are read as rewrites: `x u< C` makes `x` into
//! `umin(x, C - 1)`, `x != 0` into `umax(x, 1)`, `x % 8 == 0` into
//! `(x /u 8) * 8`. [`ScalarEvolution::apply_loop_guards_with`] rewrites an
//! expression with them, so ranges and trip counts see what the guards say.
//!
//! # Deviations
//! # Example
//! Under `if (n != 0)` the preheader's guards rewrite `%n` to `umax(1, %n)`.

use llrm_mir::constantrange::{ConstantRange, PreferredRangeType};
use llrm_mir::module::{BlockId, Operand};
use llrm_mir::opcode::{IntPredicate, Opcode};
use llrm_mir::valuetracking::pattern;
use llrm_support::hash::HashMap;

use super::ap::Ap;
use super::expr::{Kind, Scev, Type, Use, flags};
use super::{Cx, Flags, ScalarEvolution};

/// `MaxLoopGuardCollectionDepth`.
const MAX_LOOP_GUARD_COLLECTION_DEPTH: u32 = 1;

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1579
/// `ScalarEvolution::LoopGuards`.
#[derive(Clone, Debug, Default)]
pub struct LoopGuards {
    pub rewrite_map: HashMap<Scev, Scev>,
    pub not_equal: Vec<(Scev, Scev)>,
    pub preserve_nuw: bool,
    pub preserve_nsw: bool,
}

/// What the divisibility guards of one block say.
type Multiples = Vec<(Scev, Ap)>;

thread_local! {
    static COLLECTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times the guards of a loop were collected on this thread: a test
/// counts what a query asks for afresh.
pub fn collections() -> usize {
    COLLECTIONS.with(std::cell::Cell::get)
}

thread_local! {
    static VISITS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

/// The blocks a collection has been to (`VisitedBlocks`). Returns the
/// (inserts, elements examined) on this thread: a test bounds the second by
/// the first.
pub fn visits() -> (usize, usize) {
    VISITS.with(std::cell::Cell::get)
}

/// A `SmallPtrSet` of blocks.
#[derive(Default)]
struct Visited(llrm_mir::dense::IdSet<BlockId>);

impl Visited {
    /// Whether `block` was new.
    fn insert(
        &mut self,
        block: BlockId,
    ) -> bool {
        VISITS.with(|one| {
            let (inserts, examined) = one.get();
            one.set((inserts + 1, examined + 1));
        });
        self.0.insert(block)
    }
}

impl LoopGuards {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15749
    /// `LoopGuards::collect(L, SE)`.
    pub fn collect(
        se: &mut ScalarEvolution,
        cx: &Cx,
        l: BlockId,
    ) -> Self {
        COLLECTIONS.with(|one| one.set(one.get() + 1));
        let loop_ = cx.loops.get_loop_for(l).expect("the header of a loop");
        let header = loop_.get_header();
        let mut guards = Self::default();
        let Some(pred) = loop_.get_loop_predecessor(cx.function) else { return guards };
        let mut visited_blocks = Visited::default();
        Self::collect_from_block(se, cx, &mut guards, header, pred, &mut visited_blocks, 0);
        guards
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15760
    /// `LoopGuards::collectFromPHI`.
    fn collect_from_phi(
        se: &mut ScalarEvolution,
        cx: &Cx,
        guards: &mut Self,
        phi: llrm_mir::module::InstId,
        visited_blocks: &mut Visited,
        incoming_guards: &mut Vec<(BlockId, LoopGuards)>,
        depth: u32,
    ) {
        let instruction = cx.function.instruction(phi);
        let Some(phi_ty) =
            cx.function.operand_type(cx.context, Operand::Value(instruction.result.expect("a phi has a result")))
        else {
            return;
        };
        if !se.is_scevable(cx.context.types.get(phi_ty)) {
            return;
        }
        let phi_block = cx.function.parent(phi).expect("a placed phi");
        let incoming: Vec<(Operand, BlockId)> = pattern::phi_incoming(instruction).collect();

        type MinMaxPattern = Option<(Ap, Type)>;
        let get_min_max_const = |se: &mut ScalarEvolution,
                                 visited_blocks: &mut Visited,
                                 incoming_guards: &mut Vec<(BlockId, LoopGuards)>,
                                 index: usize|
         -> MinMaxPattern {
            let (value, in_block) = incoming[index];
            if !visited_blocks.insert(in_block) {
                return None;
            }

            // Avoid analyzing unreachable blocks so that we don't get trapped
            // traversing cycles with ill-formed dominance or infinite cycles.
            if !cx.tree.is_reachable(in_block) {
                return None;
            }

            let at = match incoming_guards.iter().position(|(block, _)| *block == in_block) {
                Some(at) => at,
                None => {
                    let mut fresh = LoopGuards::default();
                    Self::collect_from_block(se, cx, &mut fresh, phi_block, in_block, visited_blocks, depth + 1);
                    incoming_guards.push((in_block, fresh));
                    incoming_guards.len() - 1
                }
            };
            let rewrite_map = &incoming_guards[at].1.rewrite_map;
            if rewrite_map.is_empty() {
                return None;
            }
            let key = se.get_scev(cx, value);
            let rewritten = *rewrite_map.get(&key)?;
            let (Kind::UMax(ops) | Kind::SMax(ops) | Kind::UMin(ops) | Kind::SMin(ops)) =
                se.node(rewritten).kind.clone()
            else {
                return None;
            };
            let c0 = se.as_constant(ops[0].scev)?;
            Some((c0, se.node(rewritten).kind.tag()))
        };
        let merge_min_max_const = |p1: MinMaxPattern, p2: MinMaxPattern| -> MinMaxPattern {
            let ((c1, t1), (c2, t2)) = (p1?, p2?);
            if t1 != t2 {
                return None;
            }
            match t1 {
                Type::UMax => Some((if c1.ult(c2) { c1 } else { c2 }, t1)),
                Type::SMax => Some((if c1.slt(c2) { c1 } else { c2 }, t1)),
                Type::UMin => Some((if c2.ult(c1) { c1 } else { c2 }, t1)),
                Type::SMin => Some((if c2.slt(c1) { c1 } else { c2 }, t1)),
                _ => unreachable!("Trying to merge non-MinMaxExpr SCEVs."),
            }
        };
        let mut p = get_min_max_const(se, visited_blocks, incoming_guards, 0);
        for index in 1..incoming.len() {
            if p.is_none() {
                break;
            }
            let next = get_min_max_const(se, visited_blocks, incoming_guards, index);
            p = merge_min_max_const(p, next);
        }
        if let Some((constant, kind)) = p {
            let lhs = se.get_scev(cx, Operand::Value(instruction.result.expect("a phi has a result")));
            let c = se.get_constant(constant);
            let rhs = se.get_min_max_expr(cx, kind, vec![Use::of(c), Use::of(lhs)]);
            guards.rewrite_map.entry(lhs).or_insert(rhs);
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15933
    /// `LoopGuards::collectFromBlock`.
    fn collect_from_block(
        se: &mut ScalarEvolution,
        cx: &Cx,
        guards: &mut Self,
        block: BlockId,
        pred: BlockId,
        visited_blocks: &mut Visited,
        depth: u32,
    ) {
        debug_assert!(cx.tree.is_reachable(block) && cx.tree.is_reachable(pred));

        let mut exprs_to_rewrite: Vec<Scev> = Vec::new();

        let mut terms: Vec<(Operand, bool)> = Vec::new();
        // First, collect information from assumptions dominating the loop.
        let first = cx.function.block(block).first();
        for &assume in &cx.assumptions.assumes {
            let dominates = first.is_some_and(|first| cx.tree.instruction_dominates(cx.function, assume, first));
            if !dominates {
                continue;
            }
            terms.push((cx.function.instruction(assume).operands[0], true));
        }

        // Second, collect information from llvm.experimental.guards dominating
        // the loop.
        if let Some(first) = first {
            for guard in se.guard_calls(cx) {
                if cx.tree.instruction_dominates(cx.function, guard, first) {
                    terms.push((cx.function.instruction(guard).operands[0], true));
                }
            }
        }

        // Third, collect conditions from dominating branches. Starting at the
        // loop predecessor, climb up the predecessor chain, as long as there
        // are predecessors that can be found that have unique successors
        // leading to the original header.
        // TODO (LLVM): share this logic with isLoopEntryGuardedByCond.
        let mut num_collected_conditions = 0;
        visited_blocks.insert(block);
        let mut pair = Some((pred, block));
        // `Pair.second` after the loop: the block the climb stopped at.
        let mut stopped_at = block;
        while let Some((first_block, second)) = pair {
            stopped_at = second;
            visited_blocks.insert(second);
            if let Some(terminator) = cx.function.terminator(first_block) {
                let branch = cx.function.instruction(terminator);
                if let (Opcode::Br, [condition, Operand::Block(taken), Operand::Block(_)]) =
                    (&branch.opcode, branch.operands.as_slice())
                {
                    terms.push((*condition, *taken == second));
                    num_collected_conditions += 1;

                    // If we are recursively collecting guards stop after 2
                    // conditions to limit compile-time impact for now.
                    if depth > 0 && num_collected_conditions == 2 {
                        break;
                    }
                }
            }
            pair = se.get_predecessor_with_unique_successor_for_bb(cx, first_block);
            if pair.is_none() {
                // {nullptr, BB}
                stopped_at = first_block;
            }
        }
        // Finally, if we stopped climbing the predecessor chain because there
        // wasn't a unique one to continue, try to collect conditions for
        // PHINodes by recursively following all of their incoming blocks and
        // try to merge the found conditions to build a new one for the Phi.
        let second = stopped_at;
        if cx.function.predecessors(second).len() >= 2 && depth < MAX_LOOP_GUARD_COLLECTION_DEPTH {
            let mut incoming_guards: Vec<(BlockId, LoopGuards)> = Vec::new();
            let phis: Vec<_> = cx
                .function
                .iter_block(second)
                .take_while(|&inst| cx.function.instruction(inst).opcode == Opcode::Phi)
                .collect();
            for phi in phis {
                Self::collect_from_phi(se, cx, guards, phi, visited_blocks, &mut incoming_guards, depth);
            }
        }

        // Now apply the information from the collected conditions to
        // Guards.RewriteMap. Conditions are processed in reverse order, so the
        // earliest conditions is processed first, except guards with
        // divisibility information, which are moved to the back. This ensures
        // the SCEVs with the shortest dependency chains are constructed first.
        let mut guards_to_process: Vec<(IntPredicate, Scev, Scev)> = Vec::new();
        let query = cx.query(None);
        for &(term, enter_if_true) in terms.iter().rev() {
            let mut worklist = vec![term];
            let mut visited: Vec<Operand> = Vec::new();
            while let Some(cond) = worklist.pop() {
                if visited.contains(&cond) {
                    continue;
                }
                visited.push(cond);

                if let Some((_, instruction)) = query.instruction_of(cond)
                    && let Opcode::ICmp(cmp) = instruction.opcode
                {
                    let mut predicate = if enter_if_true { cmp } else { cmp.inverse() };
                    let mut lhs = se.get_scev(cx, instruction.operands[0]);
                    let mut rhs = se.get_scev(cx, instruction.operands[1]);
                    // If LHS is a constant, apply information to the other
                    // expression.
                    // TODO (LLVM): If LHS is not a constant, check if using
                    // CompareSCEVComplexity can improve results.
                    if se.as_constant(lhs).is_some() {
                        std::mem::swap(&mut lhs, &mut rhs);
                        predicate = predicate.swapped();
                    }
                    guards_to_process.push((predicate, lhs, rhs));
                    continue;
                }

                let sides =
                    if enter_if_true { pattern::logical_and(&query, cond) } else { pattern::logical_or(&query, cond) };
                if let Some((l, r)) = sides {
                    worklist.push(l);
                    worklist.push(r);
                }
            }
        }

        // Process divisibility guards in reverse order to populate DivGuards
        // early.
        let mut multiples: Multiples = Vec::new();
        let mut div_guards = LoopGuards::default();
        for &(predicate, lhs, rhs) in &guards_to_process {
            if !is_divisibility_guard(se, cx, lhs, rhs) {
                continue;
            }
            collect_divisibility_information(se, cx, predicate, lhs, rhs, &mut div_guards.rewrite_map, &mut multiples);
        }

        for &(predicate, lhs, rhs) in &guards_to_process {
            collect_condition(se, cx, guards, &mut exprs_to_rewrite, &div_guards, predicate, lhs, rhs);
        }

        // Apply divisibility information last. This ensures it is applied to
        // the outermost expression after other rewrites for the given value.
        for (k, divisor) in &multiples {
            let divisor_scev = se.get_constant(*divisor);
            let rewritten = se.apply_loop_guards_with(cx, *k, guards);
            let applied = apply_divisibility_on_min_max_expr(se, cx, rewritten, *divisor);
            let quotient = se.get_udiv_expr(cx, Use::of(applied), Use::of(divisor_scev));
            let product = se.get_mul_expr(cx, vec![Use::of(quotient), Use::of(divisor_scev)], Flags::default(), 0).scev;
            guards.rewrite_map.insert(*k, product);
            exprs_to_rewrite.push(*k);
        }

        // Let the rewriter preserve NUW/NSW flags if the unsigned/signed
        // ranges of the replacement expressions are contained in the ranges of
        // the replaced expressions.
        guards.preserve_nuw = true;
        guards.preserve_nsw = true;
        for &expr in &exprs_to_rewrite {
            let rewrite_to = guards.rewrite_map[&expr];
            let (expr_unsigned, to_unsigned) = (se.get_unsigned_range(cx, expr), se.get_unsigned_range(cx, rewrite_to));
            guards.preserve_nuw &= expr_unsigned.contains_range(&to_unsigned);
            let (expr_signed, to_signed) = (se.get_signed_range(cx, expr), se.get_signed_range(cx, rewrite_to));
            guards.preserve_nsw &= expr_signed.contains_range(&to_signed);
        }

        // Now that all rewrite information is collect, rewrite the collected
        // expressions with the information in the map. This applies
        // information to sub-expressions.
        if exprs_to_rewrite.len() > 1 {
            for &expr in &exprs_to_rewrite {
                let rewrite_to = guards.rewrite_map[&expr];
                guards.rewrite_map.remove(&expr);
                let rewritten = se.apply_loop_guards_with(cx, rewrite_to, guards);
                guards.rewrite_map.entry(expr).or_insert(rewritten);
            }
        }
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15849
/// `getPreviousSCEVDivisibleByDivisor`: the closest number divisible by the
/// divisor and not above `expr`. For now, only a constant `expr`.
fn get_previous_scev_divisible_by_divisor(
    se: &mut ScalarEvolution,
    expr: Scev,
    divisor: Ap,
) -> Scev {
    let Some(value) = se.match_apint(expr) else { return expr };
    if value.is_negative() || !divisor.is_strictly_positive() {
        return expr;
    }
    let rem = value.urem(divisor);
    // return the SCEV: Expr - Expr % Divisor
    se.get_constant(value.sub(rem))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15866
/// `getNextSCEVDivisibleByDivisor`: the closest number divisible by the
/// divisor and not below `expr`. For now, only a constant `expr`.
fn get_next_scev_divisible_by_divisor(
    se: &mut ScalarEvolution,
    expr: Scev,
    divisor: Ap,
) -> Scev {
    let Some(value) = se.match_apint(expr) else { return expr };
    if value.is_negative() || !divisor.is_strictly_positive() {
        return expr;
    }
    let rem = value.urem(divisor);
    if rem.is_zero() {
        return expr;
    }
    // return the SCEV: Expr + Divisor - Expr % Divisor
    se.get_constant(value.add(divisor).sub(rem))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15882
/// `collectDivisibilityInformation`.
fn collect_divisibility_information(
    se: &mut ScalarEvolution,
    cx: &Cx,
    predicate: IntPredicate,
    lhs: Scev,
    rhs: Scev,
    div_info: &mut HashMap<Scev, Scev>,
    multiples: &mut Multiples,
) -> bool {
    // If we have LHS == 0, check if LHS is computing a property of some
    // unknown SCEV %v which we can rewrite %v to express explicitly.
    if predicate != IntPredicate::Eq || !se.match_zero(rhs) {
        return false;
    }
    // If LHS is A % B, i.e. A % B == 0, rewrite A to (A /u B) * B to
    // explicitly express that.
    let Some((urem_lhs, urem_rhs)) = se.match_urem(cx, lhs) else { return false };
    if se.unknown_value(urem_lhs).is_none() {
        return false;
    }

    let quotient = se.get_udiv_expr(cx, Use::of(urem_lhs), Use::of(urem_rhs));
    let multiple = se.get_mul_expr(cx, vec![Use::of(quotient), Use::of(urem_rhs)], Flags::default(), 0).scev;
    div_info.insert(urem_lhs, multiple);
    if let Some(c) = se.match_apint(urem_rhs) {
        match multiples.iter_mut().find(|(key, _)| *key == urem_lhs) {
            Some(entry) => entry.1 = c,
            None => multiples.push((urem_lhs, c)),
        }
    }
    true
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15904
/// `isDivisibilityGuard`: the condition is a divisibility guard (A % B == 0).
fn is_divisibility_guard(
    se: &mut ScalarEvolution,
    cx: &Cx,
    lhs: Scev,
    rhs: Scev,
) -> bool {
    se.match_urem(cx, lhs).is_some() && se.match_zero(rhs)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15912
/// `applyDivisibilityOnMinMaxExpr`: apply divisibility by `divisor` on a
/// min/max with constant values, recursively, by aligning up/down the constant
/// value to the divisor.
fn apply_divisibility_on_min_max_expr(
    se: &mut ScalarEvolution,
    cx: &Cx,
    min_max_expr: Scev,
    divisor: Ap,
) -> Scev {
    // Whether `expr` is a MinMax SCEV expression with a non-negative constant
    // operand: its type, the constant operand and the other.
    let is_min_max_with_non_negative_constant = |se: &ScalarEvolution, expr: Scev| -> Option<(Type, Scev, Scev)> {
        let (Kind::SMax(ops) | Kind::UMax(ops) | Kind::SMin(ops) | Kind::UMin(ops)) = &se.node(expr).kind else {
            return None;
        };
        if ops.len() != 2 {
            return None;
        }
        let c = se.as_constant(ops[0].scev)?;
        if c.is_negative() {
            return None;
        }
        Some((se.node(expr).kind.tag(), ops[0].scev, ops[1].scev))
    };

    let Some((sc_ty, min_max_lhs, min_max_rhs)) = is_min_max_with_non_negative_constant(se, min_max_expr) else {
        return min_max_expr;
    };
    let is_min = matches!(sc_ty, Type::SMin | Type::UMin);
    debug_assert!(se.is_known_non_negative(cx, min_max_lhs), "Expected non-negative operand!");
    let divisible_expr = if is_min {
        get_previous_scev_divisible_by_divisor(se, min_max_lhs, divisor)
    } else {
        get_next_scev_divisible_by_divisor(se, min_max_lhs, divisor)
    };
    let inner = apply_divisibility_on_min_max_expr(se, cx, min_max_rhs, divisor);
    se.get_min_max_expr(cx, sc_ty, vec![Use::of(inner), Use::of(divisible_expr)])
}

/// `CollectCondition` of `collectFromBlock`.
#[allow(clippy::too_many_arguments)]
fn collect_condition(
    se: &mut ScalarEvolution,
    cx: &Cx,
    guards: &mut LoopGuards,
    exprs_to_rewrite: &mut Vec<Scev>,
    div_guards: &LoopGuards,
    mut predicate: IntPredicate,
    mut lhs: Scev,
    mut rhs: Scev,
) {
    // WARNING (LLVM): It is generally unsound to apply any wrap flags to the
    // proposed replacement SCEV which isn't directly implied by the structure
    // of that SCEV. In particular, using contextual facts to imply flags is
    // *NOT* legal. See the scoping rules for flags in the header to
    // understand why.

    // Puts rewrite rule From -> To into the rewrite map. Also if From and
    // FromRewritten are the same (i.e. there has been no rewrite registered
    // for From), then puts this value in the list of rewritten expressions.
    fn add_rewrite(
        guards: &mut LoopGuards,
        exprs_to_rewrite: &mut Vec<Scev>,
        from: Scev,
        from_rewritten: Scev,
        to: Scev,
    ) {
        if from == from_rewritten {
            exprs_to_rewrite.push(from);
        }
        guards.rewrite_map.insert(from, to);
    }

    // Check for a condition of the form (-C1 + X < C2). InstCombine will
    // create this form when combining two checks of the form (X u< C2 + C1)
    // and (X >=u C1).
    if let Some((c1, lhs_unknown)) = match_c1_plus_unknown(se, lhs)
        && let Some(c2) = se.match_apint(rhs)
    {
        let exact_region = ConstantRange::make_exact_icmp_region(predicate, c2.width, c2.bits)
            .sub(&ConstantRange::single(c1.width, c1.bits));

        // Tighten the raw range with what we already know about LHSUnknown
        // from prior guards recorded in RewriteMap, or from SCEV's own range
        // analysis.
        let rewritten_lhs = guards.rewrite_map.get(&lhs_unknown).copied().unwrap_or(lhs_unknown);
        let known = se.get_unsigned_range(cx, rewritten_lhs);
        let exact_region = exact_region.intersect_with(&known, PreferredRangeType::Unsigned);

        // Bail if the guard is inconsistent with prior facts, or if the range
        // is still not a monotonic non-wrapping interval after tightening.
        if exact_region.is_empty_set() || exact_region.is_wrapped_set() || exact_region.is_full_set() {
            // The idiom did not apply: go on to the general rules.
        } else {
            let region_min = se.get_constant(Ap::new(exact_region.width, exact_region.get_unsigned_min()));
            let region_max = se.get_constant(Ap::new(exact_region.width, exact_region.get_unsigned_max()));
            let low = se.get_umin_expr(cx, vec![Use::of(rewritten_lhs), Use::of(region_max)], false);
            let clamped_lhs = se.get_umax_expr(cx, vec![Use::of(region_min), Use::of(low)]);
            add_rewrite(guards, exprs_to_rewrite, lhs_unknown, rewritten_lhs, clamped_lhs);
            return;
        }
    }

    // Do not apply information for constants or if RHS contains an AddRec.
    if se.as_constant(lhs).is_some() || contains_add_recurrence(se, rhs) {
        return;
    }

    // If RHS is SCEVUnknown, make sure the information is applied to it.
    if se.unknown_value(lhs).is_none() && se.unknown_value(rhs).is_some() {
        std::mem::swap(&mut lhs, &mut rhs);
        predicate = predicate.swapped();
    }

    let rewritten_lhs = guards.rewrite_map.get(&lhs).copied().unwrap_or(lhs);
    // Apply divisibility information when computing the constant multiple.
    let div_rewritten = se.apply_loop_guards_with(cx, rewritten_lhs, div_guards);
    let divides_by = se.get_constant_multiple(cx, div_rewritten, None);

    // Collect rewrites for LHS and its transitive operands based on the
    // condition.
    // For min/max expressions, also apply the guard to its operands:
    //  'min(a, b) >= c'   ->   '(a >= c) and (b >= c)',
    //  'min(a, b) >  c'   ->   '(a >  c) and (b >  c)',
    //  'max(a, b) <= c'   ->   '(a <= c) and (b <= c)',
    //  'max(a, b) <  c'   ->   '(a <  c) and (b <  c)'.

    // We cannot express strict predicates in SCEV, so instead we replace them
    // with non-strict ones against plus or minus one of RHS depending on the
    // predicate.
    let one = se.get_one(cx, se.type_of(rhs));
    match predicate {
        IntPredicate::Ult | IntPredicate::Slt => {
            if predicate == IntPredicate::Ult {
                if se.type_of(rhs).is_pointer() {
                    return;
                }
                rhs = se.get_umax_expr(cx, vec![Use::of(rhs), Use::of(one)]);
            }
            rhs = se.get_minus_scev(cx, Use::of(rhs), Use::of(one), 0, 0);
            rhs = get_previous_scev_divisible_by_divisor(se, rhs, divides_by);
        }
        IntPredicate::Ugt | IntPredicate::Sgt => {
            rhs = se.get_add_expr(cx, vec![Use::of(rhs), Use::of(one)], Flags::default(), 0).scev;
            rhs = get_next_scev_divisible_by_divisor(se, rhs, divides_by);
        }
        IntPredicate::Ule | IntPredicate::Sle => {
            rhs = get_previous_scev_divisible_by_divisor(se, rhs, divides_by);
        }
        IntPredicate::Uge | IntPredicate::Sge => {
            rhs = get_next_scev_divisible_by_divisor(se, rhs, divides_by);
        }
        _ => {}
    }

    let mut worklist = vec![lhs];
    let mut visited: Vec<Scev> = Vec::new();

    while let Some(from) = worklist.pop() {
        if se.as_constant(from).is_some() {
            continue;
        }
        if visited.contains(&from) {
            continue;
        }
        visited.push(from);
        let from_rewritten = guards.rewrite_map.get(&from).copied().unwrap_or(from);
        let mut to: Option<Scev> = None;

        match predicate {
            IntPredicate::Ult | IntPredicate::Ule => {
                to = Some(se.get_umin_expr(cx, vec![Use::of(from_rewritten), Use::of(rhs)], false));
                if let Kind::UMax(ops) = se.node(from_rewritten).kind.clone() {
                    worklist.extend(ops.iter().map(|op| op.scev));
                }
            }
            IntPredicate::Slt | IntPredicate::Sle => {
                to = Some(se.get_smin_expr(cx, vec![Use::of(from_rewritten), Use::of(rhs)]));
                if let Kind::SMax(ops) = se.node(from_rewritten).kind.clone() {
                    worklist.extend(ops.iter().map(|op| op.scev));
                }
            }
            IntPredicate::Ugt | IntPredicate::Uge => {
                to = Some(se.get_umax_expr(cx, vec![Use::of(from_rewritten), Use::of(rhs)]));
                if let Kind::UMin(ops) = se.node(from_rewritten).kind.clone() {
                    worklist.extend(ops.iter().map(|op| op.scev));
                }
            }
            IntPredicate::Sgt | IntPredicate::Sge => {
                to = Some(se.get_smax_expr(cx, vec![Use::of(from_rewritten), Use::of(rhs)]));
                if let Kind::SMin(ops) = se.node(from_rewritten).kind.clone() {
                    worklist.extend(ops.iter().map(|op| op.scev));
                }
            }
            IntPredicate::Eq => {
                if se.as_constant(rhs).is_some() {
                    to = Some(rhs);
                }
            }
            IntPredicate::Ne => {
                if se.match_zero(rhs) {
                    let one_aligned_up = get_next_scev_divisible_by_divisor(se, one, divides_by);
                    to = Some(se.get_umax_expr(cx, vec![Use::of(from_rewritten), Use::of(one_aligned_up)]));
                } else {
                    // LHS != RHS can be rewritten as (LHS - RHS) = UMax(1,
                    // LHS - RHS), but creating the subtraction eagerly is
                    // expensive. Track the inequalities in a separate map, and
                    // materialize the rewrite lazily when encountering a
                    // suitable subtraction while re-writing.
                    let (mut l, mut r) = (lhs, rhs);
                    if se.type_of(l).is_pointer() {
                        l = se.get_ptr_to_addr_expr(cx, l);
                        r = se.get_ptr_to_addr_expr(cx, r);
                        if l == se.get_could_not_compute() || r == se.get_could_not_compute() {
                            break;
                        }
                    }
                    if let Some((c, a)) = se
                        .match_binary(r, Type::Add, 0)
                        .and_then(|(first, second)| se.as_constant(first).map(|_| (first, second)))
                        && let Some((first, b)) = se.match_binary(l, Type::Add, 0)
                        && first == c
                    {
                        r = a;
                        l = b;
                    }
                    if l.0 > r.0 {
                        std::mem::swap(&mut l, &mut r);
                    }
                    if !guards.not_equal.contains(&(l, r)) {
                        guards.not_equal.push((l, r));
                    }
                    continue;
                }
            }
        }

        if let Some(to) = to {
            add_rewrite(guards, exprs_to_rewrite, from, from_rewritten, to);
        }
    }
}

/// `m_scev_Add(m_SCEVConstant(C1), m_SCEVUnknown(X))`.
fn match_c1_plus_unknown(
    se: &ScalarEvolution,
    s: Scev,
) -> Option<(Ap, Scev)> {
    let (first, second) = se.match_binary(s, Type::Add, 0)?;
    let c1 = se.as_constant(first)?;
    se.unknown_value(second)?;
    Some((c1, second))
}

/// `containsAddRecurrence`.
pub fn contains_add_recurrence(
    se: &ScalarEvolution,
    s: Scev,
) -> bool {
    let mut worklist = vec![s];
    let mut visited: Vec<Scev> = Vec::new();
    while let Some(next) = worklist.pop() {
        if visited.contains(&next) {
            continue;
        }
        visited.push(next);
        match &se.node(next).kind {
            Kind::AddRec(..) => return true,
            Kind::Constant(_) | Kind::Unknown(_) => {}
            Kind::UDiv(lhs, rhs) => worklist.extend([lhs.scev, rhs.scev]),
            _ => worklist.extend(se.operands_of(next).iter().map(|op| op.scev)),
        }
    }
    false
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L16296
    /// `LoopGuards::rewrite(Expr)`, as `applyLoopGuards(Expr, Guards)`.
    pub fn apply_loop_guards_with(
        &mut self,
        cx: &Cx,
        expr: Scev,
        guards: &LoopGuards,
    ) -> Scev {
        if guards.rewrite_map.is_empty() && guards.not_equal.is_empty() {
            return expr;
        }
        let mut mask = 0;
        if guards.preserve_nuw {
            mask |= flags::NUW;
        }
        if guards.preserve_nsw {
            mask |= flags::NSW;
        }
        self.guard_rewrite(cx, guards, mask, expr)
    }

    /// `SCEVLoopGuardRewriter::visit`.
    fn guard_rewrite(
        &mut self,
        cx: &Cx,
        guards: &LoopGuards,
        mask: u8,
        s: Scev,
    ) -> Scev {
        let kind = self.node(s).kind.clone();
        match kind {
            // visitAddRecExpr: skipped, because we cannot guarantee that the
            // replacement is loop invariant in the loop of the AddRec.
            Kind::AddRec(..) => s,
            Kind::Unknown(_) => guards.rewrite_map.get(&s).copied().unwrap_or(s),
            Kind::ZeroExtend(op) => {
                if let Some(&found) = guards.rewrite_map.get(&s) {
                    return found;
                }

                // If we didn't find the extact ZExt expr in the map, check if
                // there's an entry for a smaller ZExt we can use instead.
                let ty = self.type_of(s);
                let op_bits = self.get_type_size_in_bits(cx, self.type_of(op.scev));
                let mut bitwidth = self.get_type_size_in_bits(cx, ty) / 2;
                while bitwidth % 8 == 0 && bitwidth >= 8 && bitwidth > op_bits {
                    let narrow_ty = super::expr::Ty::Int(bitwidth);
                    let narrow_ext = self.get_zero_extend_expr(cx, op, narrow_ty, 0);
                    if let Some(&found) = guards.rewrite_map.get(&narrow_ext) {
                        return self.get_zero_extend_expr(cx, Use::of(found), ty, 0);
                    }
                    bitwidth /= 2;
                }

                let mut visit = |se: &mut Self, operand: Scev| se.guard_rewrite(cx, guards, mask, operand);
                self.rewrite_operands(cx, s, &mut visit)
            }
            Kind::SignExtend(_) | Kind::UMin(_) | Kind::SMin(_) => {
                if let Some(&found) = guards.rewrite_map.get(&s) {
                    return found;
                }
                let mut visit = |se: &mut Self, operand: Scev| se.guard_rewrite(cx, guards, mask, operand);
                self.rewrite_operands(cx, s, &mut visit)
            }
            Kind::Add(operands) => {
                if let Some(&found) = guards.rewrite_map.get(&s) {
                    return found;
                }

                // Helper to check if S is a subtraction (A - B) where A != B,
                // and if so, return UMax(S, 1).
                let rewrite_subtraction = |se: &mut Self, sub: Scev| -> Option<Scev> {
                    let (mut lhs, mut rhs) = se.match_binary_sub(sub)?;
                    if lhs.0 > rhs.0 {
                        std::mem::swap(&mut lhs, &mut rhs);
                    }
                    if guards.not_equal.contains(&(lhs, rhs)) {
                        let one = se.get_one(cx, se.type_of(sub));
                        let multiple = se.get_constant_multiple(cx, sub, None);
                        let one_aligned_up = get_next_scev_divisible_by_divisor(se, one, multiple);
                        return Some(se.get_umax_expr(cx, vec![Use::of(one_aligned_up), Use::of(sub)]));
                    }
                    None
                };

                // Check if Expr itself is a subtraction pattern with guard
                // info.
                if let Some(rewritten) = rewrite_subtraction(self, s) {
                    return rewritten;
                }

                // Trip count expressions sometimes consist of adding 3
                // operands, i.e. (Const + A + B). There may be guard info for
                // A + B, and if so, apply it.
                // TODO (LLVM): Could more generally apply guards to Add
                // sub-expressions.
                if self.as_constant(operands[0].scev).is_some() {
                    let node_flags = self.get_no_wrap_flags(s, flags::MASK) & mask;
                    if operands.len() == 3 {
                        let add = self.get_add_expr(cx, vec![operands[1], operands[2]], Flags::default(), 0).scev;
                        if let Some(rewritten) = rewrite_subtraction(self, add) {
                            return self
                                .get_add_expr(cx, vec![operands[0], Use::of(rewritten)], Flags::expr(node_flags), 0)
                                .scev;
                        }
                        if let Some(&found) = guards.rewrite_map.get(&add) {
                            return self.get_add_expr(cx, vec![operands[0], Use::of(found)], Flags::default(), 0).scev;
                        }
                    }

                    // For expressions of the form (Const + A), check if we have
                    // guard info for (Const + 1 + A), and rewrite to
                    // ((Const + 1 + A) - 1). This makes sure we don't lose
                    // information when rewriting expressions based on
                    // back-edge taken counts in some cases.
                    if operands.len() == 2 {
                        let found = if self.match_all_ones(operands[0].scev) {
                            // Handle (-1 + 1 + A) without constructing SCEVs.
                            guards.rewrite_map.get(&operands[1].scev).copied()
                        } else {
                            let one = self.get_one(cx, self.type_of(s));
                            let new_c =
                                self.get_add_expr(cx, vec![operands[0], Use::of(one)], Flags::default(), 0).scev;
                            let sum =
                                self.get_add_expr(cx, vec![Use::of(new_c), operands[1]], Flags::default(), 0).scev;
                            guards.rewrite_map.get(&sum).copied()
                        };
                        if let Some(found) = found {
                            let minus_one = self.get_minus_one(cx, self.type_of(s));
                            return self
                                .get_add_expr(cx, vec![Use::of(found), Use::of(minus_one)], Flags::default(), 0)
                                .scev;
                        }
                    }
                }
                let mut rewritten = Vec::new();
                let mut changed = false;
                for op in &operands {
                    let new_op = self.guard_rewrite(cx, guards, mask, op.scev);
                    changed |= op.scev != new_op;
                    rewritten.push(Use::of(new_op));
                }
                // We are only replacing operands with equivalent values, so
                // transfer the flags from the original expression.
                if !changed {
                    s
                } else {
                    let node_flags = self.get_no_wrap_flags(s, flags::MASK) & mask;
                    self.get_add_expr(cx, rewritten, Flags::expr(node_flags), 0).scev
                }
            }
            Kind::Mul(operands) => {
                let mut rewritten = Vec::new();
                let mut changed = false;
                for op in &operands {
                    let new_op = self.guard_rewrite(cx, guards, mask, op.scev);
                    changed |= op.scev != new_op;
                    rewritten.push(Use::of(new_op));
                }
                // We are only replacing operands with equivalent values, so
                // transfer the flags from the original expression.
                if !changed {
                    s
                } else {
                    let node_flags = self.get_no_wrap_flags(s, flags::MASK) & mask;
                    self.get_mul_expr(cx, rewritten, Flags::expr(node_flags), 0).scev
                }
            }
            _ => {
                let mut visit = |se: &mut Self, operand: Scev| se.guard_rewrite(cx, guards, mask, operand);
                self.rewrite_operands(cx, s, &mut visit)
            }
        }
    }
}
