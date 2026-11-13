//! IR to wasm machine code through the DAG: the builder's view of the target,
//! its calling lowering, the selection pipeline and the emitter, then the
//! machine passes in the pin's `-O0` order.

pub mod alias;
pub mod call;
pub mod emit;
pub mod function;
pub mod pipeline;

use llrm_mir::module::Module;
use llrm_mir::types::{FloatKind, Type};

use crate::Width;
use crate::argument_move::argument_move;
use crate::cfg_sort::cfg_sort;
use crate::cfg_stackify::cfg_stackify;
use crate::custom_inserter::lower_memory_pseudos;
use crate::explicit_locals::explicit_locals;
use crate::fix_br_table_defaults::fix_br_table_defaults;
use crate::fix_irreducible::fix_irreducible_control_flow;
use crate::frame::frame_lowering;
use crate::late_eh_prepare::late_eh_prepare;
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
    match module.context.types.get(ty) {
        Type::Int(bits) if *bits <= 32 => Ok(ValType::I32),
        Type::Int(64) => Ok(ValType::I64),
        Type::Pointer(_) => Ok(if width == Width::W32 { ValType::I32 } else { ValType::I64 }),
        Type::Float(FloatKind::Float) => Ok(ValType::F32),
        Type::Float(FloatKind::Double) => Ok(ValType::F64),
        other => Err(format!("a wasm signature with {other:?}")),
    }
}

/// An `i128` or an `f128` (softened to the `i128` of its bits): two `i64` parts.
pub(crate) fn is_wide(ty: &Type) -> bool {
    matches!(ty, Type::Int(128) | Type::Float(FloatKind::F128))
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
    let pointer = if width == Width::W32 { ValType::I32 } else { ValType::I64 };
    // `canLowerReturn` is false for an `i128` (two parts, no multivalue ABI):
    // the result goes through a pointer, the first parameter.
    let returns_i128 = is_wide(module.context.types.get(*returns));
    let mut params: Vec<ValType> = if returns_i128 { vec![pointer] } else { Vec::new() };
    for &ty in parameters {
        // An `i128` argument is two `i64`, low part first.
        if is_wide(module.context.types.get(ty)) {
            params.extend([ValType::I64, ValType::I64]);
        } else {
            params.push(value_type(module, ty, width)?);
        }
    }
    if *variadic {
        // `computeSignatureVTs`: the pointer to the variadic arguments' buffer.
        params.push(if width == Width::W32 { ValType::I32 } else { ValType::I64 });
    }
    let results = match module.context.types.get(*returns) {
        Type::Void => Vec::new(),
        _ if returns_i128 => Vec::new(),
        _ => vec![value_type(module, *returns, width)?],
    };
    Ok((params, results))
}

/// `-exception-model=` of the pin's `llc` (`TargetOptions::ExceptionModel`),
/// the two it takes for wasm.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExceptionModel {
    /// No unwinding: an `invoke` is a call and a `landingpad` builds nothing.
    #[default]
    None,
    /// `-exception-model=wasm` with `-mattr=+exception-handling`: `try`,
    /// `catch` and `throw`.
    Wasm,
}

/// Which passes run.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// `RegStackify`: the pin runs it at `-O0`, LLVM 20's `llc -O0` does not.
    pub stackify: bool,
    pub exception_model: ExceptionModel,
    /// How far the shared mid-end goes before the backend.
    pub level: crate::driver::Level,
}

impl Default for Options {
    fn default() -> Options {
        Options { stackify: true, exception_model: ExceptionModel::None, level: crate::driver::Level::O0 }
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
    compile_function_with(module, name, width, options, &ModuleFacts::of(module))
}

/// What the callees of any function of one module do. Built once per module
/// and shared by every function compiled from it: a scan of the module for
/// each function was quadratic in the function count.
#[derive(Clone, Default)]
pub struct ModuleFacts {
    nothrow: std::sync::Arc<llrm_support::hash::HashSet<String>>,
    callee_memory: std::sync::Arc<llrm_support::hash::HashMap<String, (bool, bool)>>,
    tables: std::cell::OnceCell<llrm_codegen::backend::constpool::Symbols>,
}

impl ModuleFacts {
    pub fn of(module: &Module) -> Self {
        MODULE_FACTS_BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            nothrow: std::sync::Arc::new(non_throwing(module)),
            callee_memory: std::sync::Arc::new(callee_memory(module)),
            tables: Default::default(),
        }
    }

    /// The names and locality of the module's globals, made at the first
    /// function.
    fn tables(
        &self,
        module: &Module,
        dso_local_symbols: bool,
    ) -> llrm_codegen::backend::constpool::Symbols {
        self.tables
            .get_or_init(|| {
                llrm_codegen::backend::selectiondag::function_lowering::module_tables(module, dso_local_symbols, false)
            })
            .clone()
    }
}

/// [`compile_function`] with the module's [`ModuleFacts`] already built.
pub fn compile_function_with(
    module: &Module,
    name: &str,
    width: Width,
    options: Options,
    facts: &ModuleFacts,
) -> Result<Function, String> {
    let unwinding = options.exception_model == ExceptionModel::Wasm;
    let blocks =
        function::select_function(module, name, width, unwinding, options.level != crate::driver::Level::O0, facts)?;
    let (params, results) = signature(module, name, width)?;
    let names = |symbol: u32| crate::driver::symbol_name(&module.globals[symbol as usize]);
    let mut f = emit::emit(name, &params, &results, &blocks, &names);
    f.variables = blocks
        .first()
        .map(|first| {
            first.declared.iter().map(|&(record, frame)| crate::mir::Variable { record, frame, disp: 0 }).collect()
        })
        .unwrap_or_default();
    f.argument_variables = blocks.first().map(|first| first.argument_variables.clone()).unwrap_or_default();
    f.nothrow = facts.nothrow.clone();
    f.callee_memory = facts.callee_memory.clone();
    f.personality = unwinding
        && module.named(name).and_then(|id| module.global(id).function()).is_some_and(|one| one.personality.is_some());
    // The pin's order after instruction selection: `-print-after-all` of `llc
    // -O0`.
    let global = module.named(name).map(|id| module.global(id));
    f.binding = global.map_or_else(Default::default, |global| crate::mir::Binding::of(global.linkage));
    f.hidden = global.is_some_and(|global| global.visibility == llrm_mir::Visibility::Hidden);
    if let Some(global) = global {
        f.name = crate::driver::symbol_name(global);
    }
    lower_memory_pseudos(&mut f);
    // A routine the legalizer or the builder calls (`fmod`, `memcpy`) is in no
    // module: its signature is what the call passes and returns.
    let library_calls = library_callees(&f, module, if width == Width::W32 { ValType::I32 } else { ValType::I64 });
    argument_move(&mut f);
    set_p2align_operands(&mut f);
    fix_br_table_defaults(&mut f);
    if options.level != crate::driver::Level::O0 {
        if crate::early_tail_duplicate::early_tail_duplicate(&mut f) {
            remove_unreachable_blocks(&mut f);
        }
        crate::stack_coloring::stack_coloring(&mut f, blocks.first().map_or(&[], |block| &block.stack_colors));
        crate::dead_mi::dead_machine_instruction_elim(&mut f);
        crate::machine_licm::machine_licm(&mut f);
        crate::machine_cse::machine_cse(&mut f);
        crate::machine_sink::machine_sink(&mut f);
        crate::peephole_opt::peephole_opt(&mut f);
        crate::dead_mi::dead_machine_instruction_elim(&mut f);
        f.edge_probs.clear();
    }
    if options.level != crate::driver::Level::O0 {
        crate::phi_elimination::split_phi_edges(&mut f);
    }
    phi_elimination(&mut f);
    frame_lowering(&mut f, if width == Width::W32 { ValType::I32 } else { ValType::I64 })?;
    remove_unreachable_blocks(&mut f);
    late_eh_prepare(&mut f);
    // The pin turns off `RegisterCoalescer` at -O1.
    if matches!(
        options.level,
        crate::driver::Level::O2 | crate::driver::Level::O3 | crate::driver::Level::Os | crate::driver::Level::Oz
    ) {
        crate::spike_coalesce::coalesce(&mut f);
    }
    let for_size = matches!(
        options.level,
        crate::driver::Level::Os | crate::driver::Level::Oz
    );
    if options.level != crate::driver::Level::O0 {
        // `BranchFolder::OptimizeFunction`: tail merging, then the branches,
        // until neither changes anything.
        for _ in 0..=f.blocks.len() {
            let mut changed = crate::tail_merge::tail_merge(&mut f, for_size);
            changed |= crate::branch_fold::forward_branch_only_blocks(&mut f);
            changed |= crate::branch_fold::fold_branches(&mut f);
            if !changed {
                break;
            }
            remove_unreachable_blocks(&mut f);
        }
    }
    if options.level != crate::driver::Level::O0 {
        if crate::tail_duplicate::tail_duplicate(&mut f, for_size) {
            remove_unreachable_blocks(&mut f);
        }
    }
    // The pin runs `FixIrreducibleControlFlow` once, at every level, after
    // branch folding and tail duplication (a merge can make a loop with two
    // entries): run before the coalescer, its dispatch copies stay (x_switch
    // -O2 668 B against the pin's 490).
    fix_irreducible_control_flow(&mut f);
    if options.level != crate::driver::Level::O0 {
        crate::optimize_live_intervals::optimize_live_intervals(&mut f);
        crate::mem_intrinsic_results::mem_intrinsic_results(&mut f);
    }
    if options.stackify {
        reg_stackify(&mut f, options.level != crate::driver::Level::O0);
    }
    if options.stackify && options.level != crate::driver::Level::O0 {
        crate::reg_coloring::reg_coloring(&mut f);
    }
    cfg_sort(&mut f);
    cfg_stackify(&mut f)?;
    explicit_locals(&mut f);
    lower_br_unless(&mut f);
    if options.level != crate::driver::Level::O0 {
        crate::peephole::peephole(&mut f);
    }
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

/// How many times the per-module callee facts were built: a test hook.
pub static MODULE_FACTS_BUILT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// What each function of `module` reads and writes (`doesNotAccessMemory`,
/// `onlyReadsMemory`); a callee that is no function of it is assumed to do
/// both.
fn callee_memory(module: &Module) -> llrm_support::hash::HashMap<String, (bool, bool)> {
    module
        .functions()
        .filter_map(|(_, global, function)| {
            let effects = llrm_mir::memory::stated(&function.attrs);
            Some((global.name.owned()?, (effects.reads, effects.writes)))
        })
        .collect()
}

/// The callees `WebAssembly::mayThrow` takes as not throwing: functions marked
/// `nounwind`, the three that never throw, and the libcalls it lists.
fn non_throwing(module: &Module) -> llrm_support::hash::HashSet<String> {
    let mut found: llrm_support::hash::HashSet<String> = module
        .functions()
        .filter(|(_, global, function)| {
            function.attrs.iter().any(|one| {
                matches!(
                    one,
                    llrm_mir::opcode::Attribute::Flag(name) if name == "nounwind"
                )
            }) || matches!(
                global.name.as_deref(),
                Some("__cxa_begin_catch" | "_Unwind_Wasm_CallPersonality" | "_ZSt9terminatev")
            )
        })
        .filter_map(|(_, global, _)| global.name.owned())
        .collect();
    for libcall in ["memcpy", "memmove", "memset"] {
        if module.named(libcall).is_none() {
            found.insert(libcall.to_owned());
        }
    }
    found
}

/// The signature of each called symbol that is no function of `module`, from
/// its first call.
fn library_callees(
    f: &Function,
    module: &Module,
    pointer: ValType,
) -> llrm_support::hash::HashMap<String, (Vec<ValType>, Vec<ValType>)> {
    let mut found = llrm_support::hash::HashMap::default();
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
            .filter_map(|operand| match operand {
                Operand::Use(reg) => Some(f.vreg_type(*reg)),
                // A frame object's address, before the frame lowering gives it
                // a register (an `sret` slot).
                Operand::Frame(_) => Some(pointer),
                _ => None,
            })
            .collect();
        found.entry(name.clone()).or_insert((params, results));
    }
    found
}
