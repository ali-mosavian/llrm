//! Redundant extension elimination across a phi: gcc's
//! [`ree.cc`](https://gcc.gnu.org/git/?p=gcc.git;a=blob;f=gcc/ree.cc)
//! (`pass_ree`), as the rewrite of MIR before isel.
//!
//! # How it works
//! `E = sext/zext P` where `P = phi [c, L, ..]` and every arm is a constant or
//! a load: the extension is made on each arm instead (`ext c` folds; `ext L`
//! follows the load, where `extload` makes one extending load of the pair), and
//! `E` is the phi of the wide arms. A loop-carried `short` that is loaded
//! narrow and extended again at the top of every trip (`movzx ebx, word [m];
//! ...; movsx ebx, bx`) is loaded once, sign-extending.
//!
//! Only a phi that nothing but extensions of one kind and type read is changed,
//! so the narrow value does not live beside the wide one; an arm that is
//! anything else (a call, an add) leaves the phi alone: gcc's `ree` also
//! requires every reaching definition to take the extension
//! (`combine_reaching_defs`).
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | Done on MIR before isel, not on RTL after it: the arms are loads and constants, which isel selects as one extending load and an immediate. Owner llrm-target-x86-m64-b; exit 2026-11-30. |
//!
//! # Example
//! Before:
//!
//! ```mir
//! b1:
//!   %p = phi i16 [ 1, %b0 ], [ %v, %b1 ]
//!   %e = sext i16 %p to i32
//!   ...
//!   %v = load i16, ptr %q
//! ```
//!
//! After:
//!
//! ```mir
//! b1:
//!   %e = phi i32 [ 1, %b0 ], [ %v.ext, %b1 ]
//!   ...
//!   %v = load i16, ptr %q
//!   %v.ext = sext i16 %v to i32
//! ```
//!
//! From unit test `test_a_loop_carried_loaded_short_is_extended_once` in
//! `ree_tests.rs`.
//!
//! # Runs
//! Once per body, after `cgpaddr` and before `extload`. With `wanted("ree")`.
//!
//! # Needs
//! Nothing.
//!
//! # Keeps
//! Blocks, dominators and loops.
//!
//! # See also
//! Tests: `ree_tests.rs`. Levels: `docs/reference/levels.md`.

use llrm_mir::context::{ConstantKind, Context, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::tti::{CastKind, Tti, Ty};
use llrm_mir::types::TypeId;

pub struct Ree;

impl FunctionPass for Ree {
    fn name(&self) -> &'static str {
        "ree"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let Some(tti) = analyses.outer().target().tti() else { return PreservedAnalyses::all() };
        if extended_across_phis(unit.context, unit.function, &*tti) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// An arm of the phi, as the extension takes it.
enum Arm {
    Constant(u128),
    Load(InstId),
}

/// What the extension of one phi is made of.
struct Plan {
    kind: CastOp,
    wide: TypeId,
    extensions: Vec<InstId>,
    arms: Vec<(Arm, BlockId)>,
}

/// `phi`'s plan: every reader an extension of one kind and type, every arm a
/// constant or a plain load whose readers are this phi, compares and extensions
/// of the same kind (nothing that keeps the narrow value live).
fn plan(
    context: &Context,
    function: &Function,
    tti: &dyn Tti,
    phi: InstId,
) -> Option<Plan> {
    let result = function.instruction(phi).result?;
    context.types.int_bits(function.instruction(phi).ty)?;
    let mut extensions = Vec::new();
    let mut shape: Option<(CastOp, TypeId)> = None;
    for used in function.users(result) {
        let user = function.instruction(used.user);
        let Opcode::Cast(kind @ (CastOp::SExt | CastOp::ZExt)) = user.opcode else { return None };
        if shape.is_some_and(|seen| seen != (kind, user.ty)) {
            return None;
        }
        shape = Some((kind, user.ty));
        extensions.push(used.user);
    }
    let (kind, wide) = shape?;
    let mut arms = Vec::new();
    for (value, from) in crate::pageindex::arms(function, phi) {
        match value {
            Operand::Constant(id) => {
                let ConstantKind::Int(bits) = context.get(id).kind else { return None };
                arms.push((Arm::Constant(bits), from));
            }
            Operand::Value(made) => {
                let ValueDef::Instruction(def) = function.value(made).def else { return None };
                let load = function.instruction(def);
                let Opcode::Load { volatile: false, .. } = load.opcode else { return None };
                if def == phi || function.parent(def).is_none() {
                    return None;
                }
                // gcc's `ree` takes the extension only where it costs no more
                // than the one it removes: a load that extends
                // as it loads is free, a load and a separate extension are not.
                let narrow = function.instruction(phi).ty;
                let cast = if kind == CastOp::SExt { CastKind::SExt } else { CastKind::ZExt };
                let free = Ty::of(&context.types, narrow)
                    .zip(Ty::of(&context.types, wide))
                    .is_some_and(|(src, dst)| tti.ext_load_free(cast, dst, src));
                if !free {
                    return None;
                }
                for reader in function.users(made) {
                    let other = function.instruction(reader.user);
                    let ok = reader.user == phi
                        || matches!(other.opcode, Opcode::ICmp(_))
                        || (other.opcode == Opcode::Cast(kind) && other.ty == wide);
                    if !ok {
                        return None;
                    }
                }
                arms.push((Arm::Load(def), from));
            }
            _ => return None,
        }
    }
    Some(Plan { kind, wide, extensions, arms })
}

fn extended_across_phis(
    context: &mut Context,
    function: &mut Function,
    tti: &dyn Tti,
) -> bool {
    let phis: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .collect();
    let mut changed = false;
    for phi in phis {
        if function.is_erased(phi) {
            continue;
        }
        let Some(plan) = plan(context, function, tti, phi) else { continue };
        let narrow_bits = context.types.int_bits(function.instruction(phi).ty).expect("an integer");
        let mut wide_arms = Vec::new();
        for (arm, from) in &plan.arms {
            let value = match arm {
                Arm::Constant(bits) => {
                    let extended = if plan.kind == CastOp::SExt { signed(*bits, narrow_bits) } else { *bits as i128 };
                    Operand::Constant(context.int(plan.wide, extended))
                }
                Arm::Load(load) => {
                    let loaded = Operand::Value(function.instruction(*load).result.expect("a value"));
                    let made = function.create_instruction(
                        Opcode::Cast(plan.kind),
                        plan.wide,
                        vec![loaded],
                        Flags::default(),
                        None,
                    );
                    function.insert(made, Position::After(*load)).expect("a placed instruction");
                    Operand::Value(function.instruction(made).result.expect("a value"))
                }
            };
            wide_arms.push((value, *from));
        }
        let wide_phi = function.create_instruction(Opcode::Phi, plan.wide, Vec::new(), Flags::default(), None);
        function.insert(wide_phi, Position::Before(phi)).expect("a placed block");
        crate::pageindex::set_arms(function, wide_phi, &wide_arms);
        let wide_value = Operand::Value(function.instruction(wide_phi).result.expect("a value"));
        for extension in &plan.extensions {
            let value = function.instruction(*extension).result.expect("a value");
            function.replace_value(value, wide_value);
            function.erase(*extension).expect("no readers");
        }
        function.erase(phi).expect("no readers");
        changed = true;
    }
    changed
}

#[cfg(test)]
#[path = "ree_tests.rs"]
mod tests;
