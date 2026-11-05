//! One zero fill for the zeroed locals that follow each other in the entry
//! block. llrm only.
//!
//! # How it works
//! In the entry block, a run of `llvm.memset(%a, 0, n)` calls (nothing
//! between them), each over a whole static `alloca` of `n` bytes, becomes one
//! `alloca` of the sum, one `getelementptr` per member and one memset. Each
//! member sits at its own alignment inside the new `alloca`.
//!
//! The members are the memsets that loop idioms make from the zero-fill loops
//! of Nib and QB locals. SROA has run by now, so nothing splits the block
//! again.
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | The pin never joins allocas (`MemCpyOpt` merges ranges of one object). In real mode every member of 64 bytes or more is a `rep stos` fill of 9 bytes of setup (`lea di`, `mov cx`, `rep stosw`); the merged one pays it once. bench/matmul.nib -Os: three fills of a, b and c, -14 B. Owner m16; exit 2026-11-20: the members are one object in the frontend. |
//!
//! # Example
//! Before:
//!
//! ```mir
//! %a = alloca [128 x i8]
//! %b = alloca [128 x i8]
//! call void @llvm.memset.p0.i16(ptr %b, i8 0, i16 128, i1 false)
//! call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 128, i1 false)
//! ```
//!
//! After:
//!
//! ```mir
//! %z = alloca [256 x i8]
//! %a = getelementptr inbounds i8, ptr %z, i16 128
//! call void @llvm.memset.p0.i16(ptr %z, i8 0, i16 256, i1 false)
//! ```
//!
//! From unit test `test_zeroed_locals_in_a_row_share_one_fill` in
//! `zeromerge_tests.rs`.
//!
//! # Runs
//! Once per body, after `extload`, with `wanted("zeromerge")`: on where a
//! pointer is 16 bits.
//!
//! # Needs
//! Nothing.
//!
//! # Keeps
//! Blocks, dominators and loops.
//!
//! # See also
//! Tests: `zeromerge_tests.rs`.

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::memory;
use llrm_mir::module::{Function, GlobalValue, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::types::Type;

/// The smallest fill worth joining: below it a store run may be the lowering
/// (`selectiondag/builder/memops.rs`), and a join would turn it into a fill.
const MIN_BYTES: u64 = 64;

pub struct ZeroMerge;

impl FunctionPass for ZeroMerge {
    fn name(&self) -> &'static str {
        "zeromerge"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if unit.layout.pointer(0).bits != 16 {
            return PreservedAnalyses::all();
        }
        if merged(unit, &analyses.outer().globals) {
            PreservedAnalyses::none().preserve::<llrm_mir::passes::Dominators>().preserve::<llrm_mir::passes::Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// One zeroed `alloca` of the run.
struct Member {
    call: InstId,
    alloca: InstId,
    size: u64,
    align: u64,
}

/// `call` as a non-volatile zero fill of a whole static `alloca` of the entry
/// block, of at least [`MIN_BYTES`].
fn member(
    unit: &Unit,
    globals: &[GlobalValue],
    call: InstId,
) -> Option<Member> {
    let function: &Function = unit.function;
    let callee = memory::callee(unit.context, function, call)?;
    if globals.get(callee.0 as usize)?.intrinsic()? != Intrinsic::MemSet {
        return None;
    }
    let operands = &function.instruction(call).operands;
    let [Operand::Value(dest), Operand::Constant(value), Operand::Constant(length), Operand::Constant(volatile), ..] =
        operands[..]
    else {
        return None;
    };
    if !matches!(unit.context.get(value).kind, ConstantKind::Int(0))
        || !matches!(unit.context.get(volatile).kind, ConstantKind::Int(0))
    {
        return None;
    }
    let ConstantKind::Int(length) = unit.context.get(length).kind else { return None };
    let llrm_mir::module::ValueDef::Instruction(alloca) = function.value(dest).def else { return None };
    let Opcode::Alloca { allocated, align, .. } = function.instruction(alloca).opcode else { return None };
    if function.parent(alloca) != function.entry() || !function.instruction(alloca).operands.is_empty() {
        return None;
    }
    let size = unit.layout.alloc_size(&unit.context.types, allocated);
    let length = length as u64;
    if size != length || length < MIN_BYTES {
        return None;
    }
    let align = align.unwrap_or_else(|| unit.layout.align(&unit.context.types, allocated));
    // A lifetime marker or a debug record names the `alloca` itself.
    let plain = function
        .users(dest)
        .iter()
        .all(|used| used.user == call || !is_marker(unit.context, globals, function, used.user));
    plain.then_some(Member { call, alloca, size, align })
}

fn is_marker(
    context: &Context,
    globals: &[GlobalValue],
    function: &Function,
    inst: InstId,
) -> bool {
    memory::callee(context, function, inst)
        .and_then(|callee| globals.get(callee.0 as usize)?.intrinsic())
        .is_some_and(|intrinsic| matches!(intrinsic, Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd))
}

/// The run of members at the start of the entry block's calls: every
/// instruction before the first is an `alloca`, and each member follows the
/// one before.
fn run_of(
    unit: &Unit,
    globals: &[GlobalValue],
) -> Vec<Member> {
    let function: &Function = unit.function;
    let Some(entry) = function.entry() else { return Vec::new() };
    let mut run: Vec<Member> = Vec::new();
    for inst in function.iter_block(entry) {
        if run.is_empty() && matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }) {
            continue;
        }
        match member(unit, globals, inst) {
            Some(one) if !run.iter().any(|before| before.alloca == one.alloca) => run.push(one),
            _ => break,
        }
    }
    run
}

fn merged(
    unit: &mut Unit,
    globals: &[GlobalValue],
) -> bool {
    let run = run_of(unit, globals);
    if run.len() < 2 {
        return false;
    }
    let mut offsets = Vec::with_capacity(run.len());
    let mut total = 0u64;
    let mut align = 1u64;
    for one in &run {
        total = total.next_multiple_of(one.align);
        offsets.push(total);
        total += one.size;
        align = align.max(one.align);
    }
    let (i8, bits) = (unit.context.types.int(8), unit.layout.pointer(0).index_bits);
    let (pointer, index) = (unit.context.types.ptr(0), unit.context.types.int(bits));
    let array = unit.context.types.intern(Type::Array { element: i8, count: total });
    let first = &run[0];
    let whole = unit
        .function
        .create_instruction(
            Opcode::Alloca { allocated: array, align: Some(align), address_space: 0 },
            pointer,
            Vec::new(),
            Flags::default(),
            None,
        );
    unit.function.insert(whole, Position::Before(first.alloca)).expect("a place before the alloca");
    let whole = Operand::Value(unit.function.instruction(whole).result.expect("a value"));
    for (one, &offset) in run.iter().zip(&offsets) {
        let old = unit.function.instruction(one.alloca).result.expect("a value");
        let new = if offset == 0 {
            whole
        } else {
            let at = unit.context.int(index, offset as i128);
            let gep = unit
                .function
                .create_instruction(
                    Opcode::GetElementPtr { source: i8 },
                    pointer,
                    vec![whole, Operand::Constant(at)],
                    Flags::INBOUNDS,
                    None,
                );
            unit.function.insert(gep, Position::Before(first.call)).expect("a place before the fill");
            Operand::Value(unit.function.instruction(gep).result.expect("a value"))
        };
        unit.function.replace_value(old, new);
    }
    // The first fill takes the whole; the rest go with their allocas.
    let length_ty = unit
        .function
        .operand_type(unit.context, unit.function.instruction(first.call).operands[2])
        .expect("a typed length");
    let length = unit.context.int(length_ty, total as i128);
    unit.function.set_operand(first.call, 0, whole);
    unit.function.set_operand(first.call, 2, Operand::Constant(length));
    for one in &run[1..] {
        unit.function.erase(one.call).expect("a fill nothing reads");
    }
    for one in &run {
        unit.function.erase(one.alloca).expect("an alloca nothing reads");
    }
    true
}

#[cfg(test)]
#[path = "zeromerge_tests.rs"]
mod tests;
