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
//! DEVIATE: whether an instruction is safe to speculate and whether it is
//! ephemeral are asked of the caller (`Env`), which holds what they need.
//! Walks each value's users once, with a visited set: O(users).

use llrm_analysis::scev::expr::{Kind, Scev};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::pattern::phi_incoming;
use llrm_support::hash::HashSet;
use std::cell::RefCell;

/// What the analysis asks of the pass that runs it.
pub struct Env<'a> {
    /// `isSafeToSpeculativelyExecute`.
    pub safe_to_speculate: &'a dyn Fn(InstId) -> bool,
    /// Membership in the loop's ephemeral values.
    pub ephemeral: &'a dyn Fn(InstId) -> bool,
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
    processed: HashSet<InstId>,
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
                    !lp.contains(block) && se.get_scev_at_scope(cx, s, cx.loops.get_loop_for(block).map(|one| one.get_header())) != s
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
            for op in operands {
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

impl IvUsers {
    /// `IVUsers::IVUsers`: all the uses of the induction variables of the loop
    /// with header `l`, from the phis of its header.
    pub fn of(
        se: &mut ScalarEvolution,
        cx: &Cx,
        l: BlockId,
        env: &Env,
    ) -> Self {
        let mut users = Self { l, uses: Vec::new(), processed: HashSet::default() };
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
        if width > 64 || !cx.layout.legal_integer(width) {
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
        let mut unique = HashSet::default();
        let users: Vec<InstId> = cx.function.users(result).iter().map(|one| one.user).collect();
        for user in users {
            if !unique.insert(user) {
                continue;
            }
            // Do not recurse into a phi twice.
            let is_phi = cx.function.instruction(user).opcode == Opcode::Phi;
            if is_phi && self.processed.contains(&user) {
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
            self.uses.push(IvStrideUse { user, operand: value, post_inc_loops });
        }
        true
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

    /// `getExpr`: the expression for the use, normalized for its post-inc loops.
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
