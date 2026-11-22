//! `loopsplit`: a loop whose branch tests a counter against an invariant runs
//! as two loops, one on each side of the border.
//!
//! Pass: GCC's `-fsplit-loops`; the pin has no such pass (DEVIATE LSPLIT,
//! `Tti::loop_split`, off unless the target asks).
//!
//! Translation: `split_loop` and its helpers (`split_at_bb_p`,
//! `patch_loop_exit`, `connect_loops`, `connect_loop_phis`,
//! `compute_new_first_bound`) in `gcc/tree-ssa-loop-split.cc` of GCC 15, not
//! `split_loop_on_cond`. `loop_version` is the copy of the loop beside the
//! original (`loopclone`), with the guard's own branch forced one way in each.
//!
//! General solution: the first loop keeps going while the counter stays on the
//! guard's first side of the border and the original exit would also stay;
//! the second starts from the first's exit values. Both are reached from the
//! preheader test of the guard on the counter's first value.
//!
//! Not ported from `split_loop`: a guard that is not a header phi (GCC makes
//! the phi), `==` and `!=` against the first value or a range bound, a border
//! or bound computed inside the loop, `<=` and `>=` exits, a stride other than
//! one on a `!=` exit. Beyond GCC: the guard may be a compare inside a logical
//! and/or (the mid-end folds nested ifs into a `select`), and a `<` or `>` exit
//! must be entry-guarded (`beg < end`) so the new bound cannot wrap.
//!
//! # Example
//! `for (i = 2; i < n; i++) { if (i <= 31) A; B; }` becomes
//! `for (i = 2; i < min (n, 32); i++) { A; B; }` then
//! `for (; i < n; i++) { B; }`.

use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::expr::{Kind, Use, flags};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::context::{ConstantKind, Context, mask};
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Attribute, BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::types::TypeId;

use crate::edges::{self, phis};
use crate::lcssa::{arms, from_arms};
use crate::loopclone::clone_loop_with_preheader;

/// `param_max_peeled_insns`: GCC's bound on the instructions a loop copy adds.
const MAX_PEELED_INSNS: usize = 100;

/// The comparison of a guard: `counter code border`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Code {
    Lt,
    Le,
    Gt,
    Ge,
}

impl Code {
    fn predicate(
        self,
        signed: bool,
    ) -> IntPredicate {
        match (self, signed) {
            (Code::Lt, true) => IntPredicate::Slt,
            (Code::Lt, false) => IntPredicate::Ult,
            (Code::Le, true) => IntPredicate::Sle,
            (Code::Le, false) => IntPredicate::Ule,
            (Code::Gt, true) => IntPredicate::Sgt,
            (Code::Gt, false) => IntPredicate::Ugt,
            (Code::Ge, true) => IntPredicate::Sge,
            (Code::Ge, false) => IntPredicate::Uge,
        }
    }
}

/// A split `split_loop` finds, read from the function as it stands.
struct Plan {
    header: BlockId,
    preheader: BlockId,
    latch: BlockId,
    latch_is_exit: bool,
    exiting: BlockId,
    target: BlockId,
    exit_on_true: bool,
    exit_cmp: InstId,
    /// The `i1` type of a compare.
    flag: TypeId,
    /// The type of the counters.
    ty: TypeId,
    bits: u32,
    guard_block: BlockId,
    guard_branch: InstId,
    /// The compare of the counter and the border: the whole condition of the
    /// branch, or one of the conditions it is a logical and or or of.
    guard_leaf: ValueId,
    code: Code,
    signed: bool,
    initial_true: bool,
    guard_init: Operand,
    guard_next: Operand,
    border: Operand,
    /// The exit's bound, and the exit counter's first value plus `delta`: the
    /// value `beg` of `compute_new_first_bound`.
    end: Operand,
    ctl_start: Operand,
    delta: i128,
    /// Whether the exit counts up (`niter.cmp == LT_EXPR`).
    up: bool,
    add_bound: i128,
}

/// A counter's affine form in the loop.
struct Rec {
    start: llrm_analysis::scev::expr::Scev,
    step_scev: llrm_analysis::scev::expr::Scev,
    step: i128,
    flags: u8,
}

fn def(
    cx: &Cx,
    operand: Operand,
) -> Option<InstId> {
    match operand {
        Operand::Value(value) => match cx.function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        },
        _ => None,
    }
}

fn arm(
    function: &Function,
    phi: InstId,
    from: BlockId,
) -> Option<Operand> {
    arms(function, phi).into_iter().find(|&(_, block)| block == from).map(|(value, _)| value)
}

fn signed_value(
    context: &Context,
    operand: Operand,
    bits: u32,
) -> Option<i128> {
    let Operand::Constant(id) = operand else { return None };
    let raw = match context.get(id).kind {
        ConstantKind::Int(raw) => raw,
        ConstantKind::Zero => 0,
        _ => return None,
    };
    let shift = 128 - bits;
    Some(((raw << shift) as i128) >> shift)
}

fn affine(
    se: &mut ScalarEvolution,
    cx: &Cx,
    header: BlockId,
    operand: Operand,
) -> Option<Rec> {
    let scev = se.get_scev(cx, operand);
    let Kind::AddRec(ops, at) = se.node(scev).kind.clone() else { return None };
    if at != header || ops.len() != 2 {
        return None;
    }
    let step = se.as_constant(ops[1].scev)?;
    if step.is_zero() {
        return None;
    }
    Some(Rec { start: ops[0].scev, step_scev: ops[1].scev, step: step.sext_value(), flags: se.node(scev).flags })
}

/// The `i1` constant an operand is.
fn truth(
    context: &Context,
    operand: Operand,
) -> Option<bool> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(raw) => Some(raw & 1 == 1),
        ConstantKind::Zero => Some(false),
        _ => None,
    }
}

/// `inst` as a logical and (`true`) or or of the operands at the two slots:
/// `and`, `or`, `select x, y, false` and `select x, true, y` over `i1`.
fn logical(
    function: &Function,
    context: &Context,
    inst: InstId,
) -> Option<(bool, [usize; 2])> {
    let one = function.instruction(inst);
    if context.types.int_bits(one.ty) != Some(1) {
        return None;
    }
    match one.opcode {
        Opcode::Binary(BinaryOp::And) => Some((true, [0, 1])),
        Opcode::Binary(BinaryOp::Or) => Some((false, [0, 1])),
        Opcode::Select if truth(context, one.operands[2]) == Some(false) => Some((true, [0, 1])),
        Opcode::Select if truth(context, one.operands[1]) == Some(true) => Some((false, [0, 2])),
        _ => None,
    }
}

/// The compares `cond` is a logical and or or of, or `cond` where it is one.
fn logical_leaves(
    cx: &Cx,
    cond: ValueId,
) -> Vec<InstId> {
    let (mut leaves, mut work) = (Vec::new(), vec![cond]);
    let mut seen = 0;
    while let Some(value) = work.pop() {
        seen += 1;
        let Some(inst) = def(cx, Operand::Value(value)).filter(|_| seen <= 16) else { continue };
        if let Some((_, slots)) = logical(cx.function, cx.context, inst) {
            let operands = &cx.function.instruction(inst).operands;
            work.extend(
                slots.iter().filter_map(|&at| if let Operand::Value(one) = operands[at] { Some(one) } else { None }),
            );
        } else if matches!(cx.function.instruction(inst).opcode, Opcode::ICmp(_)) {
            leaves.push(inst);
        }
    }
    leaves
}

/// `cond` where `leaf` is the constant `holds`, folded: the operands of the
/// logical operations above it that change are copied before `at`.
fn forced(
    function: &mut Function,
    context: &Context,
    at: InstId,
    cond: Operand,
    leaf: ValueId,
    holds: bool,
    memo: &mut IdMap<ValueId, Operand>,
) -> Operand {
    let Operand::Value(value) = cond else { return cond };
    if value == leaf {
        return Operand::Constant(context.int(function.value(value).ty, i128::from(holds)));
    }
    if let Some(&known) = memo.get(&value) {
        return known;
    }
    let ValueDef::Instruction(inst) = function.value(value).def else { return cond };
    let Some((and, [first, second])) = logical(function, context, inst) else { return cond };
    let mut operands = function.instruction(inst).operands.clone();
    let (a, b) = (operands[first], operands[second]);
    let (a, b) =
        (forced(function, context, at, a, leaf, holds, memo), forced(function, context, at, b, leaf, holds, memo));
    let result = if a == operands[first] && b == operands[second] {
        cond
    } else {
        // And is decided by a false input, or by a true one.
        let absorbing = !and;
        match (truth(context, a), truth(context, b)) {
            (Some(x), _) if x == absorbing => a,
            (_, Some(y)) if y == absorbing => b,
            (Some(_), _) => b,
            (_, Some(_)) => a,
            _ => {
                operands[first] = a;
                operands[second] = b;
                let copy = function.clone_instruction(inst);
                function.set_operands(copy, operands);
                function.insert(copy, Position::Before(at)).expect("a placed branch");
                Operand::Value(function.instruction(copy).result.expect("a flag"))
            }
        }
    };
    memo.insert(value, result);
    result
}

/// `split_loop`'s test of the loop: one exit from a block that runs every
/// iteration, an empty latch, easy exit values and a loop that can be copied.
fn plan_for(
    se: &mut ScalarEvolution,
    cx: &Cx,
    header: BlockId,
) -> Option<Plan> {
    let f = cx.function;
    let l = cx.loops.get_loop_for(header)?;
    if l.get_header() != header || Some(header) == f.entry() || !l.is_loop_simplify_form(f) {
        return None;
    }
    let preheader = l.get_loop_preheader(f)?;
    let latch_block = l.get_loop_latch(f)?;
    let [(exiting, target)] = cx.loops.get_exit_edges(f, l)[..] else { return None };
    if cx.loops.get_loop_for(exiting)?.get_header() != header || !cx.tree.dominates(exiting, latch_block) {
        return None;
    }
    let branch = f.instruction(f.terminator(exiting)?);
    let [Operand::Value(cond), Operand::Block(on_true), Operand::Block(on_false)] = branch.operands[..] else {
        return None;
    };
    if branch.opcode != Opcode::Br || on_true == on_false {
        return None;
    }
    let exit_on_true = on_true == target;
    let stay = if exit_on_true { on_false } else { on_true };
    // The latch is empty, or is the exiting block, whose back edge gets one.
    if latch_block == exiting {
        if stay != header {
            return None;
        }
    } else {
        let mut insts = f.iter_block(latch_block);
        let jump = insts.next()?;
        if insts.next().is_some()
            || f.instruction(jump).opcode != Opcode::Br
            || f.instruction(jump).operands[..] != [Operand::Block(header)]
        {
            return None;
        }
    }
    // The exit's value on the back edge is the exit value: each header phi's
    // back-edge value is defined where the exit sees it.
    let header_phis = phis(f, header);
    for &phi in &header_phis {
        if let Some(Operand::Value(next)) = arm(f, phi, latch_block)
            && let ValueDef::Instruction(inst) = f.value(next).def
            && !cx.tree.dominates(f.parent(inst)?, exiting)
        {
            return None;
        }
    }

    // The exit compare, as the simplified shape `x stay_pred end`.
    let exit_cmp = def(cx, Operand::Value(cond))?;
    let Opcode::ICmp(pred) = f.instruction(exit_cmp).opcode else { return None };
    let [lhs, rhs] = f.instruction(exit_cmp).operands[..] else { return None };
    let (x, end, pred) = if l.is_loop_invariant(f, rhs) && !l.is_loop_invariant(f, lhs) {
        (lhs, rhs, pred)
    } else if l.is_loop_invariant(f, lhs) && !l.is_loop_invariant(f, rhs) {
        (rhs, lhs, pred.swapped())
    } else {
        return None;
    };
    let stay_pred = if exit_on_true { pred.inverse() } else { pred };
    let ty = f.operand_type(cx.context, x)?;
    let bits = cx.context.types.int_bits(ty)?;
    if bits < 2 || f.operand_type(cx.context, end)? != ty {
        return None;
    }
    let x_rec = affine(se, cx, header, x)?;
    let up = x_rec.step > 0;
    let (exit_hint, exit_ne) = match stay_pred {
        IntPredicate::Slt | IntPredicate::Ult if up => (Some(stay_pred.is_signed()), false),
        IntPredicate::Sgt | IntPredicate::Ugt if !up => (Some(stay_pred.is_signed()), false),
        IntPredicate::Ne if x_rec.step.abs() == 1 => (None, true),
        _ => return None,
    };
    // `x` is a header phi or a constant above one.
    let x_inst = def(cx, x)?;
    let (ctl_phi, offset) = match &f.instruction(x_inst).opcode {
        Opcode::Phi if f.parent(x_inst) == Some(header) => (x_inst, 0),
        Opcode::Binary(BinaryOp::Add) => {
            let [a, b] = f.instruction(x_inst).operands[..] else { return None };
            let (phi, k) = if matches!(b, Operand::Constant(_)) { (a, b) } else { (b, a) };
            let phi = def(cx, phi)?;
            if f.instruction(phi).opcode != Opcode::Phi || f.parent(phi) != Some(header) {
                return None;
            }
            (phi, signed_value(cx.context, k, bits)?)
        }
        _ => return None,
    };
    let ctl_start = arm(f, ctl_phi, preheader)?;
    let delta = offset.wrapping_sub(x_rec.step);

    // `can_copy_bbs_p`.
    if l.get_blocks().iter().any(|&b| {
        f.iter_block(b)
            .any(|i| matches!(
                f.instruction(i).opcode,
                Opcode::Invoke(_) | Opcode::LandingPad { .. } | Opcode::Resume
            ))
    }) {
        return None;
    }

    // DEVIATE LSPLIT-SIZE (owner wasm-3, exit 2026-12-17): `split_loop` has no
    // cap on the copy; its sister `split_loop_on_cond` refuses where
    // `compute_added_num_insns` passes `param_max_peeled_insns` (100,
    // params.opt), the bound GCC puts on a loop copy in this file. The loop's
    // instructions, as `eni_size_weights` counts statements: phis and jumps
    // are free. Reason: idioms -O3 split its 6-trip outer loop of inlined
    // bodies and doubled (m68k 1544 -> 3008 B, arm64 10052 -> 19184).
    let size = l
        .get_blocks()
        .iter()
        .flat_map(|&b| f.iter_block(b))
        .filter(|&i| {
            let one = f.instruction(i);
            !(one.opcode == Opcode::Phi || one.opcode == Opcode::Br && one.operands.len() == 1)
        })
        .count();
    if size > MAX_PEELED_INSNS {
        return None;
    }

    // A split point: the first guard `split_at_bb_p` takes. A guard is a
    // branch on the compare, or a `select` of values on it (the mid-end folds
    // `i < k ? a : b` into one): each loop decides it the same way.
    let mut sites: Vec<(BlockId, InstId, ValueId)> = Vec::new();
    for &block in l.get_blocks() {
        // The exiting block's branch is the exit, not a guard.
        if let Some(term) = f.terminator(block).filter(|_| block != exiting) {
            let guard = f.instruction(term);
            if let [Operand::Value(c), Operand::Block(t), Operand::Block(e)] = guard.operands[..]
                && guard.opcode == Opcode::Br
                && t != e
                && l.contains(t)
                && l.contains(e)
            {
                sites.push((block, term, c));
            }
        }
        for inst in f.iter_block(block) {
            let one = f.instruction(inst);
            if one.opcode == Opcode::Select
                && logical(f, cx.context, inst).is_none()
                && let Operand::Value(c) = one.operands[0]
            {
                sites.push((block, inst, c));
            }
        }
    }
    for (block, term, c) in sites {
        for guard_cmp in logical_leaves(cx, c) {
            let Opcode::ICmp(gpred) = f.instruction(guard_cmp).opcode else { continue };
            let [a, b] = f.instruction(guard_cmp).operands[..] else { continue };
            let is_header_phi = |operand: Operand| {
                def(cx, operand)
                    .is_some_and(|inst| f.instruction(inst).opcode == Opcode::Phi && f.parent(inst) == Some(header))
            };
            let (iv, border, gpred) = if is_header_phi(a) && l.is_loop_invariant(f, b) {
                (a, b, gpred)
            } else if is_header_phi(b) && l.is_loop_invariant(f, a) {
                (b, a, gpred.swapped())
            } else {
                continue;
            };
            let Some(phi) = def(cx, iv) else { continue };
            if f.operand_type(cx.context, iv) != Some(ty) || f.operand_type(cx.context, border) != Some(ty) {
                continue;
            }
            let Some(g_rec) = affine(se, cx, header, iv) else { continue };
            if g_rec.step != x_rec.step {
                continue;
            }
            let (Some(guard_init), Some(guard_next)) = (arm(f, phi, preheader), arm(f, phi, latch_block)) else {
                continue;
            };
            // Equality is a test of the first iteration only where the border
            // is the counter's first value.
            let (code, guard_hint) = match gpred {
                IntPredicate::Slt | IntPredicate::Ult => (Code::Lt, Some(gpred.is_signed())),
                IntPredicate::Sle | IntPredicate::Ule => (Code::Le, Some(gpred.is_signed())),
                IntPredicate::Sgt | IntPredicate::Ugt => (Code::Gt, Some(gpred.is_signed())),
                IntPredicate::Sge | IntPredicate::Uge => (Code::Ge, Some(gpred.is_signed())),
                // DEVIATE LSPLIT-EQ (owner arm64, exit 2026-11-24): a test of
                // the first iteration (`==`, `!=` against the counter's first
                // value) peels one iteration; GCC's later complete unroll
                // folds the copy, here nothing does (fpbench -O3 +612 B,
                // +161 instrs).
                IntPredicate::Eq | IntPredicate::Ne => continue,
            };
            let mut hints = [exit_hint, guard_hint].into_iter().flatten();
            let signed = match (hints.next(), hints.next()) {
                (Some(one), Some(other)) if one != other => continue,
                (Some(one), _) => one,
                (None, _) if g_rec.flags & x_rec.flags & flags::NSW != 0 => true,
                (None, _) if g_rec.flags & x_rec.flags & flags::NUW != 0 => false,
                _ => continue,
            };
            let wrap = if signed { flags::NSW } else { flags::NUW };
            if g_rec.flags & wrap == 0 || x_rec.flags & wrap == 0 {
                continue;
            }
            // A `<` or `>` exit: the loop is entered only where its first test
            // would pass, so the bound `compute_new_first_bound` makes is in
            // range.
            if !exit_ne {
                let beg = se.get_minus_scev(cx, Use::of(x_rec.start), Use::of(x_rec.step_scev), 0, 0);
                let end_scev = se.get_scev(cx, end);
                if !se.is_available_at_loop_entry(cx, beg, header)
                    || !se.is_available_at_loop_entry(cx, end_scev, header)
                {
                    continue;
                }
                let entry = match (up, signed) {
                    (true, true) => IntPredicate::Slt,
                    (true, false) => IntPredicate::Ult,
                    (false, true) => IntPredicate::Sgt,
                    (false, false) => IntPredicate::Ugt,
                };
                if !se.is_loop_entry_guarded_by_cond_pred(cx, header, entry, beg, end_scev) {
                    continue;
                }
            }
            let initial_true = match code {
                Code::Lt | Code::Le => g_rec.step > 0,
                Code::Gt | Code::Ge => g_rec.step < 0,
            };
            let add_bound = match (up, code) {
                (true, Code::Gt | Code::Le) => -1,
                (false, Code::Ge | Code::Lt) => 1,
                _ => 0,
            };
            return Some(Plan {
                header,
                preheader,
                latch: latch_block,
                latch_is_exit: latch_block == exiting,
                exiting,
                target,
                exit_on_true,
                exit_cmp,
                flag: f.instruction(exit_cmp).ty,
                ty,
                bits,
                guard_block: block,
                guard_branch: term,
                guard_leaf: f.instruction(guard_cmp).result?,
                code,
                signed,
                initial_true,
                guard_init,
                guard_next,
                border,
                end,
                ctl_start,
                delta,
                up,
                add_bound,
            });
        }
    }
    None
}

/// Instructions before `at`, folding what is constant.
struct Emit<'a> {
    context: &'a Context,
    function: &'a mut Function,
    at: InstId,
    ty: TypeId,
    bits: u32,
}

impl Emit<'_> {
    fn put(
        &mut self,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        name: &str,
    ) -> Operand {
        let inst = self.function.create_instruction(opcode, ty, operands, Flags::default(), Some(name));
        self.function.insert(inst, Position::Before(self.at)).expect("a placed terminator");
        Operand::Value(self.function.instruction(inst).result.expect("a value"))
    }

    fn constant(
        &self,
        value: i128,
    ) -> Operand {
        Operand::Constant(self.context.int(self.ty, value))
    }

    fn known(
        &self,
        operand: Operand,
    ) -> Option<u128> {
        signed_value(self.context, operand, self.bits).map(|value| value as u128 & mask(self.bits))
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
        name: &str,
    ) -> Operand {
        let (a, b) = (self.known(lhs), self.known(rhs));
        match (op, a, b) {
            (BinaryOp::Add, Some(a), Some(b)) => self.constant(a.wrapping_add(b) as i128),
            (BinaryOp::Sub, Some(a), Some(b)) => self.constant(a.wrapping_sub(b) as i128),
            (BinaryOp::Add | BinaryOp::Sub, _, Some(0)) => lhs,
            (BinaryOp::Add, Some(0), _) => rhs,
            _ => self.put(Opcode::Binary(op), self.ty, vec![lhs, rhs], name),
        }
    }
}

/// `from` branches to `header` itself, not through `bridge`, an empty block
/// only it enters.
fn merge_forwarder(
    function: &mut Function,
    from: BlockId,
    bridge: BlockId,
    header: BlockId,
) {
    let jump = function.terminator(from).expect("a branch");
    edges::retarget(function, jump, bridge, header);
    for phi in phis(function, header) {
        let incoming: Vec<_> = arms(function, phi)
            .into_iter()
            .map(|(value, source)| (value, if source == bridge { from } else { source }))
            .collect();
        function.set_operands(phi, from_arms(&incoming));
    }
    let hop = function.terminator(bridge).expect("a jump");
    function.erase(hop).expect("the forwarder's jump");
    function.erase_block(bridge).expect("an unused forwarder");
}

/// `split_loop` after its test: the loop copied, joined and bounded.
fn apply(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    plan: &Plan,
) {
    let header = plan.header;
    let latch = if plan.latch_is_exit { edges::split_edge(function, plan.exiting, header) } else { plan.latch };
    let tree = DomTree::of(function);
    let info = LoopInfo::analyze(&*function, || &tree);
    let lp = info.get_loop_for(header).expect("the loop");
    let pre = plan.preheader;
    let jump = function.terminator(pre).expect("a terminated preheader");

    // Whether the guard starts on its first side, and the new bound of the
    // first loop (`compute_new_first_bound`), before the loop.
    let (cond, newend) = {
        let mut emit = Emit { context, function, at: jump, ty: plan.ty, bits: plan.bits };
        let beg = {
            let delta = emit.constant(plan.delta);
            emit.binary(BinaryOp::Add, plan.ctl_start, delta, "split.beg")
        };
        let span = emit.binary(BinaryOp::Sub, plan.end, beg, "split.span");
        let mut bound = emit.binary(BinaryOp::Add, plan.guard_init, span, "split.bound");
        if plan.add_bound != 0 {
            let adjust = emit.constant(plan.add_bound);
            bound = emit.binary(BinaryOp::Add, bound, adjust, "split.bound");
        }
        let pick = match (plan.up, plan.signed) {
            (true, true) => IntPredicate::Slt,
            (true, false) => IntPredicate::Ult,
            (false, true) => IntPredicate::Sgt,
            (false, false) => IntPredicate::Ugt,
        };
        let smaller = emit.put(Opcode::ICmp(pick), plan.flag, vec![plan.border, bound], "split.pick");
        let newend = emit.put(Opcode::Select, plan.ty, vec![smaller, plan.border, bound], "split.end");
        let first = plan.code.predicate(plan.signed);
        let first = if plan.initial_true { first } else { first.inverse() };
        let cond = emit.put(Opcode::ICmp(first), plan.flag, vec![plan.guard_init, plan.border], "split.first");
        (cond, newend)
    };

    // `loop_version`: the loop copied beside itself.
    let ph1 = edges::split_block_at(function, pre, jump);
    let mut vmap = ValueMap::default();
    let blocks = clone_loop_with_preheader(function, ph1, lp, ".split", &mut vmap);
    for &block in &blocks {
        for inst in function.iter_block(block).collect::<Vec<_>>() {
            function.remap_instruction(inst, &vmap);
        }
    }
    let ph2 = blocks[0];
    let void = function.instruction(jump).ty;
    let old = function.terminator(pre).expect("a jump");
    let branch = function.create_instruction(
        Opcode::Br,
        void,
        vec![cond, Operand::Block(ph1), Operand::Block(ph2)],
        Flags::default(),
        None,
    );
    function.insert(branch, Position::Before(old)).expect("a placed jump");
    function.set_operands(old, Vec::new());
    function.erase(old).expect("the old jump");

    // `connect_loops`: the first loop's exit goes to the second where the
    // original exit would have stayed.
    let skip = edges::split_edge(function, plan.exiting, plan.target);
    let again = function.clone_instruction(plan.exit_cmp);
    let skip_jump = function.terminator(skip).expect("a jump");
    function.insert(again, Position::Before(skip_jump)).expect("a placed jump");
    let flag = Operand::Value(function.instruction(again).result.expect("a flag"));
    let targets = if plan.exit_on_true {
        [Operand::Block(plan.target), Operand::Block(ph2)]
    } else {
        [Operand::Block(ph2), Operand::Block(plan.target)]
    };
    let at = function.create_instruction(Opcode::Br, void, vec![flag, targets[0], targets[1]], Flags::default(), None);
    function.insert(at, Position::Before(skip_jump)).expect("a placed jump");
    function.set_operands(skip_jump, Vec::new());
    function.erase(skip_jump).expect("the old jump");

    // `connect_loop_phis`: the second loop starts from the first's values.
    let ph2_jump = function.terminator(ph2).expect("a jump");
    for phi in phis(function, header) {
        let result = function.instruction(phi).result.expect("a phi's value");
        let Some(&Operand::Value(copy)) = vmap.values.get(&result) else { unreachable!("a copied phi") };
        let ValueDef::Instruction(copy) = function.value(copy).def else { unreachable!("a phi") };
        let init = arm(function, phi, ph1).expect("the entry value");
        let next = arm(function, phi, latch).expect("the back-edge value");
        let ty = function.instruction(phi).ty;
        let merged = function.create_instruction(
            Opcode::Phi,
            ty,
            from_arms(&[(init, pre), (next, skip)]),
            Flags::default(),
            Some("split.in"),
        );
        function.insert(merged, Position::Before(ph2_jump)).expect("a placed jump");
        let merged = Operand::Value(function.instruction(merged).result.expect("a phi's value"));
        let incoming: Vec<_> = arms(function, copy)
            .into_iter()
            .map(|(value, from)| if from == ph2 { (merged, from) } else { (value, from) })
            .collect();
        function.set_operands(copy, from_arms(&incoming));
    }
    // The second loop leaves where the first did.
    let exiting2 = vmap.blocks.get(&plan.exiting).copied().expect("a copied exit");
    for phi in phis(function, plan.target) {
        let mut incoming = arms(function, phi);
        let (value, _) = incoming.iter().copied().find(|&(_, from)| from == skip).expect("the exit value");
        let value = match value {
            Operand::Value(one) => vmap.values.get(&one).copied().unwrap_or(value),
            other => other,
        };
        incoming.push((value, exiting2));
        function.set_operands(phi, from_arms(&incoming));
    }

    // `patch_loop_exit`: the first loop stays while the guard does.
    let exit_jump = function.terminator(plan.exiting).expect("a branch");
    let [_, a, b] = function.instruction(exit_jump).operands[..] else { unreachable!("a branch") };
    let stay = if a == Operand::Block(skip) { b } else { a };
    let predicate = plan.code.predicate(plan.signed);
    let stays = {
        let inst = function.create_instruction(
            Opcode::ICmp(predicate),
            plan.flag,
            vec![plan.guard_next, newend],
            Flags::default(),
            Some("split.stay"),
        );
        function.insert(inst, Position::Before(exit_jump)).expect("a placed branch");
        Operand::Value(function.instruction(inst).result.expect("a flag"))
    };
    let sides = if plan.initial_true { [stay, Operand::Block(skip)] } else { [Operand::Block(skip), stay] };
    function.set_operands(exit_jump, vec![stays, sides[0], sides[1]]);
    // GCC's forwarder latch goes again, so each loop ends in its exit test.
    if plan.latch_is_exit {
        let copy = vmap.blocks.get(&latch).copied().expect("a copied latch");
        merge_forwarder(function, plan.exiting, latch, header);
        merge_forwarder(function, exiting2, copy, vmap.blocks.get(&header).copied().expect("a copied header"));
    }

    // The guard is decided in each loop.
    let guard_copy = vmap.blocks.get(&plan.guard_block).copied().expect("a copied guard");
    let guard2 = if function.instruction(plan.guard_branch).opcode == Opcode::Select {
        let Some(&Operand::Value(copy)) =
            vmap.values.get(&function.instruction(plan.guard_branch).result.expect("a value"))
        else {
            unreachable!("a copied select")
        };
        let ValueDef::Instruction(copy) = function.value(copy).def else { unreachable!("a select") };
        copy
    } else {
        function.terminator(guard_copy).expect("a branch")
    };
    let leaf2 = match vmap.values.get(&plan.guard_leaf) {
        Some(&Operand::Value(copy)) => copy,
        _ => unreachable!("a copied compare"),
    };
    let mut dead: Vec<ValueId> = function.instruction(plan.exit_cmp).result.into_iter().collect();
    for (branch, leaf, holds) in
        [(plan.guard_branch, plan.guard_leaf, plan.initial_true), (guard2, leaf2, !plan.initial_true)]
    {
        let old = function.instruction(branch).operands[0];
        let decided = forced(function, context, branch, old, leaf, holds, &mut Default::default());
        function.set_operand(branch, 0, decided);
        if let Operand::Value(value) = old {
            dead.push(value);
        }
    }
    let mut folded = false;
    crate::simplifycfg::unreachable_removed(context, callees, function, &mut folded);
    crate::dead::dead_from(context, callees, function, dead);
}

/// `tree_ssa_split_loops`: loops from the innermost out, none whose inner loop
/// was split. Whether anything changed.
pub fn split_loops(
    context: &Context,
    layout: &llrm_mir::datalayout::DataLayout,
    function: &mut Function,
    outer: &llrm_mir::passes::Outer,
) -> bool {
    let mut changed = crate::loopsimplify::simplified(function);
    changed |= crate::lcssa::closed(context, layout, function).unwrap_or_else(|error| panic!("loopsplit: {error}"));
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    let headers: Vec<BlockId> =
        session.loops().get_loops_in_reverse_sibling_preorder().iter().rev().map(|one| one.get_header()).collect();
    let mut skip: IdSet<BlockId> = IdSet::new();
    let mut split = false;
    for header in headers {
        if skip.contains(&header) || !function.is_placed(header) || !session.loops().is_loop_header(header) {
            continue;
        }
        let plan = session.ask_within(context, layout, &outer.globals, &outer.metadata, &*function, |se, cx| {
            plan_for(se, cx, header)
        });
        let Some(plan) = plan else { continue };
        let mut up = session.loops().get_loop_for(header).and_then(|one| one.get_parent_loop());
        while let Some(parent) = up {
            skip.insert(parent.get_header());
            up = parent.get_parent_loop();
        }
        apply(context, outer.callees(), function, &plan);
        split = true;
        session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    }
    if split {
        crate::loopsimplify::simplified(function);
    }
    changed | split
}

/// `pass_loop_split` as a slot of the pipeline. `cleanup`: gcc's
/// `TODO_cleanup_cfg`, which the tests of the split itself leave off.
pub struct LoopSplit {
    pub cleanup: bool,
}

impl FunctionPass for LoopSplit {
    fn name(&self) -> &'static str {
        "loopsplit"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("gcc-split-loops")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let tti = crate::looprotate::target_tti(outer.target());
        // `optimize_loop_for_size_p`.
        let small = unit
            .function
            .attrs
            .iter()
            .any(
                |attr| matches!(
                    attr,
                    Attribute::Flag(flag) if flag == "optsize" || flag == "minsize"
                ),
            );
        if !tti.loop_split() || small {
            return PreservedAnalyses::all();
        }
        if !split_loops(unit.context, unit.layout, unit.function, &outer) {
            return PreservedAnalyses::all();
        }
        if !self.cleanup {
            return PreservedAnalyses::none();
        }
        // `pass_split_loop` returns `TODO_cleanup_cfg`: the arms of each copy are
        // folded into straight code, as the vectorizer reads only loops whose
        // blocks all run.
        let spec = crate::twoentryphi::Speculation { tti: &*tti, layout: unit.layout, outer: &outer };
        crate::simplifycfg::simplified_with(
            &crate::simplifycfg::Options { loop_form_kept: false, ..Default::default() },
            unit.context,
            &spec,
            unit.declared,
            unit.function,
        );
        // The arms are selects now. The combine makes of them what every other
        // loop reaches the vectorizer as (`sum + select(c, x, 0)`: one compare
        // per reduction), then simplify form and LCSSA are restored.
        analyses.invalidate(unit.function, &PreservedAnalyses::none());
        crate::combine_pass::Combine { enabled: true, phi_constant_users: false }.run(unit, analyses);
        analyses.invalidate(unit.function, &PreservedAnalyses::none());
        crate::loopsimplify::LoopSimplify.run(unit, analyses);
        analyses.invalidate(unit.function, &PreservedAnalyses::none());
        crate::lcssa::LoopClosedSSA.run(unit, analyses);
        PreservedAnalyses::none()
    }
}

#[cfg(test)]
#[path = "loopsplit_tests.rs"]
mod tests;
