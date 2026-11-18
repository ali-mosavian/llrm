//! The index of an `inbounds` `gep` into a small global is a byte.
//!
//! # How it works
//! `getelementptr inbounds [4 x i8], ptr @g, i16 (zext i8 %x)` is `@g + 4 * %x`
//! in a word. An `inbounds` access stays inside its object, so where `@g` is no
//! larger than the target's one-instruction index reach (`Tti::address_index_max`,
//! the 6502's `abs,x`: 255) the offset is a byte: `%x * 4` does not wrap in
//! `i8`, and a trip that would wrap is poison already. The `gep` becomes
//! `gep inbounds i8, ptr @g, i16 (zext (mul nuw i8 %x, 4))`, plus a `gep` of
//! the constant part of its indices, so isel finds `global + zext i8` and forms
//! `lda g,x`, not a 16-bit shift and add in a pointer pair.
//!
//! Only a `gep` with one variable index, the zero extension of a byte, and
//! constant others; a scale of 1 is already a byte index. O(1) per `gep`.
//!
//! DEVIATE GEP-BYTE-INDEX (no pin pass narrows by object extent: InstCombine's
//! `shl (zext X), C -> zext (shl X, C)` needs known-zero high bits of `X`
//! (`visitShl`, InstCombineShifts.cpp#L1175), and the DAG's `SimplifyDemandedBits`
//! cannot narrow an address index whose bits are all demanded; the extent is the
//! one `getBaseObjectSize` reads for `llvm.objectsize`; owner llrm-target-6502,
//! exit 2026-11-30: the 6502 backend narrows an address index from the object).
//!
//! # Example
//! ```text
//!  %z = zext i8 %x to i16                    %m = mul nuw i8 %x, 4
//!  %p = gep inbounds [4 x i8], @g, i16 %z    %z = zext i8 %m to i16
//!  %v = load i8, ptr %p                 ==>  %p = gep inbounds i8, @g, i16 %z
//!                                            %v = load i8, ptr %p
//! ```
//!
//! # Runs
//! In the strength slot after `gepoffset`; option `gepbyteindex`; only for a
//! target that sets `Tti::address_index_max`.

use llrm_mir::context::{Constant, ConstantKind, Context, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::types::TypeId;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::objectsize::{ObjectSizeOpts, get_base_object_size};

pub struct GepByteIndex;

/// A `gep` to rewrite: the byte `x` whose zero extension is its one variable
/// index, the bytes it steps by, and what the constant indices add.
struct Candidate {
    gep: InstId,
    base: Operand,
    byte: Operand,
    scale: u64,
    constant: u64,
    /// The type of the index (the pointer's width) and of the `gep`.
    index: TypeId,
}

impl FunctionPass for GepByteIndex {
    fn name(&self) -> &'static str {
        "gepbyteindex"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let Some(max) = outer.target().tti().and_then(|tti| tti.address_index_max()) else {
            return PreservedAnalyses::all();
        };
        let candidates = {
            let q = Query { globals: &outer.globals, ..Query::bare(unit.context, unit.function, unit.layout) };
            candidates(&q, unit.function, max)
        };
        if candidates.is_empty() {
            return PreservedAnalyses::all();
        }
        for candidate in &candidates {
            rewrite(unit.context, unit.function, candidate);
        }
        // Blocks and edges are as they were.
        PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
    }
}

fn integer(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Option<i128> {
    let Operand::Constant(id) = operand else { return None };
    let ConstantKind::Int(bits) = context.get(id).kind else { return None };
    let width = function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty))?;
    Some(signed(bits, width))
}

fn candidates(
    q: &Query,
    function: &Function,
    max: u64,
) -> Vec<Candidate> {
    let (context, layout) = (q.context(), q.layout);
    let opts = ObjectSizeOpts { round_to_align: false, null_is_unknown_size: true };
    let mut found = Vec::new();
    for (_, gep) in function.walk() {
        let inst = function.instruction(gep);
        let Opcode::GetElementPtr { source } = inst.opcode else { continue };
        if !inst.flags.contains(Flags::INBOUNDS) {
            continue;
        }
        let Some(extent) = get_base_object_size(q, inst.operands[0], opts).filter(|&extent| extent <= max) else {
            continue;
        };
        let indices: Vec<Option<i128>> =
            inst.operands[1..].iter().map(|&one| integer(context, function, one)).collect();
        let (constant, variable) = layout.collect_offset(&context.types, source, &indices);
        let (&[(at, scale)], Ok(constant)) = (&variable[..], u64::try_from(constant)) else { continue };
        if !(2..=max).contains(&scale) || constant > extent {
            continue;
        }
        // The one variable index is a byte, zero-extended to the index width.
        let index = inst.operands[1 + at];
        let Operand::Value(value) = index else { continue };
        let ValueDef::Instruction(def) = function.value(value).def else { continue };
        let extension = function.instruction(def);
        let Opcode::Cast(CastOp::ZExt) = extension.opcode else { continue };
        let byte = extension.operands[0];
        let (Some(from), Some(to)) = (
            function.operand_type(context, byte).and_then(|ty| context.types.int_bits(ty)),
            context.types.int_bits(extension.ty),
        ) else {
            continue;
        };
        let space = match context.types.get(inst.ty) {
            llrm_mir::types::Type::Pointer(space) => *space,
            _ => continue,
        };
        if from != 8 || to != layout.pointer(space).index_bits {
            continue;
        }
        found.push(Candidate { gep, base: inst.operands[0], byte, scale, constant, index: extension.ty });
    }
    found
}

fn rewrite(
    context: &mut Context,
    function: &mut Function,
    candidate: &Candidate,
) {
    let gep = candidate.gep;
    let (ty, old) = {
        let inst = function.instruction(gep);
        (inst.ty, inst.result.expect("an address"))
    };
    let i8 = context.types.int(8);
    let width = context.types.int_bits(candidate.index).expect("an integer index");
    let mut make = |function: &mut Function, opcode, ty, operands: Vec<Operand>, flags| {
        let inst = function.create_instruction(opcode, ty, operands, flags, None);
        function.insert(inst, Position::Before(gep)).expect("a position before the address");
        Operand::Value(function.instruction(inst).result.expect("a value"))
    };
    let scale = Operand::Constant(context.constant(Constant { ty: i8, kind: ConstantKind::Int(u128::from(candidate.scale)) }));
    let product = make(function, Opcode::Binary(BinaryOp::Mul), i8, vec![candidate.byte, scale], Flags::NUW);
    let index = make(function, Opcode::Cast(CastOp::ZExt), candidate.index, vec![product], Flags::default());
    let mut address = make(
        function,
        Opcode::GetElementPtr { source: i8 },
        ty,
        vec![candidate.base, index],
        Flags::INBOUNDS,
    );
    if candidate.constant != 0 {
        debug_assert!(width >= 16);
        let constant = Operand::Constant(
            context.constant(Constant { ty: candidate.index, kind: ConstantKind::Int(u128::from(candidate.constant)) }),
        );
        address = make(function, Opcode::GetElementPtr { source: i8 }, ty, vec![address, constant], Flags::INBOUNDS);
    }
    function.replace_all_uses_with(old, address);
    function.erase(gep).expect("nothing reads the old address");
}

#[cfg(test)]
#[path = "gepbyteindex_tests.rs"]
mod tests;
