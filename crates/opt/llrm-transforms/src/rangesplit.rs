//! A branch on a range test, `(x + c) u< n`, as two branches on `x u>= lo` and
//! `x u< hi`, for a target whose compare leaves its operand in place.
//!
//! # How it works
//! InstCombine and SimplifyCFG join `lo <= x && x < hi` into one unsigned
//! compare of `x - lo`. Where `x` is read again after the test, the subtract
//! needs a copy of it (on the 6502 the accumulator holds both, so the value
//! goes to `x` and back); two compares leave it alone. This pass undoes the
//! join for the branches whose subtract feeds nothing else; the driver prices
//! the function with and without it and keeps the cheaper.
//!
//! DEVIATE 6502-RANGE (owner llrm-target-6502, exit 2026-12-15): it is not
//! LLVM's. The pin has the join (`foldRangeChecks`, `SimplifyBranchOnICmpChain`)
//! and no inverse; llvm-mos keeps the two compares of `c >= 'a' && c <= 'z'`
//! (k_strupper, 47 B / 412 clocks). Exit: the target's own cost for the join in
//! SimplifyCFG's `FoldBranchToCommonDest`, once it can tell a branch from a value.
//!
//! # Example
//! ```text
//!   %s = add i8 %c, -97                 %g = icmp uge i8 %c, 97
//!   %t = icmp ult i8 %s, 26     =>      br i1 %g, label %in, label %out
//!   br i1 %t, label %T, label %F      in: %l = icmp ult i8 %c, 123
//!                                         br i1 %l, label %T, label %F
//! ```

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, GlobalKind, InstId, Module, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::{Type, TypeId};

/// The branches of `function` that are a range test: the block, the compare,
/// the offset instruction, the value tested, the bounds `lo` and `hi`.
struct Range {
    block: BlockId,
    jump: InstId,
    compare: InstId,
    offset: InstId,
    value: Operand,
    ty: TypeId,
    lo: u128,
    hi: u128,
    taken: BlockId,
    other: BlockId,
}

/// Every range test of `module`'s functions that `x` is read beyond; the
/// names of the functions that have one.
pub fn unmerged(module: &mut Module) -> Result<Vec<String>, String> {
    let context = &mut module.context;
    let mut changed = Vec::new();
    for global in &mut module.globals {
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let found = ranges(context, function);
        let any = !found.is_empty();
        for range in found {
            split(context, function, &range);
        }
        if let (true, Some(name)) = (any, global.name.as_deref()) {
            changed.push(name.to_owned());
        }
    }
    Ok(changed)
}

fn constant(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    }
}

fn int(
    context: &mut Context,
    ty: TypeId,
    value: u128,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(value) }))
}

fn ranges(
    context: &Context,
    function: &Function,
) -> Vec<Range> {
    let mut found = Vec::new();
    for block in function.layout().to_vec() {
        let Some(jump) = function.terminator(block) else { continue };
        let branch = function.instruction(jump);
        let [Operand::Value(condition), Operand::Block(taken), Operand::Block(other)] = branch.operands[..] else {
            continue;
        };
        if branch.opcode != Opcode::Br || taken == other {
            continue;
        }
        let llrm_mir::module::ValueDef::Instruction(compare) = function.value(condition).def else { continue };
        let test = function.instruction(compare);
        let (Opcode::ICmp(IntPredicate::Ult), [Operand::Value(sum), bound]) = (&test.opcode, &test.operands[..]) else {
            continue;
        };
        if function.parent(compare) != Some(block) || function.users(condition).iter().count() != 1 {
            continue;
        }
        let llrm_mir::module::ValueDef::Instruction(offset) = function.value(*sum).def else { continue };
        let add = function.instruction(offset);
        let (Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub)), [value, step]) = (&add.opcode, &add.operands[..])
        else {
            continue;
        };
        let (Some(step), Some(count), Some(ty)) =
            (constant(context, *step), constant(context, *bound), function.operand_type(context, *value))
        else {
            continue;
        };
        let Type::Int(bits) = context.types.get(ty) else { continue };
        let bits = *bits;
        if constant(context, *value).is_some() || bits == 0 || bits > 64 || function.users(*sum).iter().count() != 1 {
            continue;
        }
        // `x - lo` with `lo` the amount the test takes off
        let modulus = 1u128 << bits;
        let lo = if matches!(op, BinaryOp::Add) { (modulus - step % modulus) % modulus } else { step % modulus };
        let hi = lo + count;
        // a test from zero or to the top is one compare already
        if lo == 0 || count == 0 || hi >= modulus {
            continue;
        }
        // the point: `x` is read besides the subtract
        let Operand::Value(read) = *value else { continue };
        if function.users(read).iter().all(|one| one.user == offset) {
            continue;
        }
        found.push(Range { block, jump, compare, offset, value: *value, ty, lo, hi, taken, other });
    }
    found
}

fn split(
    context: &mut Context,
    function: &mut Function,
    range: &Range,
) {
    let flag = context.types.int(1);
    let void = function.instruction(range.jump).ty;
    let lo = int(context, range.ty, range.lo);
    let hi = int(context, range.ty, range.hi);
    // the second test, in a block of its own after the first
    let second = function.create_block(Some("range-hi"));
    function.insert_block(second, Some(range.block)).expect("a new block");
    let below = function.create_instruction(
        Opcode::ICmp(IntPredicate::Ult),
        flag,
        vec![range.value, hi],
        Flags::default(),
        None,
    );
    function.insert(below, Position::End(second)).expect("a placed block");
    let below = Operand::Value(function.instruction(below).result.expect("a result"));
    let jump = function.create_instruction(
        Opcode::Br,
        void,
        vec![below, Operand::Block(range.taken), Operand::Block(range.other)],
        Flags::default(),
        None,
    );
    function.insert(jump, Position::End(second)).expect("a placed block");
    // the first, in place of the subtract and its compare
    let above = function.create_instruction(
        Opcode::ICmp(IntPredicate::Uge),
        flag,
        vec![range.value, lo],
        Flags::default(),
        None,
    );
    function.insert(above, Position::Before(range.compare)).expect("a placed instruction");
    let above = Operand::Value(function.instruction(above).result.expect("a result"));
    function.set_operand(range.jump, 0, above);
    function.set_operand(range.jump, 1, Operand::Block(second));
    function.erase(range.compare).expect("a dead compare");
    function.erase(range.offset).expect("a dead subtract");
    // the taken block is now entered from the second, the other from both
    for phi in crate::edges::phis(function, range.taken) {
        let arms: Vec<(Operand, BlockId)> = function
            .instruction(phi)
            .operands
            .chunks(2)
            .map(|pair| match pair {
                [value, Operand::Block(from)] => (*value, if *from == range.block { second } else { *from }),
                _ => unreachable!("a phi's arms are value and block"),
            })
            .collect();
        function.set_operands(phi, arms.iter().flat_map(|(value, from)| [*value, Operand::Block(*from)]).collect());
    }
    for phi in crate::edges::phis(function, range.other) {
        let mut arms: Vec<(Operand, BlockId)> = function
            .instruction(phi)
            .operands
            .chunks(2)
            .map(|pair| match pair {
                [value, Operand::Block(from)] => (*value, *from),
                _ => unreachable!("a phi's arms are value and block"),
            })
            .collect();
        if let Some(&(value, _)) = arms.iter().find(|(_, from)| *from == range.block) {
            arms.push((value, second));
            function.set_operands(phi, arms.iter().flat_map(|(value, from)| [*value, Operand::Block(*from)]).collect());
        }
    }
}
