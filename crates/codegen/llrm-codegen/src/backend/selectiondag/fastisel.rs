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
    convention: &Convention,
    arch: &dyn llrm_target::Target,
    info: &mut FunctionLoweringInfo,
) -> Option<Vec<(BlockId, BlockDag)>> {
    let m32 = arch.object().bitness == 32;
    if !(m32 || arch.object().bitness == 64) || module.signature(function.ty).2 {
        return None;
    }
    // every parameter is a 4-byte stack cell: fixed object -(n + 1)
    let int32 = |ty| module.context.types.int_bits(ty) == Some(32);
    let (returns, parameters, _) = module.signature(function.ty);
    if parameters.iter().any(|&ty| !int32(ty))
        || convention.parameters.len() != parameters.len()
        || convention.parameters.iter().any(|one| !matches!(one, crate::backend::isel::Parameter::Cell(_)))
    {
        return None;
    }
    let returns_int = int32(returns);
    if !returns_int && !module.context.types.is_void(returns) {
        return None;
    }
    let eax = (returns_int && convention.returns.len() == 1).then(|| arch.registers().named(convention.returns[0], 4));
    let names = Names::new(opcode)?;
    let mut blocks = Vec::new();
    let layout = function.layout();
    for (at, &block) in layout.iter().enumerate() {
        let next = layout.get(at + 1).copied();
        let dag = fresh_dag(module, function, target, info, block, None);
        let chain = dag.entry_node();
        let dag = Fast { module, function, info: &*info, names: &names, eax, m32, dag, chain, values: Default::default(), next }
            .block(block)?;
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
    jmp: u16,
    jcc: u16,
    cmp_rr: u16,
    cmp_ri: u16,
    /// `add`, `sub`, `and`, `or`, `xor` with a register and with an immediate.
    alu: [(u16, u16); 5],
}

impl Names {
    fn new(opcode: &dyn Fn(&str) -> Option<u16>) -> Option<Names> {
        let pair = |one: &str| Some((opcode(&format!("{one}32rr"))?, opcode(&format!("{one}32ri"))?));
        Some(Names {
            mov_rm: opcode("MOV32rm")?,
            mov_mr: opcode("MOV32mr")?,
            mov_mi: opcode("MOV32mi")?,
            mov_ri: opcode("MOV32ri")?,
            ret: opcode("RET")?,
            jmp: opcode("JMP_1")?,
            jcc: opcode("JCC_1")?,
            cmp_rr: opcode("CMP32rr")?,
            cmp_ri: opcode("CMP32ri")?,
            alu: [pair("ADD")?, pair("SUB")?, pair("AND")?, pair("OR")?, pair("XOR")?],
        })
    }
}

struct Fast<'a> {
    module: &'a Module,
    function: &'a Function,
    info: &'a FunctionLoweringInfo,
    names: &'a Names,
    eax: Option<llrm_lir::registers::RegId>,
    /// Frame objects are addressed at m32 only so far (F3).
    m32: bool,
    dag: SelectionDAG,
    /// The memory chain: every load and store follows the one before.
    chain: SDValue,
    values: std::collections::HashMap<ValueId, SDValue>,
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

    /// `[base, scale, index, disp, segment]` of a frame object.
    fn address(&mut self, pointer: Operand) -> Option<Vec<SDValue>> {
        let Operand::Value(value) = pointer else { return None };
        if !self.m32 {
            return None;
        }
        let object = *self.info.static_allocas.get(&value)?;
        let ValueDef::Instruction(def) = self.function.value(value).def else { return None };
        let Opcode::Alloca { allocated, .. } = self.function.instruction(def).opcode else { return None };
        if self.module.context.types.int_bits(allocated) != Some(32) {
            return None;
        }
        Some(self.frame(object))
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

    fn block(mut self, block: BlockId) -> Option<SelectionDAG> {
        self.chain = self.dag.entry_node();
        let insts: Vec<_> = self.function.iter_block(block).collect();
        let mut at = 0;
        while at < insts.len() {
            let instruction = self.function.instruction(insts[at]).clone();
            at += 1;
            match &instruction.opcode {
                Opcode::Alloca { .. } => {
                    // a static alloca is its frame object
                    self.info.static_allocas.get(&instruction.result?)?;
                }
                Opcode::Load { volatile: false, .. } => {
                    if self.module.context.types.int_bits(instruction.ty) != Some(32) {
                        return None;
                    }
                    let mut ops = self.address(instruction.operands[0])?;
                    ops.push(self.chain);
                    let load = self.node(self.names.mov_rm, &[EVT::I32, EVT::OTHER], &ops);
                    self.chain = SDValue { node: load.node, res: 1 };
                    self.values.insert(instruction.result?, load);
                }
                Opcode::Store { volatile: false, .. } => {
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
