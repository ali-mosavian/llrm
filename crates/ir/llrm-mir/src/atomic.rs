//! Atomic instructions as the pipeline sees them: calls nothing may move.
//!
//! `Opcode::Atomic` is read, written, verified and run as itself. Between
//! `hidden` and `lowered` the optimizer sees only a call of an undefined
//! function, which every pass already treats as reading and writing any
//! memory, never removed, merged or reordered, so no pass needs to know
//! what an atomic is. `lowered` then makes each of them what the target does.
//!
//! The name of the function states the instruction: `llrm.atomic.{kind}.
//! {ordering}.{failure}.{scope}.{volatile}.a{align}.t{n}`, `n` told apart by
//! the signature.
//!
//! # Used by
//! the driver's `optimized`, around the pipeline.

use crate::module::{GlobalKind, InstId, Linkage, Module, Operand};
use crate::opcode::{AtomicInfo, AtomicKind, AtomicOrdering, CallInfo, Flags, Opcode, RMW_OPS, RmwOp, Tail};
use crate::types::{Type, TypeId};

const PREFIX: &str = "llrm.atomic.";

fn kind_name(kind: AtomicKind) -> String {
    match kind {
        AtomicKind::Load => "load".to_owned(),
        AtomicKind::Store => "store".to_owned(),
        AtomicKind::Fence => "fence".to_owned(),
        AtomicKind::CmpXchg { weak, .. } => if weak { "cmpxchgweak" } else { "cmpxchg" }.to_owned(),
        AtomicKind::Rmw(op) => {
            format!("rmw{}", RMW_OPS.iter().find(|(one, _)| *one == op).map_or("xchg", |(_, name)| name))
        }
    }
}

/// The name of the function that stands for `info`, `n` apart from the
/// others of another signature.
fn name_of(
    info: AtomicInfo,
    n: usize,
) -> String {
    let failure = match info.kind {
        AtomicKind::CmpXchg { failure, .. } => failure.spelling(),
        _ => "-",
    };
    format!(
        "{PREFIX}{}.{}.{failure}.{}.{}.a{}.t{n}",
        kind_name(info.kind),
        info.ordering.spelling(),
        if info.single_thread { "st" } else { "sys" },
        if info.volatile { "v" } else { "nv" },
        info.align.unwrap_or(0)
    )
}

/// The instruction a function name stands for.
pub fn decode(name: &str) -> Option<AtomicInfo> {
    let rest = name.strip_prefix(PREFIX)?;
    let parts: Vec<&str> = rest.split('.').collect();
    let [kind, ordering, failure, scope, volatile, align, _] = parts[..] else { return None };
    let ordering = AtomicOrdering::named(ordering)?;
    let kind = match kind {
        "load" => AtomicKind::Load,
        "store" => AtomicKind::Store,
        "fence" => AtomicKind::Fence,
        "cmpxchg" | "cmpxchgweak" => {
            AtomicKind::CmpXchg { failure: AtomicOrdering::named(failure)?, weak: kind == "cmpxchgweak" }
        }
        other => {
            let op = other.strip_prefix("rmw")?;
            AtomicKind::Rmw(RMW_OPS.iter().find(|(_, name)| *name == op)?.0)
        }
    };
    let align = align.strip_prefix('a')?.parse::<u64>().ok()?;
    Some(AtomicInfo {
        kind,
        ordering,
        single_thread: scope == "st",
        volatile: volatile == "v",
        align: (align != 0).then_some(align),
    })
}

/// Every atomic instruction of `module` a call of its function.
pub fn hidden(module: &mut Module) -> Result<(), String> {
    let mut found: Vec<(usize, InstId, AtomicInfo)> = Vec::new();
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        for (_, inst) in function.walk() {
            if let Opcode::Atomic(info) = function.instruction(inst).opcode {
                found.push((at, inst, info));
            }
        }
    }
    let mut signatures: Vec<TypeId> = Vec::new();
    for (at, inst, info) in found {
        let GlobalKind::Function(function) = &module.globals[at].kind else { continue };
        let instruction = function.instruction(inst).clone();
        let parameters: Vec<TypeId> = instruction
            .operands
            .iter()
            .map(|&one| function.operand_type(&module.context, one).ok_or("an atomic operand has a type"))
            .collect::<Result<_, _>>()?;
        let signature =
            module.context.types.intern(Type::Function { returns: instruction.ty, parameters, variadic: false });
        let n = signatures
            .iter()
            .position(|&one| one == signature)
            .unwrap_or_else(
                || {
                    signatures.push(signature);
                    signatures.len() - 1
                },
            );
        let name = name_of(info, n);
        let callee = match module.named(&name) {
            Some(id) => id,
            None => module.add_function(&name, signature, Linkage::External)?,
        };
        let reference = module.reference(callee);
        let GlobalKind::Function(function) = &mut module.globals[at].kind else { continue };
        let call = CallInfo {
            function_type: signature,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); instruction.operands.len()],
            attrs: Vec::new(),
            tail: Tail::default(),
        };
        let mut operands = instruction.operands.clone();
        operands.push(Operand::Constant(reference));
        let made =
            function.create_instruction(Opcode::Call(Box::new(call)), instruction.ty, operands, Flags::default(), None);
        function.insert(made, crate::edit::Position::Before(inst)).map_err(|why| why.to_string())?;
        if let (Some(old), Some(new)) = (instruction.result, function.instruction(made).result) {
            function.replace_all_uses_with(old, Operand::Value(new));
        }
        function.erase(inst).map_err(|why| why.to_string())?;
    }
    Ok(())
}

/// The inverse of [`hidden`]: every call of a `llrm.atomic.*` function is the
/// atomic instruction again. For printing a module the pin's `llc` reads.
pub fn restored(module: &mut Module) -> Result<(), String> {
    let mut found: Vec<(usize, InstId, AtomicInfo)> = Vec::new();
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        for (_, inst) in function.walk() {
            let Some(callee) = crate::memory::callee(&module.context, function, inst) else { continue };
            if let Some(info) = module.globals[callee.0 as usize].name.as_deref().and_then(decode) {
                found.push((at, inst, info));
            }
        }
    }
    for (at, inst, info) in found {
        let GlobalKind::Function(function) = &mut module.globals[at].kind else { continue };
        let call = function.instruction(inst).clone();
        let operands = call.operands[..call.operands.len() - 1].to_vec();
        let made = function.create_instruction(Opcode::Atomic(info), call.ty, operands, Flags::default(), None);
        function.insert(made, crate::edit::Position::Before(inst)).map_err(|why| why.to_string())?;
        if let (Some(old), Some(new)) = (call.result, function.instruction(made).result) {
            function.replace_all_uses_with(old, Operand::Value(new));
        }
        function.erase(inst).map_err(|why| why.to_string())?;
    }
    Ok(())
}

/// Every call `hidden` made an instruction of the target. A load, or a store
/// short of `seq_cst`, of at most `native` bits is a volatile plain access
/// where `tso` (x86: every access is acquire and release already) or the
/// ordering is `monotonic` or `unordered`; a fence short of `seq_cst` is
/// nothing there. Where the target `locked` selects `xchg` itself, an exchange
/// and a `seq_cst` store stay calls it takes. What the target has no
/// instruction for is refused, never made a plain access.
pub fn lowered(
    module: &mut Module,
    native: u32,
    tso: bool,
    locked: bool,
) -> Result<(), String> {
    let mut found: Vec<(usize, InstId, AtomicInfo)> = Vec::new();
    let mut kept = vec![false; module.globals.len()];
    let mut called: Vec<usize> = Vec::new();
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        for (_, inst) in function.walk() {
            let Some(callee) = crate::memory::callee(&module.context, function, inst) else { continue };
            if let Some(info) = module.globals[callee.0 as usize].name.as_deref().and_then(decode) {
                found.push((at, inst, info));
                called.push(callee.0 as usize);
            }
        }
    }
    for ((at, inst, info), callee) in found.into_iter().zip(called) {
        let GlobalKind::Function(function) = &mut module.globals[at].kind else { continue };
        let instruction = function.instruction(inst).clone();
        let accessed = match info.kind {
            AtomicKind::Load => Some(instruction.ty),
            AtomicKind::Store => function.operand_type(&module.context, instruction.operands[0]),
            AtomicKind::Fence => None,
            _ => function.operand_type(&module.context, instruction.operands[1]),
        };
        let bits = accessed.and_then(|ty| module.context.types.int_bits(ty));
        let spelled = format!("{} {}", kind_name(info.kind), info.ordering.spelling());
        if bits.is_some_and(|bits| bits > native) {
            return Err(format!(
                "`{spelled}` of {} bits is not supported: the target's widest atomic access is {native}",
                bits.unwrap_or(0)
            ));
        }
        let exchange = matches!(info.kind, AtomicKind::Rmw(RmwOp::Xchg))
            || (info.kind == AtomicKind::Store && info.ordering == AtomicOrdering::SeqCst);
        if locked && exchange {
            kept[callee] = true;
            continue;
        }
        if tso && info.kind == AtomicKind::Fence && info.ordering != AtomicOrdering::SeqCst {
            function.erase(inst).map_err(|why| why.to_string())?;
            continue;
        }
        let ordered = info.ordering > AtomicOrdering::Monotonic;
        if ordered && !tso && matches!(info.kind, AtomicKind::Load | AtomicKind::Store) {
            return Err(format!("`{spelled}` is not lowered for this target: it has no acquire or release access yet"));
        }
        let plain = match (info.kind, info.ordering) {
            (AtomicKind::Load, _) => Opcode::Load { align: info.align, volatile: true },
            (AtomicKind::Store, order) if order != AtomicOrdering::SeqCst => {
                Opcode::Store { align: info.align, volatile: true }
            }
            _ => return Err(format!("`{spelled}` is not lowered for this target yet")),
        };
        let operands = instruction.operands[..instruction.operands.len() - 1].to_vec();
        let made = function.create_instruction(plain, instruction.ty, operands, Flags::default(), None);
        function.insert(made, crate::edit::Position::Before(inst)).map_err(|why| why.to_string())?;
        if let (Some(old), Some(new)) = (instruction.result, function.instruction(made).result) {
            function.replace_all_uses_with(old, Operand::Value(new));
        }
        function.erase(inst).map_err(|why| why.to_string())?;
    }
    // Their declarations have no caller left, and the assembler has no symbol
    // for one.
    let gone: Vec<bool> = module
        .globals
        .iter()
        .enumerate()
        .map(|(n, global)| !kept[n] && global.name.as_deref().is_some_and(|name| decode(name).is_some()))
        .collect();
    module.retain_globals(&|id| !gone[id.0 as usize]);
    Ok(())
}

#[cfg(test)]
#[path = "atomic_tests.rs"]
mod tests;
