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

use std::collections::{HashMap, HashSet};
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
use super::builder::{BuilderTarget, RuntimeAnswer, build_block};
use super::dag_select::select_block;
use super::emit::{Callee, EmitBlock, EmitContext, EmitState, FrameMap, JumpTable};
use super::function_lowering::FunctionLoweringInfo;
use super::mvt::MVT;
use super::opcodes::OpcodeMap;
use super::schedule::SchedInfo;
use super::x86_lowering::X86Lowering;
use super::x86_preprocess::FrameObjects;
use crate::backend::constpool::Pool;
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
struct Globals<'a>(&'a Module, llrm_mir::spaces::Spaces);

impl super::emit::GlobalMap for Globals<'_> {
    fn segment(
        &self,
        symbol: Option<u32>,
    ) -> Addr {
        let (space, index) = match symbol {
            Some(symbol) if self.0.global(llrm_mir::context::GlobalId(symbol)).address_space != self.1.near => {
                (Space::Group, crate::backend::globals::segment_of(llrm_mir::context::GlobalId(symbol)))
            }
            _ => crate::hir::symbols::DGROUP,
        };
        Addr { index, ..Addr::new(space, 0) }
    }

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
    contracts: Option<&'a RuntimeContracts<'a>>,
    general: &'a [llrm_lir::registers::RegId],
}

/// What the program's ABI says about its runtime routines: a QB routine's
/// contract names the registers it destroys and how it returns, which its
/// declared convention does not.
pub struct RuntimeContracts<'a> {
    pub abi: &'a dyn crate::backend::assemble::Abi,
    pub segments: &'a crate::backend::target::Segments,
}

/// For each call in `function` of a `llrm.qb.` routine: `Ok` when the
/// convention models what its contract says (the callee pops what was pushed,
/// it returns in a register and to the next instruction), else why not. The old
/// selector models the rest.
pub(crate) fn runtime_call_verdicts(
    module: &Module,
    function: &llrm_mir::module::Function,
    layout: &DataLayout,
    arch: &dyn Target,
    abi: &dyn crate::backend::assemble::Abi,
) -> llrm_mir::dense::IdMap<llrm_mir::module::InstId, Result<RuntimeAnswer, &'static str>> {
    use llrm_mir::module::Operand;
    use llrm_mir::opcode::Opcode;
    let mut out = llrm_mir::dense::IdMap::new();
    for &block in function.layout() {
        for inst in function.iter_block(block) {
            let instruction = function.instruction(inst);
            let (Opcode::Call(info) | Opcode::Invoke(info)) = &instruction.opcode else { continue };
            let Some(&Operand::Constant(callee)) = instruction.operands.last() else { continue };
            let llrm_mir::context::ConstantKind::Global(global) = module.context.get(callee).kind else { continue };
            let name = module.global(global).name.owned().unwrap_or_default();
            if !name.starts_with(crate::hir::mir::RUNTIME) {
                continue;
            }
            let keep = instruction.operands.len() - if matches!(instruction.opcode, Opcode::Invoke(_)) { 3 } else { 1 };
            let types: Option<Vec<_>> = instruction.operands[..keep]
                .iter()
                .map(|argument| function.operand_type(&module.context, *argument))
                .collect();
            let first = arch.first_argument_offset(false);
            let (result, _, variadic) = module.signature(info.function_type);
            let plan = types.and_then(|types| {
                crate::backend::isel::convention_for(
                    module,
                    layout,
                    arch,
                    first,
                    info.calling_convention,
                    result,
                    variadic,
                    &types,
                    &info.argument_attrs,
                )
                .ok()
            });
            out.insert(inst, modelled(abi, &name, plan.as_ref(), first));
        }
    }
    out
}

fn modelled(
    abi: &dyn crate::backend::assemble::Abi,
    name: &str,
    plan: Option<&crate::backend::isel::Convention>,
    first: i64,
) -> Result<RuntimeAnswer, &'static str> {
    let plan = plan.ok_or("QB runtime call the convention does not describe")?;
    let pushed = (plan.stack_arguments - first).max(0);
    if abi.registers(name).is_some() {
        return Err("QB runtime routine with a register interface");
    }
    let contract = abi.contract(name, plan.popped > 0, pushed).map_err(|_| "QB runtime routine with no contract")?;
    // Raising an error, entering user code or an unstated way back (a handler
    // registration, `B$OEGA`) changes no selection: the old selector reads none
    // of them. A routine that comes back at an inline table does.
    if contract.control == llrm_qbruntime::Control::InlineTable {
        return Err("QB runtime routine that returns to an inline table");
    }
    if contract.cleanup != Some(plan.popped) || plan.popped != pushed {
        return Err("QB runtime routine whose stack cleanup the convention does not model");
    }
    Ok(if contract.flags_result { RuntimeAnswer::Flags } else { RuntimeAnswer::Registers })
}

impl super::emit::Conventions for CallConventions<'_> {
    fn clobbers_high(
        &self,
        callee: Option<&Callee>,
    ) -> std::collections::BTreeSet<llrm_lir::registers::RegId> {
        let number = match callee {
            Some(Callee::Global(id)) => {
                self.module.global(llrm_mir::context::GlobalId(*id)).function().map_or(0, |f| f.calling_convention)
            }
            _ => 0,
        };
        let Some(found) = self.arch.calling().by_number(number) else { return Default::default() };
        let regs = llrm_lir::registers::Regs(self.arch.registers());
        self.arch
            .convention_callee_saved(found)
            .into_iter()
            .filter(|(full, pushed)| regs.bytes(*pushed) < regs.bytes(*full))
            .map(|(full, _)| full)
            .collect()
    }

    fn clobbers(
        &self,
        callee: Option<&Callee>,
    ) -> Option<std::collections::BTreeSet<llrm_lir::registers::RegId>> {
        // inline assembly changes the registers its name states
        if let Some(Callee::Named(name)) = callee
            && let Some(block) = llrm_mir::intrinsics::asm(name)
        {
            let regs = llrm_lir::registers::Regs(self.arch.registers());
            return Some(
                block
                    .clobbers
                    .iter()
                    .filter(|name| !matches!(name.as_str(), "flags" | "memory") && !name.starts_with("st"))
                    .map(|reg| regs.root(self.arch.register_named(reg)))
                    .collect(),
            );
        }
        // a huge-pointer routine changes what it states and no more
        if let Some(Callee::Named(name)) = callee {
            let regs = llrm_lir::registers::Regs(self.arch.registers());
            let fills = [1u32, 2, 4].into_iter().filter_map(|cell| self.arch.huge_fill_routine(cell).ok());
            let copies = [false, true].into_iter().filter_map(|backward| self.arch.huge_copy_routine(backward).ok());
            if let Some(routine) = fills.chain(copies).find(|routine| routine.name == name.as_str()) {
                return Some(routine.clobbers.iter().map(|reg| regs.root(*reg)).collect());
            }
        }
        let number = match callee {
            Some(Callee::Global(id)) => {
                self.module.global(llrm_mir::context::GlobalId(*id)).function().map_or(0, |f| f.calling_convention)
            }
            _ => 0,
        };
        let found = self.arch.calling().by_number(number)?;
        let regs = llrm_lir::registers::Regs(self.arch.registers());
        let mut out: std::collections::BTreeSet<_> = found
            .clobbered
            .iter()
            .filter(|name| !matches!(name.as_str(), "flags") && !name.starts_with("st"))
            .map(|name| regs.root(self.arch.register_named(name)))
            .collect();
        // a runtime routine also destroys what its contract says
        if let (Some(contracts), Some(Callee::Global(id))) = (self.contracts, callee) {
            let name = self.module.global(llrm_mir::context::GlobalId(*id)).name.owned().unwrap_or_default();
            if name.starts_with(crate::hir::mir::RUNTIME)
                && let Ok(contract) = contracts.abi.contract(&name, false, 0)
            {
                out.extend(crate::backend::callregs::call_clobbers(&contract, contracts.segments, self.general));
            }
        }
        Some(out)
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
                    .chain([
                        (llrm_dagisel::x86_llrm::FARCALL_OPCODE, "CALL_FAR"),
                        (llrm_dagisel::x86_llrm::CODE_OPCODE, "CALL_CODE"),
                        (llrm_dagisel::x86_llrm::PORT_OPCODE, "PORT_IO"),
                        (llrm_dagisel::x86_llrm::FLN_OPCODE, "LN_Fp80"),
                        (llrm_dagisel::x86_llrm::FEXP_OPCODE, "EXP_Fp80"),
                        (llrm_dagisel::x86_llrm::FLOG2_OPCODE, "LOG2_Fp80"),
                        (llrm_dagisel::x86_llrm::FEXP2_OPCODE, "EXP2_Fp80"),
                        (llrm_dagisel::x86_llrm::FSIN_OPCODE, "SIN_Fp80"),
                        (llrm_dagisel::x86_llrm::FCOS_OPCODE, "COS_Fp80"),
                        (llrm_dagisel::x86_llrm::FATAN_OPCODE, "ATAN_Fp80"),
                        (llrm_dagisel::x86_llrm::LLRM_ASM_OPCODE, "CALL_ASM"),
                    ])
                    .collect(),
            })
        })
        .clone()
}

/// The addresses each LIR block can go to: the targets of its jumps and
/// branches, and the next block if it does not end in a jump or a return.
fn successors(
    emitted: &[EmitBlock],
    sinks: &HashSet<i64>,
) -> Vec<Vec<i64>> {
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
                for &target in &what.table {
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
            if !ends && !sinks.contains(&block.at) {
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

/// The fixed objects the stack parameters make, as the builder numbers them (a
/// byval aggregate one, a cell one for each register-sized part of its value:
/// an integer of twice the widest legal integer is two). The widest legal
/// integer is the builder's: 16 bits was assumed at m16 after `i32` became
/// legal there, so an `i32` got two cells and an `i64` one, and the second half
/// of an `i64` argument was read from the next parameter's cell.
pub(super) fn stack_cells(
    module: &Module,
    function: &llrm_mir::module::Function,
    convention: &crate::backend::isel::Convention,
    widest: u32,
) -> Vec<i64> {
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
    cells
}

fn refuse<T>(why: impl Into<String>) -> Result<T, Unselected> {
    Err(Unselected(format!("--isel=dag: {}", why.into())))
}

/// `name` of `module` selected by the DAG path. A panic in a stage (a builder
/// meeting a type it does not know) is a refusal of the function, as any other
/// gap: it stopped 18 QCport modules from compiling in the shadow. Constants it
/// needs in memory go to a pool of its own; [`selected_in_pool`] shares the
/// module's.
pub fn selected(
    module: &Module,
    name: &str,
    arch: &dyn Target,
    cpu: &crate::backend::cpu::Profile,
    libcalls: &[(String, u32, String)],
) -> Result<Selected, Unselected> {
    selected_in_pool(module, name, arch, cpu, libcalls, None, &mut Pool::new(module.globals.len() as i64))
}

pub fn selected_in_pool(
    module: &Module,
    name: &str,
    arch: &dyn Target,
    cpu: &crate::backend::cpu::Profile,
    libcalls: &[(String, u32, String)],
    contracts: Option<&RuntimeContracts<'_>>,
    pool: &mut Pool,
) -> Result<Selected, Unselected> {
    let mark = pool.mark();
    let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        selected_in(module, name, arch, cpu, libcalls, contracts, pool)
    }))
    .unwrap_or_else(|why| {
        let why =
            why.downcast_ref::<String>().cloned().or_else(|| why.downcast_ref::<&str>().map(|one| (*one).to_owned()));
        refuse(format!("--isel=dag: a stage stopped: {}", why.unwrap_or_default()))
    });
    if done.is_err() {
        pool.rewind(mark);
    }
    done
}

/// The module's constant pool as the emitter asks it.
struct PoolCells<'a>(std::cell::RefCell<&'a mut Pool>);

impl super::emit::ConstantCells for PoolCells<'_> {
    fn cell(
        &self,
        bytes: Vec<u8>,
    ) -> Mem {
        self.0.borrow_mut().cell(bytes)
    }
}

fn selected_in(
    module: &Module,
    name: &str,
    arch: &dyn Target,
    cpu: &crate::backend::cpu::Profile,
    libcalls: &[(String, u32, String)],
    contracts: Option<&RuntimeContracts<'_>>,
    pool: &mut Pool,
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
    let mut tli = if m16 {
        TargetLowering::x86_16(Features::default(), MVT::I16)
    } else {
        TargetLowering::x86_32(Features::default(), MVT::I32)
    };
    // SD214: with a math library the pin calls it for these; the call is not
    // translated yet, so the node is refused rather than selected inline.
    if arch.has_libm() {
        for op in [
            super::isd::ISD::FLog,
            super::isd::ISD::FLog2,
            super::isd::ISD::FExp,
            super::isd::ISD::FExp2,
            super::isd::ISD::FSin,
            super::isd::ISD::FCos,
            super::isd::ISD::FAtan,
        ] {
            for vt in [MVT::F32, MVT::F64, MVT::F80] {
                tli.set_operation_action(op, vt, super::actions::Action::LibCall);
            }
        }
    }
    let (builder_target, address_mode) =
        if m16 { (BuilderTarget::m16(), AddrMode::Real16) } else { (BuilderTarget::m32(), AddrMode::Flat32) };
    let builder_target = builder_target
        .with_runtime_calls(libcalls)
        .with_float_store(cpu.float_store)
        .with_tti(cpu.target().tti())
        .with_runtime_call_verdicts(
            contracts.map(|contracts| runtime_call_verdicts(module, function, &layout, arch, contracts.abi)),
        );
    let tables = tables(arch);
    // The builder numbers the fixed objects the stack parameters make: a byval
    // aggregate one, a cell one for each register-sized part of its value (an
    // integer of twice the widest register is two). Dropping a byval one
    // moved every later parameter by its size (byvalue_struct_address passed
    // the wrong address).
    let widest = *builder_target.legal_ints.iter().max().expect("a legal integer type");
    let mut cells = stack_cells(module, function, &convention, widest);
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
    let alloca_count = allocas.len() as i32;
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
    let mut block_at: HashMap<u32, i64> = function.layout().iter().map(|b| (b.0, state.next_address())).collect();
    // Every block is built before any is emitted: the blocks a `switch`
    // lowering makes (`CreateMachineBasicBlock`) are named by the branches
    // before them, so their addresses are fixed here too. Each is emitted
    // after the block that made it and its LIR blocks belong to that block's
    // range, where the phis of its successors look for their edges.
    let mut work: Vec<(BlockId, BlockId, crate::backend::selectiondag::builder::BlockDag)> = Vec::new();
    for &block in function.layout() {
        let mut built = build_block(module, function, &layout, &builder_target, arch, &convention, &mut info, block)
            .map_err(|why| Unselected(format!("--isel=dag: {}", why.0)))?;
        let extras = std::mem::take(&mut built.extra);
        work.push((block, block, built));
        for (id, extra) in extras {
            block_at.insert(id.0, state.next_address());
            work.push((id, block, extra));
        }
    }
    // The stack objects the builder made (a rounded float's slot) follow the
    // static allocas in the frame.
    for (at, (size, align)) in info.objects.iter().enumerate() {
        let made = Temporaries(&frame).create_stack_temporary(*size, *align);
        debug_assert_eq!(made, alloca_count + at as i32, "frame objects are numbered as the builder numbered them");
    }
    // Each jump table is a data object the pool names; its entries are the
    // addresses of the blocks.
    let tables_of_function: Vec<JumpTable> = info
        .jump_tables
        .iter()
        .map(|blocks| JumpTable { id: pool.table(), targets: blocks.iter().map(|block| block_at[block]).collect() })
        .collect();
    let globals = Globals(module, arch.layout().spaces.roles);
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
        tables: &tables_of_function,
        caller_saved: &caller_saved,
        conventions: &CallConventions { module, arch, contracts, general: &cpu.general },
        constants: &PoolCells(std::cell::RefCell::new(pool)),
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
    // the landing pad the runtime enters, which no edge reaches
    let mut landing: Option<i64> = None;
    let mut sinks: HashSet<i64> = HashSet::new();
    let mut inline_places = IndexMap::default();
    // the call sites whose callee is far code (address space 1): a far call
    let mut far = std::collections::BTreeSet::new();
    // The LIR blocks each MIR block became, and the phi inputs each exports.
    let mut made: HashMap<u32, std::ops::Range<usize>> = HashMap::new();
    let mut exports: Vec<(BlockId, Vec<(ValueId, u32)>)> = Vec::new();
    for (block, parent, mut built) in work {
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
                // the runtime's routines are far procedures where code is in
                // segments
                Callee::Named(name) => {
                    // inline code is laid down at the site: no call
                    if arch.object().bitness == 16
                        && llrm_mir::intrinsics::code(name).is_none()
                        && llrm_mir::intrinsics::asm(name).is_none()
                    {
                        far.insert(*at);
                    }
                    name.clone()
                }
            };
            (*at, name)
        }));
        inline_places.extend(done.inline_places.iter().cloned());
        emitted.extend(done.blocks);
        if block == parent && is_pad(function, block) {
            let Some(contracts) = contracts else {
                return refuse("a landing pad of a program with no runtime contracts");
            };
            if landing.replace(emitted[from].at).is_some() {
                return refuse("more than one landing pad");
            }
            let at = emitted[from].at;
            let pad = landing_pad_insn(at, contracts.segments, &cpu.general);
            emitted[from].insns.insert(0, std::sync::Arc::new(pad));
            calls.insert(at, LANDING.to_owned());
        }
        // A block that ends in `unreachable` (after a call that does not
        // return) leads nowhere: it does not fall into the next one.
        if block == parent
            && function
                .terminator(block)
                .is_some_and(|inst| function.instruction(inst).opcode == llrm_mir::opcode::Opcode::Unreachable)
        {
            sinks.extend(emitted.last().map(|last| last.at));
        }
        let start = made.get(&parent.0).map_or(from, |range| range.start);
        made.insert(parent.0, start..emitted.len());
        exports.push((parent, built.phi_updates));
    }
    let succ = successors(&emitted, &sinks);
    // Phis: the successor's phi takes, from each LIR block of the predecessor
    // that leaves for it, the register the predecessor copied its value to.
    for (pred, updates) in &exports {
        for (at_update, (phi, reg_in)) in updates.iter().enumerate() {
            let ValueDef::Instruction(def) = function.value(*phi).def else {
                return refuse("a phi that is not an instruction");
            };
            let Some(phi_block) = function.parent(def) else { return refuse("a phi outside a block") };
            let at = block_at[&phi_block.0];
            let Some(reg_out) = info.value_regs.get(phi).copied() else { return refuse("a phi with no register") };
            // A value of two registers (a far pointer, an `i64`) has one entry
            // for each part, in order: the k-th is the phi's k-th register.
            let part = updates[..at_update].iter().filter(|(one, _)| one == phi).count();
            let reg_out = if part == 0 { reg_out } else { info.part_regs(reg_out, part + 1)[part] };
            let (result, incoming) = (state.vreg(reg_out), state.vreg(*reg_in));
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
    // `UnreachableMachineBlockElim`: a block no edge leads to (the
    // `unreachable` default of a switch whose cases cover its values) is
    // dropped, with the phi inputs that came from it.
    let (emitted, succ) = without_unreachable(emitted, succ, landing);
    // Every value read is defined on every path to the read (a phi, an
    // instruction, an arrival): a LIR that reads one nothing defines on some
    // path stops the allocator's liveness, so it is a refusal here.
    if let Some((value, name)) = read_before_defined(&emitted, &succ, landing) {
        return refuse(format!("--isel=dag: value {value} is read and not defined on every path ({name:?})"));
    }
    // A floating value is a 10-byte register value: read at another width it is
    // an integer's bits taken from an x87 register (`bitcast` of a double to
    // its words, reaching the matcher as a no-op), which no code can do.
    {
        let floating: std::collections::HashSet<u32> = emitted
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter_map(|insn| insn.what.as_ref())
            .flat_map(|what| what.dests.iter())
            .filter_map(|dest| match dest {
                crate::model::ir::Loc::Held(held) if held.width == 10 => Some(held.value),
                _ => None,
            })
            .collect();
        for insn in emitted.iter().flat_map(|block| block.insns.iter()) {
            let Some(what) = insn.what.as_ref() else { continue };
            let mentions = what.dests.iter().chain(&what.sources);
            if mentions.into_iter().any(|loc| {
                matches!(
                    loc,
                    crate::model::ir::Loc::Held(held) if held.width != 10 && floating.contains(&held.value)
                )
            }) {
                return refuse(format!("--isel=dag: a floating value used as an integer ({:?})", what.name));
            }
        }
    }
    // The x87 stack is empty at a call: a floating value live across one is
    // spilled by the pin's allocator, which no pass here does for this LIR, so
    // the function is refused (the stackifier would fail the compile).
    // A function with a pad is entered through a root block that never branches
    // to it, so the pad is reachable in the machine's graph.
    let root_at = landing.map(|_| state.next_address());
    let blocks: Vec<Block> = root_at
        .zip(landing)
        .map(|(at, pad)| {
            let never = [
                crate::backend::isel::insn(
                    at,
                    crate::backend::isel::semantics(
                        crate::model::ir::Operation::Compare,
                        "cmp",
                        vec![],
                        vec![
                            crate::model::ir::Loc::Reg(crate::model::ir::Reg {
                                register: crate::model::ir::STACK,
                                width: 2,
                            }),
                            crate::model::ir::Loc::Imm(crate::model::ir::Imm { value: 0, width: 2, address: None }),
                        ],
                    ),
                ),
                crate::backend::isel::insn(
                    at,
                    crate::model::ir::Semantics {
                        target: Some(pad),
                        ..crate::backend::isel::semantics(crate::model::ir::Operation::Branch, "je", vec![], vec![])
                    },
                ),
            ];
            Block::from(LirBlock { succ: vec![pad, emitted[0].at].into(), ..LirBlock::new(at, never.to_vec().into()) })
        })
        .into_iter()
        .chain(emitted.iter().zip(succ).map(|(block, succ)| -> Block {
            LirBlock {
                succ: succ.into(),
                phis: block.phis.clone().into(),
                ..LirBlock::new(block.at, block.insns.iter().cloned().collect::<Vec<_>>().into())
            }
            .into()
        }))
        .collect();
    let entry = root_at.unwrap_or(emitted[0].at);
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
    if let Some(at) = landing {
        inline.insert(at, vec![CLD]);
    }
    // Inline code: its bytes with each frame place patched as a displacement
    // from BP (the old route's `inline_code`).
    for (at, name) in &calls {
        if let Some((mut bytes, places)) = llrm_mir::intrinsics::code(name) {
            for (offset, disp, addend) in inline_places.get(at).map_or(&[][..], |one: &Vec<(usize, i64, i64)>| &one[..])
            {
                bytes[*offset..*offset + 2].copy_from_slice(&((disp + addend) as u16).to_le_bytes());
            }
            let _ = places;
            inline.insert(*at, bytes);
        }
    }
    // The huge-pointer routines the calls name (`huge_memory`): their bytes
    // are laid down at the call as well.
    for (at, name) in &calls {
        let fills = [1u32, 2, 4].into_iter().filter_map(|cell| arch.huge_fill_routine(cell).ok());
        let copies = [false, true].into_iter().filter_map(|backward| arch.huge_copy_routine(backward).ok());
        if let Some(routine) = fills.chain(copies).find(|routine| routine.name == name.as_str()) {
            inline.insert(*at, routine.code);
        }
    }
    // An inline assembly block's bytes are laid down at its site.
    for (at, name) in &calls {
        if let Some(block) = llrm_mir::intrinsics::asm(name) {
            inline.insert(*at, block.code);
        }
    }
    Ok(Selected {
        body,
        convention,
        calls,
        inline,
        inline_places,
        inline_symbols: IndexMap::default(),
        far,
        pops: IndexMap::default(),
        depth: frame.depth.get(),
        extents: frame.locals.borrow().clone(),
        landing,
    })
}

/// `emitted` and `succ` without the blocks no path from the entry or the
/// landing pad reaches.
fn without_unreachable(
    mut emitted: Vec<EmitBlock>,
    succ: Vec<Vec<i64>>,
    landing: Option<i64>,
) -> (Vec<EmitBlock>, Vec<Vec<i64>>) {
    let index: HashMap<i64, usize> = emitted.iter().enumerate().map(|(i, b)| (b.at, i)).collect();
    let mut reached = vec![false; emitted.len()];
    let mut work: Vec<usize> = vec![0];
    work.extend(landing.and_then(|at| index.get(&at).copied()));
    while let Some(at) = work.pop() {
        if std::mem::replace(&mut reached[at], true) {
            continue;
        }
        work.extend(succ[at].iter().filter_map(|next| index.get(next).copied()));
    }
    if reached.iter().all(|one| *one) {
        return (emitted, succ);
    }
    let gone: std::collections::HashSet<i64> =
        emitted.iter().zip(&reached).filter(|(_, kept)| !**kept).map(|(block, _)| block.at).collect();
    let succ: Vec<Vec<i64>> = succ.into_iter().zip(&reached).filter(|(_, kept)| **kept).map(|(s, _)| s).collect();
    let mut kept = reached.iter();
    emitted.retain(|_| *kept.next().expect("a flag for each block"));
    for block in &mut emitted {
        for phi in &mut block.phis {
            phi.incoming.retain(|(from, _)| !gone.contains(from));
        }
    }
    (emitted, succ)
}

/// What starts a landing pad: `cld`, the one state it sets, as a call that
/// leaves no register as it was (`isel/unwind.rs`).
const LANDING: &str = "__LANDING";
const CLD: u8 = 0xFC;

/// Whether `block` starts a landing pad, which the runtime enters.
fn is_pad(
    function: &llrm_mir::module::Function,
    block: BlockId,
) -> bool {
    function
        .iter_block(block)
        .any(|one| matches!(
            function.instruction(one).opcode,
            llrm_mir::opcode::Opcode::LandingPad { .. }
        ))
}

fn landing_pad_insn(
    at: i64,
    segments: &crate::backend::target::Segments,
    general: &[llrm_lir::registers::RegId],
) -> Insn {
    use crate::backend::callregs::{call_clobbered_high, call_clobbers};
    let contract =
        crate::backend::lower_int64::_helper(LANDING, Default::default(), crate::abi::runtime::EVERY.clone());
    Insn {
        clobbers: call_clobbers(&contract, segments, general),
        clobbers_high: call_clobbered_high(&contract, segments, general),
        ..Insn::new(
            at,
            Some((at, at)),
            Some(crate::backend::isel::semantics(crate::model::ir::Operation::Call, "call", vec![], vec![])),
            vec![],
            vec![],
        )
    }
}

/// A value read where some path to it defines nothing: (value, instruction).
fn read_before_defined(
    blocks: &[EmitBlock],
    succ: &[Vec<i64>],
    landing: Option<i64>,
) -> Option<(u32, Option<String>)> {
    use std::collections::HashSet;
    let index: std::collections::HashMap<i64, usize> = blocks.iter().enumerate().map(|(i, b)| (b.at, i)).collect();
    let defines = |insn: &Insn| -> Vec<u32> {
        // A copy of a value to itself defines nothing.
        let itself = insn
            .what
            .as_ref()
            .is_some_and(
                |what| matches!(
                    (&what.dests[..], &what.sources[..]),
                    ([crate::model::ir::Loc::Held(to)], [crate::model::ir::Loc::Held(from)]) if to.value == from.value
                ),
            );
        if itself { Vec::new() } else { insn.defines.clone() }
    };
    let own: Vec<HashSet<u32>> = blocks
        .iter()
        .map(|block| {
            block.phis.iter().map(|phi| phi.result).chain(block.insns.iter().flat_map(|insn| defines(insn))).collect()
        })
        .collect();
    // What is defined on every path to the start of each block (`None`: no
    // path has reached it yet).
    let mut entry: Vec<Option<HashSet<u32>>> = vec![None; blocks.len()];
    entry[0] = Some(HashSet::new());
    // The runtime enters a landing pad with no value defined: a root of its
    // own.
    if let Some(&pad) = landing.and_then(|at| index.get(&at)) {
        entry[pad] = Some(HashSet::new());
    }
    let mut changed = true;
    while changed {
        changed = false;
        for at in 0..blocks.len() {
            let Some(here) = entry[at].clone() else { continue };
            let out: HashSet<u32> = here.union(&own[at]).copied().collect();
            for next in &succ[at] {
                let Some(&to) = index.get(next) else { continue };
                let merged = match &entry[to] {
                    None => out.clone(),
                    Some(known) => known.intersection(&out).copied().collect(),
                };
                if entry[to].as_ref() != Some(&merged) {
                    entry[to] = Some(merged);
                    changed = true;
                }
            }
        }
    }
    for (at, block) in blocks.iter().enumerate() {
        let Some(mut known) = entry[at].clone() else { continue };
        known.extend(block.phis.iter().map(|phi| phi.result));
        for insn in &block.insns {
            if let Some(value) = insn.uses.iter().find(|value| !known.contains(value)) {
                return Some((*value, insn.what.as_ref().and_then(|what| what.name.as_ref().map(|n| n.to_string()))));
            }
            known.extend(defines(insn));
        }
        // What a phi takes from an edge is defined at the end of that edge's
        // block.
        for phi in &block.phis {
            for (edge, value) in &phi.incoming {
                let Some(&from) = index.get(edge) else { continue };
                let Some(start) = entry[from].as_ref() else { continue };
                if !start.contains(value) && !own[from].contains(value) {
                    return Some((*value, Some("phi".to_owned())));
                }
            }
        }
    }
    None
}
