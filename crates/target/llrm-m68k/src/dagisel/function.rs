//! A function from IR to scheduled machine nodes: the shared builder, then
//! [`super::pipeline`]'s stages, block by block.
//!
//! DEVIATE: no static allocas, jump tables, switches or live-in registers: the
//! builder makes the frame objects and the C convention passes everything on
//! the stack, so the slice (arithmetic, branches, calls) needs none. Reason:
//! frame objects wait for the m68k machine function. Exit: M4.

use std::sync::LazyLock;

use llrm_codegen::backend::isel;
use llrm_codegen::backend::selectiondag::SelectionDAG;
use llrm_codegen::backend::selectiondag::builder::{BuilderTarget, TargetNodes, build_block};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::schedule::Unit;
use llrm_codegen::backend::selectiondag::{BooleanContents, DagConfig};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Module, ValueDef};

use super::call::M68kCalling;
use super::hooks::Cpu;
use super::lowering::m68kisd;
use super::pipeline::select_block_with;
use crate::target::M68k;

static CALLING: LazyLock<M68kCalling> = LazyLock::new(M68kCalling::default);

/// The builder's view of m68k: 32-bit pointers, `i8` to `i32` in registers,
/// and the `CALL`/`RET` nodes of the pin.
pub fn builder_target() -> BuilderTarget {
    BuilderTarget {
        // `setBooleanContents(ZeroOrNegativeOneBooleanContent)`: `Scc` sets
        // 0xFF
        dag: DagConfig {
            boolean_contents: BooleanContents::ZeroOrNegativeOne,
            big_endian: true,
            ..BuilderTarget::m32().dag
        },
        shift_amount: EVT::I32,
        jump_tables: false,
        string_instructions: false,
        // past the inline limit a copy or a fill is a call (the pin: `memcpy`, `memmove`, `memset`)
        libcalls: Some(vec![
            ("memcpy".to_owned(), 0, "memcpy".to_owned()),
            ("memmove".to_owned(), 0, "memmove".to_owned()),
            ("memset".to_owned(), 0, "memset".to_owned()),
        ]),
        target_nodes: Some(TargetNodes {
            call: m68kisd::CALL,
            ret: m68kisd::RET,
            stack_pointer: "sp",
            big_endian: true,
            calling: Some(&*CALLING),
        }),
        ..BuilderTarget::m32()
    }
}

/// A phi input this block exports: `incoming` is the register its value was
/// copied to (`PHINodesToUpdate`), `result` the phi's own register.
#[derive(Clone, Copy, Debug)]
pub struct PhiUpdate {
    pub phi_block: BlockId,
    pub result: u32,
    pub incoming: u32,
    /// The phi's width in bits, for a register no node of the block names.
    pub bits: u32,
}

/// One block, selected and scheduled.
pub struct Selected {
    pub block: BlockId,
    pub dag: SelectionDAG,
    pub units: Vec<Unit>,
    pub phis: Vec<PhiUpdate>,
    /// On the entry block: the function's static allocas, `(frame index, size,
    /// alignment)`.
    pub locals: Vec<(i32, i64, i64)>,
}

/// Function `name` of `module`, block by block in layout order.
pub fn select_function(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<Vec<Selected>, String> {
    select_function_with(module, name, cpu, false)
}

/// [`select_function`], with `load_as_source` (see
/// [`super::pipeline::select_block_with`]).
pub fn select_function_with(
    module: &Module,
    name: &str,
    cpu: Cpu,
    load_as_source: bool,
) -> Result<Vec<Selected>, String> {
    select_function_ordered(module, name, cpu, load_as_source, false)
}

/// [`select_function_with`], with `ordered_copies`: the pairs of an inline
/// copy in address order, which the overlay's `(An)+` chains need.
pub fn select_function_ordered(
    module: &Module,
    name: &str,
    cpu: Cpu,
    load_as_source: bool,
    ordered_copies: bool,
) -> Result<Vec<Selected>, String> {
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let function = module
        .global(id)
        .function()
        .filter(|one| !one.is_declaration())
        .ok_or_else(|| format!("@{name} has no body"))?;
    let text = module.datalayout.as_deref().ok_or("a module with no datalayout")?;
    let layout = DataLayout::parse(text)?;
    let arch = M68k;
    let convention = isel::convention(module, &layout, id, &arch).map_err(|why| why.0)?;
    let mut target = builder_target();
    target.ordered_copies = ordered_copies;
    let mut info = FunctionLoweringInfo::new(function);
    let mut selected = Vec::new();
    let mut updates = Vec::new();
    for &block in function.layout() {
        let mut built = build_block(module, function, &layout, &target, &arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        // the blocks a `switch` lowering made (`CreateMachineBasicBlock`) follow the block that made
        // them, as x86's driver lays them out
        let extras = std::mem::take(&mut built.extra);
        let units = select_block_with(&mut built.dag, cpu, load_as_source).map_err(|why| format!("{why:?}"))?;
        updates.push(built.phi_updates);
        selected.push(Selected { block, dag: built.dag, units, phis: Vec::new(), locals: Vec::new() });
        for (id, mut extra) in extras {
            let units = select_block_with(&mut extra.dag, cpu, load_as_source).map_err(|why| format!("{why:?}"))?;
            updates.push(extra.phi_updates);
            selected.push(Selected { block: id, dag: extra.dag, units, phis: Vec::new(), locals: Vec::new() });
        }
    }
    // `FunctionLoweringInfo`: a static alloca is a frame object of the type's
    // size (times a constant count) on the larger of the alignment asked for
    // and the type's preferred one
    let types = &module.context.types;
    let mut locals = Vec::new();
    for (value, &index) in info.static_allocas.iter() {
        let ValueDef::Instruction(def) = function.value(value).def else {
            return Err("an alloca that is no instruction".to_owned());
        };
        let instruction = function.instruction(def);
        let llrm_mir::opcode::Opcode::Alloca { allocated, align: wanted, .. } = instruction.opcode else {
            return Err("an alloca that is not an alloca".to_owned());
        };
        let count = match instruction.operands.first() {
            None => 1,
            Some(llrm_mir::module::Operand::Constant(id)) => match module.context.get(*id).kind {
                llrm_mir::context::ConstantKind::Int(count) => i64::try_from(count).map_err(|_| "an alloca count")?,
                _ => return Err("an alloca whose count is not a number".to_owned()),
            },
            Some(_) => return Err("an alloca with a count that is not a constant".to_owned()),
        };
        let size = i64::try_from(layout.alloc_size(types, allocated)).map_err(|_| "an alloca too large")? * count;
        let align =
            i64::try_from(layout.pref_align(types, allocated)).unwrap_or(1).max(wanted.map_or(1, |one| one as i64));
        locals.push((index, size, align));
    }
    locals.sort();
    if let Some(entry) = selected.first_mut() {
        entry.locals = locals;
    }
    for (one, pending) in selected.iter_mut().zip(updates) {
        for (phi, incoming) in pending {
            let ValueDef::Instruction(def) = function.value(phi).def else {
                return Err("a phi that is not an instruction".to_owned());
            };
            let phi_block = function.parent(def).ok_or("a phi outside a block")?;
            let result = *info.value_regs.get(&phi).ok_or("a phi with no register")?;
            let bits = module.context.types.int_bits(function.value(phi).ty).unwrap_or(32);
            one.phis.push(PhiUpdate { phi_block, result, incoming, bits });
        }
    }
    Ok(selected)
}
