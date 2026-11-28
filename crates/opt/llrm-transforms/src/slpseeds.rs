//! `SLPVectorizer`'s roots that are no chain, phi or reduction: the operands
//! of the stores of a block, and the instructions read once.
//!
//! # How it works
//! `vectorizeNonVectorizableInsts` (l.35156), pass 2: the stores of a block
//! whose value is an instruction (two at least) are roots whose value operands
//! are sorted by opcode and type, and each run of one opcode and type is given
//! to `tryToVectorizeSequence` (l.34805) and `tryToVectorizeList` (l.30044),
//! up to as many lanes as fill whole registers. The stores themselves stay
//! scalar and read the lanes by extracts: four `sdiv x, 2` of an FFT
//! butterfly, stored to four different arrays, are one `<4 x i64>` `sdiv`.
//!
//! `vectorizeOnceUsedSeeds` (l.35884) is the very last attempt, after every
//! other root of the function is exhausted, block by block in post order. Its
//! seeds are the instructions of the block read by exactly one use
//! (`isOnceUsedSeed`: a binary operation that is not folded away by its
//! identity or absorbing constant, a unary one, a select, a cast that is not a
//! float to integer one, none of them the index of an address): such a value
//! needs one extract per lane, whatever the vector. `vectorizeSeeds` (l.35838)
//! sorts them by type, size, group (the opcode and type of
//! `generateKeySubkey`, an integer division by a variable on its own) and
//! program order, and `tryToVectorizeSequence` (l.34805) cuts the sorted list
//! into runs of one group, giving each, and the leftovers of runs too short to
//! fill a register, to `tryToVectorizeList` (l.30044) with `StandaloneSeeds`:
//! one register of lanes at most, no lane reading another, no window of
//! identical lanes. The tree is the bundle of the seeds, its users read the
//! lanes by extracts. The butterfly of an FFT, four `sdiv x, 2` stored to four
//! different arrays, is one `<4 x i64>` `sdiv` of the four sums.
//!
//! DEVIATE SLP10 (owner arm64, exit 2026-12-21): the roots are the stores
//! alone (no return, call or atomic operand, no `ExtractsCost` gate, no
//! reduction pass); calls, `freeze` and `extractelement` are not once-used
//! seeds, and the poor-throughput seeds (`isPoorThroughputOp`) are taken with
//! them, not before them.
//!
//! # Runs
//! In `slpvectorize`, after the store chains, the reductions and the compares:
//! the operands of the stores, then the once-used seeds.
//!
//! # Used by
//! `slpvectorize`.
//!
//! # Example
//! ```text
//! %q0 = sdiv i64 %s0, 2        ; store %q0, ptr %a
//! %q1 = sdiv i64 %s1, 2        ; store %q1, ptr %b
//!         =>
//! %v  = sdiv <2 x i64> <%s0, %s1>, splat (i64 2)
//! %q0 = extractelement <2 x i64> %v, i64 0 ...
//! ```

use llrm_mir::blockorder::reverse_post_order_traversal;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::passes::{Declared, Outer};
use llrm_mir::tti::Tti;
use llrm_mir::types::{FloatKind, Type, TypeId};

use crate::slpphi::{Roots, Session};

/// A seed and what orders and groups it.
struct Seed {
    inst: InstId,
    ty: TypeId,
    /// `getTypeID` order of the scalar types the vectorizer takes: floats,
    /// then doubles, then integers.
    rank: u8,
    bits: u32,
    group: usize,
}

/// `generateKeySubkey`'s key of a seed: what makes two of them one group.
#[derive(Clone, PartialEq)]
struct Key {
    block: BlockId,
    opcode: Opcode,
    ty: TypeId,
    /// A cast's source type; the instruction itself where a division by a
    /// variable must stay on its own.
    extra: Option<(TypeId, Option<InstId>)>,
}

fn scalar_of(
    context: &Context,
    ty: TypeId,
) -> Option<(u8, u32)> {
    match context.types.get(ty) {
        Type::Float(FloatKind::Float) => Some((0, 32)),
        Type::Float(FloatKind::Double) => Some((1, 64)),
        Type::Int(bits @ (8 | 16 | 32 | 64)) => Some((2, u32::from(*bits))),
        _ => None,
    }
}

/// `isNonProfitableIndex`: a chain of one-use instructions that ends in the
/// index of an address, whose operands are not loads or constants.
fn non_profitable_index(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    const MAX_INDEX_CHAIN_LENGTH: usize = 3;
    let profitable_operand = |operand: &Operand| match operand {
        Operand::Constant(_) => true,
        Operand::Value(value) => {
            let mut def = match function.value(*value).def {
                ValueDef::Instruction(def) => def,
                _ => return false,
            };
            if matches!(function.instruction(def).opcode, Opcode::Cast(_))
                && function.instruction(def).result.is_some_and(|result| function.users(result).iter().count() == 1)
                && let Operand::Value(inner) = function.instruction(def).operands[0]
                && let ValueDef::Instruction(inner) = function.value(inner).def
            {
                def = inner;
            }
            matches!(function.instruction(def).opcode, Opcode::Load { .. })
        }
        _ => false,
    };
    let _ = context;
    let mut user = single_user(function, inst);
    for _ in 0..MAX_INDEX_CHAIN_LENGTH {
        let Some(at) = user else { return false };
        if matches!(function.instruction(at).opcode, Opcode::GetElementPtr { .. }) {
            let pointer = function.instruction(at).operands[0];
            return matches!(pointer, Operand::Constant(_))
                || !function.instruction(inst).operands.iter().any(profitable_operand);
        }
        user = single_user(function, at);
    }
    false
}

/// The one instruction that reads `inst`'s value, if one does, once.
fn single_user(
    function: &Function,
    inst: InstId,
) -> Option<InstId> {
    let result = function.instruction(inst).result?;
    let mut users = function.users(result).iter();
    let first = users.next()?;
    users.next().is_none().then_some(first.user)
}

/// `isOnceUsedSeed`, over the opcodes this pass takes.
fn once_used(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    let Some(user) = single_user(function, inst) else { return false };
    if non_profitable_index(context, function, inst) {
        return false;
    }
    let instruction = function.instruction(inst);
    match instruction.opcode {
        Opcode::Binary(op) => {
            // The identity or absorbing constant: folded away before the
            // codegen, the vector would only repack the lanes.
            let constant = |at: usize| match instruction.operands[at] {
                Operand::Constant(id) => Some(&context.get(id).kind),
                _ => None,
            };
            let is_int = |kind: Option<&ConstantKind>, wanted: u128| {
                matches!(kind, Some(ConstantKind::Int(value)) if *value == wanted)
                    || (wanted == 0 && matches!(kind, Some(ConstantKind::Zero)))
            };
            let all_ones = |kind: Option<&ConstantKind>| match kind {
                Some(ConstantKind::Int(value)) => context
                    .types
                    .int_bits(instruction.ty)
                    .is_some_and(|bits| *value == if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }),
                _ => false,
            };
            for at in 0..2 {
                let right = at == 1;
                let kind = constant(at);
                let identity = match op {
                    BinaryOp::Add | BinaryOp::Or | BinaryOp::Xor => is_int(kind, 0),
                    BinaryOp::Sub | BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr => right && is_int(kind, 0),
                    BinaryOp::Mul => is_int(kind, 1),
                    BinaryOp::SDiv | BinaryOp::UDiv => right && is_int(kind, 1),
                    BinaryOp::And => all_ones(kind),
                    _ => false,
                };
                let absorber = match op {
                    BinaryOp::Mul | BinaryOp::And => is_int(kind, 0),
                    BinaryOp::Or => all_ones(kind),
                    _ => false,
                };
                if identity || absorber {
                    return false;
                }
            }
            true
        }
        Opcode::FNeg | Opcode::Select => true,
        Opcode::Cast(op) => {
            if matches!(op, CastOp::FPToSI | CastOp::FPToUI) {
                return false;
            }
            !matches!(function.instruction(user).opcode, Opcode::Cast(_)) || single_user(function, user).is_some()
        }
        _ => false,
    }
}

/// The seeds of `block`, in program order.
fn seeds_of(
    context: &Context,
    function: &Function,
    block: BlockId,
) -> Vec<Seed> {
    let mut keys: Vec<Key> = Vec::new();
    let mut seeds = Vec::new();
    for inst in function.iter_block(block) {
        let instruction = function.instruction(inst);
        if instruction.result.is_none() || function.is_erased(inst) {
            continue;
        }
        let Some((rank, bits)) = scalar_of(context, instruction.ty) else { continue };
        if !once_used(context, function, inst) {
            continue;
        }
        let division = matches!(
            instruction.opcode,
            Opcode::Binary(BinaryOp::SDiv | BinaryOp::UDiv | BinaryOp::SRem | BinaryOp::URem)
        );
        let variable_divisor = division && !matches!(instruction.operands[1], Operand::Constant(_));
        let extra = match instruction.opcode {
            Opcode::Cast(_) => function.operand_type(context, instruction.operands[0]).map(|ty| (ty, None)),
            _ if variable_divisor => Some((instruction.ty, Some(inst))),
            _ => None,
        };
        let key = Key { block, opcode: instruction.opcode.clone(), ty: instruction.ty, extra };
        let group = match keys.iter().position(|one| *one == key) {
            Some(group) => group,
            None => {
                keys.push(key);
                keys.len() - 1
            }
        };
        seeds.push(Seed { inst, ty: instruction.ty, rank, bits, group });
    }
    seeds
}

/// Whether the seeds `a` and `b` are of one run (`AreCompatibleSeeds`).
fn compatible(
    a: &Seed,
    b: &Seed,
) -> bool {
    a.inst == b.inst || (a.ty == b.ty && a.group == b.group)
}

/// `tryToVectorizeSequence` over the sorted seeds of one block.
fn sequence(
    session: &mut Session,
    function: &mut Function,
    seeds: &[Seed],
    max_vf_only: bool,
    register_bits: usize,
) -> bool {
    let live = |function: &Function, seed: &Seed| !Session::deleted_inst(function, seed.inst);
    let mut changed = false;
    let mut candidates: Vec<usize> = Vec::new();
    let mut at = 0;
    while at < seeds.len() {
        if !live(function, &seeds[at]) {
            at += 1;
            continue;
        }
        let mut run: Vec<usize> = Vec::new();
        let mut same = at;
        while same < seeds.len()
            && (!live(function, &seeds[same]) || run.last().is_none_or(|last| compatible(&seeds[*last], &seeds[same])))
        {
            if live(function, &seeds[same]) {
                run.push(same);
            }
            same += 1;
        }
        let list: Vec<InstId> = run.iter().map(|seed| seeds[*seed].inst).collect();
        if list.len() > 1 && session.try_list(function, &list, max_vf_only) {
            changed = true;
            // What an earlier run left, the ones still standing.
            candidates.retain(|seed| live(function, &seeds[*seed]));
        } else {
            let first = &seeds[at];
            let min_elements = (register_bits / first.bits.max(1) as usize).max(2);
            let same_type = candidates.first().is_none_or(|front| seeds[*front].ty == first.ty);
            if list.len() < min_elements && same_type {
                candidates.extend(run.iter().copied().filter(|seed| live(function, &seeds[*seed])));
            }
        }
        // The final attempt on what several short runs left.
        let type_ends = same == seeds.len() || seeds[same].ty != seeds[at].ty;
        if candidates.len() > 1 && type_ends {
            let pooled: Vec<InstId> =
                candidates.iter().filter(|seed| live(function, &seeds[**seed])).map(|seed| seeds[*seed].inst).collect();
            if pooled.len() > 1 && session.try_list(function, &pooled, false) {
                changed = true;
            } else if max_vf_only {
                // Small vectors, one run of a kind at a time.
                let left: Vec<usize> =
                    candidates.iter().copied().filter(|seed| live(function, &seeds[*seed])).collect();
                let mut start = 0;
                while start < left.len() {
                    let mut end = start;
                    while end < left.len() && compatible(&seeds[left[end]], &seeds[left[start]]) {
                        end += 1;
                    }
                    let small: Vec<InstId> = left[start..end].iter().map(|seed| seeds[*seed].inst).collect();
                    if small.len() > 1 && session.try_list(function, &small, false) {
                        changed = true;
                    }
                    start = end.max(start + 1);
                }
            }
        }
        if type_ends || candidates.len() > 1 {
            candidates.clear();
        }
        at = same.max(at + 1);
    }
    changed
}

/// The value operands of the stores of `block` that stand for `Operands`
/// (`vectorizeNonVectorizableInsts`, pass 2), in program order.
fn operands_of(
    context: &Context,
    function: &Function,
    block: BlockId,
) -> Vec<Seed> {
    let stores: Vec<InstId> = function
        .iter_block(block)
        .filter(|inst| {
            let instruction = function.instruction(*inst);
            matches!(instruction.opcode, Opcode::Store { .. })
                && matches!(
                    instruction.operands[0],
                    Operand::Value(value) if matches!(function.value(value).def, ValueDef::Instruction(_))
                )
        })
        .collect();
    if stores.len() < 2 {
        return Vec::new();
    }
    let mut keys: Vec<Key> = Vec::new();
    let mut seen: Vec<InstId> = Vec::new();
    let mut seeds = Vec::new();
    for store in stores {
        let Operand::Value(value) = function.instruction(store).operands[0] else { continue };
        let ValueDef::Instruction(inst) = function.value(value).def else { continue };
        let instruction = function.instruction(inst);
        if function.parent(inst) != Some(block)
            || seen.contains(&inst)
            || matches!(instruction.opcode, Opcode::ShuffleVector(_) | Opcode::Phi)
        {
            continue;
        }
        let Some((rank, bits)) = scalar_of(context, instruction.ty) else { continue };
        seen.push(inst);
        let key = Key { block, opcode: instruction.opcode.clone(), ty: instruction.ty, extra: None };
        let group = match keys.iter().position(|one| *one == key) {
            Some(group) => group,
            None => {
                keys.push(key);
                keys.len() - 1
            }
        };
        seeds.push(Seed { inst, ty: instruction.ty, rank, bits, group });
    }
    seeds
}

/// Type, size, group, program order (`SeedSorter`, `OperandSorter`). Seeds
/// stand in `block`, so its order is the program order.
fn sorted(
    function: &Function,
    block: BlockId,
    mut seeds: Vec<Seed>,
) -> Vec<Seed> {
    if seeds.len() < 2 {
        return seeds;
    }
    let place: IdMap<InstId, usize> = function.iter_block(block).enumerate().map(|(at, inst)| (inst, at)).collect();
    seeds.sort_by_key(|seed| (seed.rank, seed.bits, seed.group, place[&seed.inst]));
    seeds
}

/// The operands of the stores and the once-used seeds of every block of
/// `function`, as vectors; whether any was made.
pub(crate) fn run_function(
    context: &mut Context,
    layout: &DataLayout,
    declared: &mut Declared,
    function: &mut Function,
    outer: &Outer,
    tti: &dyn Tti,
) -> bool {
    let register_bits = tti.vector_register_bits() as usize;
    let mut changed = false;
    // `vectorizeNonVectorizableInsts` runs with each block of
    // `vectorizeChainsInBlock`, in the order the pass visits them.
    let mut blocks = reverse_post_order_traversal(function);
    blocks.reverse();
    for block in &blocks {
        let operands = sorted(function, *block, operands_of(context, function, *block));
        if operands.len() > 1 {
            let mut session = Session::new(context, layout, declared, outer, tti, *block, Roots::Operands);
            changed |= sequence(&mut session, function, &operands, true, register_bits);
        }
    }
    // `vectorizeOnceUsedSeeds`: the very last attempt, after every block.
    for block in &blocks {
        let seeds = sorted(function, *block, seeds_of(context, function, *block));
        if seeds.len() > 1 {
            let mut session = Session::new(context, layout, declared, outer, tti, *block, Roots::Seeds);
            changed |= sequence(&mut session, function, &seeds, false, register_bits);
        }
    }
    changed
}

#[cfg(test)]
#[path = "slpseeds_tests.rs"]
mod tests;
