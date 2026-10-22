//! `SelectionDAGBuilder`: one block of MIR as a DAG, for the scalar integer
//! subset (arithmetic, casts, `icmp`, `select`, `br`, `load`, `store`,
//! `getelementptr`, and the register copies a `phi` needs).

use std::collections::HashMap;

use llrm_mir::context::{ConstantId, ConstantKind};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, Function, InstId, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate, Opcode};
use llrm_mir::types::{Type, TypeId};

<<<<<<< HEAD
=======
<<<<<<< HEAD
>>>>>>> sdag-a2b
use crate::backend::isel::{Convention, Parameter, convention_for};
=======
use crate::backend::isel::{Convention, Parameter};
>>>>>>> sdag-a2
use llrm_lir::registers::RegId;

use super::dag::{DagConfig, MemBase, MemInfo, SDNodeFlags, SDValue, SelectionDAG};
use super::fold::mask;
use super::isd::{CondCode, ISD};
use super::function_lowering::FunctionLoweringInfo;
use super::mvt::EVT;

/// An instruction the builder does not translate yet; the shadow harness
/// counts these per reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused(pub &'static str);

/// The target facts the builder asks (`TLI->` in the pin).
#[derive(Clone, Debug)]
pub struct BuilderTarget {
    pub dag: DagConfig,
    /// `getShiftAmountTy`: x86 shifts by `i8`.
    pub shift_amount: EVT,
    /// The integer widths with a register class, ascending.
    pub legal_ints: Vec<u32>,
}

impl BuilderTarget {
    /// i486 and P5: 32-bit pointers, `i8`/`i16`/`i32` in registers.
    pub fn m32() -> BuilderTarget {
        BuilderTarget { dag: DagConfig::default(), shift_amount: EVT::I8, legal_ints: vec![8, 16, 32] }
    }

    /// Real mode: 16-bit near pointers, `i8`/`i16` in registers.
    pub fn m16() -> BuilderTarget {
        BuilderTarget { dag: DagConfig { pointer: EVT::I16, ..DagConfig::default() }, shift_amount: EVT::I8, legal_ints: vec![8, 16] }
    }
}

/// A selected block: its DAG and the phi inputs it exports.
pub struct BlockDag {
    pub dag: SelectionDAG,
    /// `PHINodesToUpdate`: the phi and the register its incoming value is in.
    pub phi_updates: Vec<(ValueId, u32)>,
    /// The argument registers copied to virtual registers, in the entry block.
    pub live_ins: Vec<(RegId, u32)>,
}

/// `SelectionDAGISel::SelectBasicBlock` for `block`; the entry block also
/// lowers the arguments (`LowerArguments`), so it must be built first.
pub fn build_block(
    module: &Module,
    function: &Function,
    layout: &DataLayout,
    target: &BuilderTarget,
<<<<<<< HEAD
=======
<<<<<<< HEAD
>>>>>>> sdag-a2b
    arch: &dyn llrm_target::Target,
=======
>>>>>>> sdag-a2
    convention: &Convention,
    info: &mut FunctionLoweringInfo,
    block: BlockId,
) -> Result<BlockDag, Refused> {
    let mut builder = Builder {
        module,
        function,
        layout,
        target,
<<<<<<< HEAD
=======
<<<<<<< HEAD
>>>>>>> sdag-a2b
        arch,
=======
>>>>>>> sdag-a2
        convention,
        info,
        live_ins: Vec::new(),
        dag: SelectionDAG::new(target.dag),
        node_map: IdMap::new(),
        pending_loads: Vec::new(),
        pending_exports: Vec::new(),
        constants_out: HashMap::new(),
        phi_updates: Vec::new(),
        block,
    };
    builder.dag.global_names = module.globals.iter().map(|global| global.name.as_deref().unwrap_or("").to_string()).collect();
    builder.dag.block_names = (0..function.block_capacity()).map(|at| function.block(BlockId(at as u32)).name.clone().unwrap_or_else(|| format!("bb{at}"))).collect();
    if Some(block) == function.entry() {
        builder.lower_arguments()?;
    }
    // selection starts at the first non-phi (`getFirstNonPHIIt`)
    for inst in function.iter_block(block).filter(|inst| function.instruction(*inst).opcode != Opcode::Phi).collect::<Vec<_>>() {
        builder.visit(inst)?;
    }
    let root = builder.control_root();
    builder.dag.set_root(root);
    Ok(BlockDag { dag: builder.dag, phi_updates: builder.phi_updates, live_ins: builder.live_ins })
}

struct Builder<'a> {
    module: &'a Module,
    function: &'a Function,
    layout: &'a DataLayout,
    target: &'a BuilderTarget,
<<<<<<< HEAD
=======
<<<<<<< HEAD
>>>>>>> sdag-a2b
    arch: &'a dyn llrm_target::Target,
=======
>>>>>>> sdag-a2
    convention: &'a Convention,
    live_ins: Vec<(RegId, u32)>,
    info: &'a mut FunctionLoweringInfo,
    dag: SelectionDAG,
    node_map: IdMap<ValueId, SDValue>,
    pending_loads: Vec<SDValue>,
    pending_exports: Vec<SDValue>,
    constants_out: HashMap<ConstantId, u32>,
    phi_updates: Vec<(ValueId, u32)>,
    block: BlockId,
}

fn flags_of(flags: llrm_mir::opcode::Flags) -> SDNodeFlags {
    use llrm_mir::opcode::Flags;
    SDNodeFlags::default()
        .with(SDNodeFlags::NO_UNSIGNED_WRAP, flags.contains(Flags::NUW))
        .with(SDNodeFlags::NO_SIGNED_WRAP, flags.contains(Flags::NSW))
        .with(SDNodeFlags::EXACT, flags.contains(Flags::EXACT))
        .with(SDNodeFlags::DISJOINT, flags.contains(Flags::DISJOINT))
        .with(SDNodeFlags::NON_NEG, flags.contains(Flags::NNEG))
        .with(SDNodeFlags::SAME_SIGN, flags.contains(Flags::SAMESIGN))
}

fn cond_code(predicate: IntPredicate) -> CondCode {
    match predicate {
        IntPredicate::Eq => CondCode::SetEq,
        IntPredicate::Ne => CondCode::SetNe,
        IntPredicate::Ugt => CondCode::SetUGt,
        IntPredicate::Uge => CondCode::SetUGe,
        IntPredicate::Ult => CondCode::SetULt,
        IntPredicate::Ule => CondCode::SetULe,
        IntPredicate::Sgt => CondCode::SetGt,
        IntPredicate::Sge => CondCode::SetGe,
        IntPredicate::Slt => CondCode::SetLt,
        IntPredicate::Sle => CondCode::SetLe,
    }
}

impl Builder<'_> {
    fn type_of(&self, ty: TypeId) -> Result<EVT, Refused> {
        match self.module.context.types.get(ty) {
            Type::Int(bits) if (1..=128).contains(bits) => Ok(EVT::integer(*bits)),
            Type::Pointer(0) => Ok(self.target.dag.pointer),
            Type::Pointer(_) => Err(Refused("pointer in another address space")),
            Type::Float(_) => Err(Refused("floating point")),
            _ => Err(Refused("aggregate or vector value")),
        }
    }

    /// The register type a value of `vt` is carried in across blocks:
    /// itself when legal, else the next legal width up.
    fn register_type(&self, vt: EVT) -> Result<EVT, Refused> {
        let bits = vt.size_in_bits();
        self.target.legal_ints.iter().find(|legal| **legal >= bits).map(|legal| EVT::integer(*legal)).ok_or(Refused("integer wider than a register"))
    }

    fn operand_type(&self, operand: Operand) -> Result<EVT, Refused> {
        let ty = self.function.operand_type(&self.module.context, operand).ok_or(Refused("block operand"))?;
        self.type_of(ty)
    }

    // ---- roots (SelectionDAGBuilder::updateRoot and friends)

    fn update_root(&mut self, pending: Vec<SDValue>) -> SDValue {
        let mut root = self.dag.get_root();
        if pending.is_empty() {
            return root;
        }
        let mut pending = pending;
        if self.dag.opcode(root) != ISD::EntryToken && !pending.iter().any(|chain| self.dag.operand(*chain, 0) == root) {
            pending.push(root);
        }
        root = if pending.len() == 1 { pending[0] } else { self.dag.get_token_factor(&pending) };
        self.dag.set_root(root);
        root
    }

    fn memory_root(&mut self) -> SDValue {
        let pending = std::mem::take(&mut self.pending_loads);
        self.update_root(pending)
    }

    /// `getRoot`.
    fn root(&mut self) -> SDValue {
        self.memory_root()
    }

    fn control_root(&mut self) -> SDValue {
        let pending = std::mem::take(&mut self.pending_exports);
        self.update_root(pending)
    }

    // ---- values

    fn get_value(&mut self, operand: Operand) -> Result<SDValue, Refused> {
        match operand {
            Operand::Constant(constant) => self.constant_value(constant),
            Operand::Block(_) => Err(Refused("block as a value")),
            Operand::Value(value) => {
                if let Some(found) = self.node_map.get(&value) {
                    return Ok(*found);
                }
                if let Some(copy) = self.copy_from_regs(value)? {
                    return Ok(copy);
                }
                let made = self.value_impl(value)?;
                self.node_map.insert(value, made);
                Ok(made)
            }
        }
    }

    /// `LowerArguments` with X86's `LowerFormalArguments` over the target's
    /// `Convention`: a stack parameter is a load from a fixed object (indices
    /// -1, -2, ... in parameter order), a register parameter the copy of a
    /// live-in virtual register. A parameter read outside the entry block is
    /// also copied to a register of its own.
    fn lower_arguments(&mut self) -> Result<(), Refused> {
        let entry = self.function.entry();
        let mut outside = std::collections::HashSet::new();
        for &block in self.function.layout() {
            for inst in self.function.iter_block(block) {
                let instruction = self.function.instruction(inst);
                for operand in &instruction.operands {
                    if let Operand::Value(value) = operand {
                        if Some(block) != entry || instruction.opcode == Opcode::Switch {
                            outside.insert(*value);
                        }
                    }
                }
            }
        }
        let mut fixed = 0;
        for (at, &arg) in self.function.parameters().to_vec().iter().enumerate() {
            let vt = self.type_of(self.function.value(arg).ty)?;
            if self.register_type(vt)? != vt {
                return Err(Refused("argument of an illegal integer type"));
            }
            let entry_node = self.dag.entry_node();
            let value = match self.convention.parameters.get(at) {
                Some(Parameter::Cell(_)) => {
                    fixed += 1;
                    let ptr = self.dag.get_frame_index(self.target.dag.pointer, -fixed);
                    let align = self.layout.align(&self.module.context.types, self.function.value(arg).ty) as u32;
                    let info = MemInfo { mem_vt: vt, align, volatile: false, base: MemBase::Stack(-fixed) };
                    self.dag.get_load(vt, entry_node, ptr, info)
                }
                Some(Parameter::Registers(regs)) if regs.len() == 1 => {
                    let vreg = self.info.new_reg();
                    self.live_ins.push((regs[0], vreg));
                    self.dag.get_copy_from_reg(entry_node, vreg, vt)
                }
                _ => return Err(Refused("argument passed in several places or by value")),
            };
            self.node_map.insert(arg, value);
            if outside.contains(&arg) {
                let reg = self.info.create_reg(arg);
                self.copy_value_to_register(Operand::Value(arg), reg, ISD::AnyExtend)?;
            }
        }
        Ok(())
    }

    /// `visitCall` through `LowerCallTo` and X86's `LowerCall`, for a direct
    /// call of a function whose arguments and result fit single registers or
    /// the stack: `callseq_start`, the stack stores, the register copies,
    /// `X86ISD::CALL`, `callseq_end`, then the result read back.
    fn visit_call(&mut self, instruction: &llrm_mir::module::Instruction, info: &llrm_mir::opcode::CallInfo) -> Result<(), Refused> {
        let (callee, arguments) = instruction.operands.split_last().expect("a call names its callee");
        let Operand::Constant(callee_id) = *callee else { return Err(Refused("indirect call")) };
        let ConstantKind::Global(global) = self.module.context.get(callee_id).kind else { return Err(Refused("indirect call")) };
        if self.module.global(global).intrinsic().is_some() {
            return Err(Refused("intrinsic call"));
        }
        let types = &self.module.context.types;
        let mut arg_types = Vec::new();
        for argument in arguments {
            arg_types.push(self.function.operand_type(&self.module.context, *argument).ok_or(Refused("block argument"))?);
        }
        let first = self.arch.first_argument_offset(false);
        let plan = convention_for(self.module, self.layout, self.arch, first, info.calling_convention, info.function_type, &arg_types, &info.argument_attrs)
            .map_err(|_| Refused("call the convention does not describe"))?;
        let (result_type, _, _) = self.module.signature(info.function_type);
        // Evaluate the callee and the arguments before the call sequence starts.
        let _ = self.get_value(*callee)?;
        let callee_node = self.dag.get_target_global_address(self.target.dag.pointer, global.0, 0);
        let mut values = Vec::new();
        for argument in arguments {
            let value = self.get_value(*argument)?;
            if self.register_type(self.dag.value_type(value))? != self.dag.value_type(value) {
                return Err(Refused("argument of an illegal integer type"));
            }
            values.push(value);
        }
        let bytes = (plan.stack_arguments - first).max(0) as u64;
        let root = self.root();
        let start = self.dag.get_callseq_start(root, bytes);
        let mut chain = SDValue { node: start.node, res: 0 };
        let mut stores = Vec::new();
        let mut copies: Vec<(RegId, SDValue)> = Vec::new();
        let mut stack_pointer = None;
        for (value, parameter) in values.iter().zip(&plan.parameters) {
            match parameter {
                Parameter::Cell(displacement) => {
                    let sp = *stack_pointer.get_or_insert_with(|| {
                        let sp = self.arch.register_named(if self.target.dag.pointer == EVT::I16 { "sp" } else { "esp" });
                        self.dag.get_copy_from_phys_reg_plain(chain, sp, self.target.dag.pointer)
                    });
                    let offset = self.dag.get_constant((displacement - first) as u128, self.target.dag.pointer);
                    let address = self.dag.get_node(ISD::Add, self.target.dag.pointer, &[sp, offset]);
                    let vt = self.dag.value_type(*value);
                    let mem = MemInfo { mem_vt: vt, align: 1, volatile: false, base: MemBase::Unknown };
                    stores.push(self.dag.get_store(chain, *value, address, mem));
                }
                Parameter::Registers(regs) if regs.len() == 1 => copies.push((regs[0], *value)),
                _ => return Err(Refused("argument passed in several places or by value")),
            }
        }
        if !stores.is_empty() {
            chain = self.dag.get_token_factor(&stores);
        }
        let mut glue: Option<SDValue> = None;
        for (reg, value) in &copies {
            let copy = self.dag.get_copy_to_phys_reg(chain, *reg, *value, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
        }
        let mut ops = vec![chain, callee_node];
        for (reg, value) in &copies {
            ops.push(self.dag.get_phys_register(self.dag.value_type(*value), *reg));
        }
        ops.push(self.dag.get_register_mask());
        ops.extend(glue);
        let call = self.dag.get_node_multi(super::x86isd::CALL, &[EVT::OTHER, EVT::GLUE], &ops);
        let end = self.dag.get_callseq_end(SDValue { node: call.node, res: 0 }, bytes, plan.popped as u64, SDValue { node: call.node, res: 1 });
        let mut chain = SDValue { node: end.node, res: 0 };
        let glue = SDValue { node: end.node, res: 1 };
        if types.is_void(result_type) {
            self.dag.set_root(chain);
            return Ok(());
        }
        let vt = self.type_of(result_type)?;
        let [reg] = plan.returns[..] else { return Err(Refused("result in several registers")) };
        if self.register_type(vt)? != vt {
            return Err(Refused("result of an illegal integer type"));
        }
        let reg = self.sized_register(reg, vt);
        let result = self.dag.get_copy_from_phys_reg(chain, reg, vt, glue);
        chain = SDValue { node: result.node, res: 1 };
        self.dag.set_root(chain);
        if let Some(value) = instruction.result {
            self.set_value(value, result);
        }
        Ok(())
    }

    /// The convention names a result register by its widest view; the DAG
    /// names the one the value's width is (`AX` for an `i16`, not `EAX`).
    fn sized_register(&self, reg: RegId, vt: EVT) -> RegId {
        self.arch.registers().named(reg, i64::from(vt.size_in_bits().div_ceil(8)))
    }

    /// `visitRet` and X86's `LowerReturn`: the value copied to the return
    /// register, then `RET_GLUE chain, popped, register, glue`.
    fn visit_ret(&mut self, instruction: &llrm_mir::module::Instruction) -> Result<(), Refused> {
        let mut chain = self.control_root();
        let mut ops = Vec::new();
        let mut glue = None;
        if let Some(&operand) = instruction.operands.first() {
            let value = self.get_value(operand)?;
            let vt = self.dag.value_type(value);
            let [reg] = self.convention.returns[..] else { return Err(Refused("result in several registers")) };
            if self.register_type(vt)? != vt {
                return Err(Refused("result of an illegal integer type"));
            }
            let reg = self.sized_register(reg, vt);
            let copy = self.dag.get_copy_to_phys_reg(chain, reg, value, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
            ops.push(self.dag.get_phys_register(vt, reg));
        }
        let popped = self.dag.get_constant_with(self.convention.popped as u128, EVT::I32, true, false);
        let mut all = vec![chain, popped];
        all.extend(ops);
        all.extend(glue);
        let node = self.dag.get_node(super::x86isd::RET_GLUE, EVT::OTHER, &all);
        self.dag.set_root(node);
        Ok(())
    }

    /// `getNonRegisterValue`.
    fn non_register_value(&mut self, operand: Operand) -> Result<SDValue, Refused> {
        match operand {
            Operand::Value(value) => {
                if let Some(found) = self.node_map.get(&value) {
                    return Ok(*found);
                }
                let made = self.value_impl(value)?;
                self.node_map.insert(value, made);
                Ok(made)
            }
            other => self.get_value(other),
        }
    }

    fn constant_value(&mut self, id: ConstantId) -> Result<SDValue, Refused> {
        let constant = self.module.context.get(id);
        let vt = self.type_of(constant.ty)?;
        match &constant.kind {
            ConstantKind::Int(bits) => Ok(self.dag.get_constant(*bits, vt)),
            ConstantKind::Null | ConstantKind::Zero => Ok(self.dag.get_constant(0, vt)),
            ConstantKind::Undef => Ok(self.dag.get_undef(vt)),
            ConstantKind::Poison => Ok(self.dag.get_poison(vt)),
            ConstantKind::Global(global) => Ok(self.dag.get_global_address(vt, global.0, 0)),
            _ => Err(Refused("constant expression or aggregate")),
        }
    }

    /// `getValueImpl` for an instruction or argument.
    fn value_impl(&mut self, value: ValueId) -> Result<SDValue, Refused> {
        if let Some(index) = self.info.static_allocas.get(&value) {
            let vt = self.type_of(self.function.value(value).ty)?;
            return Ok(self.dag.get_frame_index(vt, *index));
        }
        match self.function.value(value).def {
            ValueDef::Argument(_) => Err(Refused("argument read before the entry block was built")),
            // an exported value read before its definition's block: its register
            ValueDef::Instruction(_) => {
                let reg = *self.info.value_regs.get(&value).ok_or(Refused("value with no register"))?;
                let vt = self.type_of(self.function.value(value).ty)?;
                Ok(self.copy_from_reg(reg, vt)?)
            }
        }
    }

    fn copy_from_regs(&mut self, value: ValueId) -> Result<Option<SDValue>, Refused> {
        let Some(reg) = self.info.value_regs.get(&value).copied() else { return Ok(None) };
        let vt = self.type_of(self.function.value(value).ty)?;
        Ok(Some(self.copy_from_reg(reg, vt)?))
    }

    /// `RegsForValue::getCopyFromRegs`: the register's type, truncated to the
    /// value's when the register is wider.
    fn copy_from_reg(&mut self, reg: u32, vt: EVT) -> Result<SDValue, Refused> {
        let reg_vt = self.register_type(vt)?;
        let entry = self.dag.entry_node();
        let copy = self.dag.get_copy_from_reg(entry, reg, reg_vt);
        Ok(if reg_vt == vt { copy } else { self.dag.get_node(ISD::Truncate, vt, &[copy]) })
    }

    /// `CopyValueToVirtualRegister`.
    fn copy_value_to_register(&mut self, operand: Operand, reg: u32, extend: ISD) -> Result<(), Refused> {
        let value = self.non_register_value(operand)?;
        let vt = self.dag.value_type(value);
        let reg_vt = self.register_type(vt)?;
        let value = if reg_vt == vt { value } else { self.dag.get_node(extend, reg_vt, &[value]) };
        let entry = self.dag.entry_node();
        let chain = self.dag.get_copy_to_reg(entry, reg, value);
        self.pending_exports.push(chain);
        Ok(())
    }

    fn set_value(&mut self, value: ValueId, node: SDValue) {
        self.node_map.insert(value, node);
    }

    // ---- instructions

    fn visit(&mut self, inst: InstId) -> Result<(), Refused> {
        let instruction = self.function.instruction(inst).clone();
        if instruction.opcode.is_terminator() {
            self.handle_phi_nodes_in_successor_blocks()?;
        }
        match &instruction.opcode {
            Opcode::Binary(op) => self.visit_binary(&instruction, *op)?,
            Opcode::Cast(op) => self.visit_cast(&instruction, *op)?,
            Opcode::ICmp(predicate) => self.visit_icmp(&instruction, *predicate)?,
            Opcode::Select => self.visit_select(&instruction)?,
            Opcode::Load { align, volatile } => self.visit_load(&instruction, *align, *volatile)?,
            Opcode::Store { align, volatile } => self.visit_store(&instruction, *align, *volatile)?,
            Opcode::GetElementPtr { source } => self.visit_gep(&instruction, *source)?,
            Opcode::Br => self.visit_br(&instruction)?,
            Opcode::Alloca { .. } if instruction.result.is_some_and(|result| self.info.static_allocas.contains_key(&result)) => {}
            Opcode::Unreachable => {}
            Opcode::Ret => self.visit_ret(&instruction)?,
            Opcode::Switch => return Err(Refused("switch (A2)")),
            Opcode::Call(info) => self.visit_call(&instruction, info)?,
            Opcode::Invoke(_) => return Err(Refused("invoke")),
            _ => return Err(Refused("instruction not in the scalar subset")),
        }
        if !instruction.opcode.is_terminator() {
            if let Some(result) = instruction.result {
                if let Some(reg) = self.info.value_regs.get(&result).copied() {
                    self.copy_value_to_register(Operand::Value(result), reg, ISD::AnyExtend)?;
                }
            }
        }
        Ok(())
    }

    fn visit_binary(&mut self, instruction: &llrm_mir::module::Instruction, op: BinaryOp) -> Result<(), Refused> {
        let opcode = match op {
            BinaryOp::Add => ISD::Add,
            BinaryOp::Sub => ISD::Sub,
            BinaryOp::Mul => ISD::Mul,
            BinaryOp::UDiv => ISD::UDiv,
            BinaryOp::SDiv => ISD::SDiv,
            BinaryOp::URem => ISD::URem,
            BinaryOp::SRem => ISD::SRem,
            BinaryOp::Shl => ISD::Shl,
            BinaryOp::LShr => ISD::Srl,
            BinaryOp::AShr => ISD::Sra,
            BinaryOp::And => ISD::And,
            BinaryOp::Or => ISD::Or,
            BinaryOp::Xor => ISD::Xor,
            _ => return Err(Refused("floating point")),
        };
        let lhs = self.get_value(instruction.operands[0])?;
        let mut rhs = self.get_value(instruction.operands[1])?;
        let mut flags = flags_of(instruction.flags);
        if matches!(opcode, ISD::Shl | ISD::Srl | ISD::Sra) {
            // visitShift: the amount in the target's shift type, and only these flags
            rhs = self.dag.get_zext_or_trunc(rhs, self.target.shift_amount);
            flags = SDNodeFlags(flags.0 & (SDNodeFlags::NO_UNSIGNED_WRAP.0 | SDNodeFlags::NO_SIGNED_WRAP.0 | SDNodeFlags::EXACT.0));
        } else if matches!(opcode, ISD::SDiv) {
            flags = SDNodeFlags(flags.0 & SDNodeFlags::EXACT.0);
        }
        let vt = self.dag.value_type(lhs);
        let node = self.dag.get_node_flags(opcode, vt, &[lhs, rhs], flags);
        self.set_value(instruction.result.expect("a binary operator has a result"), node);
        Ok(())
    }

    fn visit_cast(&mut self, instruction: &llrm_mir::module::Instruction, op: CastOp) -> Result<(), Refused> {
        let operand = self.get_value(instruction.operands[0])?;
        let dest = self.type_of(instruction.ty)?;
        let flags = flags_of(instruction.flags);
        let result = match op {
            CastOp::Trunc => self.dag.get_node_flags(ISD::Truncate, dest, &[operand], flags),
            CastOp::ZExt => self.dag.get_node_flags(ISD::ZeroExtend, dest, &[operand], flags),
            CastOp::SExt => self.dag.get_node(ISD::SignExtend, dest, &[operand]),
            CastOp::PtrToInt | CastOp::IntToPtr => self.dag.get_zext_or_trunc(operand, dest),
            CastOp::BitCast => {
                if dest != self.dag.value_type(operand) {
                    self.dag.get_node(ISD::BitCast, dest, &[operand])
                } else if let Some(bits) = self.dag.constant(operand) {
                    self.dag.get_constant_with(bits, dest, false, true)
                } else {
                    operand
                }
            }
            _ => return Err(Refused("cast not in the scalar subset")),
        };
        self.set_value(instruction.result.expect("a cast has a result"), result);
        Ok(())
    }

    fn visit_icmp(&mut self, instruction: &llrm_mir::module::Instruction, predicate: IntPredicate) -> Result<(), Refused> {
        let lhs = self.get_value(instruction.operands[0])?;
        let rhs = self.get_value(instruction.operands[1])?;
        let dest = self.type_of(instruction.ty)?;
        let cc = self.dag.get_cond_code(cond_code(predicate));
        let flags = flags_of(instruction.flags);
        let node = self.dag.get_node_flags(ISD::SetCC, dest, &[lhs, rhs, cc], SDNodeFlags(flags.0 & SDNodeFlags::SAME_SIGN.0));
        self.set_value(instruction.result.expect("a compare has a result"), node);
        Ok(())
    }

    fn visit_select(&mut self, instruction: &llrm_mir::module::Instruction) -> Result<(), Refused> {
        let cond = self.get_value(instruction.operands[0])?;
        let then = self.get_value(instruction.operands[1])?;
        let other = self.get_value(instruction.operands[2])?;
        let vt = self.dag.value_type(then);
        let node = self.dag.get_node(ISD::Select, vt, &[cond, then, other]);
        self.set_value(instruction.result.expect("a select has a result"), node);
        Ok(())
    }

    fn mem_info(&self, ty: TypeId, align: Option<u64>, volatile: bool, pointer: Operand) -> Result<MemInfo, Refused> {
        let mem_vt = self.type_of(ty)?;
        let align = align.unwrap_or_else(|| self.layout.align(&self.module.context.types, ty)) as u32;
        let base = match pointer {
            Operand::Constant(id) => match self.module.context.get(id).kind {
                ConstantKind::Global(global) => MemBase::Global(global.0),
                _ => MemBase::Unknown,
            },
            Operand::Value(value) => self.info.static_allocas.get(&value).map_or(MemBase::Unknown, |index| MemBase::Stack(*index)),
            Operand::Block(_) => MemBase::Unknown,
        };
        Ok(MemInfo { mem_vt, align, volatile, base })
    }

    fn visit_load(&mut self, instruction: &llrm_mir::module::Instruction, align: Option<u64>, volatile: bool) -> Result<(), Refused> {
        let ptr = self.get_value(instruction.operands[0])?;
        let info = self.mem_info(instruction.ty, align, volatile, instruction.operands[0])?;
        // `DAG.getRoot()`, not the builder's: a non-volatile load leaves the pending loads alone
        let root = if volatile { self.root() } else { self.dag.get_root() };
        let address = self.dag.get_object_ptr_offset(ptr, 0);
        let load = self.dag.get_load(info.mem_vt, root, address, info);
        let chain = SDValue { node: load.node, res: 1 };
        let token = self.dag.get_token_factor(&[chain]);
        if volatile {
            self.dag.set_root(token);
        } else {
            self.pending_loads.push(token);
        }
        self.set_value(instruction.result.expect("a load has a result"), load);
        Ok(())
    }

    fn visit_store(&mut self, instruction: &llrm_mir::module::Instruction, align: Option<u64>, volatile: bool) -> Result<(), Refused> {
        let value_type = self.function.operand_type(&self.module.context, instruction.operands[0]).ok_or(Refused("block operand"))?;
        let src = self.get_value(instruction.operands[0])?;
        let ptr = self.get_value(instruction.operands[1])?;
        let info = self.mem_info(value_type, align, volatile, instruction.operands[1])?;
        let root = if volatile { self.root() } else { self.memory_root() };
        let address = self.dag.get_object_ptr_offset(ptr, 0);
        let store = self.dag.get_store(root, src, address, info);
        let node = self.dag.get_token_factor(&[store]);
        self.dag.set_root(node);
        Ok(())
    }

    /// `visitGetElementPtr`.
    fn visit_gep(&mut self, instruction: &llrm_mir::module::Instruction, source: TypeId) -> Result<(), Refused> {
        use llrm_mir::opcode::Flags;
        let types = &self.module.context.types;
        let mut n = self.get_value(instruction.operands[0])?;
        let ptr_vt = self.dag.value_type(n);
        let index_bits = ptr_vt.size_in_bits();
        let nuw = instruction.flags.contains(Flags::NUW);
        let nusw = instruction.flags.contains(Flags::NUSW) || instruction.flags.contains(Flags::INBOUNDS);
        let inbounds = instruction.flags.contains(Flags::INBOUNDS);
        let mut current = source;
        for (at, index) in instruction.operands[1..].iter().enumerate() {
            let constant = match index {
                Operand::Constant(id) => match self.module.context.get(*id).kind {
                    ConstantKind::Int(bits) => Some(bits),
                    ConstantKind::Null | ConstantKind::Zero => Some(0),
                    _ => None,
                },
                _ => None,
            };
            let element = if at == 0 {
                self.layout.alloc_size(types, current)
            } else if let Type::Array { element, .. } | Type::Vector { element, .. } = types.get(current) {
                current = *element;
                self.layout.alloc_size(types, current)
            } else {
                let field = constant.ok_or(Refused("variable struct index"))? as usize;
                let offset = self.layout.struct_layout(types, current).1[field];
                current = types.member(current, field as u64).ok_or(Refused("bad struct field"))?;
                if field != 0 {
                    let flags = SDNodeFlags::default().with(SDNodeFlags::NO_UNSIGNED_WRAP, nuw || nusw);
                    let flags = flags.with(SDNodeFlags::IN_BOUNDS, inbounds);
                    let offset = self.dag.get_constant(u128::from(offset), ptr_vt);
                    n = self.dag.get_mem_base_plus_offset(n, offset, flags);
                }
                continue;
            };
            if constant == Some(0) {
                continue;
            }
            if let Some(bits) = constant {
                let idx_ty = self.index_operand_width(*index)?;
                let signed = ((bits << (128 - idx_ty)) as i128) >> (128 - idx_ty);
                let offs = (signed as u128).wrapping_mul(u128::from(element)) & mask(index_bits);
                let non_negative = offs >> (index_bits - 1) == 0;
                let flags = SDNodeFlags::default().with(SDNodeFlags::NO_UNSIGNED_WRAP, nuw || (non_negative && nusw)).with(SDNodeFlags::IN_BOUNDS, inbounds);
                let offs = self.dag.get_constant(offs, ptr_vt);
                n = self.dag.get_mem_base_plus_offset(n, offs, flags);
                continue;
            }
            let idx = self.get_value(*index)?;
            let mut idx = self.dag.get_sext_or_trunc(idx, ptr_vt);
            let scale_flags = SDNodeFlags::default().with(SDNodeFlags::NO_SIGNED_WRAP, nusw).with(SDNodeFlags::NO_UNSIGNED_WRAP, nuw);
            if element != 1 {
                if element.is_power_of_two() {
                    let amount = self.dag.get_constant(u128::from(element.trailing_zeros()), self.target.shift_amount);
                    idx = self.dag.get_node_flags(ISD::Shl, ptr_vt, &[idx, amount], scale_flags);
                } else {
                    let scale = self.dag.get_constant(u128::from(element), ptr_vt);
                    idx = self.dag.get_node_flags(ISD::Mul, ptr_vt, &[idx, scale], scale_flags);
                }
            }
            let add_flags = SDNodeFlags::default().with(SDNodeFlags::NO_UNSIGNED_WRAP, nuw).with(SDNodeFlags::IN_BOUNDS, inbounds);
            n = self.dag.get_mem_base_plus_offset(n, idx, add_flags);
        }
        self.set_value(instruction.result.expect("a gep has a result"), n);
        Ok(())
    }

    fn index_operand_width(&self, operand: Operand) -> Result<u32, Refused> {
        Ok(self.operand_type(operand)?.size_in_bits())
    }

    /// `visitUncondBr` and `visitCondBr` (through `visitSwitchCase`), without
    /// `FindMergedConditions` (DEVIATE SD15).
    fn visit_br(&mut self, instruction: &llrm_mir::module::Instruction) -> Result<(), Refused> {
        let next = self.next_block();
        let block_of = |operand: Operand| match operand {
            Operand::Block(block) => block,
            _ => unreachable!("a branch target is a block"),
        };
        if instruction.operands.len() == 1 {
            let target = block_of(instruction.operands[0]);
            if Some(target) != next {
                let root = self.control_root();
                let dest = self.dag.get_basic_block(target.0);
                let node = self.dag.get_node(ISD::Br, EVT::OTHER, &[root, dest]);
                self.dag.set_root(node);
            }
            return Ok(());
        }
        let mut cond = self.get_value(instruction.operands[0])?;
        let (mut on_true, mut on_false) = (block_of(instruction.operands[1]), block_of(instruction.operands[2]));
        if Some(on_true) == next {
            std::mem::swap(&mut on_true, &mut on_false);
            let one = self.dag.get_constant(1, self.dag.value_type(cond));
            cond = self.dag.get_node(ISD::Xor, self.dag.value_type(cond), &[cond, one]);
        }
        let root = self.control_root();
        let dest = self.dag.get_basic_block(on_true.0);
        let brcond = self.dag.get_node(ISD::BrCond, EVT::OTHER, &[root, cond, dest]);
        let fall = self.dag.get_basic_block(on_false.0);
        let node = self.dag.get_node(ISD::Br, EVT::OTHER, &[brcond, fall]);
        self.dag.set_root(node);
        Ok(())
    }

    fn next_block(&self) -> Option<BlockId> {
        let layout = self.function.layout();
        let at = layout.iter().position(|block| *block == self.block)?;
        layout.get(at + 1).copied()
    }

    /// `HandlePHINodesInSuccessorBlocks`: the incoming value of each phi of
    /// each successor copied to a register, once per successor.
    fn handle_phi_nodes_in_successor_blocks(&mut self) -> Result<(), Refused> {
        let successors: Vec<BlockId> = self.function.successors(self.block).iter().copied().collect();
        for successor in successors {
            let phis: Vec<InstId> = self
                .function
                .iter_block(successor)
                .take_while(|inst| self.function.instruction(*inst).opcode == Opcode::Phi)
                .collect();
            for phi in phis {
                let instruction = self.function.instruction(phi).clone();
                let result = instruction.result.expect("a phi has a result");
                if self.function.users(result).is_empty() {
                    continue;
                }
                let incoming = instruction
                    .operands
                    .chunks(2)
                    .find(|pair| pair[1] == Operand::Block(self.block))
                    .map(|pair| pair[0])
                    .ok_or(Refused("phi without an input from a predecessor"))?;
                let reg = match incoming {
                    Operand::Constant(constant) => match self.constants_out.get(&constant) {
                        Some(reg) => *reg,
                        None => {
                            let reg = self.info.new_reg();
                            self.constants_out.insert(constant, reg);
                            let extend = match self.module.context.get(constant).kind {
                                ConstantKind::Int(_) => ISD::ZeroExtend,
                                _ => ISD::AnyExtend,
                            };
                            self.copy_value_to_register(incoming, reg, extend)?;
                            reg
                        }
                    },
                    Operand::Value(value) => match self.info.value_regs.get(&value) {
                        Some(reg) => *reg,
                        None => {
                            let reg = self.info.new_reg();
                            self.copy_value_to_register(incoming, reg, ISD::AnyExtend)?;
                            reg
                        }
                    },
                    Operand::Block(_) => return Err(Refused("block as a phi input")),
                };
                self.phi_updates.push((result, reg));
            }
        }
        self.constants_out.clear();
        Ok(())
    }
}
