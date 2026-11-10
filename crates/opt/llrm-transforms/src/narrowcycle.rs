//! A cycle of integer operations through a phi whose values only matter at a
//! narrower native width is carried at that width. Beyond the pin:
//! `TruncInstCombine` starts from a `trunc` and wants every value of its graph
//! unused outside it, and instcombine demotes a `trunc` of one operation; a
//! phi cycle read by `and` masks and compares of masked bits has no `trunc`
//! and has users outside any graph.
//!
//! DEVIATE NARROW-CYCLE (owner m68k, exit 2026-11-17): the gap is named by
//! `TruncInstCombine.cpp`'s header (graphs "post-dominated by TruncInst" whose
//! instructions "must not have users outside the graph") and by
//! [`crate::narrowiv`], which narrows a counter and not a data cycle. Reason:
//! crc16's `crc` is read through `& 0xFFFF` and `& 0x8000` and carried in a
//! long; a word `add` or `eor` is half the 68000's clocks, and a word cycle is
//! the shape `add.w %d0, %d0; bcc` wants (m68k `cmpcarry`).
//!
//! # How it works
//! For a legal width `w` (8, then 16), [`narrowed`] finds the largest set `S`
//! of `phi`, `and`, `or`, `xor`, `add`, `sub`, `shl` by a constant and
//! `select` instructions wider than `w` that a greatest fixpoint leaves with
//! every use one of:
//! - an operand of an `S` member that reads only the low `w` bits of it;
//! - an `and` with a constant below `2^w` (read, the result is zero above `w`);
//! - a `trunc` to `w` bits or fewer;
//! - anything, when the value is known zero above `w` bits (a use of the narrow
//!   value, zero-extended).
//!
//! A connected piece of `S` with a phi is made again at `w` bits when
//! `Tti::narrowing_gain` finds none of its operations dearer and the whole
//! cheaper on an axis and dearer on none, a doubling whose old top bit the
//! piece tests counting `shift_out_test_gain`, the `zext` at each outside use
//! that reads the wide value costing `extension_cost`.
//! Operands from outside are truncated (constants folded); uses outside read
//! a `zext` of the narrow value; the `and` masks become a narrow `and` and a
//! `zext`. The `nuw`/`nsw` flags are dropped: they were about the wide result.
//!
//! ```text
//!   %p = phi i32 [ %a, %e ], [ %r, %l ]      %p = phi i16 [ %a.n, %e ], [ %r.n, %l ]
//!   %b = and i32 %p, 32768                   %b = and i16 %p, -32768
//!   %s = shl i32 %p, 1                       %s = shl i16 %p, 1
//!   %r = select i1 %c, i32 %x, i32 %s        %r.n = select i1 %c, i16 %x.n, i16 %s
//! ```
//!
//! # See also
//! [`crate::narrowiv`], [`crate::trunc_inst_combine`].

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::context::{Constant, ConstantKind, Context, mask};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::tti::{Saving, Tti};
use llrm_mir::types::TypeId;

use crate::lcssa::{arms, from_arms};

/// `LLRM_NO_NARROWCYCLE`, read once; set, the step is skipped.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_NARROWCYCLE")
}

/// The cycles of `function` narrowed where the target asks
/// (`Tti::narrows_demanded_cycles`); whether anything changed. The replaced
/// instructions are erased.
pub fn narrowed(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let Some(tti) = outer.target().tti() else { return false };
    if !tti.narrows_demanded_cycles() {
        return false;
    }
    let widest = layout.largest_legal_integer();
    let mut changed = false;
    for bits in [8u32, 16] {
        if bits >= widest || !layout.legal_integer(bits) {
            continue;
        }
        while let Some(plan) = plan(context, function, &*tti, bits) {
            apply(context, function, &plan);
            changed = true;
        }
    }
    changed
}

/// What to narrow: the members, operands before their users (phis wherever
/// they fall), and the width.
struct Plan {
    members: Vec<InstId>,
    bits: u32,
}

fn int_bits(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Option<u32> {
    context.types.int_bits(function.operand_type(context, operand)?)
}

fn defining(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        },
        _ => None,
    }
}

fn constant_of(
    context: &Context,
    operand: Operand,
) -> Option<u64> {
    let Operand::Constant(id) = operand else { return None };
    match &context.get(id).kind {
        ConstantKind::Int(bits) => u64::try_from(*bits).ok(),
        ConstantKind::Zero => Some(0),
        _ => None,
    }
}

/// Whether `inst` may be a member: the operations whose low `w` bits depend
/// only on the low `w` bits of their operands.
fn member_shape(
    context: &Context,
    function: &Function,
    inst: InstId,
    bits: u32,
) -> bool {
    let instruction = function.instruction(inst);
    match context.types.int_bits(instruction.ty) {
        Some(wide) if wide > bits => {}
        _ => return false,
    }
    match instruction.opcode {
        Opcode::Phi | Opcode::Select => true,
        Opcode::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::Add | BinaryOp::Sub) => true,
        Opcode::Binary(BinaryOp::Shl) => {
            constant_of(context, instruction.operands[1]).is_some_and(|amount| amount < u64::from(bits))
        }
        _ => false,
    }
}

/// Whether operand `index` of the member `user` reads only the low bits of
/// its operand.
fn reads_low_bits(
    function: &Function,
    user: InstId,
    index: usize,
) -> bool {
    match function.instruction(user).opcode {
        Opcode::Phi => index % 2 == 0,
        Opcode::Select => index != 0,
        Opcode::Binary(BinaryOp::Shl) => index == 0,
        Opcode::Binary(_) => true,
        _ => false,
    }
}

/// Whether the bits of `operand` from `bits` up are zero, by the shapes this
/// pass makes: a cycle of phis is assumed so while it is asked.
fn zero_above(
    context: &Context,
    function: &Function,
    operand: Operand,
    bits: u32,
    asking: &mut BTreeSet<InstId>,
) -> bool {
    if let Some(value) = constant_of(context, operand) {
        return value >> bits == 0;
    }
    let Some(inst) = defining(function, operand) else { return false };
    if !asking.insert(inst) {
        return true;
    }
    let instruction = function.instruction(inst);
    let operands = instruction.operands.clone();
    let result = match instruction.opcode {
        Opcode::Cast(CastOp::ZExt) => int_bits(context, function, operands[0]).is_some_and(|from| from <= bits),
        Opcode::Binary(BinaryOp::And) => operands.iter().any(|&one| zero_above(context, function, one, bits, asking)),
        Opcode::Binary(BinaryOp::Or | BinaryOp::Xor) => {
            operands.iter().all(|&one| zero_above(context, function, one, bits, asking))
        }
        Opcode::Select => operands[1..].iter().all(|&one| zero_above(context, function, one, bits, asking)),
        Opcode::Phi => operands.chunks(2).all(|pair| zero_above(context, function, pair[0], bits, asking)),
        _ => false,
    };
    asking.remove(&inst);
    result
}

/// Whether `user`, not a member, reads the member `value` in a way that
/// survives narrowing it.
fn outside_use_ok(
    context: &Context,
    function: &Function,
    value: Operand,
    user: InstId,
    bits: u32,
) -> bool {
    let instruction = function.instruction(user);
    match instruction.opcode {
        Opcode::Binary(BinaryOp::And) => {
            let other =
                if instruction.operands[0] == value { instruction.operands[1] } else { instruction.operands[0] };
            if constant_of(context, other).is_some_and(|mask| mask >> bits == 0) {
                return true;
            }
        }
        Opcode::Cast(CastOp::Trunc) => {
            if context.types.int_bits(instruction.ty).is_some_and(|to| to <= bits) {
                return true;
            }
        }
        _ => {}
    }
    zero_above(context, function, value, bits, &mut BTreeSet::new())
}

/// The next piece to narrow at `bits`, if there is one.
fn plan(
    context: &Context,
    function: &Function,
    tti: &dyn Tti,
    bits: u32,
) -> Option<Plan> {
    let mut members: BTreeSet<InstId> =
        function.walk().map(|(_, inst)| inst).filter(|&inst| member_shape(context, function, inst, bits)).collect();
    // The greatest fixpoint: drop a member with a use that is none of the
    // reads above, until none is left to drop.
    loop {
        let mut dropped = Vec::new();
        for &inst in &members {
            let Some(result) = function.instruction(inst).result else {
                dropped.push(inst);
                continue;
            };
            let value = Operand::Value(result);
            let fine = function
                .users(result)
                .iter()
                .all(
                    |used| (members.contains(&used.user) && reads_low_bits(function, used.user, used.index as usize))
                        || outside_use_ok(context, function, value, used.user, bits),
                );
            if !fine {
                dropped.push(inst);
            }
        }
        if dropped.is_empty() {
            break;
        }
        for inst in dropped {
            members.remove(&inst);
        }
    }
    // Connected pieces of members, through operands and users.
    let mut seen: BTreeSet<InstId> = BTreeSet::new();
    for &start in &members {
        if seen.contains(&start) {
            continue;
        }
        let mut piece = BTreeSet::new();
        let mut work = vec![start];
        while let Some(inst) = work.pop() {
            if !members.contains(&inst) || !piece.insert(inst) {
                continue;
            }
            for &operand in &function.instruction(inst).operands {
                if let Some(def) = defining(function, operand) {
                    work.push(def);
                }
            }
            if let Some(result) = function.instruction(inst).result {
                work.extend(function.users(result).iter().map(|used| used.user));
            }
        }
        seen.extend(piece.iter().copied());
        let has_phi = piece.iter().any(|&inst| function.instruction(inst).opcode == Opcode::Phi);
        if has_phi && pays(context, function, tti, &piece, bits) {
            return Some(Plan { members: ordered(function, &piece), bits });
        }
    }
    None
}

/// None of the piece's operations dearer on an axis, and the whole cheaper on
/// one and dearer on none: its operations narrowed, a doubling whose old top
/// bit it tests counted, less the extension at each use outside that reads the
/// wide value. A per-iteration saving must pay for the extensions, which run
/// once.
fn pays(
    context: &Context,
    function: &Function,
    tti: &dyn Tti,
    piece: &BTreeSet<InstId>,
    bits: u32,
) -> bool {
    let mut total = Saving::default();
    let mut wide_bits = bits;
    for &inst in piece {
        let instruction = function.instruction(inst);
        let Some(wide) = context.types.int_bits(instruction.ty) else { return false };
        wide_bits = wide_bits.max(wide);
        let Opcode::Binary(op) = instruction.opcode else { continue };
        let constant = constant_of(context, instruction.operands[1]);
        // An `and` with the narrow mask is a zero extension: at the narrow
        // width it is gone, and the `zext` at its users is counted below.
        let kept = if op == BinaryOp::And && constant == Some(mask(bits) as u64) { 0 } else { bits };
        let gain = tti.narrowing_gain(op, wide, kept, constant);
        if gain.clocks < 0 || gain.bytes < 0 {
            return false;
        }
        total = total + gain;
    }
    // A doubling whose old top bit the piece tests: the carry reads it.
    let top = 1u64 << (bits - 1);
    for &inst in piece {
        let instruction = function.instruction(inst);
        if instruction.opcode != Opcode::Binary(BinaryOp::Shl)
            || constant_of(context, instruction.operands[1]) != Some(1)
        {
            continue;
        }
        let Operand::Value(source) = instruction.operands[0] else { continue };
        let tested = function
            .users(source)
            .iter()
            .any(
                |used| {
                    let user = function.instruction(used.user);
                    user.opcode == Opcode::Binary(BinaryOp::And)
                        && constant_of(context, user.operands[1 - used.index as usize]) == Some(top)
                },
            );
        if tested {
            total = total + tti.shift_out_test_gain(bits);
        }
    }
    let extension = tti.extension_cost(bits, wide_bits);
    for &inst in piece {
        let Some(result) = function.instruction(inst).result else { continue };
        for used in function.users(result) {
            if !piece.contains(&used.user) && !masks_or_cuts(context, function, used.user, result, bits) {
                total = total - extension;
            }
        }
    }
    total.clocks >= 0 && total.bytes >= 0 && (total.clocks > 0 || total.bytes > 0)
}

/// Whether `user` reads `value` through a constant mask below `2^bits`, a
/// `trunc` to `bits` or fewer or a compare with a constant that fits: no
/// extension is left at it.
fn masks_or_cuts(
    context: &Context,
    function: &Function,
    user: InstId,
    value: llrm_mir::module::ValueId,
    bits: u32,
) -> bool {
    let instruction = function.instruction(user);
    match instruction.opcode {
        Opcode::Binary(BinaryOp::And) => instruction.operands.iter().any(|&operand| {
            operand != Operand::Value(value) && constant_of(context, operand).is_some_and(|mask| mask >> bits == 0)
        }),
        Opcode::Cast(CastOp::Trunc) => context.types.int_bits(instruction.ty).is_some_and(|to| to <= bits),
        // A compare with a constant that fits is made at the narrow width.
        Opcode::ICmp(
            IntPredicate::Eq
            | IntPredicate::Ne
            | IntPredicate::Ugt
            | IntPredicate::Uge
            | IntPredicate::Ult
            | IntPredicate::Ule,
        ) => instruction.operands.iter().any(|&operand| {
            operand != Operand::Value(value) && constant_of(context, operand).is_some_and(|c| c >> bits == 0)
        }),
        _ => false,
    }
}

/// The piece with operands before their users, the phis (which close the
/// cycles) wherever they fall.
fn ordered(
    function: &Function,
    piece: &BTreeSet<InstId>,
) -> Vec<InstId> {
    let mut order = Vec::new();
    let mut done = BTreeSet::new();
    for &inst in piece {
        visit(function, piece, inst, &mut done, &mut order);
    }
    order
}

fn visit(
    function: &Function,
    piece: &BTreeSet<InstId>,
    inst: InstId,
    done: &mut BTreeSet<InstId>,
    order: &mut Vec<InstId>,
) {
    if !done.insert(inst) {
        return;
    }
    if function.instruction(inst).opcode != Opcode::Phi {
        for &operand in &function.instruction(inst).operands {
            if let Some(def) = defining(function, operand).filter(|def| piece.contains(def)) {
                visit(function, piece, def, done, order);
            }
        }
    }
    order.push(inst);
}

struct Builder<'a> {
    context: &'a mut Context,
    function: &'a mut Function,
    narrow: TypeId,
    bits: u32,
    made: BTreeMap<InstId, Operand>,
}

impl Builder<'_> {
    fn emit(
        &mut self,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        at: InstId,
    ) -> Operand {
        let new = self.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        self.function.insert(new, Position::Before(at)).expect("a placed instruction");
        Operand::Value(self.function.instruction(new).result.expect("a value"))
    }

    /// The narrow form of `operand` for a use before `at`: a made member, a
    /// constant folded, or a `trunc`.
    fn reduced(
        &mut self,
        operand: Operand,
        at: InstId,
    ) -> Operand {
        if let Operand::Constant(id) = operand {
            let kind = match self.context.get(id).kind.clone() {
                ConstantKind::Int(value) => ConstantKind::Int(value & mask(self.bits)),
                ConstantKind::Undef => ConstantKind::Undef,
                ConstantKind::Poison => ConstantKind::Poison,
                _ => ConstantKind::Zero,
            };
            return Operand::Constant(self.context.constant(Constant { ty: self.narrow, kind }));
        }
        if let Some(def) = defining(self.function, operand)
            && let Some(&made) = self.made.get(&def)
        {
            return made;
        }
        if self.function.operand_type(self.context, operand) == Some(self.narrow) {
            return operand;
        }
        self.emit(Opcode::Cast(CastOp::Trunc), self.narrow, vec![operand], at)
    }
}

/// Makes the plan's members again at `plan.bits` and erases the old ones.
fn apply(
    context: &mut Context,
    function: &mut Function,
    plan: &Plan,
) {
    let narrow = context.types.int(plan.bits);
    let mut build = Builder { context, function, narrow, bits: plan.bits, made: BTreeMap::new() };
    let members: BTreeSet<InstId> = plan.members.iter().copied().collect();
    // The phis first, empty, so an operand before its phi in the order finds
    // one.
    let mut phis = Vec::new();
    for &inst in &plan.members {
        if build.function.instruction(inst).opcode == Opcode::Phi {
            let new = build.emit(Opcode::Phi, narrow, Vec::new(), inst);
            build.made.insert(inst, new);
            phis.push((inst, new));
        }
    }
    for &inst in &plan.members {
        let instruction = build.function.instruction(inst).clone();
        let new = match instruction.opcode {
            Opcode::Phi => continue,
            Opcode::Binary(op) => {
                let lhs = build.reduced(instruction.operands[0], inst);
                let rhs = build.reduced(instruction.operands[1], inst);
                build.emit(Opcode::Binary(op), narrow, vec![lhs, rhs], inst)
            }
            Opcode::Select => {
                let lhs = build.reduced(instruction.operands[1], inst);
                let rhs = build.reduced(instruction.operands[2], inst);
                build.emit(Opcode::Select, narrow, vec![instruction.operands[0], lhs, rhs], inst)
            }
            other => unreachable!("not a member: {other:?}"),
        };
        build.made.insert(inst, new);
    }
    for &(old, new) in &phis {
        let Some(new) = defining(build.function, new) else { continue };
        let mut incoming = Vec::new();
        for (value, block) in arms(build.function, old) {
            let at = build.function.terminator(block).expect("a terminated block");
            incoming.push((build.reduced(value, at), block));
        }
        build.function.set_operands(new, from_arms(&incoming));
    }
    // The uses outside the members.
    let mut consumed: BTreeSet<InstId> = BTreeSet::new();
    for &inst in &plan.members {
        let result = build.function.instruction(inst).result.expect("a member has a value");
        let wide = build.function.instruction(inst).ty;
        let new = build.made[&inst];
        let uses: Vec<_> = build.function.users(result).iter().copied().collect();
        for used in uses {
            if members.contains(&used.user) || consumed.contains(&used.user) {
                continue;
            }
            let user = build.function.instruction(used.user).clone();
            match user.opcode {
                Opcode::Binary(BinaryOp::And) => {
                    let other = user.operands[1 - used.index as usize];
                    if constant_of(build.context, other).is_some_and(|mask| mask >> plan.bits == 0) {
                        let masked = build.reduced(other, used.user);
                        let (lhs, rhs) = if used.index == 0 { (new, masked) } else { (masked, new) };
                        let narrow_and = build.emit(Opcode::Binary(BinaryOp::And), narrow, vec![lhs, rhs], used.user);
                        let widened = build.emit(Opcode::Cast(CastOp::ZExt), user.ty, vec![narrow_and], used.user);
                        build.function.replace_all_uses_with(user.result.expect("an and has a value"), widened);
                        consumed.insert(used.user);
                        continue;
                    }
                }
                Opcode::Cast(CastOp::Trunc) => {
                    let to = build.context.types.int_bits(user.ty).unwrap_or(plan.bits);
                    if to <= plan.bits {
                        let cut = if to == plan.bits {
                            new
                        } else {
                            build.emit(Opcode::Cast(CastOp::Trunc), user.ty, vec![new], used.user)
                        };
                        build.function.replace_all_uses_with(user.result.expect("a trunc has a value"), cut);
                        consumed.insert(used.user);
                        continue;
                    }
                }
                _ => {}
            }
            // Anything else read the zero-extended value.
            let at = if user.opcode == Opcode::Phi {
                match user.operands[used.index as usize + 1] {
                    Operand::Block(block) => build.function.terminator(block).expect("a terminated block"),
                    _ => unreachable!("a phi's block"),
                }
            } else {
                used.user
            };
            let widened = build.emit(Opcode::Cast(CastOp::ZExt), wide, vec![new], at);
            let mut operands = user.operands.clone();
            operands[used.index as usize] = widened;
            build.function.set_operands(used.user, operands);
        }
    }
    for inst in consumed {
        build.function.erase(inst).expect("its uses were replaced");
    }
    // The old members: the phis first, so the cycle's uses are poisoned.
    let old_phis: Vec<InstId> = phis.iter().map(|&(old, _)| old).collect();
    for inst in old_phis.iter().copied().chain(plan.members.iter().rev().copied().filter(|m| !old_phis.contains(m))) {
        let result = build.function.instruction(inst).result.expect("a value");
        let ty = build.function.instruction(inst).ty;
        let poison = Operand::Constant(build.context.constant(Constant { ty, kind: ConstantKind::Poison }));
        build.function.replace_all_uses_with(result, poison);
        build.function.erase(inst).expect("its uses were replaced");
    }
}
