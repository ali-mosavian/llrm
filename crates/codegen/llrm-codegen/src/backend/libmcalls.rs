//! Turns the rounding intrinsics x87 has no instruction for into calls of the
//! C library: `llvm.floor.f64` is a call of `floor`.
//! Translated from the legalizer's libcall expansion (`ConvertNodeToLibcall`),
//! which the DAG path does not have for floating point yet.
//!
//! # How it works
//! `floor`, `ceil`, `trunc`, `round`, `roundeven` and `nearbyint` of `f64`,
//! `f32` (`floorf`) and `f80` (`floorl`) are each a call of the routine of that
//! name in the `libcalls` given, with its convention and its symbol. A name the
//! list lacks leaves the intrinsic alone. The result keeps the callee's
//! function type.
//!
//! # Example
//! `%a = call double @llvm.floor.f64(double %x)` becomes
//! `%a = call double @floor(double %x)`.
//!
//! # Runs
//! Once per module in `driver::optimized`, after the pipeline: the optimizer
//! folds and combines the intrinsic, not the call.
//!
//! # See also
//! LLVM's `RuntimeLibcalls` (FLOOR_F64, CEIL_F64 and the rest of the family).

use llrm_mir::edit::Position;
use llrm_mir::module::{GlobalKind, InstId, Linkage, Module, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::types::TypeId;

const FAMILY: [&str; 6] = ["floor", "ceil", "trunc", "round", "roundeven", "nearbyint"];

/// The C library routine of the intrinsic `name`, if it is one of the family.
fn routine(name: &str) -> Option<String> {
    let rest = name.strip_prefix("llvm.")?;
    let (op, ty) = rest.split_once('.')?;
    let suffix = match ty {
        "f64" => "",
        "f32" => "f",
        "f80" => "l",
        _ => return None,
    };
    FAMILY.contains(&op).then(|| format!("{op}{suffix}"))
}

/// Every call of the family in `module` as a call of its routine in `libcalls`
/// (name, LLVM's convention number, symbol).
pub fn expanded(
    module: &mut Module,
    libcalls: &[(String, u32, String)],
) -> Result<(), String> {
    let routines: Vec<Option<&(String, u32, String)>> = module
        .globals
        .iter()
        .map(|global| {
            let name = routine(global.name.as_deref()?)?;
            libcalls.iter().find(|(one, ..)| *one == name)
        })
        .collect();
    let mut found: Vec<(usize, InstId, usize, TypeId)> = Vec::new();
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        for (_, inst) in function.walk() {
            let Some(callee) = llrm_mir::memory::callee(&module.context, function, inst) else { continue };
            let (Some(_), Opcode::Call(info)) = (routines[callee.0 as usize], &function.instruction(inst).opcode)
            else {
                continue;
            };
            found.push((at, inst, callee.0 as usize, info.function_type));
        }
    }
    let mut references = std::collections::BTreeMap::new();
    for &(_, _, callee, ty) in &found {
        let (_, _, symbol) = routines[callee].expect("picked above");
        if references.contains_key(symbol) {
            continue;
        }
        let id = match module.named(symbol) {
            Some(id) => id,
            None => module.add_function(symbol, ty, Linkage::External)?,
        };
        references.insert(symbol.clone(), module.reference(id));
    }
    for (at, call, callee, _) in found {
        let (_, convention, symbol) = routines[callee].expect("picked above");
        let GlobalKind::Function(function) = &mut module.globals[at].kind else { continue };
        let instruction = function.instruction(call).clone();
        let Opcode::Call(info) = &instruction.opcode else { continue };
        let mut info = info.clone();
        info.calling_convention = *convention;
        let mut operands = instruction.operands.clone();
        *operands.last_mut().ok_or("a callee")? = Operand::Constant(references[symbol]);
        let made = function.create_instruction(Opcode::Call(info), instruction.ty, operands, Flags::default(), None);
        function.insert(made, Position::Before(call)).map_err(|why| why.to_string())?;
        if let (Some(old), Some(new)) = (instruction.result, function.instruction(made).result) {
            function.replace_all_uses_with(old, Operand::Value(new));
        }
        function.erase(call).map_err(|why| why.to_string())?;
    }
    Ok(())
}
