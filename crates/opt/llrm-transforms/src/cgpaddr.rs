//! CodeGenPrepare: comparison and address sinking, so isel sees one form:
//! LLVM's `sinkCmpExpression`, `sinkAndCmp0Expression` and `optimizeMemoryInst`
//! (address-mode sinking, below), and GVNSink's merge of equal arms.
//!
//! # How it works
//! Four rewrites, each on one function, none of which touches an edge.
//!
//! A comparison (`icmp` or `fcmp`) read by a branch, select or other user in
//! another block is made again at the top of that block (one copy per block);
//! the original goes once nothing reads it. A phi's incoming block is not a
//! read. isel can then keep the result in the flags instead of an `i1` held
//! across the loop body (`sete`, a store of the byte, `cmp`, `jne`). This runs
//! only where the target has a single condition register
//! (`!target().multiple_condition_registers()`).
//!
//! An `and` that only compares of it with zero read (`x & mask == 0`), some in
//! other blocks than its own, is made again before each of those compares,
//! where the target's lowering says the pair folds
//! (`Lowering::is_mask_and_cmp0_folding_beneficial`) and the copy would not
//! stretch two values read nowhere else (`sinkAndCmp0Expression`). Numbering
//! made one value of the repeated tests of a bit; the compares sunk into the
//! blocks that branch on it left the `and` held across all of them.
//!
//! A phi read only as an address (`spill::address_only`: loads, stores through
//! it, and GEPs of up to three levels that are themselves only addresses) whose
//! arms each compute the same pure operation (`gep`, `zext`, `sext`, `trunc`,
//! `add`, `sub`, `mul`, `shl`, `and`, `or`, `xor`, of equal type and flags, to
//! depth 3) over operands that reach the join (they dominate it) is that
//! computation made once, before the join's first non-phi. What only the phi
//! read, the arms' copies, is erased.
//!
//! Any phi that `simplifyInstruction` reduces to a value (arms all one value,
//! or the phi itself; `optimizeInst`'s phi case, L8947) is that value, whatever
//! reads it; this is how a closing phi left by `lcssa` goes. Each phi is
//! visited once, and the phis that read it are queued again.
//!
//! A phi whose arms differ in kind stays.
//!
//! DEVIATE C1 (m32): the rest of CodeGenPrepare's work is done by isel, which
//! rebuilds an address expression in the block that uses it, and by the select
//! lowering; the width of a switch condition is `cgpswitch`'s
//! (`optimizeSwitchType`); this file is the compare and address-phi part.
//! Evidence, 2026-10-23, per function on the same IR against the pin's `llc`
//! with and without the pass (`docs/working/codegenprepare.md`): at m32 the
//! pass changes 16-17 of about 160 functions in the pin and ours is at its
//! level (at -Os 1287 instructions against 1271 with and 1343 without), and
//! the `lea` count equals the pin's with the pass (2092 against 2139, 3983
//! without, QCport -Os). Owner w-4, review 2026-12-01. At m16 (QCport -Os)
//! ours has fewer `lea` than the pin with the pass (2118 against 2264) and
//! holds 59% of its gain; the residual 1505 instructions are spills and
//! reloads, not addresses: row AM-C2, resolved.
//!
//! # Example
//! Before:
//!
//! ```mir
//! b1:
//!   %k1 = sub i16 %i, -3
//!   %p1 = getelementptr inbounds i16, ptr @g, i16 %k1
//!   store i16 0, ptr %p1
//!   br label %b5
//!
//! ; ...
//!
//! b5:
//!   %p = phi ptr [ %p1, %b1 ], [ %p3, %b3 ], [ %p4, %b4 ]
//!   store i16 %c, ptr %p
//!   ret void
//! ```
//!
//! After:
//!
//! ```mir
//! b1:
//!   %k1 = sub i16 %i, -3
//!   %p1 = getelementptr inbounds i16, ptr @g, i16 %k1
//!   store i16 0, ptr %p1
//!   br label %b5
//!
//! ; ...
//!
//! b5:
//!   %0 = sub i16 %i, -3
//!   %1 = getelementptr inbounds i16, ptr @g, i16 %0
//!   store i16 %c, ptr %1
//!   ret void
//! ```
//!
//! The phi of the three arms' addresses is the address computed once in the
//! join (`b3` and `b4` are as `b1`, with their own stores); the arms' copies
//! stay where their own stores read them.
//!
//! From unit test
//! `test_a_phi_of_one_address_in_every_arm_is_that_address_in_the_join` in
//! `cgpaddr_sink_tests.rs`.
//!
//! # Runs
//! Once, last of the function passes before `spares` and `homes`, after
//! `gepoffset`: until then gvn's partial redundancy elimination makes exactly
//! this phi so a load after the join reads what the arms stored, and an earlier
//! sink would be undone. Off with `-fno-strength-reduce` and at -O0.
//!
//! # Keeps
//! Blocks and edges are unchanged, so dominators and loops are preserved. A
//! change that only replaced trivial phis also keeps the assumption cache and
//! the exposed frame objects; value facts are recomputed.
//!
//! # See also
//! LLVM `CodeGenPrepare::sinkCmpExpression`, `sinkAndCmp0Expression` and
//! `optimizeMemoryInst`; issue
//! #386 (RGBLIGHTS `a(i) = ...` under an `IF`).

//!
//! Address modes (`optimizeMemoryInst`): see [`sunk`]. A `load` or `store`
//! whose address is a `gep` chain made in another block, matched as `base +
//! offset + scale * index` where `isLegalAddressingMode` says the whole is one
//! access, has the address made again just before it; a `select` of two offsets
//! of one base selects the offsets. DEVIATE CGP1: no promoted extensions
//! (`TypePromotionTransaction`), no phi combiner (the phi of equal addresses is
//! merged above) and no matching through
//! `ptrtoint`/`inttoptr`. Exit: those translated.

use std::collections::BTreeMap;

use llrm_analysis::cfg;
use llrm_analysis::manager::{AssumptionCache, ExposedFrames};
use llrm_analysis::num::Num as BigInt;
use llrm_mir::context::{ConstantKind, Context, signed};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use llrm_mir::tti::Tti;
use llrm_mir::types::{Type, TypeId};

use crate::counting;

/// `MaxDepth` of the matcher's recursion
/// ([CodeGenPrepare.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L2998)
/// `matchAddr(V, 5)`): the `gep` chain read goes to this many.
const DEPTH: u32 = 4;

pub struct CgpAddr;

#[cfg(test)]
#[path = "cgpaddr_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "cgpaddr_sink_tests.rs"]
mod sink_tests;

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
        // A closing phi (`lcssa`) whose arms are all one value is that value:
        // nothing after this needs loop-closed form, and isel cannot
        // fold a phi into an addressing mode.
        let merged = crate::emptyblocks::eliminated(unit.context, unit.function);
        // `dupRetToEnableTailCallOpts`: a return after a tail call is the
        // call's.
        let merged =
            crate::cgpretdup::returns_duplicated(unit.context, &analyses.outer().globals, unit.function) | merged;
        let closed = trivial_phis(unit.context, unit.layout, unit.function);
        let branched = crate::cgpbranch::branches_optimized(
            unit.context,
            unit.function,
            analyses.outer().target().lowering().as_deref(),
        );
        // `optimizeSwitchInst`'s `optimizeSwitchType`.
        let switches = match analyses.outer().target().lowering() {
            Some(lowering) => {
                crate::cgpswitch::switch_types_widened(unit.context, unit.layout, &*lowering, unit.function)
            }
            None => false,
        };
        // Blocks and edges are as they were.
        let one_flag = !analyses.outer().target().multiple_condition_registers();
        let sunk_compares = one_flag && compares_sunk(unit.context, &analyses.outer().globals, unit.function);
        let compares = match analyses.outer().target().lowering() {
            Some(lowering) => {
                let outer = std::rc::Rc::clone(analyses.outer());
                crate::cmpopt::compares_optimized(unit.context, &outer.globals, unit.layout, &*lowering, unit.function)
            }
            None => false,
        };
        let masks =
            one_flag && masks_sunk(unit.context, unit.function, analyses.outer().target().lowering().as_deref());
        let phis = phis_sunk(unit.function);
        // `optimizeCallInst`'s memory intrinsics: their pointers' known
        // alignment.
        let aligned = {
            let outer = std::rc::Rc::clone(analyses.outer());
            crate::cgpmemalign::raised(unit.context, unit.layout, &outer.globals, unit.function)
        };
        let sunk_casts = match analyses.outer().target().lowering() {
            Some(lowering) => crate::sinkcast::casts_sunk(
                unit.context,
                unit.layout,
                &*lowering,
                analyses.outer().target().spaces(),
                unit.function,
                false,
            ),
            None => false,
        };
        let extended = match analyses.outer().target().lowering() {
            Some(lowering) => {
                let mut state = crate::extpromotion::State::default();
                let promoted = exts_optimized(unit, &*lowering, &mut state);
                ext_uses(unit, &*lowering, &mut state) | promoted
            }
            None => false,
        };
        // `optimizeInst`'s urem case: the remainder of a loop counter.
        let remainders = crate::uremloop::remainders_folded(
            unit.context,
            unit.layout,
            |function| {
                let info = analyses.get::<Loops>(&*unit.context, unit.layout, function);
                llrm_analysis::cfg::natural(function, &info)
            },
            unit.function,
        );
        // `optimizeInst`'s store case: a merged store is split before its
        // address is sunk.
        let split = match analyses.outer().target().lowering() {
            Some(lowering) => {
                crate::splitstore::merged_stores_split(unit.context, unit.layout, &*lowering, unit.function)
            }
            None => false,
        };
        let addresses = match analyses.outer().target().tti() {
            Some(tti) => sunk(unit, &*tti),
            None => false,
        };
        // An address sunk into a loop reads the cast from where it was; made
        // again there, the far pointer's zero offset folds into the address.
        let resunk = addresses
            && match analyses.outer().target().lowering() {
                Some(lowering) => crate::sinkcast::casts_sunk(
                    unit.context,
                    unit.layout,
                    &*lowering,
                    analyses.outer().target().spaces(),
                    unit.function,
                    true,
                ),
                None => false,
            };
        if merged {
            // A block went: the blocks and edges are not as they were.
            PreservedAnalyses::none()
        } else if phis
            || aligned
            || branched
            || sunk_compares
            || masks
            || addresses
            || extended
            || sunk_casts
            || resunk
            || split
            || remainders
            || compares
            || switches
        {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else if closed {
            // A phi's value is replaced by the one it equalled: the blocks and
            // loops stand, and what is said of an assumption or of
            // the frame objects reads no phi. What is said of values (their
            // facts, a loop's bound, the annotations, the edges a
            // condition dominates) may name the phi, and is worked out again.
            PreservedAnalyses::none()
                .preserve::<ExposedFrames>()
                .preserve::<Dominators>()
                .preserve::<Loops>()
                .preserve::<AssumptionCache>()
        } else {
            PreservedAnalyses::all()
        }
    }
}
/// `optimizeExt` for each `zext` and `sext` of the function whose type is a
/// register of the target (the others are `SinkCast`'s); whether any moved.
fn exts_optimized(
    unit: &mut passes::Unit,
    lowering: &dyn llrm_mir::lowering::Lowering,
    state: &mut crate::extpromotion::State,
) -> bool {
    let exts: Vec<InstId> = unit
        .function
        .walk()
        .filter(|&(_, inst)| {
            matches!(
                unit.function.instruction(inst).opcode,
                Opcode::Cast(CastOp::ZExt | CastOp::SExt)
            )
        })
        .map(|(_, inst)| inst)
        .collect();
    let mut changed = false;
    for ext in exts {
        if unit.function.is_erased(ext) {
            continue;
        }
        let one = unit.function.instruction(ext);
        if matches!(one.operands[0], Operand::Constant(_))
            || !unit.context.types.int_bits(one.ty).is_some_and(|bits| lowering.is_type_legal(bits))
        {
            continue;
        }
        changed |= crate::extpromotion::optimize_ext(unit.context, unit.layout, lowering, unit.function, state, ext);
    }
    changed
}

/// `optimizeExtUses`
/// ([CodeGenPrepare.cpp#L7361](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7361)):
/// where a `zext` or `sext` and its source are both live out of the source's
/// block, the other blocks that read the source read a `trunc` of the
/// extension instead, one for each block; only where truncating is free
/// (`TargetLowering::isTruncateFree`). Whether any was made.
///
/// MIRROR: every reader of the source outside the extension's block is
/// rewritten, as the pin does. A reader of a value made in the extension's
/// block is dominated by it, so the extension reaches it, unless the reader is
/// unreachable; the pin deletes unreachable blocks before it gets here
/// (`removeUnreachableBlocks`), and `llc -stop-after=codegenprepare` on a
/// source read in a dead block shows it gone, so no pin input has the case.
fn ext_uses(
    unit: &mut passes::Unit,
    lowering: &dyn llrm_mir::lowering::Lowering,
    state: &mut crate::extpromotion::State,
) -> bool {
    let exts: Vec<(BlockId, InstId)> = unit
        .function
        .walk()
        .filter(|&(_, inst)| {
            matches!(
                unit.function.instruction(inst).opcode,
                Opcode::Cast(CastOp::ZExt | CastOp::SExt)
            )
        })
        .collect();
    let mut changed = false;
    for (home, ext) in exts {
        let inst = unit.function.instruction(ext).clone();
        let (Operand::Value(source), Some(result)) = (inst.operands[0], inst.result) else { continue };
        // A source read once has no other reader to move.
        if unit.function.users(source).has_n_uses(1) {
            continue;
        }
        let (Some(from), Some(to)) = (
            unit.function.operand_type(unit.context, inst.operands[0]).and_then(|ty| unit.context.types.int_bits(ty)),
            unit.context.types.int_bits(inst.ty),
        ) else {
            continue;
        };
        if !lowering.is_truncate_free(to, from) {
            continue;
        }
        // Only safe where the source is made in this block.
        let ValueDef::Instruction(made) = unit.function.value(source).def else { continue };
        if unit.function.parent(made) != Some(home) {
            continue;
        }
        let live_out = unit.function.users(result).iter().any(|one| unit.function.parent(one.user) != Some(home));
        if !live_out {
            continue;
        }
        // Be conservative: no phi, load or store of the source elsewhere.
        let readers: Vec<(InstId, u32)> = unit.function.users(source).iter().map(|one| (one.user, one.index)).collect();
        let elsewhere = |reader: &InstId| unit.function.parent(*reader).is_some_and(|block| block != home);
        if readers.iter().filter(|(reader, _)| elsewhere(reader)).any(|(reader, _)| {
            matches!(
                unit.function.instruction(*reader).opcode,
                Opcode::Phi | Opcode::Load { .. } | Opcode::Store { .. }
            )
        }) {
            continue;
        }
        let mut truncs: Vec<(BlockId, Operand)> = Vec::new();
        for (reader, index) in readers {
            let Some(block) = unit.function.parent(reader) else { continue };
            if block == home {
                continue;
            }
            let trunc = match truncs.iter().find(|(seen, _)| *seen == block) {
                Some(&(_, trunc)) => trunc,
                None => {
                    let first = unit
                        .function
                        .iter_block(block)
                        .find(|&one| unit.function.instruction(one).opcode != Opcode::Phi)
                        .expect("a terminated block");
                    let made = unit
                        .function
                        .create_instruction(
                            Opcode::Cast(CastOp::Trunc),
                            unit.function.value(source).ty,
                            vec![Operand::Value(result)],
                            Flags::default(),
                            None,
                        );
                    unit.function
                        .insert(made, Position::Before(first))
                        .expect("a position before the first instruction");
                    state.inserted.insert(made);
                    let trunc = Operand::Value(unit.function.instruction(made).result.expect("a value"));
                    truncs.push((block, trunc));
                    trunc
                }
            };
            unit.function.set_operand(reader, index as usize, trunc);
            changed = true;
        }
    }
    changed
}

/// `base + offset + scale * index`, the instructions that made it, and what it
/// is made of.
struct Mode {
    base: Operand,
    /// A second register, scale 1, under a global base (`BaseGV` with `BaseReg`
    /// and `ScaledReg`: `@g + reg + scale * index`).
    reg: Option<Operand>,
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

/// Whether folding `address` (made from the register `held` and a global)
/// into an access in `block` extends no live range that is not folded away:
/// `held` is read in `block`, or `address` is read only as memory addresses.
fn register_free(
    unit: &passes::Unit,
    block: BlockId,
    held: Operand,
    address: Operand,
) -> bool {
    let live = match held {
        Operand::Value(v) => unit.function.users(v).iter().any(|one| unit.function.parent(one.user) == Some(block)),
        _ => true,
    };
    live || matches!(
        address,
        Operand::Value(v) if crate::spill::address_only(unit.function, v, DEPTH)
    )
}

fn is_global(
    context: &Context,
    operand: Operand,
) -> bool {
    matches!(
        operand,
        Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Global(_))
    )
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

/// `index` as `shl x, C` folded into `scale` (the pin's `matchOperationAddr`
/// `Shl` case through `matchScaledValue`): the `shl`, `x` and the new scale,
/// when the scaled mode is legal and folding extends no live range
/// (`isProfitableToFoldIntoAddressingMode`: `x` is already used in the
/// access's block, or every use of the `shl` is an access that folds it).
/// DEVIATE SHL2:
/// not where a scaled index costs instructions of its own
/// (`scaled_index_insns`, real mode's prefix): each access would pay them,
/// where the shift is made once (quicksort at m16 grew 21 bytes, a spill for
/// the longer-lived index).
fn shifted(
    unit: &passes::Unit,
    tti: &dyn Tti,
    space: u32,
    block: BlockId,
    index: Operand,
    scale: u64,
    displacement: i128,
) -> Option<(InstId, Operand, u64)> {
    let Operand::Value(value) = index else { return None };
    let ValueDef::Instruction(shl) = unit.function.value(value).def else { return None };
    let one = unit.function.instruction(shl);
    if tti.scaled_index_insns() != 0 || one.opcode != Opcode::Binary(BinaryOp::Shl) {
        return None;
    }
    let &[x, amount] = &one.operands[..] else { return None };
    let amount = u32::try_from(integer(unit.context, amount)?).ok().filter(|&amount| amount < 64)?;
    let scaled = scale.checked_shl(amount).filter(|&scaled| scaled >> amount == scale)?;
    if !tti.is_legal_addressing_mode_in(
        space,
        false,
        i64::try_from(displacement).ok()?,
        true,
        i64::try_from(scaled).ok()?,
    ) {
        return None;
    }
    // `valueAlreadyLiveAtInst(x)`: a constant, or read in the access's block.
    let live = match x {
        Operand::Value(v) => unit.function.users(v).iter().any(|one| unit.function.parent(one.user) == Some(block)),
        _ => true,
    };
    // `isProfitableToFoldIntoAddressingMode`, the last case: every use of the
    // `shl` is a memory access that folds it, so it goes with them.
    let folds = unit
        .function
        .instruction(shl)
        .result
        .is_some_and(|result| crate::spill::address_only(unit.function, result, DEPTH));
    (live || folds).then_some((shl, x, scaled))
}

/// `matchAddr`: `address` read as a mode, `depth` more `gep`s allowed.
fn matched(
    unit: &passes::Unit,
    tti: &dyn Tti,
    space: u32,
    block: BlockId,
    address: Operand,
    depth: u32,
) -> Mode {
    let plain = || Mode { base: address, reg: None, offset: 0, index: None, insts: Vec::new() };
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
    let inner = matched(unit, tti, space, block, instruction.operands[0], depth - 1);
    let mut insts = inner.insts;
    let mut reg = inner.reg;
    let index = match (variable.first(), inner.index) {
        // `BaseGV + BaseReg + ScaledReg`: the earlier index is a register of
        // scale 1 beside the global.
        // `isProfitableToFoldIntoAddressingMode`: only when the register is
        // read in the access's block already, or every read of the address it
        // makes is a memory access that folds it too.
        (Some(&(at, scale)), Some((held, 1)))
            if reg.is_none()
                && is_global(unit.context, inner.base)
                && register_free(unit, block, held, instruction.operands[0]) =>
        {
            reg = Some(held);
            Some((instruction.operands[1 + at], scale))
        }
        (Some(_), Some(_)) => return plain(),
        (Some(&(at, scale)), None) => {
            let operand = instruction.operands[1 + at];
            // `matchScaledValue(index, scale)` of a `shl x, C`: the shift is
            // the scale of the mode (`matchOperationAddr`, `Instruction::Shl`).
            match shifted(unit, tti, space, block, operand, scale, offset + inner.offset) {
                Some((shl, folded, scale)) => {
                    insts.push(shl);
                    Some((folded, scale))
                }
                None => Some((operand, scale)),
            }
        }
        (None, held) => held,
    };
    let total = offset + inner.offset;
    let scale = index.map_or(0, |(_, scale)| i64::try_from(scale).unwrap_or(i64::MAX));
    let Ok(displacement) = i64::try_from(total) else { return plain() };
    if !tti.is_legal_addressing_mode_in(space, reg.is_some(), displacement, true, scale) {
        return plain();
    }
    insts.push(gep);
    Mode { base: inner.base, reg, offset: total, index, insts }
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
        if let Some(ready) = select_combined(unit, tti, space, block, access, address, &mut made, &mut touched) {
            unit.function.set_operand(access, at, ready);
            changed = true;
            continue;
        }
        let mode = matched(unit, tti, space, block, address, DEPTH);
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

/// `AddressingModeCombiner` for a `select` of two addresses that differ only in
/// their constant offset from one base: the offsets are selected and the access
/// adds the one to the base (`select c, base+k1, base+k2` is `base + select c,
/// k1, k2`), made before `access` when either arm is made in another block.
fn select_combined(
    unit: &mut passes::Unit,
    tti: &dyn Tti,
    space: u32,
    block: BlockId,
    access: InstId,
    address: Operand,
    made: &mut BTreeMap<(BlockId, Operand), Operand>,
    touched: &mut Vec<InstId>,
) -> Option<Operand> {
    if let Some(&ready) = made.get(&(block, address)) {
        return Some(ready);
    }
    let Operand::Value(value) = address else { return None };
    let ValueDef::Instruction(select) = unit.function.value(value).def else { return None };
    let chosen = unit.function.instruction(select).clone();
    if chosen.opcode != Opcode::Select {
        return None;
    }
    let (condition, first, second) = (chosen.operands[0], chosen.operands[1], chosen.operands[2]);
    let (one, other) =
        (matched(unit, tti, space, block, first, DEPTH), matched(unit, tti, space, block, second, DEPTH));
    if one.base != other.base
        || one.reg.is_some()
        || other.reg.is_some()
        || one.index.is_some()
        || other.index.is_some()
        || one.offset == other.offset
    {
        return None;
    }
    if !tti.is_legal_addressing_mode_in(space, false, 0, true, 1) {
        return None;
    }
    let local = |inst: &InstId| unit.function.parent(*inst) == Some(block);
    if unit.function.parent(select) == Some(block) && one.insts.iter().chain(&other.insts).all(local) {
        return None;
    }
    let bits = unit.layout.pointer(space).index_bits;
    let wide = unit.context.types.int(bits);
    let pointer = unit.function.operand_type(unit.context, one.base).expect("a typed base");
    let i8 = unit.context.types.int(8);
    let (first_offset, second_offset) = (
        counting::constant(unit.context, &BigInt::from(one.offset), bits),
        counting::constant(unit.context, &BigInt::from(other.offset), bits),
    );
    let put = |unit: &mut passes::Unit, opcode: Opcode, ty, operands: Vec<Operand>| -> Operand {
        let inst = unit.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        unit.function.insert(inst, Position::Before(access)).expect("a position before the access");
        Operand::Value(unit.function.instruction(inst).result.expect("a value"))
    };
    let offset = put(unit, Opcode::Select, wide, vec![condition, first_offset, second_offset]);
    let ready = put(unit, Opcode::GetElementPtr { source: i8 }, pointer, vec![one.base, offset]);
    made.insert((block, address), ready);
    touched.extend(one.insts);
    touched.extend(other.insts);
    touched.push(select);
    Some(ready)
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
    for (index, scale) in mode.reg.map(|reg| (reg, 1)).into_iter().chain(mode.index) {
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

/// CodeGenPrepare's `optimizeInst` phi case
/// ([L8947](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/CodeGenPrepare.cpp#L8947)):
/// each phi that `simplifyInstruction` reduces to a value is replaced by it,
/// and the phis that read it looked at again; whether any was. One visit of
/// each phi, not a pass over the function until nothing changes.
fn trivial_phis(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
) -> bool {
    let mut pending: Vec<InstId> = function
        .walk()
        .filter(|&(_, inst)| function.instruction(inst).opcode == Opcode::Phi)
        .map(|(_, inst)| inst)
        .collect();
    let mut changed = false;
    while let Some(phi) = pending.pop() {
        if function.is_erased(phi) {
            continue;
        }
        let Some(result) = function.instruction(phi).result else { continue };
        let Some(value) = llrm_mir::simplify::simplify_instruction(context, layout, function, phi) else { continue };
        if value == Operand::Value(result) {
            continue;
        }
        pending.extend(
            function
                .users(result)
                .iter()
                .map(|one| one.user)
                .filter(|&user| user != phi && function.instruction(user).opcode == Opcode::Phi),
        );
        function.replace_all_uses_with(result, value);
        function.erase(phi).expect("its uses were replaced");
        changed = true;
    }
    changed
}

/// What the join makes of its phi's arms: a value every arm reads, or an
/// instruction of one pure kind over merged operands.
enum Merged {
    Shared(Operand),
    Made(Opcode, TypeId, Flags, Vec<Merged>),
}

fn phis_sunk(function: &mut Function) -> bool {
    let mut changed = false;
    // Blocks and edges stay as they are: found once, for every phi.
    let dominance = cfg::Dominance::of(function);
    for (block, phi) in
        function.walk().filter(|&(_, inst)| function.instruction(inst).opcode == Opcode::Phi).collect::<Vec<_>>()
    {
        let Some(result) = function.instruction(phi).result else { continue };
        let arms = function.instruction(phi).operands.chunks(2).map(|pair| pair[0]).collect::<Vec<_>>();
        if arms.len() < 2 || arms.iter().all(|one| *one == arms[0]) || !crate::spill::address_only(function, result, 3)
        {
            continue;
        }
        let Some(plan) = merged(function, &dominance, block, &arms, 3) else { continue };
        let at = function
            .iter_block(block)
            .find(|&one| function.instruction(one).opcode != Opcode::Phi)
            .expect("a terminated block");
        let made = built(function, &plan, at);
        function.replace_all_uses_with(result, made);
        function.erase(phi).expect("its uses were replaced");
        // What only the phi read is dead: an arm's copy of the address.
        let mut orphans = arms;
        while let Some(operand) = orphans.pop() {
            let Operand::Value(value) = operand else { continue };
            let ValueDef::Instruction(def) = function.value(value).def else { continue };
            let op = function.instruction(def);
            if !function.users(value).is_empty()
                || !matches!(
                    op.opcode,
                    Opcode::GetElementPtr { .. }
                        | Opcode::Cast(CastOp::ZExt | CastOp::SExt | CastOp::Trunc)
                        | Opcode::Binary(
                            BinaryOp::Add
                                | BinaryOp::Sub
                                | BinaryOp::Mul
                                | BinaryOp::Shl
                                | BinaryOp::And
                                | BinaryOp::Or
                                | BinaryOp::Xor
                        )
                )
                || function.is_erased(def)
            {
                continue;
            }
            orphans.extend(op.operands.clone());
            function.erase(def).expect("no one reads it");
        }
        changed = true;
    }
    changed
}

/// LLVM's `sinkCmpExpression`: each comparison made again at the top of every
/// block that reads it and is not its own (a phi's incoming block is not a
/// read), and erased once nothing reads the original.
///
/// DEVIATE: a compare of a runtime call's flags result stays by the call; the
/// flags do not survive to another block.
fn compares_sunk(
    context: &Context,
    globals: &[GlobalValue],
    function: &mut Function,
) -> bool {
    let compares: Vec<(BlockId, InstId)> = function
        .walk()
        .filter(|&(_, inst)| matches!(
            function.instruction(inst).opcode,
            Opcode::ICmp(_) | Opcode::FCmp(_)
        ))
        .collect();
    let mut changed = false;
    for (home, compare) in compares {
        let Some(result) = function.instruction(compare).result else { continue };
        if llrm_mir::facts::compares_flags_result(context, globals, function, compare) {
            continue;
        }
        let mut made: Vec<(BlockId, Operand)> = Vec::new();
        for one in function.users(result).snapshot() {
            if function.instruction(one.user).opcode == Opcode::Phi {
                continue;
            }
            let Some(there) = function.parent(one.user) else { continue };
            if there == home {
                continue;
            }
            let copy = match made.iter().find(|(block, _)| *block == there) {
                Some((_, copy)) => *copy,
                None => {
                    let original = function.instruction(compare);
                    let (opcode, ty, operands, flags) =
                        (original.opcode.clone(), original.ty, original.operands.clone(), original.flags);
                    let first = function
                        .iter_block(there)
                        .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
                        .expect("a terminated block");
                    let inserted = function.create_instruction(opcode, ty, operands, flags, None);
                    function.insert(inserted, Position::Before(first)).expect("a placed instruction");
                    let copy = Operand::Value(function.instruction(inserted).result.expect("a value"));
                    made.push((there, copy));
                    copy
                }
            };
            let mut operands = function.instruction(one.user).operands.clone();
            operands[one.index as usize] = copy;
            function.set_operands(one.user, operands);
            changed = true;
        }
        if function.users(result).is_empty() && !function.is_erased(compare) {
            function.erase(compare).expect("no one reads it");
        }
    }
    changed
}

/// LLVM's `sinkAndCmp0Expression`: an `and` read only by compares of it with
/// zero, some in other blocks, is made again before each of those compares
/// that is not in its block, and the original goes once nothing reads it. It
/// is `compares_sunk`'s other half: with the compare made in each block the
/// `and` it reads was one value held from its block to the last of them (800
/// diamonds held 31 such values across up to 30 diamonds: 419 stack operands
/// where a test at each branch has none).
///
/// Only where the target's lowering says the pair folds
/// (`isMaskAndCmp0FoldingBeneficial`; none: the base class's `false`), and not
/// where both operands of the `and` are values read nowhere else, whose live
/// ranges the copy would stretch to the compare (CodeGenPrepare.cpp:2224).
fn masks_sunk(
    context: &Context,
    function: &mut Function,
    lowering: Option<&dyn llrm_mir::lowering::Lowering>,
) -> bool {
    if !lowering.is_some_and(|one| one.is_mask_and_cmp0_folding_beneficial()) {
        return false;
    }
    let masks: Vec<(BlockId, InstId)> = function
        .walk()
        .filter(|&(_, inst)| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::And))
        .collect();
    let mut changed = false;
    for (home, mask) in masks {
        let Some(result) = function.instruction(mask).result else { continue };
        let read_elsewhere = |operand: &Operand| match operand {
            Operand::Value(value) => function.users(*value).has_n_uses_or_more(2),
            _ => true,
        };
        let operands = &function.instruction(mask).operands;
        if !operands.iter().any(|operand| matches!(operand, Operand::Constant(_)) || read_elsewhere(operand)) {
            continue;
        }
        let users = function.users(result).snapshot();
        let against_zero = |user: &llrm_mir::module::Use| {
            let one = function.instruction(user.user);
            matches!(one.opcode, Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne))
                && user.index == 0
                && matches!(
                    one.operands[1],
                    Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(0))
                )
        };
        if users.is_empty() || !users.iter().all(against_zero) {
            continue;
        }
        let mut made: Vec<(BlockId, Operand)> = Vec::new();
        for one in users {
            let Some(there) = function.parent(one.user) else { continue };
            if there == home {
                continue;
            }
            let copy = match made.iter().find(|(block, _)| *block == there) {
                Some((_, copy)) => *copy,
                None => {
                    // Before the first of this block's compares.
                    let first = function
                        .iter_block(there)
                        .find(|&inst| function.users(result).iter().any(|user| user.user == inst))
                        .expect("a compare in its block");
                    let original = function.instruction(mask);
                    let (opcode, ty, operands, flags) =
                        (original.opcode.clone(), original.ty, original.operands.clone(), original.flags);
                    let inserted = function.create_instruction(opcode, ty, operands, flags, None);
                    function.insert(inserted, Position::Before(first)).expect("a placed instruction");
                    let copy = Operand::Value(function.instruction(inserted).result.expect("a value"));
                    made.push((there, copy));
                    copy
                }
            };
            let mut operands = function.instruction(one.user).operands.clone();
            operands[one.index as usize] = copy;
            function.set_operands(one.user, operands);
            changed = true;
        }
        if function.users(result).is_empty() && !function.is_erased(mask) {
            function.erase(mask).expect("no one reads it");
        }
    }
    changed
}

/// The arms as one `Merged`, if they are one computation over values that reach
/// `join`.
fn merged(
    function: &Function,
    dominance: &cfg::Dominance,
    join: BlockId,
    arms: &[Operand],
    depth: u32,
) -> Option<Merged> {
    let reaches = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => {
                function.parent(def).is_some_and(|block| dominance.dominates(function, cfg::id(block), cfg::id(join)))
            }
            ValueDef::Argument(_) => true,
        },
        _ => true,
    };
    if arms.iter().all(|one| *one == arms[0]) {
        return reaches(arms[0]).then_some(Merged::Shared(arms[0]));
    }
    let defined = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => Some(def),
            ValueDef::Argument(_) => None,
        },
        _ => None,
    };
    let made = arms.iter().map(|&one| defined(one)).collect::<Option<Vec<InstId>>>()?;
    let first = function.instruction(made[0]);
    let pure = matches!(
        first.opcode,
        Opcode::GetElementPtr { .. }
            | Opcode::Cast(CastOp::ZExt | CastOp::SExt | CastOp::Trunc)
            | Opcode::Binary(
                BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Shl
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Xor
            )
    );
    if depth == 0
        || !pure
        || made.iter().any(|&one| {
            let other = function.instruction(one);
            other.opcode != first.opcode
                || other.ty != first.ty
                || other.flags != first.flags
                || other.operands.len() != first.operands.len()
        })
    {
        return None;
    }
    let operands = (0..first.operands.len())
        .map(|at| {
            merged(
                function,
                dominance,
                join,
                &made.iter().map(|&one| function.instruction(one).operands[at]).collect::<Vec<_>>(),
                depth - 1,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Merged::Made(first.opcode.clone(), first.ty, first.flags, operands))
}

/// `plan` made before `at`.
fn built(
    function: &mut Function,
    plan: &Merged,
    at: InstId,
) -> Operand {
    match plan {
        Merged::Shared(operand) => *operand,
        Merged::Made(opcode, ty, flags, operands) => {
            let operands = operands.iter().map(|one| built(function, one, at)).collect();
            let made = function.create_instruction(opcode.clone(), *ty, operands, *flags, None);
            function.insert(made, Position::Before(at)).expect("a placed instruction");
            Operand::Value(function.instruction(made).result.expect("a value"))
        }
    }
}
