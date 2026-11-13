//! A function from IR to scheduled machine nodes: the shared builder, then
//! [`super::pipeline`]'s stages, block by block.
//!
//! DEVIATE: no live-in registers: the C convention passes everything on the
//! stack, so the slice (arithmetic, branches, calls) needs none. Exit: M4.

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
use super::pipeline::{Temporaries, select_block_with};
use crate::target::M68k;

static CALLING: LazyLock<M68kCalling> = LazyLock::new(M68kCalling::default);

/// The DAG builder lowers the module's `switch` instructions (clusters, bit
/// tests and jump tables, `SwitchLoweringUtils`), as the pin's does with no
/// `LowerSwitch` pass in the codegen pipeline. The mid-end's switch lowering
/// is off wherever this is on.
pub const SWITCHES_IN_DAG: bool = true;

/// The builder's view of m68k: 32-bit pointers, `i8` to `i32` in registers,
/// and the `CALL`/`RET` nodes of the pin.
pub fn builder_target() -> BuilderTarget {
    BuilderTarget {
        // `setBooleanContents(ZeroOrNegativeOneBooleanContent)`: `Scc` sets
        // 0xFF
        dag: DagConfig {
            boolean_contents: BooleanContents::ZeroOrNegativeOne,
            big_endian: true,
            ..BuilderTarget::for_pointer_bits(32).dag
        },
        shift_amount: EVT::I32,
        jump_tables: SWITCHES_IN_DAG,
        string_instructions: false,
        // no FPU at any CPU: a float travels as the integer of its width
        soft_float: true,
        // past the inline limit a copy or a fill is a call (the
        // pin: `memcpy`, `memmove`, `memset`)
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
        // the ABI packs stack arguments at 4 (gcc callers do); DEVIATE VA-ALIGN
        // in `BuilderTarget`
        va_arg_align_cap: Some(super::lowering::VA_SLOT),
        // `Machine::max_alloca_align`: the frame realigns the stack pointer
        max_alloca_align: None,
        ..BuilderTarget::for_pointer_bits(32)
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
    /// The IR block this was selected for: `block` itself, or the block a
    /// lowering made out of it (`&&` chains, `switch`).
    pub parent: BlockId,
    pub dag: SelectionDAG,
    pub units: Vec<Unit>,
    pub phis: Vec<PhiUpdate>,
    /// On the entry block: the function's static allocas, `(frame index, size,
    /// alignment)`.
    pub locals: Vec<(i32, i64, i64)>,
    /// IR `unreachable` ends the block. The pin emits nothing for it (the
    /// machine CFG comes from the IR); here a block with no terminator falls
    /// into the next one, so emission states the end.
    pub ends_unreachable: bool,
    /// The blocks each of the function's jump tables goes to, by table number
    /// (`MachineJumpTableInfo`; the same on every block).
    pub tables: Vec<Vec<u32>>,
}

/// Function `name` of `module`, block by block in layout order.
pub fn select_function(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<Vec<Selected>, String> {
    select_function_with(
        module,
        name,
        cpu,
        false,
        false,
        false,
        false,
        false,
        true,
        Default::default(),
        &Default::default(),
        &[],
    )
}

/// [`select_function`], with `source_order`: the pin's source-order scheduler
/// in place of the ILP list scheduler, and `indexed`: `(An)+` and `-(An)`
/// accesses, `short`: `addq` and `subq`, and `absolute_data`: a global's
/// address is an absolute one, not PC-relative ([`crate::compile::CodeModel`]).
pub fn select_function_with(
    module: &Module,
    name: &str,
    cpu: Cpu,
    source_order: bool,
    indexed: bool,
    short: bool,
    absolute_data: bool,
    size: bool,
    carry_chain: bool,
    price: crate::timing::Price,
    mul: &super::mulprice::Chain,
    libcalls: &[crate::libcalls::Kind],
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
    // DEVIATE M68K-DIVSIZE: from the 68020 a divide by a constant stays one
    // `divu.l` in a size build (4-6 B) where the pin's default (`isIntDivCheap`
    // false) makes the multiply-high sequence (10-14 B, `mulu.l` and shifts);
    // X86 and AArch64 return the size flag here too. Owner m68k. Exit: none.
    // DEVIATE M68K-DIVSIZE-NARROW: below the 68020 the byte and word divides
    // are still one `divs.w`/`divu.w` (4 B) in a size build, where the wide
    // one is `__divsi3`. Owner m68k. Exit: none.
    let mut target = builder_target()
        .with_int_div_cheap(size && cpu.at_least(Cpu::M68020))
        .with_int_div_cheap_narrow(if size { 16 } else { 0 });
    // The pin's O1 has no correlated-propagation to narrow a divide of values that fit 16 bits
    // (`narrowSDivOrSRem`, `narrowUDivOrURem`): below the 68020 that is `__divsi3`, where one `divs.w`
    // is 158 clk. DEVIATE M68K-NARROW-DIV (same as the 6502's): owner m68k. Exit: none.
    if !cpu.at_least(Cpu::M68020) {
        target = target.with_narrow_divrem(EVT::I16, false);
    }
    target.ordered_copies = indexed;
    // DEVIATE M68K-JT-PRICE (`super::switchprice`): the table's limits are the
    // level's price of a table against the compare chain, not the pin's 4
    // entries and 10% (40% at -Os)
    let limits = super::switchprice::limits(cpu, price);
    target.min_jump_table_entries = limits.min_entries;
    target.min_jump_table_density = Some(limits.min_density);
    let mut info = FunctionLoweringInfo::new(function);
    let mut temporaries = Temporaries::new(info.static_allocas.iter().map(|(_, &index)| index + 1).max().unwrap_or(0));
    let mut selected = Vec::new();
    let mut updates = Vec::new();
    for &block in function.layout() {
        let mut built = build_block(module, function, &layout, &target, &arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        // the blocks a `switch` lowering made (`CreateMachineBasicBlock`)
        // follow the block that made them, as x86's driver lays them
        // out
        let extras = std::mem::take(&mut built.extra);
        let units = select_block_with(
            &mut built.dag,
            cpu,
            source_order,
            indexed,
            short,
            absolute_data,
            carry_chain,
            price,
            mul.clone(),
            libcalls,
            &mut temporaries,
        )
        .map_err(|why| format!("{why:?}"))?;
        updates.push(built.phi_updates);
        let ends_unreachable = function
            .terminator(block)
            .is_some_and(|last| function.instruction(last).opcode == llrm_mir::opcode::Opcode::Unreachable);
        selected.push(Selected {
            block,
            parent: block,
            dag: built.dag,
            units,
            phis: Vec::new(),
            locals: Vec::new(),
            ends_unreachable,
            tables: Vec::new(),
        });
        for (id, mut extra) in extras {
            let units = select_block_with(
                &mut extra.dag,
                cpu,
                source_order,
                indexed,
                short,
                absolute_data,
                carry_chain,
                price,
                mul.clone(),
                libcalls,
                &mut temporaries,
            )
            .map_err(|why| format!("{why:?}"))?;
            updates.push(extra.phi_updates);
            selected.push(Selected {
                block: id,
                parent: block,
                dag: extra.dag,
                units,
                phis: Vec::new(),
                locals: Vec::new(),
                ends_unreachable: false,
                tables: Vec::new(),
            });
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
    locals.extend(temporaries.made);
    locals.sort();
    if let Some(entry) = selected.first_mut() {
        entry.locals = locals;
    }
    for one in &mut selected {
        one.tables.clone_from(&info.jump_tables);
    }
    for (one, pending) in selected.iter_mut().zip(updates) {
        for (at, &(phi, incoming)) in pending.iter().enumerate() {
            let ValueDef::Instruction(def) = function.value(phi).def else {
                return Err("a phi that is not an instruction".to_owned());
            };
            let phi_block = function.parent(def).ok_or("a phi outside a block")?;
            let result = *info.value_regs.get(&phi).ok_or("a phi with no register")?;
            // a phi of two registers (an `i64`) has an entry for each part, in
            // order: the k-th is the phi's k-th register
            let part = pending[..at].iter().filter(|(one, _)| *one == phi).count();
            let result = if part == 0 { result } else { info.part_regs(result, part + 1)[part] };
            let bits = module.context.types.int_bits(function.value(phi).ty).unwrap_or(32);
            one.phis.push(PhiUpdate { phi_block, result, incoming, bits });
        }
    }
    Ok(selected)
}
