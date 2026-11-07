//! `FastISel` for x86 at -O0: the pin's `-fast-isel` path, which selects an
//! IR block straight to machine nodes without the combiner, the legalizers or
//! the matcher.
//!
//! Off unless `LLRM_FAST_ISEL=1`, and then only at -O0 (`!cpu.optimized`); -O1
//! and up never see it. A function it cannot take is selected by the DAG path,
//! whole (DEVIATE F2).
//!
//! # See also
//! `docs`: `/home/alim/scratch/handoff/o0-fastisel-poc-plan-2026-11-06.md`.

use std::cell::Cell;

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, Function, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;

use super::builder::{BlockDag, BuilderTarget, fresh_dag};
use super::dag::{NodeData, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::EVT;
use super::function_lowering::FunctionLoweringInfo;
use crate::backend::isel::Convention;

thread_local! {
    static TOOK: Cell<bool> = const { Cell::new(false) };
}

/// Whether `LLRM_FAST_ISEL=1` asks for the fast path.
pub fn enabled() -> bool {
    llrm_support::env_var("LLRM_FAST_ISEL").is_some_and(|value| value == "1")
}

/// Records whether the function being selected goes through FastISel.
pub fn note(fast: bool) {
    TOOK.with(|took| took.set(fast));
}

/// The function just selected went through FastISel: read once by the caller
/// that records the selector.
pub fn take_took() -> bool {
    TOOK.with(|took| took.replace(false))
}

/// Logs why FastISel left a whole function to the DAG (`LLRM_DEBUG=isel`).
fn declined<T>(why: &str) -> Option<T> {
    llrm_support::debug!("isel", "fast declined: {why}");
    None
}

/// A 32-bit integer, or at m32 a pointer in the default space (the same register).
fn word(types: &llrm_mir::types::Types, ty: llrm_mir::types::TypeId, m32: bool) -> bool {
    types.int_bits(ty) == Some(32) || (m32 && matches!(types.get(ty), llrm_mir::types::Type::Pointer(0)))
}

/// FastISel's blocks for every block of `function`, or none: one block it
/// cannot take sends the whole function to the DAG path.
// DEVIATE F1: hand-written subset; exit 2026-11-27 (a generator over X86GenFastISel.inc).
// DEVIATE F2: whole-function fallback; exit with integer + call coverage.
// DEVIATE F3: no FP, vector, atomics or intrinsics; exit after parity.
#[allow(clippy::too_many_arguments)]
pub fn build_all(
    module: &Module,
    function: &Function,
    target: &BuilderTarget,
    opcode: &dyn Fn(&str) -> Option<u16>,
    data_layout: &llrm_mir::datalayout::DataLayout,
    convention: &Convention,
    arch: &dyn llrm_target::Target,
    info: &mut FunctionLoweringInfo,
) -> Option<Vec<(BlockId, BlockDag)>> {
    let m32 = arch.object().bitness == 32;
    if !(m32 || arch.object().bitness == 64) || module.signature(function.ty).2 {
        return declined("a variadic function or another width");
    }
    // every parameter is a 4-byte stack cell: fixed object -(n + 1)
    let int32 = |ty| word(&module.context.types, ty, m32);
    let (returns, parameters, _) = module.signature(function.ty);
    if parameters.iter().any(|&ty| !int32(ty))
        || convention.parameters.len() != parameters.len()
        || convention.parameters.iter().any(|one| !matches!(one, crate::backend::isel::Parameter::Cell(_)))
    {
        return declined("a parameter that is not a 4-byte stack cell");
    }
    let returns_int = int32(returns);
    if !returns_int && !module.context.types.is_void(returns) {
        return declined("a result that is not an int, a pointer or void");
    }
    let eax = (returns_int && convention.returns.len() == 1).then(|| arch.registers().named(convention.returns[0], 4));
    let names = Names::new(opcode, if m32 { llrm_dagisel::fastisel::Table::I486 } else { llrm_dagisel::fastisel::Table::X64 })?;
    let mut blocks = Vec::new();
    let layout = function.layout();
    for (at, &block) in layout.iter().enumerate() {
        let next = layout.get(at + 1).copied();
        let dag = fresh_dag(module, function, target, info, block, None);
        let chain = dag.entry_node();
        let mut declined = String::new();
        let dag = Fast { layout: data_layout, module, function, info: &*info, names: &names, eax, any_eax: arch.registers().named(arch.register_named("eax"), 4), esp: arch.stack_pointer(), edx: arch.registers().named(arch.register_named("edx"), 4), m32, dag, chain, values: Default::default(), next }
            .block(block, &mut declined);
        let Some(dag) = dag else {
            llrm_support::debug!("isel", "fast declined at {declined}");
            return None;
        };
        blocks.push((
            block,
            BlockDag { dag, phi_updates: Vec::new(), live_ins: Vec::new(), extra: Vec::new(), tail_calls: Vec::new(), values: Vec::new() },
        ));
    }
    Some(blocks)
}

/// The machine opcodes FastISel emits.
struct Names {
    mov_rm: u16,
    mov_mr: u16,
    mov_mi: u16,
    mov_ri: u16,
    ret: u16,
    lea: Option<u16>,
    jmp: u16,
    jcc: u16,
    cmp_rr: u16,
    cmp_ri: u16,
    /// `add`, `sub`, `and`, `or`, `xor` with a register and with an immediate.
    alu: [(u16, u16); 5],
    /// m32 only so far: calls and the signed divide.
    calls: Option<Calls>,
}

struct Calls {
    call_down: u16,
    call_up: u16,
    call: u16,
    cdq: u16,
    idiv: u16,
    div: u16,
}

impl Names {
    fn new(opcode: &dyn Fn(&str) -> Option<u16>, table: llrm_dagisel::fastisel::Table) -> Option<Names> {
        // The pin's own rows (`fastEmit_ISD_<OP>_MVT_i32_rr` / `_ri`) name the opcodes.
        let pair = |one: &str| {
            let row = |shape| llrm_dagisel::fastisel::lookup(table, "ISD", one, "i32", "", shape).map(|(name, _)| name);
            Some((opcode(row(1)?)?, opcode(row(2)?)?))
        };
        Some(Names {
            mov_rm: opcode("MOV32rm")?,
            mov_mr: opcode("MOV32mr")?,
            mov_mi: opcode("MOV32mi")?,
            mov_ri: opcode("MOV32ri")?,
            ret: opcode("RET")?,
            lea: opcode("LEA32r"),
            jmp: opcode("JMP_1")?,
            jcc: opcode("JCC_1")?,
            cmp_rr: opcode("CMP32rr")?,
            cmp_ri: opcode("CMP32ri")?,
            alu: [pair("ADD")?, pair("SUB")?, pair("AND")?, pair("OR")?, pair("XOR")?],
            calls: (|| {
                Some(Calls {
                    call_down: opcode("ADJCALLSTACKDOWN32")?,
                    call_up: opcode("ADJCALLSTACKUP32")?,
                    call: opcode("CALLpcrel32")?,
                    cdq: opcode("CDQ")?,
                    idiv: opcode("IDIV32r")?,
                    div: opcode("DIV32r")?,
                })
            })(),
        })
    }
}

struct Fast<'a> {
    layout: &'a llrm_mir::datalayout::DataLayout,
    module: &'a Module,
    function: &'a Function,
    info: &'a FunctionLoweringInfo,
    names: &'a Names,
    eax: Option<llrm_lir::registers::RegId>,
    any_eax: llrm_lir::registers::RegId,
    esp: llrm_lir::registers::RegId,
    edx: llrm_lir::registers::RegId,
    /// Frame objects are addressed at m32 only so far (F3).
    m32: bool,
    dag: SelectionDAG,
    /// The memory chain: every load and store follows the one before.
    chain: SDValue,
    values: llrm_mir::dense::IdMap<ValueId, SDValue>,
    next: Option<BlockId>,
}

impl Fast<'_> {
    fn node(&mut self, op: u16, vts: &[EVT], ops: &[SDValue]) -> SDValue {
        self.dag.intern(ISD::Machine(u32::from(op)), vts, ops, Default::default(), NodeData::None)
    }

    fn int(&self, operand: Operand) -> Option<i128> {
        let Operand::Constant(id) = operand else { return None };
        match self.module.context.get(id).kind {
            ConstantKind::Int(value) => i32::try_from(value).ok().map(i128::from).or_else(|| u32::try_from(value).ok().map(|v| i128::from(v as i32))),
            _ => None,
        }
    }

    /// `[base, scale, index, disp, segment]` of a frame object, or of the pointer in a register.
    fn address(&mut self, pointer: Operand) -> Option<Vec<SDValue>> {
        if !self.m32 {
            return None;
        }
        if let Operand::Value(value) = pointer
            && let Some(&object) = self.info.static_allocas.get(&value)
        {
            let ValueDef::Instruction(def) = self.function.value(value).def else { return None };
            let Opcode::Alloca { allocated, .. } = self.function.instruction(def).opcode else { return None };
            if !word(&self.module.context.types, allocated, true) {
                return None;
            }
            return Some(self.frame(object));
        }
        let base = self.value(pointer)?;
        Some(vec![
            base,
            self.dag.get_constant_with(1, EVT::I8, true, false),
            self.dag.get_register(EVT::I32, 0),
            self.dag.get_constant_with(0, EVT::I32, true, false),
            self.dag.get_register(EVT::I16, 0),
        ])
    }

    fn eax_reg(&self) -> Option<llrm_lir::registers::RegId> {
        Some(self.any_eax)
    }

    fn frame(&mut self, object: i32) -> Vec<SDValue> {
        vec![
            self.dag.get_target_frame_index(EVT::I32, object),
            self.dag.get_constant_with(1, EVT::I8, true, false),
            self.dag.get_register(EVT::I32, 0),
            self.dag.get_constant_with(0, EVT::I32, true, false),
            self.dag.get_register(EVT::I16, 0),
        ]
    }

    /// The value of `operand` in a register: a computed one, a constant, or a parameter loaded from its cell.
    fn value(&mut self, operand: Operand) -> Option<SDValue> {
        match operand {
            Operand::Constant(_) => {
                // DEVIATE F4: the pin (X86MaterializeInt) makes 0 with `MOV32r0` (xor) and a 64-bit
                // constant with `getMOVriOpcode`; here every constant is `MOV32ri`. Exit 2026-11-27 with F1.
                let imm = self.dag.get_constant_with(self.int(operand)? as u128, EVT::I32, true, false);
                Some(self.node(self.names.mov_ri, &[EVT::I32], &[imm]))
            }
            Operand::Value(value) => {
                if let Some(&done) = self.values.get(&value) {
                    return Some(done);
                }
                let ValueDef::Argument(n) = self.function.value(value).def else { return None };
                if !self.m32 {
                    return None;
                }
                let mut ops = self.frame(-(n as i32) - 1);
                ops.push(self.chain);
                let load = self.node(self.names.mov_rm, &[EVT::I32, EVT::OTHER], &ops);
                self.chain = SDValue { node: load.node, res: 1 };
                self.values.insert(value, load);
                Some(load)
            }
            Operand::Block(_) => None,
        }
    }

    fn block(mut self, block: BlockId, declined: &mut String) -> Option<SelectionDAG> {
        self.chain = self.dag.entry_node();
        let insts: Vec<_> = self.function.iter_block(block).collect();
        let mut at = 0;
        while at < insts.len() {
            let instruction = self.function.instruction(insts[at]).clone();
            at += 1;
            *declined = format!("{:?}", instruction.opcode);
            match &instruction.opcode {
                Opcode::Alloca { .. } => {
                    // a static alloca is its frame object
                    self.info.static_allocas.get(&instruction.result?)?;
                }
                Opcode::Load { volatile: false, .. } => {
                    if !word(&self.module.context.types, instruction.ty, self.m32) {
                        return None;
                    }
                    let mut ops = self.address(instruction.operands[0])?;
                    ops.push(self.chain);
                    let load = self.node(self.names.mov_rm, &[EVT::I32, EVT::OTHER], &ops);
                    self.chain = SDValue { node: load.node, res: 1 };
                    self.values.insert(instruction.result?, load);
                }
                Opcode::Store { volatile: false, .. } => {
                    let stored = self.function.operand_type(&self.module.context, instruction.operands[0])?;
                    if !word(&self.module.context.types, stored, self.m32) {
                        return None;
                    }
                    let mut ops = self.address(instruction.operands[1])?;
                    let op = if let Some(imm) = self.int(instruction.operands[0]) {
                        ops.push(self.dag.get_constant_with(imm as u128, EVT::I32, true, false));
                        self.names.mov_mi
                    } else {
                        let value = self.value(instruction.operands[0])?;
                        ops.push(value);
                        self.names.mov_mr
                    };
                    ops.push(self.chain);
                    let store = self.node(op, &[EVT::OTHER], &ops);
                    self.chain = store;
                }
                Opcode::GetElementPtr { source } if self.m32 => {
                    // one index: base + index * size(source), `lea`
                    let [base, index] = instruction.operands[..] else { return None };
                    let size = self.layout.alloc_size(&self.module.context.types, *source) as i64;
                    let base = self.value(base)?;
                    let noreg = self.dag.get_register(EVT::I32, 0);
                    let segment = self.dag.get_register(EVT::I16, 0);
                    let ops = if let Some(imm) = self.int(index) {
                        let disp = i32::try_from(i64::try_from(imm).ok()?.checked_mul(size)?).ok()?;
                        let one = self.dag.get_constant_with(1, EVT::I8, true, false);
                        let disp = self.dag.get_constant_with(disp as u32 as u128, EVT::I32, true, false);
                        vec![base, one, noreg, disp, segment]
                    } else {
                        if !matches!(size, 1 | 2 | 4 | 8)
                            || self.function.operand_type(&self.module.context, index).and_then(|ty| self.module.context.types.int_bits(ty)) != Some(32)
                        {
                            return None;
                        }
                        let index = self.value(index)?;
                        let scale = self.dag.get_constant_with(size as u128, EVT::I8, true, false);
                        let zero = self.dag.get_constant_with(0, EVT::I32, true, false);
                        vec![base, scale, index, zero, segment]
                    };
                    let lea = self.names.lea?;
                    let done = self.node(lea, &[EVT::I32], &ops);
                    self.values.insert(instruction.result?, done);
                }
                Opcode::Binary(op @ (llrm_mir::opcode::BinaryOp::SDiv | llrm_mir::opcode::BinaryOp::SRem | llrm_mir::opcode::BinaryOp::UDiv | llrm_mir::opcode::BinaryOp::URem)) => {
                    // signed `cdq; idiv r`, unsigned `mov edx,0; div r`: quotient in eax, remainder in edx
                    use llrm_mir::opcode::BinaryOp::*;
                    let calls = self.names.calls.as_ref()?;
                    if !self.m32 || self.module.context.types.int_bits(instruction.ty) != Some(32) {
                        return None;
                    }
                    let (left, right) = (self.value(instruction.operands[0])?, self.value(instruction.operands[1])?);
                    let eax = self.eax_reg()?;
                    let copy = self.dag.get_copy_to_phys_reg(self.dag.entry_node(), eax, left, None);
                    let glue = SDValue { node: copy.node, res: 1 };
                    let divided = if matches!(op, SDiv | SRem) {
                        let sign = self.node(calls.cdq, &[EVT::GLUE], &[glue]);
                        self.node(calls.idiv, &[EVT::GLUE], &[right, sign])
                    } else {
                        let zero = self.dag.get_constant_with(0, EVT::I32, true, false);
                        let clear = self.dag.get_copy_to_phys_reg(self.dag.entry_node(), self.edx, zero, Some(glue));
                        self.node(calls.div, &[EVT::GLUE], &[right, SDValue { node: clear.node, res: 1 }])
                    };
                    let register = if matches!(op, SDiv | UDiv) { eax } else { self.edx };
                    let result = self.dag.get_copy_from_phys_reg(self.dag.entry_node(), register, EVT::I32, divided);
                    self.values.insert(instruction.result?, SDValue { node: result.node, res: 0 });
                }
                Opcode::Call(call) if self.m32 && call.calling_convention == self.function.calling_convention => {
                    let calls = self.names.calls.as_ref()?;
                    let (callee, arguments) = instruction.operands.split_last()?;
                    let Operand::Constant(callee) = *callee else { return None };
                    let ConstantKind::Global(global) = self.module.context.get(callee).kind else { return None };
                    let (returns, parameters, variadic) = self.module.signature(call.function_type);
                    if variadic || arguments.len() != parameters.len() || self.module.global(global).function().is_none_or(|callee| callee.calling_convention != call.calling_convention) {
                        return None;
                    }
                    let int32 = |ty| word(&self.module.context.types, ty, true);
                    let void = self.module.context.types.is_void(returns);
                    if !parameters.iter().all(|&ty| int32(ty)) || !(void || int32(returns)) {
                        return None;
                    }
                    let values: Vec<SDValue> = arguments.iter().map(|&one| self.value(one)).collect::<Option<_>>()?;
                    let bytes = self.dag.get_constant_with(4 * values.len() as u128, EVT::I32, true, false);
                    let zero = self.dag.get_constant_with(0, EVT::I32, true, false);
                    let down = self.node(calls.call_down, &[EVT::I32, EVT::OTHER, EVT::GLUE], &[bytes, zero, zero, self.chain]);
                    let after_down = SDValue { node: down.node, res: 1 };
                    let mut stores = Vec::new();
                    for (at, value) in values.into_iter().enumerate() {
                        let esp_register = self.dag.get_phys_register(EVT::I32, self.esp);
                        let esp = self.dag.get_node_multi(ISD::CopyFromReg, &[EVT::I32, EVT::OTHER], &[after_down, esp_register]);
                        let scale = self.dag.get_constant_with(1, EVT::I8, true, false);
                        let noreg = self.dag.get_register(EVT::I32, 0);
                        let disp = self.dag.get_constant_with(4 * at as u128, EVT::I32, true, false);
                        let segment = self.dag.get_register(EVT::I16, 0);
                        stores.push(self.node(
                            self.names.mov_mr,
                            &[EVT::OTHER],
                            &[esp, scale, noreg, disp, segment, value, after_down],
                        ));
                    }
                    let chain = if stores.is_empty() { after_down } else { self.dag.get_token_factor(&stores) };
                    let target = self.dag.get_target_global_address(EVT::I32, global.0, 0);
                    let mask = self.dag.get_register_mask_clobbering(&super::dagisel_adapter::call_clobbered_flags());
                    let called = self.node(calls.call, &[EVT::OTHER, EVT::GLUE], &[target, mask, chain]);
                    let up = self.node(
                        calls.call_up,
                        &[EVT::I32, EVT::OTHER, EVT::GLUE],
                        &[bytes, zero, SDValue { node: called.node, res: 0 }, SDValue { node: called.node, res: 1 }],
                    );
                    let mut chain = SDValue { node: up.node, res: 1 };
                    if !void {
                        let eax = self.eax_reg()?;
                        let result = self.dag.get_copy_from_phys_reg(chain, eax, EVT::I32, SDValue { node: up.node, res: 2 });
                        chain = SDValue { node: result.node, res: 1 };
                        self.values.insert(instruction.result?, SDValue { node: result.node, res: 0 });
                    }
                    self.chain = chain;
                }
                Opcode::Binary(op) => {
                    use llrm_mir::opcode::BinaryOp::*;
                    if self.module.context.types.int_bits(instruction.ty) != Some(32) {
                        return None;
                    }
                    let (rr, ri) = self.names.alu[match op {
                        Add => 0,
                        Sub => 1,
                        And => 2,
                        Or => 3,
                        Xor => 4,
                        _ => return None,
                    }];
                    let left = self.value(instruction.operands[0])?;
                    let done = if let Some(imm) = self.int(instruction.operands[1]) {
                        let imm = self.dag.get_constant_with(imm as u128, EVT::I32, true, false);
                        self.node(ri, &[EVT::I32, EVT::I32], &[left, imm])
                    } else {
                        let right = self.value(instruction.operands[1])?;
                        self.node(rr, &[EVT::I32, EVT::I32], &[left, right])
                    };
                    self.values.insert(instruction.result?, done);
                }
                Opcode::ICmp(predicate) => {
                    // only as the condition of the branch that follows it
                    let next = self.function.instruction(*insts.get(at)?).clone();
                    let (Opcode::Br, [Operand::Value(cond), Operand::Block(on_true), Operand::Block(on_false)]) =
                        (&next.opcode, next.operands.as_slice())
                    else {
                        return None;
                    };
                    if Some(*cond) != instruction.result || at + 1 != insts.len() {
                        return None;
                    }
                    let left = self.value(instruction.operands[0])?;
                    let compared = if let Some(imm) = self.int(instruction.operands[1]) {
                        let imm = self.dag.get_constant_with(imm as u128, EVT::I32, true, false);
                        self.node(self.names.cmp_ri, &[EVT::I32], &[left, imm])
                    } else {
                        let right = self.value(instruction.operands[1])?;
                        self.node(self.names.cmp_rr, &[EVT::I32], &[left, right])
                    };
                    use llrm_mir::opcode::IntPredicate::*;
                    let mut code: u8 = match predicate {
                        Eq => 4,
                        Ne => 5,
                        Ult => 2,
                        Uge => 3,
                        Ule => 6,
                        Ugt => 7,
                        Slt => 12,
                        Sge => 13,
                        Sle => 14,
                        Sgt => 15,
                    };
                    let (mut on_true, mut on_false) = (*on_true, *on_false);
                    if Some(on_true) == self.next {
                        std::mem::swap(&mut on_true, &mut on_false);
                        code ^= 1;
                    }
                    let flags = self.dag.get_copy_to_phys_reg(self.chain, super::dagisel_adapter::EFLAGS_ID, compared, None);
                    let glue = SDValue { node: flags.node, res: 1 };
                    let dest = self.dag.get_basic_block(on_true.0);
                    let cc = self.dag.get_constant_with(u128::from(code), EVT::I8, true, false);
                    let jcc = self.node(self.names.jcc, &[EVT::OTHER], &[dest, cc, flags, glue]);
                    let fall = self.dag.get_basic_block(on_false.0);
                    let jmp = self.node(self.names.jmp, &[EVT::OTHER], &[fall, jcc]);
                    self.chain = jmp;
                    at += 1;
                }
                Opcode::Br if instruction.operands.len() == 1 => {
                    let Operand::Block(target) = instruction.operands[0] else { return None };
                    if Some(target) != self.next {
                        let dest = self.dag.get_basic_block(target.0);
                        self.chain = self.node(self.names.jmp, &[EVT::OTHER], &[dest, self.chain]);
                    }
                }
                Opcode::Ret => {
                    let zero = self.dag.get_constant_with(0, EVT::I32, true, false);
                    let ret = match (instruction.operands.first().copied(), self.eax) {
                        (None, _) => self.node(self.names.ret, &[EVT::OTHER], &[zero, self.chain]),
                        (Some(operand), Some(eax)) => {
                            let value = self.value(operand)?;
                            let copy = self.dag.get_copy_to_phys_reg(self.chain, eax, value, None);
                            let register = self.dag.get_phys_register(EVT::I32, eax);
                            let glue = SDValue { node: copy.node, res: 1 };
                            self.node(self.names.ret, &[EVT::OTHER], &[zero, register, copy, glue])
                        }
                        _ => return None,
                    };
                    self.chain = ret;
                }
                _ => return None,
            }
        }
        self.dag.set_root(self.chain);
        Some(self.dag)
    }
}
