//! CodeGenPrepare's `optimizePhiTypes`: a web of phis that only loads, bitcasts
//! and stores touch is made the type of its bitcasts
//! ([CodeGenPrepare.cpp#L7168](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7168),
//! `optimizePhiType` at
//! [L7026](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7026)).
//!
//! # How it works
//! From each phi, the interconnected phis are gathered with what feeds them
//! (loads, bitcasts, constants) and what reads them (stores of the phi,
//! bitcasts). Anything else in the web, a volatile access or a non-load source,
//! ends the attempt. When the bitcasts all go to one type, and at least one
//! bitcast is not between a load and a store (an anchor, or the new bitcasts
//! would only move), the phis are made of that type: each load gets a bitcast
//! after it, each bitcast feeding the web is dropped, each store reads a
//! bitcast back.
//!
//! ```text
//!  %l = load i32, ptr %p                  %l = load i32, ptr %p
//!                                         %l.bc = bitcast i32 %l to float
//!  %f = phi i32 [ %l, a ], [ 0, b ]       %f.tc = phi float [ %l.bc, a ], [ 0.0, b ]
//!  %x = bitcast i32 %f to float           (uses of %x read %f.tc)
//! ```
//!
//! MIRROR: `optimizePhiType` and `optimizePhiTypes`, the worklist, the anchor
//! rule, `shouldConvertPhiType` (the hook: x86 answers no below 64 bits). The
//! pin collects its sets in pointer order; here they are in the order met.
//!
//! DEVIATE PT1: a web with an `x86_fp80` constant, or an `x86_fp80` as the new
//! type, is left alone: a constant of that type is held as a double when it can
//! be, and the bitcast of such a constant to an integer is not translated.
//! Owner wasm-b, exit 2026-11-30. `ExtractElement` sources and users are not
//! translated either: MIR has no vectors.
//!
//! # Example
//! `a_phi_of_loads_read_as_a_float_is_a_float_phi` in `cgpphitypes_tests.rs` is
//! the sketch above.
//!
//! # Runs
//! In `cgpaddr`'s run, after the per-instruction steps (`CgpStep::PhiTypes`).

use std::collections::{HashMap, HashSet};

use llrm_mir::context::{Constant, ConstantId, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::lowering::Lowering;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::types::{FloatKind, Type, TypeId};

/// An integer or a floating-point type: the types `shouldConvertPhiType` takes.
fn scalar(
    context: &Context,
    ty: TypeId,
) -> bool {
    context.types.int_bits(ty).is_some() || matches!(context.types.get(ty), Type::Float(_))
}

fn is_bitcast(
    function: &Function,
    inst: InstId,
) -> bool {
    function.instruction(inst).opcode == Opcode::Cast(CastOp::BitCast)
}

/// The instruction that makes `operand`, if it is a result.
fn definition(
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

/// `ConstantExpr::getBitCast(constant, to)` for the constants of `ConstantData`
/// that an integer or a float is: the same bits in the other type. `None` for
/// any other constant, and for DEVIATE PT1.
fn bitcast_constant(
    context: &Context,
    constant: ConstantId,
    to: TypeId,
) -> Option<ConstantId> {
    let one = context.get(constant);
    let to_int = context.types.int_bits(to);
    let to_float = match context.types.get(to) {
        Type::Float(kind) => Some(*kind),
        _ => None,
    };
    if to_float == Some(FloatKind::X86Fp80) {
        return None;
    }
    let kind = match (&one.kind, to_int, to_float) {
        (ConstantKind::Undef, ..) => ConstantKind::Undef,
        (ConstantKind::Poison, ..) => ConstantKind::Poison,
        (ConstantKind::Zero, ..) => ConstantKind::Zero,
        (&ConstantKind::Int(bits), _, Some(FloatKind::Float | FloatKind::Double)) => ConstantKind::Float(bits as u64),
        (&ConstantKind::Int(bits), _, Some(FloatKind::F128)) => ConstantKind::Float128(bits),
        (&ConstantKind::Float(bits), Some(_), _) => ConstantKind::Int(u128::from(bits)),
        (&ConstantKind::Float128(bits), Some(_), _) => ConstantKind::Int(bits),
        _ => return None,
    };
    Some(context.constant(Constant { ty: to, kind }))
}

/// What a web of phis is made of, once gathered.
struct Web {
    phis: Vec<InstId>,
    constants: Vec<ConstantId>,
    defs: Vec<InstId>,
    uses: Vec<InstId>,
    convert_to: TypeId,
}

/// The gathering half of `optimizePhiType`: the web of `first`, or `None` where
/// the pin returns false before it changes anything.
fn gathered(
    context: &Context,
    lowering: &dyn Lowering,
    function: &Function,
    first: InstId,
    visited: &mut HashSet<InstId>,
) -> Option<Web> {
    let phi_ty = function.instruction(first).ty;
    if visited.contains(&first) || !scalar(context, phi_ty) {
        return None;
    }
    let mut worklist = vec![first];
    let mut phis = vec![first];
    let mut phi_set: HashSet<InstId> = HashSet::from([first]);
    let mut constants: Vec<ConstantId> = Vec::new();
    let mut defs: Vec<InstId> = Vec::new();
    let mut uses: Vec<InstId> = Vec::new();
    let mut convert_to: Option<TypeId> = None;
    let mut anchored = false;
    visited.insert(first);

    while let Some(inst) = worklist.pop() {
        let one = function.instruction(inst);
        if one.opcode == Opcode::Phi {
            // The values that feed it.
            for incoming in one.operands.chunks(2).map(|pair| pair[0]) {
                match incoming {
                    Operand::Constant(constant) => {
                        if !matches!(
                            context.get(constant).kind,
                            ConstantKind::Int(_)
                                | ConstantKind::Float(_)
                                | ConstantKind::Float80(_)
                                | ConstantKind::Float128(_)
                                | ConstantKind::Null
                                | ConstantKind::Undef
                                | ConstantKind::Poison
                                | ConstantKind::Zero
                        ) {
                            return None;
                        }
                        if !constants.contains(&constant) {
                            constants.push(constant);
                        }
                    }
                    Operand::Value(_) => {
                        let source = definition(function, incoming)?;
                        match function.instruction(source).opcode {
                            Opcode::Phi => {
                                if !phi_set.contains(&source) {
                                    if !visited.insert(source) {
                                        return None;
                                    }
                                    phi_set.insert(source);
                                    phis.push(source);
                                    worklist.push(source);
                                }
                            }
                            Opcode::Load { volatile, .. } => {
                                if volatile {
                                    return None;
                                }
                                if !defs.contains(&source) {
                                    defs.push(source);
                                    worklist.push(source);
                                }
                            }
                            Opcode::Cast(CastOp::BitCast) => {
                                let from = function.operand_type(context, function.instruction(source).operands[0])?;
                                if *convert_to.get_or_insert(from) != from {
                                    return None;
                                }
                                if !defs.contains(&source) {
                                    defs.push(source);
                                    worklist.push(source);
                                    let origin = definition(function, function.instruction(source).operands[0]);
                                    anchored |= !origin.is_some_and(|origin| {
                                        matches!(function.instruction(origin).opcode, Opcode::Load { .. })
                                    });
                                }
                            }
                            _ => return None,
                        }
                    }
                    Operand::Block(_) => return None,
                }
            }
        }
        // The instructions that read it.
        let result = one.result?;
        let readers: Vec<(InstId, u32)> = function.users(result).iter().map(|read| (read.user, read.index)).collect();
        for (reader, index) in readers {
            match function.instruction(reader).opcode {
                Opcode::Phi => {
                    if !phi_set.contains(&reader) {
                        if visited.contains(&reader) {
                            return None;
                        }
                        phi_set.insert(reader);
                        visited.insert(reader);
                        phis.push(reader);
                        worklist.push(reader);
                    }
                }
                Opcode::Store { volatile, .. } => {
                    if volatile || index != 0 {
                        return None;
                    }
                    if !uses.contains(&reader) {
                        uses.push(reader);
                    }
                }
                Opcode::Cast(CastOp::BitCast) => {
                    let ty = function.instruction(reader).ty;
                    if *convert_to.get_or_insert(ty) != ty {
                        return None;
                    }
                    if !uses.contains(&reader) {
                        uses.push(reader);
                    }
                    let bitcast = function.instruction(reader).result?;
                    anchored |= function
                        .users(bitcast)
                        .iter()
                        .any(|read| !matches!(function.instruction(read.user).opcode, Opcode::Store { .. }));
                }
                _ => return None,
            }
        }
    }

    let convert_to = convert_to?;
    if !anchored
        || convert_to == phi_ty
        || !scalar(context, convert_to)
        || !lowering.should_convert_phi_type()
        || constants.iter().any(|&constant| bitcast_constant(context, constant, convert_to).is_none())
    {
        return None;
    }
    Some(Web { phis, constants, defs, uses, convert_to })
}

/// The rewriting half of `optimizePhiType`. The instructions to delete are
/// added to `deleted`.
fn converted(
    context: &Context,
    function: &mut Function,
    web: &Web,
    visited: &mut HashSet<InstId>,
    deleted: &mut Vec<InstId>,
) {
    let phi_ty = function.instruction(web.phis[0]).ty;
    let mut values: HashMap<Operand, Operand> = HashMap::new();
    for &constant in &web.constants {
        let made = bitcast_constant(context, constant, web.convert_to).expect("checked while gathering");
        values.insert(Operand::Constant(constant), Operand::Constant(made));
    }
    for &def in &web.defs {
        let result = Operand::Value(function.instruction(def).result.expect("a def has a value"));
        if is_bitcast(function, def) {
            values.insert(result, function.instruction(def).operands[0]);
            deleted.push(def);
        } else {
            let cast = function.create_instruction(
                Opcode::Cast(CastOp::BitCast),
                web.convert_to,
                vec![result],
                Flags::default(),
                None,
            );
            function.insert(cast, Position::After(def)).expect("a position after a placed def");
            values.insert(result, Operand::Value(function.instruction(cast).result.expect("a cast has a value")));
        }
    }
    let mut fresh: Vec<(InstId, InstId)> = Vec::new();
    for &phi in &web.phis {
        let new = function.create_instruction(Opcode::Phi, web.convert_to, Vec::new(), Flags::default(), None);
        function.insert(new, Position::Before(phi)).expect("a position before a placed phi");
        let result = Operand::Value(function.instruction(phi).result.expect("a phi has a value"));
        values.insert(result, Operand::Value(function.instruction(new).result.expect("a phi has a value")));
        fresh.push((phi, new));
    }
    // Pipe together all the phis.
    for &(old, new) in &fresh {
        let operands: Vec<Operand> =
            function.instruction(old).operands.chunks(2).flat_map(|pair| [values[&pair[0]], pair[1]]).collect();
        function.set_operands(new, operands);
        visited.insert(new);
    }
    // And finally the stores and bitcasts.
    for &read in &web.uses {
        let source = function.instruction(read).operands[0];
        if is_bitcast(function, read) {
            deleted.push(read);
            let result = function.instruction(read).result.expect("a bitcast has a value");
            function.replace_all_uses_with(result, values[&source]);
        } else {
            let back = function.create_instruction(
                Opcode::Cast(CastOp::BitCast),
                phi_ty,
                vec![values[&source]],
                Flags::default(),
                None,
            );
            function.insert(back, Position::Before(read)).expect("a position before a placed store");
            function.set_operand(
                read,
                0,
                Operand::Value(function.instruction(back).result.expect("a cast has a value")),
            );
        }
    }
    deleted.extend(web.phis.iter().copied());
}

/// `optimizePhiTypes`: whether a web of phis was made another type.
pub fn phi_types_optimized(
    context: &Context,
    lowering: &dyn Lowering,
    function: &mut Function,
) -> bool {
    let phis: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .collect();
    let mut visited: HashSet<InstId> = HashSet::new();
    let mut deleted: Vec<InstId> = Vec::new();
    let mut changed = false;
    for phi in phis {
        if let Some(web) = gathered(context, lowering, function, phi, &mut visited) {
            converted(context, function, &web, &mut visited, &mut deleted);
            changed = true;
        }
    }
    // Remove the phis, and the bitcasts, that were converted.
    let mut seen: HashSet<InstId> = HashSet::new();
    deleted.retain(|&inst| seen.insert(inst));
    for &inst in &deleted {
        if let Some(result) = function.instruction(inst).result {
            let ty = function.value(result).ty;
            let poison = context.constant(Constant { ty, kind: ConstantKind::Poison });
            function.replace_all_uses_with(result, Operand::Constant(poison));
        }
    }
    for inst in deleted {
        function.erase(inst).expect("a converted instruction has no users left");
    }
    changed
}

#[cfg(test)]
#[path = "cgpphitypes_tests.rs"]
mod tests;
