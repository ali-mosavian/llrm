//! IR to wasm machine code through the DAG: the builder's view of the target,
//! its calling lowering, the selection pipeline and the emitter, then the
//! machine passes in the pin's `-O0` order.

pub mod call;
pub mod emit;
pub mod function;
pub mod pipeline;

use llrm_mir::module::Module;
use llrm_mir::types::Type;

use crate::Width;
use crate::argument_move::argument_move;
use crate::cfg_sort::cfg_sort;
use crate::cfg_stackify::cfg_stackify;
use crate::custom_inserter::lower_memory_pseudos;
use crate::explicit_locals::explicit_locals;
use crate::fix_br_table_defaults::fix_br_table_defaults;
use crate::fix_irreducible::fix_irreducible_control_flow;
use crate::frame::frame_lowering;
use crate::lower_br_unless::lower_br_unless;
use crate::mir::{Function, Operand, ValType};
use crate::phi_elimination::phi_elimination;
use crate::reg_stackify::reg_stackify;
use crate::set_p2align::set_p2align_operands;
use crate::unreachable_blocks::remove_unreachable_blocks;

/// The wasm value type of an IR scalar type.
pub(crate) fn value_type(
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

/// `computeSignatureVTs` for the slice: the parameters and the results (none
/// for `void`).
pub fn signature(
    module: &Module,
    name: &str,
    width: Width,
) -> Result<(Vec<ValType>, Vec<ValType>), String> {
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let function = module.global(id).function().ok_or_else(|| format!("@{name} is no function"))?;
    let Type::Function { returns, parameters, variadic } = module.context.types.get(function.ty) else {
        return Err(format!("@{name} has no function type"));
    };
    let mut params = parameters.iter().map(|&ty| value_type(module, ty, width)).collect::<Result<Vec<_>, _>>()?;
    if *variadic {
        // `computeSignatureVTs`: the pointer to the variadic arguments' buffer.
        params.push(if width == Width::W32 { ValType::I32 } else { ValType::I64 });
    }
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

/// Function `name` of `module` as wasm machine code after the `-O0` passes up
/// to `ExplicitLocals`.
pub fn compile_function(
    module: &Module,
    name: &str,
    width: Width,
    options: Options,
) -> Result<Function, String> {
    let blocks = function::select_function(module, name, width)?;
    let (params, results) = signature(module, name, width)?;
    let names = |symbol: u32| crate::driver::symbol_name(&module.globals[symbol as usize]);
    let mut f = emit::emit(name, &params, &results, &blocks, &names);
    // The pin's order after instruction selection: `-print-after-all` of `llc
    // -O0`.
    f.binding =
        module.named(name).map_or_else(Default::default, |id| crate::mir::Binding::of(module.global(id).linkage));
    lower_memory_pseudos(&mut f);
    // A routine the legalizer or the builder calls (`fmod`, `memcpy`) is in no module: its signature
    // is what the call passes and returns.
    let library_calls = library_callees(&f, module);
    argument_move(&mut f);
    set_p2align_operands(&mut f);
    fix_br_table_defaults(&mut f);
    phi_elimination(&mut f);
    frame_lowering(&mut f, if width == Width::W32 { ValType::I32 } else { ValType::I64 })?;
    remove_unreachable_blocks(&mut f);
    fix_irreducible_control_flow(&mut f);
    if options.stackify {
        reg_stackify(&mut f);
    }
    cfg_sort(&mut f);
    cfg_stackify(&mut f);
    explicit_locals(&mut f);
    lower_br_unless(&mut f);
    for (symbol, _) in f.symbols.clone() {
        if module.named(&symbol).is_some_and(|id| module.global(id).function().is_some()) {
            let (params, results) = signature(module, &symbol, width)?;
            f.callees.push((symbol, params, results));
        } else if let Some((params, results)) = library_calls.get(&symbol) {
            f.callees.push((symbol, params.clone(), results.clone()));
        }
    }
    Ok(f)
}

/// The signature of each called symbol that is no function of `module`, from its first call.
fn library_callees(
    f: &Function,
    module: &Module,
) -> std::collections::HashMap<String, (Vec<ValType>, Vec<ValType>)> {
    let mut found = std::collections::HashMap::new();
    for inst in f.blocks.iter().flatten().filter(|inst| inst.opcode == crate::dagisel::post_ids::CALL) {
        let Some(&Operand::Sym(at)) = inst.operands.iter().find(|operand| matches!(operand, Operand::Sym(_))) else {
            continue;
        };
        let name = &f.symbols[at as usize].0;
        if module.named(name).is_some_and(|id| module.global(id).function().is_some()) {
            continue;
        }
        let results = inst.defs().map(|reg| f.vreg_type(reg)).collect();
        let params = inst
            .operands
            .iter()
            .filter_map(|operand| if let Operand::Use(reg) = operand { Some(f.vreg_type(*reg)) } else { None })
            .collect();
        found.entry(name.clone()).or_insert((params, results));
    }
    found
}
