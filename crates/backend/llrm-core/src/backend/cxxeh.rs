//! Make a C++ landing pad one the unwinder can enter, before instruction selection.
//!
//! # How it works
//! Clang's IR is the Itanium model: a pad is entered by the unwinder with the exception pointer and a selector, and
//! `resume` hands both back. A pad here is entered as QB's is (see [`super::ehprepare`]): the unwinder jumps to it with
//! no register intact, so what it carries is kept in memory, in `__eh_exception` and `__eh_selector`, and the pad reads
//! them. `resume` becomes a call of `__unwind_resume(exception, selector)`, which does not return.
//!
//! `llvm.eh.typeid.for(T)` is the selector the unwinder reports for a catch of `T`: the place of `T` among the catch
//! clauses of the function, counted from 1 in the order they first appear. The tables of the next stage number them the
//! same way.
//!
//! Values live into a pad are kept in stack slots, as for QB's pad ([`super::ehprepare`]'s `demote_live`).
//!
//! # Runs
//! In `driver::optimized`, with `ehprepare`; a module with no function using `__gxx_personality_v0` is untouched.
//!
//! # See also
//! The plan: `docs/working/cxx-eh.md`.

use llrm_mir::context::{ConstantId, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, GlobalKind, GlobalVariable, InstId, Linkage, Operand};
use llrm_mir::facts::Fact;
use llrm_mir::opcode::{CallInfo, CastOp, Flags, Opcode};
use llrm_mir::{GlobalId, Module};

use super::ehprepare::{demote_live, demote_phis};

pub const PERSONALITY: &str = "__gxx_personality_v0";
/// The exception pointer the unwinder kept for the pad it enters.
pub const EXCEPTION: &str = "__eh_exception";
/// The selector it kept.
pub const SELECTOR: &str = "__eh_selector";
/// Continues unwinding from a pad: `(exception, selector)`.
pub const RESUME: &str = "__unwind_resume";
const TYPEID_FOR: &str = "llvm.eh.typeid.for";

pub fn prepared(module: &mut Module) -> Result<(), String> {
    let used: Vec<String> = module
        .functions()
        .filter(|(_, _, function)| personality(module, function))
        .filter_map(|(_, global, _)| global.name.owned())
        .collect();
    if used.is_empty() {
        return Ok(());
    }
    let runtime = declare(module)?;
    let typeid_for: Vec<GlobalId> = module
        .functions()
        .filter(|(_, global, _)| global.name.as_deref().is_some_and(|name| name.starts_with(TYPEID_FOR)))
        .map(|(id, _, _)| id)
        .collect();
    for name in used {
        let (context, function) = module.function_mut(&name).expect("a function");
        let why = |what: String| format!("@{name}: {what}");
        let table = catch_types(context, function);
        typeids(context, function, &table, &typeid_for).map_err(why)?;
        resumed(context, function, &runtime).map_err(why)?;
        let pads: Vec<BlockId> =
            function.layout().iter().copied().filter(|&block| landing_pad(function, block).is_some()).collect();
        for &pad in &pads {
            read_from_memory(context, function, pad, &runtime).map_err(why)?;
        }
        for &pad in &pads {
            demote_phis(context, function, pad).map_err(why)?;
            demote_live(context, function, pad).map_err(why)?;
        }
    }
    Ok(())
}

struct Runtime {
    exception: ConstantId,
    selector: ConstantId,
    resume: ConstantId,
    resume_type: llrm_mir::TypeId,
}

fn declare(module: &mut Module) -> Result<Runtime, String> {
    let types = &mut module.context.types;
    let (void, i16, pointer) = (types.void(), types.int(16), types.ptr(0));
    let resume_type =
        types.intern(llrm_mir::Type::Function { returns: void, parameters: vec![pointer, i16], variadic: false });
    for (name, ty) in [(EXCEPTION, pointer), (SELECTOR, i16)] {
        if module.named(name).is_none() {
            module.add_variable(
                name,
                GlobalVariable { externally_initialized: false, ty, constant: false, initializer: None, align: None },
                Linkage::External,
            )?;
        }
    }
    if module.named(RESUME).is_none() {
        let id = module.add_function(RESUME, resume_type, Linkage::External)?;
        let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else { unreachable!("a function") };
        function.attrs.extend([Fact::NoReturn, Fact::NoUnwind].map(Fact::carrier));
    }
    let [exception, selector, resume] = [EXCEPTION, SELECTOR, RESUME].map(|one| module.named(one).expect("declared"));
    let [exception, selector, resume] = [exception, selector, resume].map(|one| module.reference(one));
    Ok(Runtime { exception, selector, resume, resume_type })
}

fn personality(
    module: &Module,
    function: &Function,
) -> bool {
    function.personality.is_some_and(|one| match module.context.get(one).kind {
        ConstantKind::Global(global) => module.global(global).name.as_deref() == Some(PERSONALITY),
        _ => false,
    })
}

fn landing_pad(
    function: &Function,
    block: BlockId,
) -> Option<InstId> {
    function.iter_block(block).find(|&one| matches!(function.instruction(one).opcode, Opcode::LandingPad { .. }))
}

/// The types the function's pads catch, each once, in the order they first appear.
fn catch_types(
    context: &llrm_mir::context::Context,
    function: &Function,
) -> Vec<ConstantId> {
    let mut table = Vec::new();
    for &block in function.layout() {
        for inst in function.iter_block(block) {
            let instruction = function.instruction(inst);
            let Opcode::LandingPad { clauses, .. } = &instruction.opcode else { continue };
            for (clause, operand) in clauses.iter().zip(&instruction.operands) {
                if let (llrm_mir::opcode::Clause::Catch, &Operand::Constant(one)) = (clause, operand)
                    && context.get(one).kind != ConstantKind::Null
                    && !table.contains(&one)
                {
                    table.push(one);
                }
            }
        }
    }
    table
}

/// Each call of `llvm.eh.typeid.for(T)` the constant place of `T` in `table`.
fn typeids(
    context: &llrm_mir::context::Context,
    function: &mut Function,
    table: &[ConstantId],
    typeid_for: &[GlobalId],
) -> Result<(), String> {
    let calls: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| callee(context, function, inst).is_some_and(|one| typeid_for.contains(&one)))
        .collect();
    for inst in calls {
        let instruction = function.instruction(inst);
        let (ty, result) = (instruction.ty, instruction.result.expect("a value"));
        let Some(&Operand::Constant(named)) = instruction.operands.first() else {
            return Err("llvm.eh.typeid.for of a type that is not constant".to_owned());
        };
        let place = table.iter().position(|&one| one == named).ok_or("llvm.eh.typeid.for of a type no pad catches")?;
        let id = Operand::Constant(context.int(ty, place as i128 + 1));
        function.replace_all_uses_with(result, id);
        function.erase(inst)?;
    }
    Ok(())
}

/// The callee of a direct call.
fn callee(
    context: &llrm_mir::context::Context,
    function: &Function,
    inst: InstId,
) -> Option<GlobalId> {
    let instruction = function.instruction(inst);
    if !matches!(instruction.opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
        return None;
    }
    match instruction.operands.last() {
        Some(&Operand::Constant(one)) => match context.get(one).kind {
            ConstantKind::Global(global) => Some(global),
            _ => None,
        },
        _ => None,
    }
}

/// Each `resume agg` a call of `__unwind_resume(agg.0, agg.1)` that does not return.
fn resumed(
    context: &llrm_mir::context::Context,
    function: &mut Function,
    runtime: &Runtime,
) -> Result<(), String> {
    let (void, i16, pointer) = (context.types.void(), context.types.int(16), context.types.ptr(0));
    let resumes: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| function.instruction(inst).opcode == Opcode::Resume)
        .collect();
    for inst in resumes {
        let block = function.parent(inst).expect("a placed terminator");
        let aggregate = function.instruction(inst).operands[0];
        let selector_type = {
            let Operand::Value(value) = aggregate else { return Err("a resume of a constant".to_owned()) };
            function.value(value).ty
        };
        let _ = selector_type;
        let member = |function: &mut Function, index: u32, ty| -> Result<Operand, String> {
            let made = function.create_instruction(
                Opcode::ExtractValue(vec![index]),
                ty,
                vec![aggregate],
                Flags::default(),
                None,
            );
            function.insert(made, Position::Before(inst))?;
            Ok(Operand::Value(function.instruction(made).result.expect("a value")))
        };
        let i32 = context.types.int(32);
        let exception = member(function, 0, pointer)?;
        let selector = member(function, 1, i32)?;
        let narrowed = function.create_instruction(
            Opcode::Cast(CastOp::Trunc),
            i16,
            vec![selector],
            Flags::default(),
            None,
        );
        function.insert(narrowed, Position::Before(inst))?;
        let narrowed = Operand::Value(function.instruction(narrowed).result.expect("a value"));
        let info = CallInfo {
            function_type: runtime.resume_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(), Vec::new()],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        let call = function.create_instruction(
            Opcode::Call(Box::new(info)),
            void,
            vec![exception, narrowed, Operand::Constant(runtime.resume)],
            Flags::default(),
            None,
        );
        function.insert(call, Position::Before(inst))?;
        function.erase(inst)?;
        let end = function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
        function.insert(end, Position::End(block))?;
    }
    Ok(())
}

/// The pad's members read from what the unwinder kept: `extractvalue 0` is the exception pointer, `1` the selector.
fn read_from_memory(
    context: &llrm_mir::context::Context,
    function: &mut Function,
    pad: BlockId,
    runtime: &Runtime,
) -> Result<(), String> {
    let lp = landing_pad(function, pad).expect("a pad");
    let Some(value) = function.instruction(lp).result else { return Ok(()) };
    let (i16, pointer) = (context.types.int(16), context.types.ptr(0));
    // Each user is replaced and erased, so the list shrinks to empty or the loop leaves with an error.
    while let Some(&one) = function.users(value).iter().next() {
        let user = function.instruction(one.user);
        let (ty, result) = (user.ty, user.result.expect("a value"));
        let (kept, kept_type, cell) = match &user.opcode {
            Opcode::ExtractValue(indices) if indices[..] == [0] => (runtime.exception, pointer, pointer),
            Opcode::ExtractValue(indices) if indices[..] == [1] => (runtime.selector, i16, i16),
            _ => return Err("a landing pad read other than by extractvalue".to_owned()),
        };
        let _ = cell;
        let load = function.create_instruction(
            Opcode::Load { align: None, volatile: false },
            kept_type,
            vec![Operand::Constant(kept)],
            Flags::default(),
            Some("unwound"),
        );
        function.insert(load, Position::Before(one.user))?;
        let loaded = Operand::Value(function.instruction(load).result.expect("a value"));
        let widened = if ty == kept_type {
            loaded
        } else {
            let cast =
                function.create_instruction(Opcode::Cast(CastOp::SExt), ty, vec![loaded], Flags::default(), None);
            function.insert(cast, Position::Before(one.user))?;
            Operand::Value(function.instruction(cast).result.expect("a value"))
        };
        function.replace_all_uses_with(result, widened);
        function.erase(one.user)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "cxxeh_tests.rs"]
mod tests;
