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
use llrm_dagisel::generated::{x86_i486, x86_m16};
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
use crate::model::ir::{Addr, Held, Mem, Space};
use crate::model::lir::{Block, Insn, LirBlock, LirBody, Phi};

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

    fn extent(
        &self,
        index: i32,
    ) -> Option<(i64, i64)> {
        self.locals.borrow().get(usize::try_from(index).ok()?).copied()
    }
}

/// `CreateStackTemporary`: the preprocess makes its slots here.
pub(super) struct Temporaries<'a>(pub(super) &'a Frame);

impl FrameObjects for Temporaries<'_> {
    fn objects(&self) -> i32 {
        self.0.locals.borrow().len() as i32
    }

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

/// What a call destroys, by the convention of the function it names: a
/// declaration's own, the C one for a runtime routine, and for a call through
/// a pointer (whose convention the node does not carry) the C one too.
struct CallConventions<'a> {
    module: &'a Module,
    arch: &'a dyn Target,
}

impl super::emit::Conventions for CallConventions<'_> {
    fn clobbers(
        &self,
        callee: Option<&Callee>,
    ) -> Option<std::collections::BTreeSet<llrm_lir::registers::RegId>> {
        let number = match callee {
            Some(Callee::Global(id)) => {
                self.module.global(llrm_mir::context::GlobalId(*id)).function().map_or(0, |f| f.calling_convention)
            }
            _ => 0,
        };
        let found = self.arch.calling().by_number(number)?;
        let regs = llrm_lir::registers::Regs(self.arch.registers());
        Some(
            found
                .clobbered
                .iter()
                .filter(|name| !matches!(name.as_str(), "flags") && !name.starts_with("st"))
                .map(|name| regs.root(self.arch.register_named(name)))
                .collect(),
        )
    }
}

/// What does not change between functions: the opcode map and names, built
/// once (`x86.instr` is parsed at 5,000 instructions a row).
struct Tables {
    map: OpcodeMap,
    names: HashMap<u16, &'static str>,
}

/// One per target width.
static TABLES: LazyLock<Mutex<HashMap<u32, std::sync::Arc<Tables>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn tables(arch: &dyn Target) -> std::sync::Arc<Tables> {
    let bits = arch.object().bitness;
    let mut slots = TABLES.lock().expect("the DAG tables");
    slots
        .entry(bits)
        .or_insert_with(|| {
            let forms = llrm_instr::parse(&arch.forms_text(), arch.dialect()).expect("x86.instr parses");
            let opcodes = if bits == 16 { x86_m16::MACHINE_OPCODES } else { x86_i486::MACHINE_OPCODES };
            std::sync::Arc::new(Tables {
                map: OpcodeMap::new(forms),
                names: opcodes
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

/// The byte width of each live-in's value: the type of the `CopyFromReg` that
/// reads it.
fn arrival_widths(
    dag: &super::SelectionDAG,
    live_ins: &[(llrm_lir::registers::RegId, u32)],
) -> Vec<(u32, u32)> {
    use super::dag::NodeData;
    let mut out = Vec::new();
    for node in dag.nodes() {
        if node.opcode != super::isd::ISD::CopyFromReg {
            continue;
        }
        if let NodeData::Register(vreg) = dag.node(node.ops[1]).data {
            if live_ins.iter().any(|(_, v)| *v == vreg) {
                out.push((vreg, node.values[0].size_in_bits() / 8));
            }
        }
    }
    out
}

fn arrived_width(
    widths: &[(u32, u32)],
    vreg: u32,
) -> Option<u32> {
    widths.iter().find(|(v, _)| *v == vreg).map(|(_, w)| *w)
}

fn refuse<T>(why: impl Into<String>) -> Result<T, Unselected> {
    Err(Unselected(format!("--isel=dag: {}", why.into())))
}

/// `name` of `module` selected by the DAG path. A panic in a stage (a builder
/// meeting a type it does not know) is a refusal of the function, as any other
/// gap: it stopped 18 QCport modules from compiling in the shadow.
pub fn selected(
    module: &Module,
    name: &str,
    arch: &dyn Target,
    cpu: &crate::backend::cpu::Profile,
    libcalls: &[(String, u32, String)],
) -> Result<Selected, Unselected> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| selected_in(module, name, arch, cpu, libcalls)))
        .unwrap_or_else(|why| {
            let why = why
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| why.downcast_ref::<&str>().map(|one| (*one).to_owned()));
            refuse(format!("--isel=dag: a stage stopped: {}", why.unwrap_or_default()))
        })
}

fn selected_in(
    module: &Module,
    name: &str,
    arch: &dyn Target,
    cpu: &crate::backend::cpu::Profile,
    libcalls: &[(String, u32, String)],
) -> Result<Selected, Unselected> {
    let m16 = match arch.object().bitness {
        16 => true,
        32 => false,
        _ => return refuse("only m16 and m32 are built so far"),
    };
    let Some(id) = module.named(name) else { return refuse(format!("no function @{name}")) };
    let Some(function) = module.global(id).function().filter(|one| !one.is_declaration()) else {
        return refuse(format!("@{name} has no body"));
    };
    let Some(text) = module.datalayout.as_deref() else { return refuse("a module with no datalayout") };
    let layout = DataLayout::parse(text).map_err(Unselected)?;
    let convention = crate::backend::isel::convention(module, &layout, id, arch)?;
    let mut info = FunctionLoweringInfo::new(function);
    let tli = if m16 {
        TargetLowering::x86_16(Features::default(), MVT::I16)
    } else {
        TargetLowering::x86_32(Features::default(), MVT::I32)
    };
    let (builder_target, address_mode) =
        if m16 { (BuilderTarget::m16(), AddrMode::Real16) } else { (BuilderTarget::m32(), AddrMode::Flat32) };
    let builder_target = builder_target.with_runtime_calls(libcalls);
    let tables = tables(arch);
    // The builder numbers the fixed objects the stack parameters make: a byval
    // aggregate one, a cell one for each register-sized part of its value (an
    // integer of twice the widest register is two). Dropping a byval one
    // moved every later parameter by its size (byvalue_struct_address passed
    // the wrong address).
    let widest = if m16 { 16 } else { 32 };
    let mut cells = Vec::new();
    for (at, parameter) in convention.parameters.iter().enumerate() {
        match parameter {
            Parameter::Bytes(disp) => cells.push(*disp),
            Parameter::Cell(disp) => {
                let two = function
                    .parameters()
                    .get(at)
                    .is_some_and(
                        |arg| matches!(
                            module.context.types.get(function.value(*arg).ty),
                            llrm_mir::types::Type::Int(bits) if *bits == 2 * widest
                        ),
                    );
                cells.extend((0..if two { 2 } else { 1 }).map(|part| disp + part * i64::from(widest / 8)));
            }
            Parameter::Registers(_) => {}
        }
    }
    // `LowerFormalArguments`: the first variadic argument is the next fixed
    // object, where the convention puts the first stack argument it does not
    // name.
    if module.signature(function.ty).2 {
        cells.push(convention.stack_arguments);
    }
    let frame = Frame::new(cells);
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
        cpu: if m16 { Cpu::M16 } else { Cpu::I486 },
        opt_none: false,
        opt_for_size: false,
        min_size: false,
        has_frame_pointer: true,
        address_mode,
    });
    let selector = Selector::new(if m16 { x86_m16::TABLE } else { x86_i486::TABLE });
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
    let stack_pointer = arch.stack_pointer();
    let frame_registers = [stack_pointer, llrm_lir::registers::RegId::BP].map(|r| regs.root(r));
    let caller_saved: std::collections::BTreeSet<_> = cpu
        .general
        .iter()
        .map(|r| regs.root(*r))
        .filter(|r| !kept.contains(r) && !frame_registers.contains(r))
        .collect();
    let ctx = EmitContext {
        caller_saved: &caller_saved,
        conventions: &CallConventions { module, arch },
        stack_pointer,
        globals: &globals,
        block_at: &block_at,
        frame: &frame,
        address_mode,
        map: &tables.map,
        names: &tables.names,
    };
    let mut emitted: Vec<EmitBlock> = Vec::new();
    let mut calls = IndexMap::default();
    // the call sites whose callee is far code (address space 1): a far call
    let mut far = std::collections::BTreeSet::new();
    // The LIR blocks each MIR block became, and the phi inputs each exports.
    let mut made: HashMap<u32, std::ops::Range<usize>> = HashMap::new();
    let mut exports: Vec<(BlockId, Vec<(ValueId, u32)>)> = Vec::new();
    for &block in function.layout() {
        let mut built = build_block(module, function, &layout, &builder_target, arch, &convention, &mut info, block)
            .map_err(|why| Unselected(format!("--isel=dag: {}", why.0)))?;
        if !built.extra.is_empty() {
            return refuse("a switch lowering");
        }
        if !built.live_ins.is_empty() && block != function.layout()[0] {
            return refuse("register arguments outside the entry block");
        }
        let arrived = arrival_widths(&built.dag, &built.live_ins);
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
        let mut done = done;
        if !built.live_ins.is_empty() {
            // The entry says which registers the arguments arrive in; it is no
            // code, and nothing placed at the entry may come before
            // it.
            let regs = llrm_lir::registers::Regs(arch.registers());
            let mut arrival =
                Insn::new(block_at[&block.0], None, Some(crate::model::lir::inert()), Vec::new(), Vec::new());
            for (register, vreg) in &built.live_ins {
                let value = state.vreg(*vreg);
                // The argument arrives in the view of the register as wide as
                // its type.
                let width = arrived_width(&arrived, *vreg).unwrap_or_else(|| regs.bytes(*register).unwrap_or(4) as u32);
                let view = regs.view(regs.root(*register), width * 8).unwrap_or(*register);
                arrival.defines.push(value);
                arrival.delivers.push((Held { value, width }, view));
            }
            done.blocks[0].insns.insert(0, std::sync::Arc::new(arrival));
        }
        let from = emitted.len();
        calls.extend(done.calls.iter().map(|(at, callee)| {
            let name = match callee {
                Callee::Global(symbol) => {
                    let global = module.global(llrm_mir::context::GlobalId(*symbol));
                    if crate::backend::isel::far(global).unwrap_or(false) {
                        far.insert(*at);
                    }
                    global.name.as_deref().unwrap_or_default().to_owned()
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
    // The inline routines the calls name (`ExpandDivRemLibCall`'s `divN_s`):
    // their bytes are laid down at the call, as the old route did.
    let mut inline = IndexMap::default();
    for (at, name) in &calls {
        for signed in [true, false] {
            for wide in [true, false] {
                if let Ok(routine) = arch.divide_routine(signed, wide) {
                    if routine.name == name.as_str() {
                        inline.insert(*at, routine.code);
                    }
                }
            }
        }
    }
    Ok(Selected {
        body,
        convention,
        calls,
        inline,
        inline_places: IndexMap::default(),
        inline_symbols: IndexMap::default(),
        far,
        pops: IndexMap::default(),
        depth: frame.depth.get(),
        extents: frame.locals.borrow().clone(),
        landing: None,
    })
}
