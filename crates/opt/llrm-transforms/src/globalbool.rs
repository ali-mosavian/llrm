//! Shrink an internal integer global that holds one of two values into an `i1`.
//!
//! # How it works
//! `TryToShrinkGlobalToBoolean`
//! ([GlobalOpt.cpp:1157](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/GlobalOpt.cpp#L1157)):
//! the only values ever stored into the variable are its initializer and one
//! constant. It becomes an `i1` starting false; a store writes `false` for the
//! initializer and `true` for the constant, and a load reads the `i1` and
//! selects between the two (a `zext` when the initializer is zero and the
//! constant one). A store of a value loaded from the variable copies the `i1`.
//!
//! Every use must be a non-volatile load or store of the variable's own type
//! at its address, and some load must read it (a never-loaded variable is
//! `WriteOnly`'s).
//!
//! DEVIATE 1: integer variables only; the pin also shrinks aggregates, whose
//! select MIR does not hold. Exit 2026-11-20.
//!
//! DEVIATE 2: an `undef` initializer is left alone; the pin first turns the
//! stored constant into the initializer (`optimizeOnceStoredGlobal`'s
//! neighbour in `processInternalGlobal`), which is not translated.
//!
//! # Example
//! Before:
//!
//! ```mir
//! @bad = internal global i32 0
//!
//! define i32 @f(i32 %x) {
//! b0:
//!   store i32 1, ptr @bad
//!   %v = load i32, ptr @bad
//!   ret i32 %v
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! @bad = internal global i1 false
//!
//! define i32 @f(i32 %x) {
//! b0:
//!   store i1 true, ptr @bad
//!   %v.b = load i1, ptr @bad
//!   %v = zext i1 %v.b to i32
//!   ret i32 %v
//! }
//! ```
//!
//! # Runs
//! With `GlobalOpt`, once before the per-body pipeline.
//!
//! # See also
//! [`crate::globalopt`], [`crate::globalsra`].

use std::collections::BTreeSet;

use llrm_mir::context::{Constant, ConstantId, ConstantKind, GlobalId};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, GlobalKind, Linkage, Module, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Flags, Opcode};

use crate::globalsra::{Direct, direct_uses, is_the_variable, reaching};

/// The functions changed.
pub fn shrink(module: &mut Module) -> Vec<GlobalId> {
    let held = crate::globalopt::held_by_live(module);
    let reached = reaching(module);
    let mut changed = BTreeSet::new();
    for at in 0..module.globals.len() {
        let id = GlobalId(at as u32);
        let Some(plan) = plan(module, id, &held, &reached) else { continue };
        let (void, i1) = (module.context.types.void(), module.context.types.int(1));
        let (zero, one) = (
            module.context.constant(Constant { ty: i1, kind: ConstantKind::Int(0) }),
            module.context.constant(Constant { ty: i1, kind: ConstantKind::Int(1) }),
        );
        if let GlobalKind::Variable(variable) = &mut module.globals[at].kind {
            variable.ty = i1;
            variable.initializer = Some(zero);
        }
        for direct in &plan.direct {
            let GlobalKind::Function(function) = &mut module.globals[direct.function.0 as usize].kind else { continue };
            rewrite(function, direct, &plan, (void, i1), (zero, one));
            changed.insert(direct.function);
        }
    }
    changed.into_iter().collect()
}

/// A variable that qualifies: its uses by function, the value its type
/// shrinks from and the constant stored besides the initializer.
struct Plan {
    direct: Vec<Direct>,
    initial: ConstantId,
    other: ConstantId,
    /// `InitVal->isNullValue() && CI->isOne()`: a `zext` stands for the select.
    one_zero: bool,
}

fn plan(
    module: &Module,
    id: GlobalId,
    held: &BTreeSet<GlobalId>,
    reached: &std::collections::BTreeMap<
        GlobalId,
        std::collections::BTreeMap<GlobalId, Option<crate::globalopt::Reach>>,
    >,
) -> Option<Plan> {
    let global = &module.globals[id.0 as usize];
    let GlobalKind::Variable(variable) = &global.kind else { return None };
    let initial = variable.initializer?;
    let internal = matches!(global.linkage, Linkage::Internal | Linkage::Private);
    if !internal || variable.constant || variable.externally_initialized || held.contains(&id) {
        return None;
    }
    let bits = module.context.types.int_bits(variable.ty)?;
    if bits == 1 || module.context.get(initial).kind == ConstantKind::Undef {
        return None;
    }
    let direct = direct_uses(module, id, reached)?;
    let mut other = None;
    for one in &direct {
        let GlobalKind::Function(function) = &module.globals[one.function.0 as usize].kind else { return None };
        if one.loads.iter().any(|&load| function.instruction(load).ty != variable.ty) {
            return None;
        }
        for &store in &one.stores {
            let value = function.instruction(store).operands[0];
            if function.operand_type(&module.context, value) != Some(variable.ty) {
                return None;
            }
            match value {
                Operand::Constant(constant) if constant == initial => {}
                Operand::Value(loaded) if loads_the_variable(module, function, loaded, id) => {}
                Operand::Constant(_) if other.is_none() || other == Some(value) => other = Some(value),
                _ => return None,
            }
        }
    }
    let Some(Operand::Constant(other)) = other else { return None };
    if direct.iter().all(|one| one.loads.is_empty()) {
        return None;
    }
    let zero = matches!(
        module.context.get(initial).kind,
        ConstantKind::Zero | ConstantKind::Int(0)
    );
    let one = matches!(module.context.get(other).kind, ConstantKind::Int(1));
    Some(Plan { direct, initial, other, one_zero: zero && one })
}

/// Whether `value` is a load of the variable itself.
fn loads_the_variable(
    module: &Module,
    function: &Function,
    value: llrm_mir::module::ValueId,
    id: GlobalId,
) -> bool {
    let ValueDef::Instruction(def) = function.value(value).def else { return false };
    let load = function.instruction(def);
    matches!(load.opcode, Opcode::Load { .. })
        && matches!(
            load.operands[0],
            Operand::Constant(address) if is_the_variable(&module.context, address, id)
        )
}

/// One function's loads and stores of the variable, in the pin's two forms.
fn rewrite(
    function: &mut Function,
    direct: &Direct,
    plan: &Plan,
    (void, i1): (llrm_mir::types::TypeId, llrm_mir::types::TypeId),
    (zero, one): (ConstantId, ConstantId),
) {
    for &load in &direct.loads {
        let old = function.instruction(load).clone();
        let name = function.value(old.result.expect("a load's value")).name.clone().map(|name| format!("{name}.b"));
        let narrow = function.create_instruction(
            Opcode::Load { align: None, volatile: false },
            i1,
            old.operands.clone(),
            Flags::default(),
            name.as_deref(),
        );
        function.insert(narrow, Position::Before(load)).expect("a new instruction");
        let narrow_value = Operand::Value(function.instruction(narrow).result.expect("a load's value"));
        let wide = if plan.one_zero {
            function.create_instruction(Opcode::Cast(CastOp::ZExt), old.ty, vec![narrow_value], Flags::default(), None)
        } else {
            let operands = vec![narrow_value, Operand::Constant(plan.other), Operand::Constant(plan.initial)];
            function.create_instruction(Opcode::Select, old.ty, operands, Flags::default(), None)
        };
        function.insert(wide, Position::Before(load)).expect("a new instruction");
        let result = old.result.expect("a load's value");
        let wide_value = function.instruction(wide).result.expect("a value");
        function.replace_all_uses_with(result, Operand::Value(wide_value));
        function.erase(load).expect("a load nothing reads is erased");
    }
    for &store in &direct.stores {
        let old = function.instruction(store).clone();
        let stored = match old.operands[0] {
            Operand::Constant(value) => Operand::Constant(if value == plan.other { one } else { zero }),
            // A copy: its loads were made `load i1` then a `zext` or `select`.
            Operand::Value(copied) => {
                let ValueDef::Instruction(def) = function.value(copied).def else { continue };
                function.instruction(def).operands[0]
            }
            Operand::Block(_) => continue,
        };
        let narrow = function.create_instruction(
            Opcode::Store { align: None, volatile: false },
            void,
            vec![stored, old.operands[1]],
            Flags::default(),
            None,
        );
        function.insert(narrow, Position::Before(store)).expect("a new instruction");
        function.erase(store).expect("a store defines nothing");
    }
}

#[cfg(test)]
#[path = "globalbool_tests.rs"]
mod tests;
