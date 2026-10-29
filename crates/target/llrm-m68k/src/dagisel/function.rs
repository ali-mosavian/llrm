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
        dag: DagConfig { boolean_contents: BooleanContents::ZeroOrNegativeOne, ..BuilderTarget::m32().dag },
        shift_amount: EVT::I32,
        jump_tables: false,
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
    let target = builder_target();
    let mut info = FunctionLoweringInfo::new(function);
    let mut selected = Vec::new();
    let mut updates = Vec::new();
    for &block in function.layout() {
        let mut built = build_block(module, function, &layout, &target, &arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        if !built.extra.is_empty() {
            return Err("a switch lowering made blocks".to_owned());
        }
        let units = select_block_with(&mut built.dag, cpu, load_as_source).map_err(|why| format!("{why:?}"))?;
        updates.push(built.phi_updates);
        selected.push(Selected { block, dag: built.dag, units, phis: Vec::new() });
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
