//! One function through the DAG path to the `Selected` the machine phases
//! take: the other half of `--isel=dag`.
//!
//! # How it works
//! [`selected`] builds the entry block's DAG, runs
//! [`super::dag_select::select_block`] and turns the emitted blocks into a
//! `LirBody`: the blocks the select diamond made, their successors, the machine
//! facts the phases read. A function the path cannot yet take (more than one
//! MIR block, a call, a frame object, an unsupported opcode) is a refusal with
//! its reason: the shadow harness counts them.
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
//! DEVIATE SD403: no branch probabilities (`odds`) or counted-loop trip
//! counts, so the executed-work estimate of a loop is lower than the old
//! selector's until they are carried; and no calls, locals, register arguments,
//! switch lowering or multi-register phis yet. Reason: they need the call
//! sequence and frame objects in the emitter (D1e). Exit: D1e. Owner: sdag-d.
//! Review 2026-12-03.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use indexmap::IndexMap;
use llrm_dagisel::generated::x86_i486::{MACHINE_OPCODES, TABLE};
use llrm_dagisel::interp::Selector;
use llrm_dagisel::x86_addr::AddrMode;
use llrm_dagisel::x86_hooks::{Cpu, Options, X86Hooks};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Module, ValueDef, ValueId};
use llrm_target::Target;

use super::actions::{Features, TargetLowering};
use super::builder::{BuilderTarget, build_block};
use super::dag_select::select_block;
use super::emit::{Callee, EmitBlock, EmitContext, EmitState, FrameMap};
use super::function_lowering::FunctionLoweringInfo;
use super::mvt::MVT;
use super::opcodes::OpcodeMap;
use super::schedule::SchedInfo;
use super::x86_lowering::X86Lowering;
use super::x86_preprocess::FrameObjects;
use crate::backend::isel::{Parameter, Selected, Unselected};
use crate::model::ir::{Addr, Mem, Space};
use crate::model::lir::{Block, LirBlock, LirBody, Phi};

struct Info;

impl SchedInfo for Info {
    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        llrm_dagisel::instr::desc(opcode as u16).map_or(0, |d| usize::from(d.num_defs))
    }

    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16] {
        llrm_dagisel::instr::desc(opcode as u16).map_or(&[], |d| d.implicit_defs)
    }

    fn is_call(
        &self,
        opcode: u32,
    ) -> bool {
        llrm_dagisel::instr::desc(opcode as u16).is_some_and(|d| d.is_call())
    }
}

/// The frame's objects: fixed object `-k` is the k-th stack-passed
/// parameter; a stack temporary or alloca `k >= 0` is a cell below BP, laid
/// out as the old selector lays them (`depth` grows, a slot is at `-depth`).
pub(super) struct Frame {
    params: Vec<i64>,
    /// (displacement, size) of each object made, by index.
    locals: std::cell::RefCell<Vec<(i64, i64)>>,
    depth: std::cell::Cell<i64>,
}

impl Frame {
    pub(super) fn new(params: Vec<i64>) -> Frame {
        Frame { params, locals: Default::default(), depth: Default::default() }
    }
}

impl FrameMap for Frame {
    fn cell(
        &self,
        index: i32,
        width: u32,
    ) -> Option<Mem> {
        let disp = if index < 0 {
            *self.params.get(usize::try_from(-index - 1).ok()?)?
        } else {
            self.locals.borrow().get(usize::try_from(index).ok()?)?.0
        };
        Some(Mem {
            through: llrm_lir::registers::RegId::BP,
            disp_width: 2,
            ..Mem::new(Some(Addr::new(Space::Frame, disp)), width)
        })
    }
}

/// `CreateStackTemporary`: the preprocess makes its slots here.
pub(super) struct Temporaries<'a>(pub(super) &'a Frame);

impl FrameObjects for Temporaries<'_> {
    fn create_stack_temporary(
        &mut self,
        size: u32,
        align: u32,
    ) -> i32 {
        let (size, align) = (i64::from(size), i64::from(align.max(1)));
        let depth = (self.0.depth.get() + size + align - 1).div_euclid(align) * align;
        self.0.depth.set(depth);
        let mut locals = self.0.locals.borrow_mut();
        locals.push((-depth, size));
        (locals.len() - 1) as i32
    }
}

/// The module's globals by `GlobalId`: where each symbol is.
struct Globals<'a>(&'a Module);

impl super::emit::GlobalMap for Globals<'_> {
    fn address(
        &self,
        symbol: u32,
        offset: i64,
    ) -> Addr {
        let space = crate::backend::globals::space(self.0, llrm_mir::context::GlobalId(symbol));
        Addr { index: i64::from(symbol), ..Addr::new(space, offset) }
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
        std::sync::Arc::new(Tables {
            map: OpcodeMap::new(forms),
            names: MACHINE_OPCODES
                .iter()
                .copied()
                .chain(llrm_dagisel::generated::ids::x86::NAMES.iter().copied())
                .collect(),
        })
    })
    .clone()
}

/// The addresses each LIR block can go to: the targets of its jumps and
/// branches, and the next block if it does not end in a jump or a return.
fn successors(emitted: &[EmitBlock]) -> Vec<Vec<i64>> {
    use crate::model::ir::Operation;
    emitted
        .iter()
        .enumerate()
        .map(|(i, block)| {
            let mut out: Vec<i64> = Vec::new();
            for what in block.insns.iter().filter_map(|insn| insn.what.as_ref()) {
                if let (Operation::Branch | Operation::Jump, Some(target)) = (what.op, what.target) {
                    if !out.contains(&target) {
                        out.push(target);
                    }
                }
            }
            let ends = block
                .insns
                .last()
                .and_then(|insn| insn.what.as_ref())
                .is_some_and(|w| matches!(w.op, Operation::Jump | Operation::Return));
            if !ends {
                if let Some(next) = emitted.get(i + 1) {
                    if !out.contains(&next.at) {
                        out.push(next.at);
                    }
                }
            }
            out
        })
        .collect()
}

fn refuse<T>(why: impl Into<String>) -> Result<T, Unselected> {
    Err(Unselected(format!("--isel=dag: {}", why.into())))
}

/// `name` of `module` selected by the DAG path.
pub fn selected(
    module: &Module,
    name: &str,
    arch: &dyn Target,
    cpu: &crate::backend::cpu::Profile,
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
    let convention = crate::backend::isel::convention(module, &layout, id, arch)?;
    let mut info = FunctionLoweringInfo::new(function);
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let tables = tables(arch);
    let frame = Frame::new(
        convention
            .parameters
            .iter()
            .filter_map(|p| if let Parameter::Cell(disp) = p { Some(*disp) } else { None })
            .collect(),
    );
    // The function's static allocas are frame objects 0, 1, ... in the order
    // `FunctionLoweringInfo` numbered them: laid out below BP, each on its
    // type's alignment.
    let mut allocas: Vec<(i32, llrm_mir::module::ValueId)> =
        info.static_allocas.iter().map(|(value, index)| (*index, value.clone())).collect();
    allocas.sort();
    for (index, value) in allocas {
        let ValueDef::Instruction(def) = function.value(value).def else {
            return refuse("an alloca that is not an instruction");
        };
        let instruction = function.instruction(def);
        let llrm_mir::opcode::Opcode::Alloca { allocated, .. } = instruction.opcode else {
            return refuse("an alloca that is not an alloca");
        };
        if !instruction.operands.is_empty() {
            return refuse("an alloca with a count");
        }
        let types = &module.context.types;
        let size = layout.alloc_size(types, allocated) as i64;
        let align =
            crate::backend::isel::natural_align(types, &layout, allocated).max(size & size.wrapping_neg()).clamp(1, 4);
        let made = Temporaries(&frame).create_stack_temporary(size as u32, align as u32);
        debug_assert_eq!(made, index, "frame objects are numbered as the builder numbered them");
    }
    let mut hooks = X86Hooks::new(Options {
        cpu: Cpu::I486,
        opt_none: false,
        opt_for_size: false,
        min_size: false,
        has_frame_pointer: true,
        address_mode: AddrMode::Flat32,
    });
    let selector = Selector::new(TABLE);
    let mut state = EmitState::default();
    // A block's address is fixed before any block is emitted: a branch names
    // the blocks after it.
    let block_at: HashMap<u32, i64> = function.layout().iter().map(|b| (b.0, state.next_address())).collect();
    let globals = Globals(module);
    // What a call destroys: the general registers the convention does not
    // keep for the caller.
    let regs = llrm_lir::registers::Regs(arch.registers());
    let kept: std::collections::BTreeSet<_> = arch
        .callee_saved()
        .into_iter()
        .filter(|(full, pushed)| full == pushed)
        .map(|(full, _)| regs.root(full))
        .collect();
    let stack_pointer = arch.register_named("esp");
    let frame_registers = [stack_pointer, llrm_lir::registers::RegId::BP].map(|r| regs.root(r));
    let caller_saved: std::collections::BTreeSet<_> = cpu
        .general
        .iter()
        .map(|r| regs.root(*r))
        .filter(|r| !kept.contains(r) && !frame_registers.contains(r))
        .collect();
    let ctx = EmitContext {
        caller_saved: &caller_saved,
        stack_pointer,
        globals: &globals,
        block_at: &block_at,
        frame: &frame,
        address_mode: AddrMode::Flat32,
        map: &tables.map,
        names: &tables.names,
    };
    let mut emitted: Vec<EmitBlock> = Vec::new();
    let mut calls = IndexMap::default();
    // The LIR blocks each MIR block became, and the phi inputs each exports.
    let mut made: HashMap<u32, std::ops::Range<usize>> = HashMap::new();
    let mut exports: Vec<(BlockId, Vec<(ValueId, u32)>)> = Vec::new();
    for &block in function.layout() {
        let mut built =
            build_block(module, function, &layout, &BuilderTarget::m32(), arch, &convention, &mut info, block)
                .map_err(|why| Unselected(format!("--isel=dag: {}", why.0)))?;
        if !built.live_ins.is_empty() || !built.extra.is_empty() {
            return refuse("register arguments or a switch lowering");
        }
        let done = select_block(
            &mut built.dag,
            &tli,
            &mut X86Lowering { cmov: false },
            &selector,
            &mut hooks,
            &Info,
            &ctx,
            &mut state,
            block_at[&block.0],
            arch,
            &mut Temporaries(&frame),
        )
        .map_err(|why| Unselected(format!("--isel=dag: {why:?}")))?;
        let from = emitted.len();
        calls.extend(done.calls.iter().map(|(at, callee)| {
            let name = match callee {
                Callee::Global(symbol) => {
                    module.global(llrm_mir::context::GlobalId(*symbol)).name.as_deref().unwrap_or_default().to_owned()
                }
                Callee::Named(name) => name.clone(),
            };
            (*at, name)
        }));
        emitted.extend(done.blocks);
        made.insert(block.0, from..emitted.len());
        exports.push((block, built.phi_updates));
    }
    let succ = successors(&emitted);
    // Phis: the successor's phi takes, from each LIR block of the predecessor
    // that leaves for it, the register the predecessor copied its value to.
    for (pred, updates) in &exports {
        for (phi, reg_in) in updates {
            let ValueDef::Instruction(def) = function.value(*phi).def else {
                return refuse("a phi that is not an instruction");
            };
            let Some(phi_block) = function.parent(def) else { return refuse("a phi outside a block") };
            let at = block_at[&phi_block.0];
            let Some(reg_out) = info.value_regs.get(phi) else { return refuse("a phi with no register") };
            let (result, incoming) = (state.vreg(*reg_out), state.vreg(*reg_in));
            let edges: Vec<i64> =
                made[&pred.0].clone().filter(|i| succ[*i].contains(&at)).map(|i| emitted[i].at).collect();
            let target = made[&phi_block.0].start;
            for edge in edges {
                let block = &mut emitted[target];
                match block.phis.iter_mut().find(|p| p.result == result) {
                    Some(p) => {
                        if !p.incoming.iter().any(|(e, _)| *e == edge) {
                            p.incoming.push((edge, incoming));
                        }
                    }
                    None => block.phis.push(Phi { result, incoming: vec![(edge, incoming)] }),
                }
            }
        }
    }
    let blocks: Vec<Block> = emitted
        .iter()
        .zip(succ)
        .map(|(block, succ)| -> Block {
            LirBlock {
                succ: succ.into(),
                phis: block.phis.clone().into(),
                ..LirBlock::new(block.at, block.insns.iter().cloned().collect::<Vec<_>>().into())
            }
            .into()
        })
        .collect();
    let entry = emitted[0].at;
    let mut body = LirBody::new(name, entry, blocks, IndexMap::default(), IndexMap::default());
    body.bits = arch.object().bitness;
    body.registers = Some(llrm_lir::registers::Regs(arch.registers()));
    body.float_stack = arch.float_stack();
    body.sealed_arguments = true;
    body.ordered = true;
    body.frequencies = Some(crate::analysis::frequency::Frequency::over(&body, &body.blocks).table());
    Ok(Selected {
        body,
        convention,
        calls,
        inline: IndexMap::default(),
        inline_places: IndexMap::default(),
        inline_symbols: IndexMap::default(),
        far: Default::default(),
        pops: IndexMap::default(),
        depth: frame.depth.get(),
        extents: frame.locals.borrow().clone(),
        landing: None,
    })
}
