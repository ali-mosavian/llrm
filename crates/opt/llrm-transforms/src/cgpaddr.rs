//! Sinks the arithmetic of an address into the block that uses it, where the
//! target folds it into the access: CodeGenPrepare's `optimizeMemoryInst`
//! ([CodeGenPrepare.cpp#L5903](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L5903)).
//!
//! # How it works
//! For a `load` or `store` the address is matched as `AddressingModeMatcher`
//! does: a `gep` chain read as `base + constant + scale * index` (one variable
//! index in all, to depth 4), each step kept only while the target's
//! `isLegalAddressingMode` says the whole is one access. When some
//! instruction of the match is in another block, the address is made again
//! just before the access, from the match alone (`sunkaddr`: a `gep` of the
//! scaled index, a `gep` of the constant), one copy for each block and
//! address. The `gep`s left unread go.
//!
//! ```text
//!  b1: %p = gep i32, ptr %a, i16 %i       b1: br b2
//!      br b2                              b2: %s = mul i16 %i, 4
//!  b2: store i32 0, ptr %p                    %q = gep i8, ptr %a, i16 %s
//!                                             store i32 0, ptr %q
//! ```
//!
//! MIRROR: the depth limit, the one-index-and-a-scale form of the match,
//! the legality test at each step, the rule that nothing is made when every
//! matched instruction is already in the access's block, and one `sunkaddr`
//! for each (block, address).
//!
//! DEVIATE CGP1: no promoted extensions (`TypePromotionTransaction`), no
//! `AddressingModeCombiner` for a phi or select of addresses (`addresssink`
//! makes the phi one address first), no base-register-and-global form
//! (a global stays the base) and no matching through `ptrtoint`/`inttoptr`.
//! Exit: those parts translated.
//!
//! # Example
//! The test `test_an_address_made_in_another_block_is_made_where_it_is_used`
//! in `cgpaddr_tests.rs` is the sketch above.
//!
//! # Runs
//! Once, after `addresssink`, in the same gate. Off with `-fno-strength-reduce`
//! and at -O0.
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.

use std::collections::BTreeMap;

use llrm_analysis::num::Num as BigInt;
use llrm_mir::context::{ConstantKind, Context, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use llrm_mir::tti::Tti;
use llrm_mir::types::Type;

use crate::counting;

/// `MaxDepth` of the matcher's recursion
/// ([CodeGenPrepare.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L2998)
/// `matchAddr(V, 5)`): the `gep` chain read goes to this many.
const DEPTH: u32 = 4;

pub struct CgpAddr;

#[cfg(test)]
#[path = "cgpaddr_tests.rs"]
mod tests;

impl FunctionPass for CgpAddr {
    fn name(&self) -> &'static str {
        "cgpaddr"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let Some(tti) = analyses.outer().target().tti() else { return PreservedAnalyses::all() };
        if sunk(unit, &*tti) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// `base + offset + scale * index`, the instructions that made it, and what it
/// is made of.
struct Mode {
    base: Operand,
    offset: i128,
    index: Option<(Operand, u64)>,
    insts: Vec<InstId>,
}

/// The index of the address among a memory access's operands.
fn address_of(
    function: &Function,
    inst: InstId,
) -> Option<usize> {
    match function.instruction(inst).opcode {
        Opcode::Load { .. } => Some(0),
        Opcode::Store { .. } => Some(1),
        _ => None,
    }
}

fn integer(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        _ => None,
    }
}

/// `matchAddr`: `address` read as a mode, `depth` more `gep`s allowed.
fn matched(
    unit: &passes::Unit,
    tti: &dyn Tti,
    space: u32,
    address: Operand,
    depth: u32,
) -> Mode {
    let plain = || Mode { base: address, offset: 0, index: None, insts: Vec::new() };
    let Operand::Value(value) = address else { return plain() };
    let ValueDef::Instruction(gep) = unit.function.value(value).def else { return plain() };
    let instruction = unit.function.instruction(gep);
    let Opcode::GetElementPtr { source } = instruction.opcode else { return plain() };
    if depth == 0 {
        return plain();
    }
    let known: Vec<Option<i128>> = instruction.operands[1..]
        .iter()
        .map(|&one| {
            integer(unit.context, one).map(|bits| {
                let width = unit
                    .function
                    .operand_type(unit.context, one)
                    .and_then(|ty| unit.context.types.int_bits(ty))
                    .unwrap_or(64);
                signed(bits, width)
            })
        })
        .collect();
    let (offset, variable) = unit.layout.collect_offset(&unit.context.types, source, &known);
    if variable.len() > 1 {
        return plain();
    }
    let inner = matched(unit, tti, space, instruction.operands[0], depth - 1);
    let index = match (variable.first(), inner.index) {
        (Some(_), Some(_)) => return plain(),
        (Some(&(at, scale)), None) => Some((instruction.operands[1 + at], scale)),
        (None, held) => held,
    };
    let total = offset + inner.offset;
    let scale = index.map_or(0, |(_, scale)| i64::try_from(scale).unwrap_or(i64::MAX));
    let Ok(displacement) = i64::try_from(total) else { return plain() };
    if !tti.is_legal_addressing_mode_in(space, false, displacement, true, scale) {
        return plain();
    }
    let mut insts = inner.insts;
    insts.push(gep);
    Mode { base: inner.base, offset: total, index, insts }
}

/// Each load and store whose address is made in another block, with the address
/// made where it is used; whether any was.
fn sunk(
    unit: &mut passes::Unit,
    tti: &dyn Tti,
) -> bool {
    let accesses: Vec<(BlockId, InstId, usize)> = unit
        .function
        .walk()
        .filter_map(|(block, inst)| Some((block, inst, address_of(unit.function, inst)?)))
        .collect();
    let mut made: BTreeMap<(BlockId, Operand), Operand> = BTreeMap::new();
    let mut touched: Vec<InstId> = Vec::new();
    let mut changed = false;
    for (block, access, at) in accesses {
        let address = unit.function.instruction(access).operands[at];
        let Some(Type::Pointer(space)) =
            unit.function.operand_type(unit.context, address).map(|ty| unit.context.types.get(ty).clone())
        else {
            continue;
        };
        let mode = matched(unit, tti, space, address, DEPTH);
        let local = |inst: &InstId| unit.function.parent(*inst) == Some(block);
        if mode.insts.is_empty() || mode.insts.iter().all(local) {
            continue;
        }
        let ready = match made.get(&(block, address)) {
            Some(&ready) => ready,
            None => {
                let ready = remade(unit, access, space, &mode);
                made.insert((block, address), ready);
                ready
            }
        };
        unit.function.set_operand(access, at, ready);
        touched.extend(mode.insts);
        changed = true;
    }
    // `RecursivelyDeleteTriviallyDeadInstructions`: the `gep`s nothing reads,
    // last made first.
    touched.sort();
    touched.dedup();
    for inst in touched.into_iter().rev() {
        if unit.function.is_erased(inst) {
            continue;
        }
        let dead = unit.function.instruction(inst).result.is_some_and(|result| unit.function.users(result).is_empty());
        if dead {
            unit.function.erase(inst).expect("no one reads it");
        }
    }
    changed
}

/// `sunkaddr`: the address of `mode` made before `access`.
fn remade(
    unit: &mut passes::Unit,
    access: InstId,
    space: u32,
    mode: &Mode,
) -> Operand {
    let bits = unit.layout.pointer(space).index_bits;
    let pointer = unit.function.operand_type(unit.context, mode.base).expect("a typed base");
    let i8 = unit.context.types.int(8);
    let mut current = mode.base;
    let put = |unit: &mut passes::Unit, opcode: Opcode, ty, operands: Vec<Operand>| -> Operand {
        let inst = unit.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        unit.function.insert(inst, Position::Before(access)).expect("a position before the access");
        Operand::Value(unit.function.instruction(inst).result.expect("a value"))
    };
    if let Some((index, scale)) = mode.index {
        let width = unit
            .function
            .operand_type(unit.context, index)
            .and_then(|ty| unit.context.types.int_bits(ty))
            .expect("an integer index");
        let wide = unit.context.types.int(bits);
        // The `gep` sign-extends an index narrower than the pointer's.
        let index = match width.cmp(&bits) {
            std::cmp::Ordering::Less => put(unit, Opcode::Cast(CastOp::SExt), wide, vec![index]),
            std::cmp::Ordering::Greater => put(unit, Opcode::Cast(CastOp::Trunc), wide, vec![index]),
            std::cmp::Ordering::Equal => index,
        };
        let scaled = if scale == 1 {
            index
        } else {
            let by = counting::constant(unit.context, &BigInt::from(scale), bits);
            put(unit, Opcode::Binary(BinaryOp::Mul), wide, vec![index, by])
        };
        current = put(unit, Opcode::GetElementPtr { source: i8 }, pointer, vec![current, scaled]);
    }
    if mode.offset != 0 {
        let constant = counting::constant(unit.context, &BigInt::from(mode.offset), bits);
        current = put(unit, Opcode::GetElementPtr { source: i8 }, pointer, vec![current, constant]);
    }
    current
}
