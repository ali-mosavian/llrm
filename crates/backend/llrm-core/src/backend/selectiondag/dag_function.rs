//! One function through the DAG path to the `Selected` the machine phases
//! take: the other half of `--isel=dag`.
//!
//! # How it works
//! [`selected`] builds the entry block's DAG, runs [`super::dag_select::select_block`]
//! and turns the emitted blocks into a `LirBody`: the blocks the select
//! diamond made, their successors, the machine facts the phases read. A
//! function the path cannot yet take (more than one MIR block, a call, a
//! frame object, an unsupported opcode) is a refusal with its reason: the
//! shadow harness counts them.
//!
//! ```text
//! @f(a, b) = a + b      LIR:  v1 := mov [arg0]      (builder, legalizer,
//!                              v2 := add v1, [arg1]   selector, scheduler,
//!                              eax := mov v2          emitter)
//!                              ret
//! ```
//!
//! # Example
//! `int f(int a, int b) { return a + b; }` at `-m32`: four instructions; the
//! second load is folded into the add (`ADD32rm`).
//!
//! # Runs
//! `assemble::staged` when `--isel=dag`.
//!
//! DEVIATE SD403: one MIR block per function so far; no calls, locals or
//! phis. Reason: multi-block needs the branch nodes mapped (`JCC_1`, `JMP_1`
//! are unmapped opcodes) and the phis `PHINodesToUpdate` names. Exit: the
//! next piece (D1d-2). Owner: sdag-d. Review 2026-12-03.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use indexmap::IndexMap;
use llrm_dagisel::generated::x86_i486::{MACHINE_OPCODES, TABLE};
use llrm_dagisel::interp::Selector;
use llrm_dagisel::x86_addr::AddrMode;
use llrm_dagisel::x86_hooks::{Cpu, Options, X86Hooks};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;
use llrm_target::Target;

use super::actions::{Features, TargetLowering};
use super::builder::{BuilderTarget, build_block};
use super::dag_select::select_block;
use super::emit::{EmitContext, FrameMap};
use super::function_lowering::FunctionLoweringInfo;
use super::mvt::MVT;
use super::opcodes::OpcodeMap;
use super::schedule::SchedInfo;
use super::x86_lowering::X86Lowering;
use crate::backend::isel::{Parameter, Selected, Unselected};
use crate::model::ir::{Addr, Mem, Space};
use crate::model::lir::{Block, LirBlock, LirBody};

struct Info;

impl SchedInfo for Info {
    fn num_defs(&self, opcode: u32) -> usize {
        llrm_dagisel::instr::desc(opcode as u16).map_or(0, |d| usize::from(d.num_defs))
    }

    fn implicit_defs(&self, opcode: u32) -> &[u16] {
        llrm_dagisel::instr::desc(opcode as u16).map_or(&[], |d| d.implicit_defs)
    }

    fn is_call(&self, opcode: u32) -> bool {
        llrm_dagisel::instr::desc(opcode as u16).is_some_and(|d| d.is_call())
    }
}

/// Fixed object `-k` is the k-th stack-passed parameter.
struct Frame(Vec<i64>);

impl FrameMap for Frame {
    fn cell(
        &self,
        index: i32,
        width: u32,
    ) -> Option<Mem> {
        let disp = *self.0.get(usize::try_from(-index - 1).ok()?)?;
        Some(Mem {
            through: llrm_lir::registers::RegId::BP,
            disp_width: 2,
            ..Mem::new(Some(Addr::new(Space::Frame, disp)), width)
        })
    }
}

/// What does not change between functions: the opcode map and names, built
/// once (`x86.instr` is parsed at 5,000 instructions a row).
struct Tables {
    map: OpcodeMap,
    names: HashMap<u16, &'static str>,
}

static TABLES: LazyLock<Mutex<Option<std::sync::Arc<Tables>>>> = LazyLock::new(|| Mutex::new(None));

fn tables(arch: &dyn Target) -> std::sync::Arc<Tables> {
    let mut slot = TABLES.lock().expect("the DAG tables");
    slot.get_or_insert_with(|| {
        let forms = llrm_instr::parse(&arch.forms_text(), arch.dialect()).expect("x86.instr parses");
        std::sync::Arc::new(Tables { map: OpcodeMap::new(forms), names: MACHINE_OPCODES.iter().copied().collect() })
    })
    .clone()
}

fn refuse<T>(why: impl Into<String>) -> Result<T, Unselected> {
    Err(Unselected(format!("--isel=dag: {}", why.into())))
}

/// `name` of `module` selected by the DAG path.
pub fn selected(
    module: &Module,
    name: &str,
    arch: &dyn Target,
) -> Result<Selected, Unselected> {
    if arch.object().bitness != 32 {
        return refuse("only m32 is built so far");
    }
    let Some(id) = module.named(name) else { return refuse(format!("no function @{name}")) };
    let Some(function) = module.global(id).function().filter(|one| !one.is_declaration()) else {
        return refuse(format!("@{name} has no body"));
    };
    let Some(text) = module.datalayout.as_deref() else { return refuse("a module with no datalayout") };
    let layout = DataLayout::parse(text).map_err(Unselected)?;
    if function.layout().len() != 1 {
        return refuse("a function of several blocks");
    }
    let convention = crate::backend::isel::convention(module, &layout, id, arch)?;
    let mut info = FunctionLoweringInfo::new(function);
    let mut built = build_block(
        module,
        function,
        &layout,
        &BuilderTarget::m32(),
        arch,
        &convention,
        &mut info,
        function.layout()[0],
    )
    .map_err(|why| Unselected(format!("--isel=dag: {}", why.0)))?;
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let tables = tables(arch);
    let frame = Frame(
        convention.parameters.iter().filter_map(|p| if let Parameter::Cell(disp) = p { Some(*disp) } else { None }).collect(),
    );
    let ctx = EmitContext { frame: &frame, address_mode: AddrMode::Flat32, map: &tables.map, names: &tables.names };
    let mut hooks = X86Hooks::new(Options {
        cpu: Cpu::I486,
        opt_none: false,
        opt_for_size: false,
        min_size: false,
        has_frame_pointer: true,
        address_mode: AddrMode::Flat32,
    });
    let emitted = select_block(
        &mut built.dag,
        &tli,
        &mut X86Lowering { cmov: false },
        &Selector::new(TABLE),
        &mut hooks,
        &Info,
        &ctx,
    )
    .map_err(|why| Unselected(format!("--isel=dag: {why:?}")))?;
    let blocks: Vec<Block> = emitted
        .blocks
        .iter()
        .enumerate()
        .map(|(at, block)| -> Block {
            let succ: Vec<i64> = match (at, emitted.blocks.len()) {
                (0, 3) => vec![emitted.blocks[1].at, emitted.blocks[2].at],
                (1, 3) => vec![emitted.blocks[2].at],
                _ => Vec::new(),
            };
            LirBlock {
                succ: succ.into(),
                phis: block.phis.clone().into(),
                ..LirBlock::new(block.at, block.insns.iter().cloned().collect::<Vec<_>>().into())
            }
            .into()
        })
        .collect();
    let entry = emitted.blocks[0].at;
    let mut body = LirBody::new(name, entry, blocks, IndexMap::default(), IndexMap::default());
    body.bits = arch.object().bitness;
    body.registers = Some(llrm_lir::registers::Regs(arch.registers()));
    body.float_stack = arch.float_stack();
    body.sealed_arguments = true;
    body.ordered = true;
    Ok(Selected {
        body,
        convention,
        calls: IndexMap::default(),
        inline: IndexMap::default(),
        inline_places: IndexMap::default(),
        inline_symbols: IndexMap::default(),
        far: Default::default(),
        pops: IndexMap::default(),
        depth: 0,
        extents: Vec::new(),
        landing: None,
    })
}
