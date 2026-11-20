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
        if let Some(bits) = bits.filter(|&bits| bits > native) {
            libcall(module, at, inst, info, bits)?;
            continue;
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
        .map(|(n, global)| {
            !kept.get(n).copied().unwrap_or(false) && global.name.as_deref().is_some_and(|name| decode(name).is_some())
        })
        .collect();
    module.retain_globals(&|id| !gone[id.0 as usize]);
    Ok(())
}

/// The `__atomic_*` function that stands for `info` at `bytes`, and its
/// parameter list: `T load(ptr, order)`, `void store(ptr, T, order)`,
/// `T exchange|fetch_OP(ptr, T, order)`, `bool compare_exchange(ptr, ptr
/// expected, T, success, failure)`.
fn libcall_name(
    info: AtomicInfo,
    bytes: u32,
) -> Option<String> {
    let op = match info.kind {
        AtomicKind::Load => "load".to_owned(),
        AtomicKind::Store => "store".to_owned(),
        AtomicKind::CmpXchg { .. } => "compare_exchange".to_owned(),
        AtomicKind::Rmw(RmwOp::Xchg) => "exchange".to_owned(),
        AtomicKind::Rmw(op @ (RmwOp::Add | RmwOp::Sub | RmwOp::And | RmwOp::Nand | RmwOp::Or | RmwOp::Xor)) => {
            format!("fetch_{}", RMW_OPS.iter().find(|(one, _)| *one == op)?.1)
        }
        _ => return None,
    };
    Some(format!("__atomic_{op}_{bytes}"))
}

/// The C ABI's `memory_order` (`toCABI`): `relaxed` is 0 for `unordered` and
/// `monotonic`.
fn memory_order(ordering: AtomicOrdering) -> i128 {
    match ordering {
        AtomicOrdering::Unordered | AtomicOrdering::Monotonic => 0,
        AtomicOrdering::Acquire => 2,
        AtomicOrdering::Release => 3,
        AtomicOrdering::AcqRel => 4,
        AtomicOrdering::SeqCst => 5,
    }
}

/// `AtomicExpandPass::expandAtomicOpToLibcall`: an access wider than the
/// target's widest atomic one (`bits`) is a call of the runtime's
/// `__atomic_*_N` (libatomic, compiler-rt), as the pin's `llc` makes it on a
/// 486 for `i64`. A compare-exchange passes its expected value through a stack
/// slot and gets it back with the result; a read-modify-write the runtime has
/// no `fetch_` for (`max`, `min`, `umax`, `umin`) is refused: the pass's
/// compare-exchange loop is not translated.
fn libcall(
    module: &mut Module,
    at: usize,
    inst: InstId,
    info: AtomicInfo,
    bits: u32,
) -> Result<(), String> {
    let spelled = format!("{} {}", kind_name(info.kind), info.ordering.spelling());
    let name =
        (matches!(bits, 8 | 16 | 32 | 64 | 128)).then(|| libcall_name(info, bits / 8)).flatten().ok_or_else(|| {
            format!("`{spelled}` of {bits} bits is not supported: no `__atomic_*` function stands for it")
        })?;
    let types = &module.context.types;
    let (pointer, void, i8_type, i32_type) = (types.ptr(0), types.void(), types.int(8), types.int(32));
    let GlobalKind::Function(function) = &module.globals[at].kind else { return Ok(()) };
    let call = function.instruction(inst).clone();
    let operands = call.operands[..call.operands.len() - 1].to_vec();
    // (what the call returns, its parameters)
    let (value_type, parameters, returns) = match info.kind {
        AtomicKind::Load => (call.ty, vec![pointer, i32_type], call.ty),
        AtomicKind::Store => {
            let value = function.operand_type(&module.context, operands[0]).ok_or("a stored value has a type")?;
            (value, vec![pointer, value, i32_type], void)
        }
        AtomicKind::Rmw(_) => {
            let value = function.operand_type(&module.context, operands[1]).ok_or("an operand has a type")?;
            (value, vec![pointer, value, i32_type], value)
        }
        AtomicKind::CmpXchg { .. } => {
            let value = function.operand_type(&module.context, operands[1]).ok_or("an operand has a type")?;
            (value, vec![pointer, pointer, value, i32_type, i32_type], i8_type)
        }
        AtomicKind::Fence => return Err(format!("`{spelled}` has no `__atomic_*` function")),
    };
    let signature = module.context.types.intern(Type::Function { returns, parameters, variadic: false });
    let callee = match module.named(&name) {
        Some(id) => id,
        None => module.add_function(&name, signature, Linkage::External)?,
    };
    let reference = module.reference(callee);
    let order = |module: &Module, ordering| Operand::Constant(module.context.int(i32_type, memory_order(ordering)));
    let (first_order, second_order) = match info.kind {
        AtomicKind::CmpXchg { failure, .. } => (order(module, info.ordering), Some(order(module, failure))),
        _ => (order(module, info.ordering), None),
    };
    let GlobalKind::Function(function) = &mut module.globals[at].kind else { return Ok(()) };
    let place = crate::edit::Position::Before(inst);
    let slot = if matches!(info.kind, AtomicKind::CmpXchg { .. }) {
        // the expected value's slot, among the entry block's allocas
        let entry = function.entry().ok_or("an atomic in a function with a body")?;
        let first =
            function.iter_block(entry).find(|&one| !matches!(function.instruction(one).opcode, Opcode::Alloca { .. }));
        let alloca = function.create_instruction(
            Opcode::Alloca { allocated: value_type, align: None, address_space: 0 },
            pointer,
            Vec::new(),
            Flags::default(),
            Some("atomic.expected"),
        );
        function.insert(alloca, first.map_or(crate::edit::Position::End(entry), crate::edit::Position::Before))?;
        let slot = Operand::Value(function.instruction(alloca).result.ok_or("an alloca is a pointer")?);
        let store = function.create_instruction(
            Opcode::Store { align: None, volatile: false },
            void,
            vec![operands[1], slot],
            Flags::default(),
            None,
        );
        function.insert(store, place).map_err(|why| why.to_string())?;
        Some(slot)
    } else {
        None
    };
    let mut arguments: Vec<Operand> = match info.kind {
        AtomicKind::Load => vec![operands[0]],
        AtomicKind::Store => vec![operands[1], operands[0]],
        AtomicKind::Rmw(_) => vec![operands[0], operands[1]],
        AtomicKind::CmpXchg { .. } => vec![operands[0], slot.ok_or("a slot")?, operands[2]],
        AtomicKind::Fence => Vec::new(),
    };
    arguments.push(first_order);
    arguments.extend(second_order);
    let info_call = CallInfo {
        function_type: signature,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new(); arguments.len()],
        attrs: Vec::new(),
        tail: Tail::default(),
    };
    arguments.push(Operand::Constant(reference));
    let made =
        function.create_instruction(Opcode::Call(Box::new(info_call)), returns, arguments, Flags::default(), None);
    function.insert(made, place).map_err(|why| why.to_string())?;
    let result = function.instruction(made).result;
    if let AtomicKind::CmpXchg { .. } = info.kind {
        // `{ T, i1 }`: the slot's value and whether the call said it swapped
        let flag_type = module.context.types.int(1);
        let zero = Operand::Constant(module.context.int(i8_type, 0));
        let poison = Operand::Constant(
            module
                .context
                .constant(crate::context::Constant { ty: call.ty, kind: crate::context::ConstantKind::Poison }),
        );
        let GlobalKind::Function(function) = &mut module.globals[at].kind else { return Ok(()) };
        let loaded = function.create_instruction(
            Opcode::Load { align: None, volatile: false },
            value_type,
            vec![slot.ok_or("a slot")?],
            Flags::default(),
            None,
        );
        function.insert(loaded, place).map_err(|why| why.to_string())?;
        let swapped = function.create_instruction(
            Opcode::ICmp(crate::opcode::IntPredicate::Ne),
            flag_type,
            vec![Operand::Value(result.ok_or("a call result")?), zero],
            Flags::default(),
            None,
        );
        function.insert(swapped, place).map_err(|why| why.to_string())?;
        let with_value = function.create_instruction(
            Opcode::InsertValue(vec![0]),
            call.ty,
            vec![poison, Operand::Value(function.instruction(loaded).result.ok_or("a load result")?)],
            Flags::default(),
            None,
        );
        function.insert(with_value, place).map_err(|why| why.to_string())?;
        let both = function.create_instruction(
            Opcode::InsertValue(vec![1]),
            call.ty,
            vec![
                Operand::Value(function.instruction(with_value).result.ok_or("an insertvalue result")?),
                Operand::Value(function.instruction(swapped).result.ok_or("a compare result")?),
            ],
            Flags::default(),
            None,
        );
        function.insert(both, place).map_err(|why| why.to_string())?;
        if let (Some(old), Some(new)) = (call.result, function.instruction(both).result) {
            function.replace_all_uses_with(old, Operand::Value(new));
        }
    } else if let (Some(old), Some(new)) = (call.result, result) {
        function.replace_all_uses_with(old, Operand::Value(new));
    }
    let GlobalKind::Function(function) = &mut module.globals[at].kind else { return Ok(()) };
    function.erase(inst).map_err(|why| why.to_string())?;
    Ok(())
}

/// `LowerAtomicPass`, which a target without atomics runs before selection
/// (wasm without the `atomics` feature): an atomic load or store is a plain
/// one, a fence is nothing. A read-modify-write and a compare-exchange are
/// refused until the pass's expansion into a load, an operation and a store is
/// translated.
pub fn lowered_single_thread(module: &mut Module) -> Result<(), String> {
    let mut found: Vec<(usize, InstId, AtomicInfo)> = Vec::new();
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        for (_, inst) in function.walk() {
            if let Opcode::Atomic(info) = function.instruction(inst).opcode {
                found.push((at, inst, info));
            }
        }
    }
    for (at, inst, info) in found {
        let GlobalKind::Function(function) = &mut module.globals[at].kind else { continue };
        let instruction = function.instruction(inst).clone();
        let plain = match info.kind {
            AtomicKind::Fence => {
                function.erase(inst).map_err(|why| why.to_string())?;
                continue;
            }
            AtomicKind::Load => Opcode::Load { align: info.align, volatile: info.volatile },
            AtomicKind::Store => Opcode::Store { align: info.align, volatile: info.volatile },
            _ => return Err(format!("`{}` is not lowered without atomics yet", kind_name(info.kind))),
        };
        let made =
            function.create_instruction(plain, instruction.ty, instruction.operands.clone(), Flags::default(), None);
        function.insert(made, crate::edit::Position::Before(inst)).map_err(|why| why.to_string())?;
        if let (Some(old), Some(new)) = (instruction.result, function.instruction(made).result) {
            function.replace_all_uses_with(old, Operand::Value(new));
        }
        function.erase(inst).map_err(|why| why.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "atomic_tests.rs"]
mod tests;
