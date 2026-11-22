//! CodeGenPrepare's `splitLargeGEPOffsets`: the constant-offset GEPs of one
//! base that the target cannot fold into an address are made from one new base.
//! [CodeGenPrepare.cpp#L6902](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L6902)
//!
//! # How it works
//! A load or store whose address is a `gep` of constant indices off an
//! argument, a global or an instruction that is no cast or `gep`, at a
//! positive offset the target's addressing mode does not take, is recorded
//! under that base (`matchOperationAddr`, the `EnableGEPOffsetSplit` arm). Per
//! base the `gep`s sort by offset; where they do not all share one, a new base
//! `gep i8, base, B` is made (at the base's definition, or in the entry block
//! for an argument or a global) and each `gep` becomes `gep i8, new, offset -
//! B`, as long as that fits the addressing mode: a farther one starts a new
//! base. `B` is the target's `getPreferredLargeGEPBaseOffset` of the smallest
//! and largest offset where it names one, else the smallest offset itself.
//!
//! # Runs
//! In `cgpaddr`, for a target whose `shouldConsiderGEPOffsetSplit` is true
//! (arm64).
//!
//! # Example
//! A struct field at 11936 and others at 12938 and 12944 become one `add` of
//! 11936 and offsets 0, 1002 and 1008 from it, where each was a `mov` of its
//! own offset and a register-offset access.
//!
//! ```text
//! %a = gep i8, %s, 11936      %n = gep i8, %s, 11936
//! %b = gep i8, %s, 12938  =>  %b = gep i8, %n, 1002
//! %c = gep i8, %s, 12944      %c = gep i8, %n, 1008
//! ```

use llrm_analysis::num::Num as BigInt;
use llrm_mir::context::{ConstantKind, Context, signed};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::tti::Tti;
use llrm_mir::types::{Type, TypeId};

use crate::counting;

/// A recorded `gep`: where it is, its offset and the order it was found in
/// (`LargeOffsetGEPID`).
#[derive(Clone, Copy)]
struct Large {
    gep: InstId,
    offset: i64,
    id: usize,
}

/// The type a `gep` of `source` with these constant indices addresses
/// (`GetElementPtrInst::getResultElementType`).
fn element_type(
    context: &Context,
    source: TypeId,
    indices: &[Option<i128>],
) -> Option<TypeId> {
    let types = &context.types;
    let mut ty = source;
    for index in indices.iter().skip(1) {
        ty = match types.get(ty) {
            Type::Array { element, .. } => *element,
            _ => *types.fields(ty)?.get(usize::try_from((*index)?).ok()?)?,
        };
    }
    Some(ty)
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

fn is_global(
    context: &Context,
    operand: Operand,
) -> bool {
    matches!(
        operand,
        Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Global(_))
    )
}

/// The `gep`s of `function` the target cannot fold, by base, in the order
/// found.
fn collected(
    context: &Context,
    layout: &DataLayout,
    tti: &dyn Tti,
    function: &Function,
) -> Vec<(Operand, Vec<Large>)> {
    let mut groups: Vec<(Operand, Vec<Large>)> = Vec::new();
    let mut ids: Vec<InstId> = Vec::new();
    for (_, access) in function.walk() {
        let instruction = function.instruction(access);
        let (address, bytes) = match instruction.opcode {
            Opcode::Load { .. } => (instruction.operands[0], layout.store_size(&context.types, instruction.ty)),
            Opcode::Store { .. } => {
                let Some(ty) = function.operand_type(context, instruction.operands[0]) else { continue };
                (instruction.operands[1], layout.store_size(&context.types, ty))
            }
            _ => continue,
        };
        let Operand::Value(value) = address else { continue };
        let ValueDef::Instruction(gep) = function.value(value).def else { continue };
        let Opcode::GetElementPtr { source } = function.instruction(gep).opcode else { continue };
        let operands = &function.instruction(gep).operands;
        let known: Vec<Option<i128>> = operands[1..]
            .iter()
            .map(|&one| {
                integer(context, one).map(|bits| {
                    let width =
                        function.operand_type(context, one).and_then(|ty| context.types.int_bits(ty)).unwrap_or(64);
                    signed(bits, width)
                })
            })
            .collect();
        let (offset, variable) = layout.collect_offset(&context.types, source, &known);
        let Ok(offset) = i64::try_from(offset) else { continue };
        if !variable.is_empty() || offset <= 0 {
            continue;
        }
        let base = operands[0];
        // The base is an argument, a global, or an instruction that is no cast
        // or `gep`, in a block that can take a non-phi before its terminator.
        let usable = match base {
            Operand::Value(base) => match function.value(base).def {
                ValueDef::Argument(_) => true,
                ValueDef::Instruction(def) => {
                    !matches!(
                        function.instruction(def).opcode,
                        Opcode::Cast(_) | Opcode::GetElementPtr { .. }
                    )
                }
            },
            _ => is_global(context, base),
        };
        if !usable {
            continue;
        }
        let space = match context.types.get(instruction.ty) {
            _ => 0,
        };
        let folds = if is_global(context, base) {
            tti.is_legal_addressing_mode_at(space, bytes, true, offset, false, 0)
        } else {
            tti.is_legal_addressing_mode_at(space, bytes, false, offset, true, 0)
        };
        if folds {
            continue;
        }
        let group = match groups.iter().position(|(one, _)| *one == base) {
            Some(at) => at,
            None => {
                groups.push((base, Vec::new()));
                groups.len() - 1
            }
        };
        let id = match ids.iter().position(|one| *one == gep) {
            Some(at) => at,
            None => {
                ids.push(gep);
                ids.len() - 1
            }
        };
        groups[group].1.push(Large { gep, offset, id });
    }
    groups
}

/// Where a new base of `base` goes: after its definition, or the entry block's
/// first non-phi for an argument or a global; none where the base is an invoke.
fn insertion(
    function: &Function,
    base: Operand,
) -> Option<Position> {
    let first_non_phi = |block| {
        function.iter_block(block).find(|&inst| function.instruction(inst).opcode != Opcode::Phi).map(Position::Before)
    };
    match base {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => match function.instruction(def).opcode {
                Opcode::Phi => first_non_phi(function.parent(def)?),
                Opcode::Invoke(_) => None,
                _ => Some(Position::After(def)),
            },
            ValueDef::Argument(_) => first_non_phi(function.entry()?),
        },
        _ => first_non_phi(function.entry()?),
    }
}

/// `splitLargeGEPOffsets`; whether any `gep` was remade.
pub fn large_offsets_split(
    context: &Context,
    layout: &DataLayout,
    tti: &dyn Tti,
    function: &mut Function,
) -> bool {
    if !tti.should_consider_gep_offset_split() {
        return false;
    }
    let groups = collected(context, layout, tti, function);
    let i8 = context.types.int(8);
    let index_bits = layout.pointer(0).index_bits;
    let mut changed = false;
    for (base, mut gep_list) in groups {
        gep_list.sort_by_key(|one| (one.offset, one.id));
        gep_list.dedup_by_key(|one| (one.gep, one.offset));
        let (front, back) = (gep_list[0], gep_list[gep_list.len() - 1]);
        // "Skip if all the GEPs have the same offsets."
        if front.offset == back.offset {
            continue;
        }
        if insertion(function, base).is_none() {
            continue;
        }
        let Some(pointer) = function.operand_type(context, base) else { continue };
        let mut base_offset = front.offset;
        let mut new_base: Option<Operand> = None;
        let make = |function: &mut Function, offset: i64| -> Operand {
            let index = counting::constant(context, &BigInt::from(offset), index_bits);
            let made = function.create_instruction(
                Opcode::GetElementPtr { source: i8 },
                pointer,
                vec![base, index],
                Flags::default(),
                None,
            );
            // Where the base's definition, or the entry block's first non-phi,
            // is now.
            let position = insertion(function, base).expect("a position for the new base");
            function.insert(made, position).expect("a position for the new base");
            Operand::Value(function.instruction(made).result.expect("a value"))
        };
        // "Check whether all the offsets can be encoded with prefered common
        // base."
        let preferred = tti.preferred_large_gep_base_offset(front.offset, back.offset);
        if preferred != 0 {
            base_offset = preferred;
            new_base = Some(make(function, base_offset));
        }
        for large in gep_list {
            let Large { gep, offset, .. } = large;
            if function.is_erased(gep) {
                continue;
            }
            let Opcode::GetElementPtr { source } = function.instruction(gep).opcode else { continue };
            if offset != base_offset {
                let known: Vec<Option<i128>> = function.instruction(gep).operands[1..]
                    .iter()
                    .map(|&one| integer(context, one).map(|bits| bits as i128))
                    .collect();
                let element = element_type(context, source, &known).unwrap_or(i8);
                let bytes = layout.store_size(&context.types, element);
                // "We need to create a new base if the offset to the current
                // base is too large to fit into the addressing
                // mode."
                if !tti.is_legal_addressing_mode_at(0, bytes, false, offset - base_offset, true, 0) {
                    base_offset = offset;
                    new_base = None;
                }
            }
            let new_base = *new_base.get_or_insert_with(|| make(function, base_offset));
            let replacement = if offset != base_offset {
                let index = counting::constant(context, &BigInt::from(offset - base_offset), index_bits);
                let made = function.create_instruction(
                    Opcode::GetElementPtr { source: i8 },
                    pointer,
                    vec![new_base, index],
                    Flags::default(),
                    None,
                );
                function.insert(made, Position::Before(gep)).expect("a position before the replaced gep");
                Operand::Value(function.instruction(made).result.expect("a value"))
            } else {
                new_base
            };
            let result = function.instruction(gep).result.expect("a value");
            function.replace_all_uses_with(result, replacement);
            function.erase(gep).expect("a gep nothing reads goes");
            changed = true;
        }
    }
    changed
}
