//! What `InlineFunction` carries from the call site onto the copy of the body:
//! `AddParamAndFnBasicAttributes`
//! ([L1473](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp#L1473))
//! and `AddReturnAttributes`
//! ([L1652](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp#L1652)).
//!
//! # How it works
//! A call in the body that passes one of the callee's own parameters on gets
//! the attributes the site states for that parameter (`nonnull`,
//! `dereferenceable`, `dereferenceable_or_null`, `align`, and `range` where the
//! call has none); a pointer into a parameter's object gets its `readonly` and
//! `readnone`. A call whose value the callee returns, in the return's block with
//! nothing that may leave between, gets the return attributes the site states.
//!
//! # Deviations
//! - IA1 (owner w9, exit 2026-11-30): `nofpclass` is not an attribute here, and
//!   two `range`s are not intersected (the existing one stays). A call that may
//!   throw or not return between the value and the return is any call or
//!   invoke, not `isGuaranteedToTransferExecutionToSuccessor` (no callee
//!   attributes at hand).
//! - A copy is never simplified while it is made (`isSimplified`): the
//!   check is always false.

use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, CastOp, Opcode};

/// What a call site states, taken before the call is replaced.
pub struct SiteAttrs {
    pub arguments: Vec<Vec<Attribute>>,
    pub returned: Vec<Attribute>,
}

impl SiteAttrs {
    /// The attributes of `call`, if it is a call.
    pub fn of(
        function: &Function,
        call: InstId,
    ) -> Option<Self> {
        match &function.instruction(call).opcode {
            Opcode::Call(info) => {
                Some(Self { arguments: info.argument_attrs.clone(), returned: info.return_attrs.clone() })
            }
            _ => None,
        }
    }
}

fn flag(
    attrs: &[Attribute],
    name: &str,
) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name))
}

fn int(
    attrs: &[Attribute],
    name: &str,
) -> Option<u64> {
    attrs.iter().find_map(|one| match one {
        Attribute::Int(found, value) if found == name => Some(*value),
        _ => None,
    })
}

fn range(attrs: &[Attribute]) -> Option<Attribute> {
    attrs.iter().find(|one| matches!(one, Attribute::Range { .. })).cloned()
}

fn set_flag(
    attrs: &mut Vec<Attribute>,
    name: &str,
) {
    if !flag(attrs, name) {
        attrs.push(Attribute::Flag(name.into()));
    }
}

fn remove_flag(
    attrs: &mut Vec<Attribute>,
    name: &str,
) {
    attrs.retain(|one| !matches!(one, Attribute::Flag(flag) if flag == name));
}

/// `name` at `value` unless the attributes already state more of it.
fn set_int_at_least(
    attrs: &mut Vec<Attribute>,
    name: &str,
    value: u64,
) {
    match attrs.iter_mut().find(|one| matches!(one, Attribute::Int(found, _) if found == name)) {
        Some(Attribute::Int(_, existing)) => *existing = (*existing).max(value),
        _ => attrs.push(Attribute::Int(name.into(), value)),
    }
}

/// The pointer parameter of `body` that `operand` is, or points into by
/// in-bounds offsets and pointer casts (`getUnderlyingObject`).
fn parameter_of(
    body: &Function,
    operand: Operand,
    exact: bool,
) -> Option<usize> {
    let mut current = operand;
    loop {
        let Operand::Value(value) = current else { return None };
        if let Some(at) = body.parameters().iter().position(|&one| one == value) {
            return Some(at);
        }
        if exact {
            return None;
        }
        let ValueDef::Instruction(def) = body.value(value).def else { return None };
        let instruction = body.instruction(def);
        match instruction.opcode {
            Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => {
                current = instruction.operands[0];
            }
            _ => return None,
        }
    }
}

/// `AddParamAndFnBasicAttributes` and `AddReturnAttributes` for the copy of
/// `body` whose instructions are `copies`, in the order of `body`'s that are
/// not returns.
pub fn propagated(
    function: &mut Function,
    site: &SiteAttrs,
    body: &Function,
    copies: &[InstId],
) {
    let originals: Vec<InstId> = body
        .walk()
        .filter(|&(_, inst)| body.instruction(inst).opcode != Opcode::Ret)
        .map(|(_, inst)| inst)
        .collect();
    if originals.len() != copies.len() {
        return;
    }
    let copy_of = |inst: InstId| originals.iter().position(|&one| one == inst).map(|at| copies[at]);
    param_attributes(function, site, body, &originals, copies);
    return_attributes(function, site, body, &copy_of);
}

fn param_attributes(
    function: &mut Function,
    site: &SiteAttrs,
    body: &Function,
    originals: &[InstId],
    copies: &[InstId],
) {
    let exact_names = ["nonnull", "dereferenceable", "dereferenceable_or_null", "align"];
    let has_any = site.arguments.iter().any(|attrs| {
        flag(attrs, "readnone")
            || flag(attrs, "readonly")
            || exact_names.iter().any(|name| flag(attrs, name) || int(attrs, name).is_some())
            || range(attrs).is_some()
    });
    if !has_any {
        return;
    }
    for (&original, &copy) in originals.iter().zip(copies) {
        let Opcode::Call(inner) = &body.instruction(original).opcode else { continue };
        let Opcode::Call(made) = &function.instruction(copy).opcode else { continue };
        let mut info = made.clone();
        let arguments = inner.argument_attrs.len().min(body.instruction(original).operands.len());
        let mut changed = false;
        for at in 0..arguments {
            if info.argument_attrs.get(at).is_some_and(|attrs| attrs.iter().any(|one| matches!(one, Attribute::Type(name, _) if name == "byval"))) {
                continue;
            }
            // Don't bother propagating attrs to constants.
            if matches!(function.instruction(copy).operands.get(at), Some(Operand::Constant(_)) | None) {
                continue;
            }
            let passed = body.instruction(original).operands[at];
            let exact = parameter_of(body, passed, true);
            let Some(parameter) = exact.or_else(|| parameter_of(body, passed, false)) else { continue };
            let Some(stated) = site.arguments.get(parameter) else { continue };
            while info.argument_attrs.len() <= at {
                info.argument_attrs.push(Vec::new());
            }
            let attrs = &mut info.argument_attrs[at];
            let before = attrs.clone();
            if exact.is_some() {
                if flag(stated, "nonnull") {
                    set_flag(attrs, "nonnull");
                }
                for name in ["dereferenceable", "dereferenceable_or_null", "align"] {
                    if let Some(value) = int(stated, name) {
                        set_int_at_least(attrs, name, value);
                    }
                }
                if range(attrs).is_none()
                    && let Some(stated_range) = range(stated)
                {
                    attrs.push(stated_range);
                }
            }
            // Its access attributes go to any pointer into the same object.
            for name in ["readnone", "readonly"] {
                if flag(stated, name) {
                    set_flag(attrs, name);
                }
            }
            // We can have conflicting attributes from the inner call site and
            // the one inlined: the most restrictive.
            if flag(attrs, "readonly") && flag(attrs, "writeonly") {
                set_flag(attrs, "readnone");
            }
            if flag(attrs, "readnone") {
                remove_flag(attrs, "readonly");
                remove_flag(attrs, "writeonly");
            }
            if flag(attrs, "readonly") || flag(attrs, "readnone") {
                remove_flag(attrs, "writable");
            }
            changed |= *attrs != before;
        }
        if changed {
            function.set_opcode(copy, Opcode::Call(info));
        }
    }
}

fn return_attributes(
    function: &mut Function,
    site: &SiteAttrs,
    body: &Function,
    copy_of: &dyn Fn(InstId) -> Option<InstId>,
) {
    let ub_flags = ["noalias", "noundef"];
    let pg_flags = ["nonnull"];
    let stated = &site.returned;
    let has_ub = int(stated, "dereferenceable").is_some()
        || int(stated, "dereferenceable_or_null").is_some()
        || ub_flags.iter().any(|name| flag(stated, name));
    let has_pg = pg_flags.iter().any(|name| flag(stated, name)) || int(stated, "align").is_some() || range(stated).is_some();
    if !has_ub && !has_pg {
        return;
    }
    for (_, ret) in body.walk().filter(|&(_, inst)| body.instruction(inst).opcode == Opcode::Ret) {
        let Some(&Operand::Value(returned)) = body.instruction(ret).operands.first() else { continue };
        let ValueDef::Instruction(def) = body.value(returned).def else { continue };
        let Opcode::Call(original) = &body.instruction(def).opcode else { continue };
        let Some(copy) = copy_of(def) else { continue };
        // Control flow dependent otherwise: the call and the return in one
        // block with nothing between that may throw or exit.
        if body.parent(def) != body.parent(ret) {
            continue;
        }
        let between = body.iter_block(body.parent(def).expect("placed")).skip_while(|&one| one != def).skip(1);
        if between.take_while(|&one| one != ret).any(|one| matches!(body.instruction(one).opcode, Opcode::Call(_) | Opcode::Invoke(_))) {
            continue;
        }
        let Opcode::Call(made) = &function.instruction(copy).opcode else { continue };
        let mut info = made.clone();
        let existing = info.return_attrs.clone();
        let attrs = &mut info.return_attrs;
        for name in ["dereferenceable", "dereferenceable_or_null"] {
            if let Some(value) = int(stated, name) {
                // The attribute the call already has, if bigger, stays.
                if int(attrs, name).is_none_or(|have| have < value) {
                    set_int_at_least(attrs, name, value);
                }
            }
        }
        for name in ub_flags {
            if flag(stated, name) {
                set_flag(attrs, name);
            }
        }
        // Poison generating attributes: the site says `noundef`, or the value
        // has one use and the original call does not say it.
        let returned_has_one_use = body.users(returned).only().is_some();
        let original_noundef = flag(&original.return_attrs, "noundef");
        if flag(stated, "noundef") || (returned_has_one_use && !original_noundef) {
            if flag(stated, "nonnull") {
                set_flag(attrs, "nonnull");
            }
            if let Some(value) = int(stated, "align")
                && int(attrs, "align").is_none_or(|have| have <= value)
            {
                set_int_at_least(attrs, "align", value);
            }
            if range(attrs).is_none()
                && let Some(stated_range) = range(stated)
            {
                attrs.push(stated_range);
            }
        }
        if *attrs != existing {
            function.set_opcode(copy, Opcode::Call(info));
        }
    }
}
