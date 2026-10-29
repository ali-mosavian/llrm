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
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Module};

use super::call::M68kCalling;
use super::hooks::Cpu;
use super::lowering::m68kisd;
use super::pipeline::select_block;
use crate::target::M68k;

static CALLING: LazyLock<M68kCalling> = LazyLock::new(M68kCalling::default);

/// The builder's view of m68k: 32-bit pointers, `i8` to `i32` in registers,
/// and the `CALL`/`RET` nodes of the pin.
pub fn builder_target() -> BuilderTarget {
    BuilderTarget {
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

/// One block, selected and scheduled.
pub struct Selected {
    pub block: BlockId,
    pub dag: SelectionDAG,
    pub units: Vec<Unit>,
}

/// Function `name` of `module`, block by block in layout order.
pub fn select_function(
    module: &Module,
    name: &str,
    cpu: Cpu,
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
    for &block in function.layout() {
        let mut built = build_block(module, function, &layout, &target, &arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        if !built.extra.is_empty() {
            return Err("a switch lowering made blocks".to_owned());
        }
        let units = select_block(&mut built.dag, cpu).map_err(|why| format!("{why:?}"))?;
        selected.push(Selected { block, dag: built.dag, units });
    }
    Ok(selected)
}
