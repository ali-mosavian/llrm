//! A float-to-integer conversion, or a float compare, that only one arm of a
//! select with a constant arm needs, made on that arm's path only.
//!
//! # How it works
//! `%r = fptosi (select c, K, x)`, where the select has no other user, is
//! `br c, join, other` with `%f = fptosi x` in `other` and `%r = phi [K', b],
//! [%f, other]` in `join`, `K'` the conversion of `K` made here. The path that
//! takes the constant converts nothing. Applied only where the conversion costs
//! more than twice a branch (`float_to_int` against `branch` in the target's
//! costs): on x87 `fistp` with the control word loaded around it is 43 clocks
//! against 3, so the half of the iterations that take `K` save about 21; where
//! one `cvttsd2si` converts, the select stays.
//!
//! ```text
//!  %s = select c, -3.0, %x            br c, %join, %other
//!  %r = fptosi %s                 other: %f = fptosi %x ; br %join
//!                                 join:  %r = phi [-3, %b], [%f, %other]
//! ```
//!
//! The same for a compare: `select c1, K, (zext (fcmp ..))` is `br c1, join,
//! other` with the compare and its extension in `other` (`a < b ? -1 : a > b`:
//! the second `fcom; fnstsw; sahf` runs on the path that needs it, 9 clocks at
//! 486, against the branch). Applied where `float_compare` is more than twice
//! a branch.
//!
//! A clamp is a select of a select; the diamond made for the outer one holds
//! `fptosi (select ..)` again and is taken in the next round.
//!
//! DEVIATE CS1 (exit 2026-11-20: the pin's
//! `SelectOptimize`/`optimizeSelectInst` priced by `getCastInstrCost`): the pin
//! keeps one conversion after the select (`FoldOpIntoSelect` is skipped when
//! the compare has the select's type) and its x86 has no price that makes a
//! conversion dear. gcc converts the constant at compile time (x_ldcmp -O2:
//! 2020 `fistp` against 1180).
//!
//! The condition is branched on as it is (no `freeze`): a poison condition
//! made a poison select, and now a branch on poison, which the pin's
//! `optimizeSelectInst` guards with `freeze`.

use llrm_mir::constantfolding::constant_fold_cast_operand;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};

use crate::edges::split_block_at;

/// Every conversion of a select that costs more than twice a branch, made on
/// its non-constant arm's path. Whether any changed.
pub fn casts_branched(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    conversion: i64,
    compare: i64,
    branch: i64,
) -> bool {
    let (convert, compared) = (conversion > 2 * branch, compare > 2 * branch);
    if !convert && !compared {
        return false;
    }
    let mut changed = false;
    while let Some(site) = find(context, function, convert, compared) {
        match site {
            Site::Convert { .. } => branched(context, layout, function, site),
            Site::Arm { .. } => arm_branched(function, site),
            Site::Compare { .. } => compare_branched(function, site),
        }
        changed = true;
    }
    changed
}

enum Site {
    /// `select c, K, %v` of integers where `%v` is a conversion nothing else
    /// reads: the conversion goes to the path that does not take `K`.
    Arm { cast: InstId, select: InstId, constant_is_true: bool },
    Convert {
        cast: InstId,
        select: InstId,
        /// The arm that is a constant: the true one or the false one.
        constant_is_true: bool,
    },
    Compare {
        select: InstId,
        constant_is_true: bool,
        /// The compare and the extension of it only the select reads, in order.
        chain: Vec<InstId>,
    },
}

fn find(
    context: &Context,
    function: &Function,
    convert: bool,
    compared: bool,
) -> Option<Site> {
    // The outer select of a clamp first: its conversion moves to its own path
    // with the inner select, and the next round branches the inner one.
    if convert {
        for &block in function.layout() {
            for select in function.iter_block(block) {
                if let Some(site) = arm_site(context, function, block, select) {
                    return Some(site);
                }
            }
        }
    }
    for &block in function.layout() {
        for cast in function.iter_block(block) {
            let instruction = function.instruction(cast);
            if compared
                && instruction.opcode == Opcode::Select
                && let Some(site) = compare_site(context, function, block, cast)
            {
                return Some(site);
            }
            if !convert || !matches!(
                instruction.opcode,
                Opcode::Cast(CastOp::FPToSI | CastOp::FPToUI)
            ) {
                continue;
            }
            let [Operand::Value(value)] = instruction.operands[..] else { continue };
            let ValueDef::Instruction(select) = function.value(value).def else { continue };
            if function.instruction(select).opcode != Opcode::Select
                || function.parent(select) != Some(block)
                || !function.users(value).has_one_use()
            {
                continue;
            }
            let [Operand::Value(condition), yes, no] = function.instruction(select).operands[..] else { continue };
            if context.types.int_bits(function.value(condition).ty) != Some(1) {
                continue;
            }
            let float_constant = |arm: Operand| {
                matches!(
                    arm,
                    Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Float(_) | ConstantKind::Float80(_))
                )
            };
            // Two constant arms are a select of two integers: no branch.
            match (float_constant(yes), float_constant(no)) {
                (true, false) => return Some(Site::Convert { cast, select, constant_is_true: true }),
                (false, true) => return Some(Site::Convert { cast, select, constant_is_true: false }),
                _ => {}
            }
        }
    }
    None
}

/// `select c, K, %v` of integers (either way round) where `%v` is a
/// float-to-integer conversion in the select's block that nothing else reads.
fn arm_site(
    context: &Context,
    function: &Function,
    block: BlockId,
    select: InstId,
) -> Option<Site> {
    if function.instruction(select).opcode != Opcode::Select {
        return None;
    }
    let [Operand::Value(condition), yes, no] = function.instruction(select).operands[..] else { return None };
    if context.types.int_bits(function.value(condition).ty) != Some(1) {
        return None;
    }
    let constant = |arm: Operand| {
        matches!(
            arm,
            Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(_))
        )
    };
    let (constant_is_true, other) = match (constant(yes), constant(no)) {
        (true, false) => (true, no),
        (false, true) => (false, yes),
        _ => return None,
    };
    let Operand::Value(value) = other else { return None };
    let ValueDef::Instruction(cast) = function.value(value).def else { return None };
    (matches!(
        function.instruction(cast).opcode,
        Opcode::Cast(CastOp::FPToSI | CastOp::FPToUI)
    )
        && function.parent(cast) == Some(block)
        && function.users(value).has_one_use())
    .then_some(Site::Arm { cast, select, constant_is_true })
}

/// The conversion, with the select and compare only it reads, moves to a block
/// taken when the constant is not; the select becomes a phi of the constant
/// and the conversion.
fn arm_branched(
    function: &mut Function,
    site: Site,
) {
    let Site::Arm { cast, select, constant_is_true } = site else { unreachable!("an arm site") };
    let [condition, yes, no] = function.instruction(select).operands[..] else { unreachable!("a select") };
    let (constant, other) = if constant_is_true { (yes, no) } else { (no, yes) };
    let ty = function.instruction(select).ty;
    let result = function.instruction(select).result.expect("a select's value");
    let home = function.parent(select).expect("a placed select");
    // What only the conversion reads, deepest first: the select that is its
    // operand, and that select's compare.
    let mut moved = Vec::new();
    if let Operand::Value(value) = function.instruction(cast).operands[0]
        && let ValueDef::Instruction(inner) = function.value(value).def
        && function.instruction(inner).opcode == Opcode::Select
        && function.parent(inner) == Some(home)
        && function.users(value).has_one_use()
    {
        if let Operand::Value(test) = function.instruction(inner).operands[0]
            && let ValueDef::Instruction(compare) = function.value(test).def
            && matches!(function.instruction(compare).opcode, Opcode::FCmp(_))
            && function.parent(compare) == Some(home)
            && function.users(test).has_one_use()
        {
            moved.push(compare);
        }
        moved.push(inner);
    }
    moved.push(cast);
    let join = split_block_at(function, home, select);
    let jump = function.terminator(home).expect("the jump the split made");
    let void = function.instruction(jump).ty;
    function.erase(jump).expect("nothing reads a jump");
    let side = function.create_block(None);
    function.insert_block(side, Some(home)).expect("a new block");
    for &inst in &moved {
        function.move_to(inst, Position::End(side)).expect("a placed instruction");
    }
    let made = function.instruction(cast).result.expect("a conversion's value");
    let back = function.create_instruction(Opcode::Br, void, vec![Operand::Block(join)], Flags::default(), None);
    function.insert(back, Position::End(side)).expect("an unplaced instruction");
    let (on_true, on_false) = if constant_is_true { (join, side) } else { (side, join) };
    let branch = function.create_instruction(
        Opcode::Br,
        void,
        vec![condition, Operand::Block(on_true), Operand::Block(on_false)],
        Flags::default(),
        None,
    );
    function.insert(branch, Position::End(home)).expect("an unplaced instruction");
    let phi = function.create_instruction(
        Opcode::Phi,
        ty,
        vec![constant, Operand::Block(home), Operand::Value(made), Operand::Block(side)],
        Flags::default(),
        None,
    );
    function.insert(phi, Position::Before(select)).expect("an unplaced instruction");
    let value = function.instruction(phi).result.expect("a phi's value");
    function.replace_all_uses_with(result, Operand::Value(value));
    function.erase(select).expect("nothing reads it now");
    let _ = other;
}

/// `select c, K, V` (either way round) where `V` is a float compare, or the
/// extension of one, that nothing else reads and that sits in the select's
/// block.
fn compare_site(
    context: &Context,
    function: &Function,
    block: llrm_mir::module::BlockId,
    select: InstId,
) -> Option<Site> {
    let [Operand::Value(condition), yes, no] = function.instruction(select).operands[..] else { return None };
    if context.types.int_bits(function.value(condition).ty) != Some(1) {
        return None;
    }
    let constant = |arm: Operand| {
        matches!(
            arm,
            Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(_))
        )
    };
    let (constant_is_true, other) = match (constant(yes), constant(no)) {
        (true, false) => (true, no),
        (false, true) => (false, yes),
        _ => return None,
    };
    let Operand::Value(mut value) = other else { return None };
    let mut chain = Vec::new();
    loop {
        let ValueDef::Instruction(inst) = function.value(value).def else { return None };
        if function.parent(inst) != Some(block) || !function.users(value).has_one_use() {
            return None;
        }
        chain.push(inst);
        match function.instruction(inst).opcode {
            Opcode::FCmp(_) => break,
            Opcode::Cast(CastOp::ZExt | CastOp::SExt) => {
                let [Operand::Value(inner)] = function.instruction(inst).operands[..] else { return None };
                value = inner;
            }
            _ => return None,
        }
    }
    chain.reverse();
    Some(Site::Compare { select, constant_is_true, chain })
}

/// The compare chain moves to a block of its own, taken when the constant is
/// not; the select becomes a phi of the constant and the chain's value.
fn compare_branched(
    function: &mut Function,
    site: Site,
) {
    let Site::Compare { select, constant_is_true, chain } = site else { unreachable!("a compare site") };
    let [condition, yes, no] = function.instruction(select).operands[..] else { unreachable!("a select") };
    let (constant, other) = if constant_is_true { (yes, no) } else { (no, yes) };
    let ty = function.instruction(select).ty;
    let result = function.instruction(select).result.expect("a select's value");
    let home = function.parent(select).expect("a placed select");
    let join = split_block_at(function, home, select);
    let jump = function.terminator(home).expect("the jump the split made");
    let void = function.instruction(jump).ty;
    function.erase(jump).expect("nothing reads a jump");
    let side = function.create_block(None);
    function.insert_block(side, Some(home)).expect("a new block");
    for &inst in &chain {
        function.move_to(inst, Position::End(side)).expect("a placed instruction");
    }
    let back = function.create_instruction(Opcode::Br, void, vec![Operand::Block(join)], Flags::default(), None);
    function.insert(back, Position::End(side)).expect("an unplaced instruction");
    let (on_true, on_false) = if constant_is_true { (join, side) } else { (side, join) };
    let branch = function.create_instruction(
        Opcode::Br,
        void,
        vec![condition, Operand::Block(on_true), Operand::Block(on_false)],
        Flags::default(),
        None,
    );
    function.insert(branch, Position::End(home)).expect("an unplaced instruction");
    let phi = function.create_instruction(
        Opcode::Phi,
        ty,
        vec![constant, Operand::Block(home), other, Operand::Block(side)],
        Flags::default(),
        None,
    );
    function.insert(phi, Position::Before(select)).expect("an unplaced instruction");
    let value = function.instruction(phi).result.expect("a phi's value");
    function.replace_all_uses_with(result, Operand::Value(value));
    function.erase(select).expect("nothing reads it now");
}

fn branched(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    site: Site,
) {
    let Site::Convert { cast, select, constant_is_true } = site else { unreachable!("a conversion site") };
    let Opcode::Cast(op) = function.instruction(cast).opcode else { unreachable!("a cast") };
    let ty = function.instruction(cast).ty;
    let result = function.instruction(cast).result.expect("a conversion's value");
    let [condition, yes, no] = function.instruction(select).operands[..] else { unreachable!("a select") };
    let (constant, other) = if constant_is_true { (yes, no) } else { (no, yes) };
    let Operand::Constant(constant) = constant else { unreachable!("a constant arm") };
    let Some(converted) = constant_fold_cast_operand(context, layout, op, constant, ty) else {
        return;
    };
    if !matches!(context.get(converted).kind, ConstantKind::Int(_)) {
        return;
    }
    let home = function.parent(cast).expect("a placed conversion");
    let join = split_block_at(function, home, cast);
    let jump = function.terminator(home).expect("the jump the split made");
    let void = function.instruction(jump).ty;
    function.erase(jump).expect("nothing reads a jump");
    let side = function.create_block(None);
    function.insert_block(side, Some(home)).expect("a new block");
    let moved = function.create_instruction(Opcode::Cast(op), ty, vec![other], Flags::default(), None);
    function.insert(moved, Position::End(side)).expect("an unplaced instruction");
    let back = function.create_instruction(Opcode::Br, void, vec![Operand::Block(join)], Flags::default(), None);
    function.insert(back, Position::End(side)).expect("an unplaced instruction");
    let (on_true, on_false) = if constant_is_true { (join, side) } else { (side, join) };
    let branch = function.create_instruction(
        Opcode::Br,
        void,
        vec![condition, Operand::Block(on_true), Operand::Block(on_false)],
        Flags::default(),
        None,
    );
    function.insert(branch, Position::End(home)).expect("an unplaced instruction");
    let made = function.instruction(moved).result.expect("a conversion's value");
    let phi = function.create_instruction(
        Opcode::Phi,
        ty,
        vec![Operand::Constant(converted), Operand::Block(home), Operand::Value(made), Operand::Block(side)],
        Flags::default(),
        None,
    );
    function.insert(phi, Position::Before(cast)).expect("an unplaced instruction");
    let value = function.instruction(phi).result.expect("a phi's value");
    function.replace_all_uses_with(result, Operand::Value(value));
    function.erase(cast).expect("nothing reads it now");
    function.erase(select).expect("nothing reads it now");
    // A select only the conversion reads moves with it: the next round finds
    // the pair in one block.
    if let Operand::Value(value) = other
        && let ValueDef::Instruction(inner) = function.value(value).def
        && function.instruction(inner).opcode == Opcode::Select
        && function.parent(inner) == Some(home)
        && function.users(value).has_one_use()
    {
        function.move_to(inner, Position::Before(moved)).expect("a placed instruction");
    }
}

/// [`select_arms_sunk`] as a pass, after `cgpaddr`, at the speed rate (-O2 and
/// -O3) and, where the target states `sinks_select_arms_for_size`, in a size
/// build (-Os); not at -O1, where the branch form is up to 3 B longer on some
/// targets. The target states `sinks_select_arms` and has no conditional move.
pub struct SelectArms {
    /// A size build (-Os): the target must state `sinks_select_arms_for_size`.
    pub size: bool,
}

impl FunctionPass for SelectArms {
    fn name(&self) -> &'static str {
        "cgpselectarms"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let size = self.size;
        let wanted = analyses
            .outer()
            .target()
            .tti()
            .is_some_and(
                |tti| !tti.has_cmov() && tti.sinks_select_arms() && (!size || tti.sinks_select_arms_for_size()),
            );
        let fused = |bits: u32, signed: bool| {
            analyses.outer().target().tti().is_some_and(|tti| tti.has_div_rem_op(bits, signed))
        };
        if wanted && select_arms_sunk(unit.context, unit.function, &fused) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// DEVIATE CS2 (owner x86-m32; exit 2026-12-10: the machine-level
/// `EmitLoweredSelect` sinks the operand, or the target states a price): where
/// the target has no conditional move, a select is a branch around a copy
/// whatever is done, and an arm only the select reads (`x - 65521` of
/// `x > 65520 ? x - 65521 : x`) is made on its own path instead of before the
/// branch. The pin computes both operands first; that costs a register copy
/// (`mov dst, x; sub dst, 65521; jae`) where gcc compares, and subtracts on
/// the taken path (x_adler -O2: +7.5k clocks, x_isqrt +12.6k). Applied to
/// integer selects with a pure, single-use arithmetic or extension arm in the
/// select's block.
pub fn select_arms_sunk(
    context: &Context,
    function: &mut Function,
    fused: &dyn Fn(u32, bool) -> bool,
) -> bool {
    let mut changed = false;
    while let Some((select, yes, no)) = sinkable(context, function, fused) {
        sunk(function, select, yes, no);
        changed = true;
    }
    changed
}

/// The first integer select with an arm to sink, and each arm's instruction.
fn sinkable(
    context: &Context,
    function: &Function,
    fused: &dyn Fn(u32, bool) -> bool,
) -> Option<(InstId, Option<InstId>, Option<InstId>)> {
    for &block in function.layout() {
        for select in function.iter_block(block) {
            let one = function.instruction(select);
            if one.opcode != Opcode::Select || !matches!(context.types.int_bits(one.ty), Some(8 | 16 | 32)) {
                continue;
            }
            let [Operand::Value(_), yes, no] = one.operands[..] else { continue };
            if yes == no {
                continue;
            }
            let arm = |operand: Operand| -> Option<InstId> { sinkable_def(function, block, operand) };
            let (yes, no) = (arm(yes), arm(no));
            if (yes.is_some() || no.is_some()) && !splits_div_rem(context, function, block, select, fused) {
                return Some((select, yes, no));
            }
        }
    }
    None
}

/// Whether the branch made at `select` would part a quotient from the
/// remainder of the same operands (`u % r` then `u / r`, a trip of `itoa`):
/// the DAG makes one `udivrem` of the pair only within a block, and where the
/// target states a combined operation (`has_div_rem_op`) the pair is one
/// runtime call, two once parted (k_itoa -O2: 2747 -> 4720 clocks, +41 B).
fn splits_div_rem(
    context: &Context,
    function: &Function,
    block: BlockId,
    select: InstId,
    fused: &dyn Fn(u32, bool) -> bool,
) -> bool {
    use llrm_mir::opcode::BinaryOp::{SDiv, SRem, UDiv, URem};
    let divisions = |before: bool| -> Vec<(bool, bool, Operand, Operand)> {
        let mut seen_select = false;
        let mut found = Vec::new();
        for inst in function.iter_block(block) {
            if inst == select {
                seen_select = true;
                continue;
            }
            if seen_select == before {
                continue;
            }
            let one = function.instruction(inst);
            let (Opcode::Binary(op), [a, b]) = (&one.opcode, &one.operands[..]) else { continue };
            let signed = matches!(op, SDiv | SRem);
            let Some(bits) = context.types.int_bits(one.ty) else { continue };
            if matches!(op, UDiv | URem | SDiv | SRem) && fused(bits, signed) {
                found.push((signed, matches!(op, URem | SRem), *a, *b));
            }
        }
        found
    };
    let (before, after) = (divisions(true), divisions(false));
    before.iter().any(|&(signed, rem, a, b)| after.contains(&(signed, !rem, a, b)))
}

/// The instruction of `block` that makes `operand`, where it is pure and only
/// one instruction reads its value.
fn sinkable_def(
    function: &Function,
    block: BlockId,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    let pure = matches!(
        function.instruction(inst).opcode,
        Opcode::Binary(
            BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Shl
                | BinaryOp::LShr
                | BinaryOp::AShr
                | BinaryOp::And
                | BinaryOp::Or
                | BinaryOp::Xor
        ) | Opcode::Cast(CastOp::ZExt | CastOp::SExt | CastOp::Trunc)
    );
    (pure && function.parent(inst) == Some(block) && function.users(value).has_one_use()).then_some(inst)
}

/// `arm` and what only it reads (`3 * x` of `3 * x + 1`), in block order: the
/// whole chain moves, or the selector no longer folds it (`lea [x+x*2+1]`).
fn arm_chain(
    function: &Function,
    block: BlockId,
    arm: InstId,
) -> Vec<InstId> {
    let mut found = vec![arm];
    let mut at = 0;
    while at < found.len() {
        for operand in function.instruction(found[at]).operands.clone() {
            if let Some(def) = sinkable_def(function, block, operand)
                && !found.contains(&def)
            {
                found.push(def);
            }
        }
        at += 1;
    }
    function.iter_block(block).filter(|inst| found.contains(inst)).collect()
}

/// `select c, a, b` made `br c, ..` with each sunk arm in a block of its own
/// and a phi of the arms in the join.
fn sunk(
    function: &mut Function,
    select: InstId,
    yes: Option<InstId>,
    no: Option<InstId>,
) {
    let one = function.instruction(select).clone();
    let [condition, yes_value, no_value] = one.operands[..] else { unreachable!("a select") };
    let result = one.result.expect("a select's value");
    let home = function.parent(select).expect("a placed select");
    let join = split_block_at(function, home, select);
    let jump = function.terminator(home).expect("the jump the split made");
    let void = function.instruction(jump).ty;
    function.erase(jump).expect("nothing reads a jump");
    let mut side_of = |function: &mut Function, arm: Option<InstId>| -> (BlockId, Operand) {
        let Some(inst) = arm else { return (home, Operand::Block(join)) };
        let side = function.create_block(None);
        function.insert_block(side, Some(home)).expect("a new block");
        for moved in arm_chain(function, home, inst) {
            function.move_to(moved, Position::End(side)).expect("a placed instruction");
        }
        let back = function.create_instruction(Opcode::Br, void, vec![Operand::Block(join)], Flags::default(), None);
        function.insert(back, Position::End(side)).expect("an unplaced instruction");
        (side, Operand::Block(side))
    };
    // The true arm's block first after `home`: it is the branch's fallthrough,
    // as `EmitLoweredSelect` places the false copy after the branch to the
    // join.
    let (no_from, no_target) = side_of(function, no);
    let (yes_from, yes_target) = side_of(function, yes);
    let branch =
        function.create_instruction(Opcode::Br, void, vec![condition, yes_target, no_target], Flags::default(), None);
    function.insert(branch, Position::End(home)).expect("an unplaced instruction");
    let phi = function.create_instruction(
        Opcode::Phi,
        one.ty,
        vec![yes_value, Operand::Block(yes_from), no_value, Operand::Block(no_from)],
        Flags::default(),
        None,
    );
    function.insert(phi, Position::Before(select)).expect("an unplaced instruction");
    let value = function.instruction(phi).result.expect("a phi's value");
    function.replace_all_uses_with(result, Operand::Value(value));
    function.erase(select).expect("nothing reads it now");
}

#[cfg(test)]
#[path = "cgpselect_tests.rs"]
mod tests;
