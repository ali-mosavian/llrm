//! The users of a loop's induction variables (`IVUsers.cpp` at `d1106deb71cc`),
//! which loop strength reduction rewrites.
//!
//! # How it works
//! From each phi of the loop header, follow the users while the SCEV of the
//! value stays "interesting" (an affine add recurrence of the loop, or an add
//! with exactly one such operand). A user that is not itself reducible is an
//! `IvStrideUse`. For a user outside the loop that the latch dominates, the
//! post-incremented value is used: the expression is normalized for the loop,
//! and the use is dropped if denormalizing does not give the original back.
//!
//! # Example
//! `a[i]` in `for (i = 0; i < n; i++)`: the load's address is `{a,+,4}<loop>`,
//! one use whose user is the load and whose operand is the address.
//!
//! # Runs
//! Nothing runs it yet: stage 3 of the translation of LSR.
//!
//! # Limits
//! DEVIATE IU1: unless the target asks for use-list order
//! (`Tti::lsr_users_in_use_list_order`), users of a value are visited in
//! descending program order, not use-list order. Use-list order is edit history; the pin's order on parsed
//! IR is the only reproducible one, and it breaks ties in LSR's register
//! choice. With use-list order and `Tti::lsr_phi_users_first` a phi is met
//! first all the same: the pin keeps the one operand of a phi it meets first
//! (`Processed`), and which that is must not depend on edits (div_constants
//! m64 -O1: 4334 B -> 4173 B, 6960 B -> 6854 B). Cost (IU1-COST): one map of the layout per loop and a block's
//! instruction numbers, where the pin has none (it was a map of every
//! instruction: 1.9 M on d_faces -O2). Exit: use lists keep LLVM's order.
//! Owner: w-5. Review: 2026-11-19.
//!
//! DEVIATE: whether an instruction is safe to speculate and whether it is
//! ephemeral are asked of the caller (`Env`), which holds what they need.
//! Walks each value's users once, with a visited set: O(users).

use std::cell::RefCell;

use llrm_analysis::scev::expr::{Kind, Scev};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::dense::IdSet;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::context::ConstantKind;
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::valuetracking::pattern::phi_incoming;

#[cfg(test)]
thread_local! {
    /// How many times [`IvUsers::of`] ran on this thread.
    pub static BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// What the analysis asks of the pass that runs it.
pub struct Env<'a> {
    /// `isSafeToSpeculativelyExecute`.
    pub safe_to_speculate: &'a dyn Fn(InstId) -> bool,
    /// Membership in the loop's ephemeral values.
    pub ephemeral: &'a dyn Fn(InstId) -> bool,
    /// DEVIATE LSR-N: widths LSR takes as native besides the datalayout's `n`
    /// (`Tti::lsr_native_integer_widths`); the pin's check is
    /// [`!DL.isLegalInteger(Width)`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/IVUsers.cpp#L157).
    pub native_widths: &'a [u32],
    /// `Tti::lsr_users_in_use_list_order`: the users of a value are met in
    /// use-list order, as the pin's `for (Use &U : I->uses())`, not in
    /// descending program order (DEVIATE IU1).
    pub use_list_order: bool,
    /// DEVIATE LSR-PHI: `Tti::lsr_phi_operand_uses`.
    pub phi_operand_uses: bool,
    /// DEVIATE IU1: `Tti::lsr_phi_users_first`.
    pub phi_users_first: bool,
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/IVUsers.h#L33
/// One operand of a user that is an induction variable expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IvStrideUse {
    pub user: InstId,
    /// The value of the user's operand that is replaced.
    pub operand: Operand,
    /// The loops (by header) whose post-incremented value the user takes.
    pub post_inc_loops: Vec<BlockId>,
}

/// `IVUsers`.
#[derive(Debug)]
pub struct IvUsers {
    /// The header of the loop.
    pub l: BlockId,
    pub uses: Vec<IvStrideUse>,
    processed: IdSet<InstId>,
    /// The phis recorded as a use (DEVIATE LSR-PHI).
    phi_uses: IdSet<InstId>,
    /// How many users had their place worked out.
    ordered: usize,
    use_list_order: bool,
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/IVUsers.cpp#L52
/// `isInteresting`: whether `s`, used by `inst`, is for the loop `l`.
fn is_interesting(
    se: &mut ScalarEvolution,
    cx: &Cx,
    s: Scev,
    inst: InstId,
    l: BlockId,
) -> bool {
    let lp = cx.loops.get_loop_for(l).expect("a loop");
    match se.node(s).kind.clone() {
        // An addrec is interesting if it is affine or has an interesting start.
        Kind::AddRec(operands, header) => {
            // Loop-variant strides are left alone unless only used outside the
            // loop and they simplify there.
            if header == l {
                return se.is_affine(s) || {
                    let block = cx.function.parent(inst).expect("a placed instruction");
                    !lp.contains(block)
                        && se.get_scev_at_scope(cx, s, cx.loops.get_loop_for(block).map(|one| one.get_header())) != s
                };
            }
            // Otherwise the start is interesting and the step is not: there is
            // no effective expansion of an addrec with an interesting step.
            let step = se.get_step_recurrence(cx, s);
            is_interesting(se, cx, operands[0].scev, inst, l) && !is_interesting(se, cx, step, inst, l)
        }
        // An add is interesting if exactly one operand is.
        Kind::Add(operands) => {
            let mut any = false;
            for op in operands.iter().copied() {
                if is_interesting(se, cx, op.scev, inst, l) {
                    if any {
                        return false;
                    }
                    any = true;
                }
            }
            any
        }
        _ => false,
    }
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/IVUsers.cpp#L92
/// `IVUseShouldUsePostIncValue`: whether `user` of `operand` takes the value
/// after the increment of loop `l`. Choosing wrong breaks dominance (post-inc
/// where it cannot be) or adds a live range (pre-inc where post-inc would do).
fn should_use_post_inc_value(
    cx: &Cx,
    user: InstId,
    operand: Operand,
    l: BlockId,
) -> bool {
    let Some(lp) = cx.loops.get_loop_for(l) else { return false };
    // A user in the loop takes the pre-inc value.
    if lp.contains_instruction(cx.function, user) {
        return false;
    }
    let Some(latch) = lp.get_loop_latch(cx.function) else { return false };
    let block = cx.function.parent(user).expect("a placed instruction");
    // Outside the loop and dominated by the latch: post-inc.
    if cx.tree.dominates(latch, block) {
        return true;
    }
    // A phi lives in a block the latch may not dominate, but uses its operand
    // in the predecessor: that is where to look.
    let instruction = cx.function.instruction(user);
    if instruction.opcode != Opcode::Phi {
        return false;
    }
    phi_incoming(instruction).all(|(value, from)| value != operand || cx.tree.dominates(latch, from))
}

/// The first way the uses a pass carried through its edits differ from the
/// ones made afresh for the function as it is now, or none.
pub fn difference(
    carried: &[IvStrideUse],
    fresh: &[IvStrideUse],
) -> Option<String> {
    if let Some((at, (a, b))) = carried.iter().zip(fresh).enumerate().find(|(_, (a, b))| a != b) {
        return Some(format!("use {at}: carried {a:?}, fresh {b:?}"));
    }
    (carried.len() != fresh.len()).then(|| format!("{} uses carried, {} fresh", carried.len(), fresh.len()))
}

/// `LLRM_CHECK_IVUSERS`: a pass that carries its `IvUsers` through an edit
/// instead of making them again rebuilds them and fails on a difference.
pub fn check_enabled() -> bool {
    llrm_support::env_flag!("LLRM_CHECK_IVUSERS")
}

impl IvUsers {
    /// Under `LLRM_CHECK_IVUSERS`: panics unless `self`, carried to the
    /// function as it is now, is what [`Self::of`] makes of it.
    pub fn assert_current(
        &self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        env: &Env,
    ) {
        if !check_enabled() {
            return;
        }
        let fresh = Self::of(se, cx, self.l, env);
        if let Some(found) = difference(&self.uses, &fresh.uses) {
            panic!("IvUsers carried through an edit are stale: {found}");
        }
    }

    /// `IVUsers::IVUsers`: all the uses of the induction variables of the loop
    /// with header `l`, from the phis of its header.
    pub fn of(
        se: &mut ScalarEvolution,
        cx: &Cx,
        l: BlockId,
        env: &Env,
    ) -> Self {
        #[cfg(test)]
        BUILDS.with(|made| made.set(made.get() + 1));
        let mut users =
            Self { l, uses: Vec::new(), processed: IdSet::with_capacity(cx.function.instruction_count()), phi_uses: IdSet::with_capacity(0), ordered: 0, use_list_order: env.use_list_order };
        let phis: Vec<InstId> =
            cx.function.iter_block(l).take_while(|&inst| cx.function.instruction(inst).opcode == Opcode::Phi).collect();
        for phi in phis {
            users.add_users_if_interesting(se, cx, env, phi);
        }
        users
    }

    /// `AddUsersIfInteresting`: if `inst` is a reducible SCEV, add its users,
    /// recursively, and say so.
    fn add_users_if_interesting(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        env: &Env,
        inst: InstId,
    ) -> bool {
        // Everything seen is in the set, so a user is found once.
        if !self.processed.insert(inst) {
            return true;
        }
        let instruction = cx.function.instruction(inst);
        let Some(result) = instruction.result else { return false };
        let value = Operand::Value(result);
        // Void and floating-point values cannot be reduced.
        if !se.is_value_scevable(cx, value) {
            return false;
        }
        // LSR expands every expression: none of an operation that is unsafe to
        // speculate, a division.
        if instruction.opcode != Opcode::Phi && !(env.safe_to_speculate)(inst) {
            return false;
        }
        // No integers wider than 64 bits, nor of a width that is not native: a
        // 64-bit IV in 32-bit code for one 64-bit cast is no gain.
        let ty = cx.function.operand_type(cx.context, value).and_then(|ty| se.ty_of(cx, ty)).expect("a SCEVable type");
        let width = se.get_type_size_in_bits(cx, ty);
        if width > 64 || !(cx.layout.legal_integer(width) || env.native_widths.contains(&width)) {
            return false;
        }
        // DEVIATE LU19: a width the target declares native (and the layout does
        // not) is for induction variables of that width, not for a narrower
        // one an extension widens: `arr[zext i8 i]` stays an index (`adc
        // arr,x`), not a 16-bit pointer walk. Unless a user scales the
        // extension (`v[zext i8 j]` of 4-byte elements, `3 * zext j`): that is
        // a shift chain per trip, and the stepped value (`{v,+,4}`) is the
        // form the pin's LSR reduces it to.
        if !cx.layout.legal_integer(width)
            && matches!(instruction.opcode, Opcode::Cast(CastOp::ZExt | CastOp::SExt))
            && !scaled_by_a_user(cx, result)
        {
            return false;
        }
        // An ephemeral value is removed later anyway.
        if (env.ephemeral)(inst) {
            return false;
        }
        let mut ise = se.get_scev(cx, value);
        // An uninteresting expression ends the walk: this is a user.
        if !is_interesting(se, cx, ise, inst, self.l) {
            return false;
        }

        let l = self.l;
        let lp = cx.loops.get_loop_for(l).expect("a loop");
        let mut users: Vec<InstId> = cx.function.users(result).iter().map(|one| one.user).collect();
        if env.use_list_order {
            // One instruction using the result twice is visited once.
            let mut seen = std::collections::HashSet::new();
            users.retain(|user| seen.insert(*user));
            // DEVIATE IU1: a phi first, as in program order (stable, so the rest
            // keep the list's): the pin's "do not recurse into a phi twice" keeps the
            // operand met first, and edit history decides which that is.
            // Where the target asks (`Tti::lsr_phi_users_first`).
            if env.phi_users_first {
                users.sort_by_key(|user| cx.function.instruction(*user).opcode != Opcode::Phi);
            }
        } else if users.len() > 1 {
            self.ordered += users.len();
            // The function keeps each block's index in the layout, made once
            // for all the loops (IU1).
            let place = |user: &InstId| {
                let block = cx.function.parent(*user)?;
                Some((cx.function.layout_index(block)?, cx.function.number_in_block(*user)?))
            };
            users.sort_by_key(|user| std::cmp::Reverse(place(user)));
            // One instruction using the result twice sorts to one place.
            users.dedup();
        }
        for user in users {
            // Do not recurse into a phi twice.
            let is_phi = cx.function.instruction(user).opcode == Opcode::Phi;
            // DEVIATE LSR-PHI: a phi that is a use takes another operand as a
            // use of its own, where the pin drops it
            // (https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/IVUsers.cpp#L180).
            if is_phi
                && self.processed.contains(&user)
                && !(env.phi_operand_uses
                    && self.phi_uses.contains(&user)
                    && !self.uses.iter().any(|one| one.user == user && one.operand == value))
            {
                continue;
            }
            // Descend, but not into phis outside the loop: the whole expression
            // outside it matters for the addressing modes, though not every
            // reference there is considered.
            let block = cx.function.parent(user).expect("a placed instruction");
            let in_this_loop = cx.loops.get_loop_for(block) == Some(lp);
            let add_user = if !in_this_loop {
                is_phi || self.processed.contains(&user) || !self.add_users_if_interesting(se, cx, env, user)
            } else {
                self.processed.contains(&user) || !self.add_users_if_interesting(se, cx, env, user)
            };
            if !add_user {
                continue;
            }
            // A user that cannot be reduced further.
            // As LLVM's, `ise` stays normalized for the users after this one.
            let original = ise;
            let post_inc = RefCell::new(Vec::new());
            ise = se.normalize_for_post_inc_use_if(cx, ise, &|se, ar| {
                let Kind::AddRec(_, header) = se.node(ar).kind else { return false };
                let result = should_use_post_inc_value(cx, user, value, header);
                if result && !post_inc.borrow().contains(&header) {
                    post_inc.borrow_mut().push(header);
                }
                result
            });
            let post_inc_loops = post_inc.into_inner();
            // Normalizing simplifies under pre-increment assumptions (no
            // wrapping) that may not hold post-inc: it must be invertible, or
            // the user is discarded.
            if original != ise && se.denormalize_for_post_inc_use(cx, ise, &post_inc_loops) != original {
                return false;
            }
            if is_phi {
                self.phi_uses.insert(user);
            }
            self.uses.push(IvStrideUse { user, operand: value, post_inc_loops });
        }
        true
    }

    /// How many instructions had their place in program order worked out: the
    /// users sorted, not the function.
    pub fn ordered_instructions(&self) -> usize {
        self.ordered
    }

    /// `isIVUserOrOperand`: whether the walk reached `inst`.
    pub fn is_iv_user_or_operand(
        &self,
        inst: InstId,
    ) -> bool {
        self.processed.contains(&inst)
    }

    /// The exit test `old` was moved beside the branch, or copied there as
    /// `copy`, which is then a use of its own: whether the uses are still the
    /// ones [`Self::of`] would make. They are not where the test's operand has
    /// users the move put out of program order (IU1); then they are made again.
    pub fn carry_exit_test(
        &mut self,
        function: &Function,
        old: InstId,
        copy: Option<InstId>,
    ) -> bool {
        let Some(at) = self.uses.iter().position(|u| u.user == old) else { return true };
        if self.use_list_order && copy.is_some() {
            // A copy is a new use, first in its operands' lists, which the walk
            // would meet before the others: the uses are made again. A move
            // leaves the lists as they were.
            return false;
        }
        // A test of two induction variables is a user of both: which of them
        // finds it first depends on where it is.
        let walked = |value: &Operand| match value {
            Operand::Value(v) => {
                matches!(
                    function.value(*v).def,
                    ValueDef::Instruction(def) if self.processed.contains(&def)
                )
            }
            _ => false,
        };
        if function.instruction(old).operands.iter().filter(|op| walked(op)).count() > 1 {
            return false;
        }
        if self.use_list_order {
            return true;
        }
        let place = |user: InstId| {
            let block = function.parent(user)?;
            let index = function.layout().iter().position(|&one| one == block)?;
            Some((index, function.number_in_block(user)?))
        };
        if let Some(copy) = copy {
            // The copy is found just before the test it copies, or among the
            // descendants of a user between them: only the first is kept.
            let Operand::Value(value) = self.uses[at].operand else { return false };
            let mut users: Vec<InstId> = function.users(value).iter().map(|one| one.user).collect();
            users.sort_by_key(|&user| std::cmp::Reverse(place(user)));
            users.dedup();
            let next = users.iter().position(|&user| user == copy).and_then(|i| users.get(i + 1));
            if next != Some(&old) {
                return false;
            }
            let mut one = self.uses[at].clone();
            one.user = copy;
            self.uses.insert(at, one);
        }
        let operand = self.uses[at].operand;
        let places: Vec<_> = self.uses.iter().filter(|u| u.operand == operand).map(|u| place(u.user)).collect();
        places.windows(2).all(|pair| pair[0] >= pair[1])
    }

    /// `transformToPostInc`: the use of `cond` takes the post-incremented value
    /// of loop `l`.
    pub fn transform_to_post_inc(
        &mut self,
        cond: InstId,
        l: BlockId,
    ) {
        if let Some(u) = self.uses.iter_mut().find(|u| u.user == cond)
            && !u.post_inc_loops.contains(&l)
        {
            u.post_inc_loops.push(l);
        }
    }

    /// `getReplacementExpr`: the expression that computes the replaced operand.
    pub fn replacement_expr(
        se: &mut ScalarEvolution,
        cx: &Cx,
        u: &IvStrideUse,
    ) -> Scev {
        se.get_scev(cx, u.operand)
    }

    /// `getExpr`: the expression for the use, normalized for its post-inc
    /// loops.
    pub fn expr(
        se: &mut ScalarEvolution,
        cx: &Cx,
        u: &IvStrideUse,
    ) -> Scev {
        let replacement = Self::replacement_expr(se, cx, u);
        se.normalize_for_post_inc_use(cx, replacement, &u.post_inc_loops, false).expect("not asked to be invertible")
    }

    /// `getStride`: the step of the recurrence of loop `l` in the use's
    /// expression, if it has one.
    pub fn stride(
        se: &mut ScalarEvolution,
        cx: &Cx,
        u: &IvStrideUse,
        l: BlockId,
    ) -> Option<Scev> {
        let expr = Self::expr(se, cx, u);
        let ar = find_add_rec_for_loop(se, expr, l)?;
        Some(se.get_step_recurrence(cx, ar))
    }
}

/// `findAddRecForLoop`: the recurrence of `l` in `s`, through starts and adds.
fn find_add_rec_for_loop(
    se: &ScalarEvolution,
    s: Scev,
    l: BlockId,
) -> Option<Scev> {
    match &se.node(s).kind {
        Kind::AddRec(operands, header) => {
            if *header == l {
                Some(s)
            } else {
                find_add_rec_for_loop(se, operands[0].scev, l)
            }
        }
        Kind::Add(operands) => operands.iter().find_map(|op| find_add_rec_for_loop(se, op.scev, l)),
        _ => None,
    }
}

/// Whether a user scales `value` (an address by more than one byte, or a
/// multiply or left shift by a constant): rebuilt each trip, the scale is a
/// shift chain, and stepping the scaled value is one add.
fn scaled_by_a_user(
    cx: &Cx,
    value: llrm_mir::module::ValueId,
) -> bool {
    cx.function.users(value).iter().any(|one| {
        let user = cx.function.instruction(one.user);
        match user.opcode {
            Opcode::GetElementPtr { source } => {
                let indices = vec![None; user.operands.len() - 1];
                let (_, variable) = cx.layout.collect_offset(&cx.context.types, source, &indices);
                variable.iter().any(|&(at, scale)| scale > 1 && user.operands[1 + at] == Operand::Value(value))
            }
            Opcode::Binary(op @ (BinaryOp::Mul | BinaryOp::Shl)) => {
                let [Operand::Value(a), Operand::Constant(c)] = user.operands[..] else { return false };
                let ConstantKind::Int(by) = cx.context.get(c).kind else { return false };
                a == value && (op == BinaryOp::Shl || by > 1)
            }
            _ => false,
        }
    })
}
