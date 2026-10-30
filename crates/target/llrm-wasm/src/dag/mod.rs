//! IR to wasm machine code through the DAG: the builder's view of the target, its
//! calling lowering, the selection pipeline and the emitter, then the machine passes in
//! the pin's `-O0` order.

pub mod call;
pub mod emit;
pub mod function;
pub mod pipeline;

use llrm_mir::module::Module;
use llrm_mir::types::Type;

use crate::argument_move::argument_move;
use crate::explicit_locals::explicit_locals;
use crate::mir::{Function, ValType};
use crate::reg_stackify::reg_stackify;
use crate::set_p2align::set_p2align_operands;
use crate::Width;

/// The wasm value type of an IR scalar type.
fn value_type(
    module: &Module,
    ty: llrm_mir::types::TypeId,
    width: Width,
) -> Result<ValType, String> {
    use llrm_mir::types::FloatKind;
    match module.context.types.get(ty) {
        Type::Int(bits) if *bits <= 32 => Ok(ValType::I32),
        Type::Int(64) => Ok(ValType::I64),
        Type::Pointer(_) => Ok(if width == Width::W32 { ValType::I32 } else { ValType::I64 }),
        Type::Float(FloatKind::Float) => Ok(ValType::F32),
        Type::Float(FloatKind::Double) => Ok(ValType::F64),
        other => Err(format!("a wasm signature with {other:?}")),
    }
}

/// `computeSignatureVTs` for the slice: the parameters and the results (none for `void`).
fn signature(
    module: &Module,
    name: &str,
    width: Width,
) -> Result<(Vec<ValType>, Vec<ValType>), String> {
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let function = module.global(id).function().ok_or_else(|| format!("@{name} is no function"))?;
    let Type::Function { returns, parameters, .. } = module.context.types.get(function.ty) else {
        return Err(format!("@{name} has no function type"));
    };
    let params = parameters.iter().map(|&ty| value_type(module, ty, width)).collect::<Result<Vec<_>, _>>()?;
    let results = match module.context.types.get(*returns) {
        Type::Void => Vec::new(),
        _ => vec![value_type(module, *returns, width)?],
    };
    Ok((params, results))
}

/// Which passes run.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// `RegStackify`: the pin runs it at `-O0`, LLVM 20's `llc -O0` does not.
    pub stackify: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options { stackify: true }
    }
}

/// Function `name` of `module` as wasm machine code after the `-O0` passes up to
/// `ExplicitLocals`.
pub fn compile_function(
    module: &Module,
    name: &str,
    width: Width,
    options: Options,
) -> Result<Function, String> {
    let blocks = function::select_function(module, name, width)?;
    let (params, results) = signature(module, name, width)?;
    let names = |symbol: u32| module.globals[symbol as usize].name.as_deref().unwrap_or("").to_string();
    let mut f = emit::emit(name, &params, &results, &blocks, &names);
    argument_move(&mut f);
    set_p2align_operands(&mut f);
    if options.stackify {
        reg_stackify(&mut f);
    }
    explicit_locals(&mut f);
    Ok(f)
}
