//! Instruction selection from MIR: each function's instructions as LIR over
//! virtual registers, as LLVM's SelectionDAG makes MachineInstrs. It chooses
//! instructions and addressing only; two-address form, registers and the
//! frame's size are the machine phases'.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::{BinaryOp, CastOp, ConstantKind, IntPredicate, Module, Opcode, Type, TypeId};

use crate::backend::lower::{_read, _written};
use crate::model::ir::{Addr, Address, Held, Imm, Loc, Mem, Operation, Semantics, Space};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi};
use crate::support::hash::IndexMap;

/// Where a function's parameters arrive and its result leaves: the call
/// ABI its frontend chose.
#[derive(Clone, Debug)]
pub struct Convention {
    pub parameters: Vec<Home>,
    /// The registers a result leaves in, low part first.
    pub returns: Vec<Register>,
}

#[derive(Clone, Copy, Debug)]
pub enum Home {
    /// A cell at this displacement from BP.
    Frame(i64),
    Register(Register),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unselected(pub String);

fn refuse<T>(what: impl Into<String>) -> Result<T, Unselected> {
    Err(Unselected(what.into()))
}

/// Where a pointer points, when it need not be a register: a frame slot
/// is an addressing mode, and a constant offset is its displacement.
#[derive(Clone, Copy, Debug)]
enum Pointer {
    Frame(i64),
    Based { base: Held, offset: i64 },
}

pub fn selected(module: &Module, name: &str, convention: &Convention) -> Result<LirBody, Unselected> {
    let Some(global) = module.named(name) else { return refuse(format!("no function @{name}")) };
    let Some(function) = module.global(global).function().filter(|one| !one.is_declaration()) else {
        return refuse(format!("@{name} has no body"));
    };
    let Some(layout) = module.datalayout.as_deref() else { return refuse("a module with no datalayout") };
    let layout = DataLayout::parse(layout).map_err(Unselected)?;
    let mut selector = Selector {
        module,
        function,
        layout,
        values: IndexMap::default(),
        next: 0,
        pointers: IndexMap::default(),
        depth: 0,
        ats: IndexMap::default(),
        fused: BTreeSet::new(),
        pending: IndexMap::default(),
        phi_inputs: IndexMap::default(),
        pins: IndexMap::default(),
        inputs: BTreeSet::new(),
    };
    selector.body(name, convention)
}

struct Selector<'m> {
    module: &'m Module,
    function: &'m Function,
    layout: DataLayout,
    values: IndexMap<ValueId, u32>,
    next: u32,
    pointers: IndexMap<ValueId, Pointer>,
    depth: i64,
    ats: IndexMap<InstId, i64>,
    /// Comparisons a branch reads as flags, made beside it.
    fused: BTreeSet<InstId>,
    /// What each block computes for its successors' phis, before its terminator.
    pending: IndexMap<BlockId, Vec<Arc<Insn>>>,
    /// The register each (phi, predecessor) reads, where that block made it.
    phi_inputs: IndexMap<(InstId, BlockId), u32>,
    pins: IndexMap<u32, Register>,
    inputs: BTreeSet<u32>,
}

impl Selector<'_> {
    fn body(&mut self, name: &str, convention: &Convention) -> Result<LirBody, Unselected> {
        let function = self.function;
        let layout = function.layout();
        let mut at = 0;
        let mut block_at = IndexMap::default();
        for &block in layout {
            block_at.insert(block, at);
            for &inst in function.block(block).instructions() {
                self.ats.insert(inst, at);
                at += 1;
                if let Opcode::Alloca { allocated, .. } = function.instruction(inst).opcode {
                    let size = self.layout.alloc_size(self.types(), allocated) as i64;
                    self.depth += size + size % 2;
                    self.pointers.insert(function.instruction(inst).result.expect("an address"), Pointer::Frame(-self.depth));
                }
            }
        }
        let entry = function.entry().expect("a body");
        let mut prologue = Vec::new();
        if function.parameters().len() != convention.parameters.len() {
            return refuse("the convention places a different number of parameters");
        }
        for (&parameter, home) in function.parameters().iter().zip(&convention.parameters) {
            let width = self.width(function.value(parameter).ty)?;
            let held = Held { value: self.value(parameter), width };
            match *home {
                Home::Frame(disp) => {
                    let what = semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Mem(frame(disp, width))]);
                    prologue.push(insn(block_at[&entry], what));
                }
                Home::Register(register) => {
                    self.inputs.insert(held.value);
                    self.pins.insert(held.value, register);
                }
            }
        }
        for &block in layout {
            for &inst in function.block(block).instructions() {
                self.fuse(block, inst);
            }
            self.phis_from(block)?;
        }
        let mut blocks = Vec::new();
        for &block in layout {
            let mut insns = if block == entry { std::mem::take(&mut prologue) } else { Vec::new() };
            let mut phis = Vec::new();
            for &inst in function.block(block).instructions() {
                let instruction = function.instruction(inst);
                if instruction.opcode == Opcode::Phi {
                    let result = instruction.result.expect("a phi's value");
                    let incoming = self.incoming(inst, &block_at)?;
                    self.width(instruction.ty)?;
                    phis.push(Phi { result: self.value(result), incoming });
                    continue;
                }
                if instruction.opcode.is_terminator() {
                    insns.extend(self.pending.shift_remove(&block).unwrap_or_default());
                }
                self.instruction(inst, &block_at, &mut insns, convention)?;
            }
            let succ = function.successors(block).iter().map(|one| block_at[one]).collect();
            blocks.push(LirBlock { succ, phis, ..LirBlock::new(block_at[&block], insns) });
        }
        let mut body = LirBody::new(name, block_at[&entry], blocks, IndexMap::default(), self.pins.clone());
        body.inputs = self.inputs.clone();
        body.ordered = true;
        Ok(body)
    }

    /// A comparison whose only reader is its block's branch stays flags.
    fn fuse(&mut self, block: BlockId, inst: InstId) {
        let function = self.function;
        let instruction = function.instruction(inst);
        if !matches!(instruction.opcode, Opcode::ICmp(_)) {
            return;
        }
        let result = instruction.result.expect("a comparison's value");
        if let [only] = function.users(result)
            && function.terminator(block) == Some(only.user)
            && function.instruction(only.user).opcode == Opcode::Br
        {
            self.fused.insert(inst);
        }
    }

    /// What each phi in `block` reads that is no register -- a constant,
    /// an address -- made in the predecessor, before its terminator.
    fn phis_from(&mut self, block: BlockId) -> Result<(), Unselected> {
        let function = self.function;
        for &inst in function.block(block).instructions() {
            let instruction = function.instruction(inst);
            if instruction.opcode != Opcode::Phi {
                break;
            }
            for pair in instruction.operands.chunks(2) {
                let [value, Operand::Block(from)] = *pair else { unreachable!("a phi's pairs") };
                if let Operand::Value(one) = value
                    && self.folded(one)?.is_none()
                {
                    continue;
                }
                let at = self.ats[&function.terminator(from).expect("a terminator")];
                let mut made = Vec::new();
                let held = self.held(value, instruction.ty, at, &mut made)?;
                self.pending.entry(from).or_default().extend(made);
                self.phi_inputs.insert((inst, from), held.value);
            }
        }
        Ok(())
    }

    fn incoming(&mut self, inst: InstId, block_at: &IndexMap<BlockId, i64>) -> Result<Vec<(i64, u32)>, Unselected> {
        let instruction = self.function.instruction(inst);
        let mut out = Vec::new();
        for pair in instruction.operands.chunks(2) {
            let [value, Operand::Block(from)] = *pair else { unreachable!("a phi's pairs") };
            let held = match (self.phi_inputs.get(&(inst, from)), value) {
                (Some(&made), _) => made,
                (None, Operand::Value(one)) => self.value(one),
                (None, _) => unreachable!("phis_from made every other input"),
            };
            out.push((block_at[&from], held));
        }
        Ok(out)
    }

    fn value(&mut self, value: ValueId) -> u32 {
        let next = &mut self.next;
        *self.values.entry(value).or_insert_with(|| {
            *next += 1;
            *next
        })
    }

    fn fresh(&mut self) -> u32 {
        self.next += 1;
        self.next
    }

    fn types(&self) -> &llrm_mir::Types {
        &self.module.context.types
    }

    /// A value's width in bytes, if a register holds it.
    fn width(&self, ty: TypeId) -> Result<u32, Unselected> {
        match self.types().get(ty) {
            Type::Int(bits @ (8 | 16 | 32)) => Ok(bits / 8),
            Type::Pointer(0) => Ok(self.layout.pointer(0).bits / 8),
            _ => refuse(format!("a {} value", self.types().display(ty))),
        }
    }

    fn constant(&self, operand: Operand, width: u32) -> Option<i64> {
        let Operand::Constant(id) = operand else { return None };
        let bits = match self.module.context.get(id).kind {
            ConstantKind::Int(bits) => bits,
            ConstantKind::Null | ConstantKind::Zero => 0,
            _ => return None,
        };
        let shift = 128 - 8 * width;
        Some(((bits << shift) as i128 >> shift) as i64)
    }

    /// An operand an instruction reads: a register, or an immediate.
    fn source(&mut self, operand: Operand, ty: TypeId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<Loc, Unselected> {
        let width = self.width(ty)?;
        if let Some(value) = self.constant(operand, width) {
            return Ok(Loc::Imm(Imm { value, width, address: None }));
        }
        Ok(Loc::Held(self.held(operand, ty, at, out)?))
    }

    /// An operand in a register, made there if it is not one already.
    fn held(&mut self, operand: Operand, ty: TypeId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<Held, Unselected> {
        let width = self.width(ty)?;
        if let Some(value) = self.constant(operand, width) {
            let held = Held { value: self.fresh(), width };
            let what = semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Imm(Imm { value, width, address: None })]);
            out.push(insn(at, what));
            return Ok(held);
        }
        let Operand::Value(value) = operand else { return refuse("a global's address") };
        match self.folded(value)? {
            None => Ok(Held { value: self.value(value), width }),
            Some(pointer) => {
                let held = Held { value: self.fresh(), width };
                out.push(insn(at, self.address(pointer, held)));
                Ok(held)
            }
        }
    }

    /// `held` made the address `pointer` names.
    fn address(&self, pointer: Pointer, held: Held) -> Semantics {
        match pointer {
            Pointer::Frame(disp) => {
                let address = Address { through: Register::BP, disp_width: 2, ..Address::new(Some(Addr::new(Space::Frame, disp))) };
                semantics(Operation::Address, "lea", vec![Loc::Held(held)], vec![Loc::Address(address)])
            }
            Pointer::Based { base, offset } => {
                let step = Loc::Imm(Imm { value: offset, width: held.width, address: None });
                semantics(Operation::Binary, "add", vec![Loc::Held(held)], vec![Loc::Held(base), step])
            }
        }
    }

    /// Where a pointer operand points.
    fn pointer(&mut self, operand: Operand) -> Result<Pointer, Unselected> {
        let Operand::Value(value) = operand else { return refuse("a constant address") };
        if let Some(pointer) = self.folded(value)? {
            return Ok(pointer);
        }
        let ty = self.function.value(value).ty;
        if !matches!(self.types().get(ty), Type::Pointer(0)) {
            return refuse(format!("an access through a {}", self.types().display(ty)));
        }
        let width = self.width(ty)?;
        Ok(Pointer::Based { base: Held { value: self.value(value), width }, offset: 0 })
    }

    /// Where `value` points, if it is an address an access folds: an
    /// alloca's slot, or a constant offset from any pointer.
    fn folded(&mut self, value: ValueId) -> Result<Option<Pointer>, Unselected> {
        if let Some(pointer) = self.pointers.get(&value) {
            return Ok(Some(*pointer));
        }
        let ValueDef::Instruction(inst) = self.function.value(value).def else { return Ok(None) };
        let instruction = self.function.instruction(inst);
        let Opcode::GetElementPtr { source } = instruction.opcode else { return Ok(None) };
        let (offset, variable) = self.layout.collect_offset(self.types(), source, &self.indices(inst));
        if !variable.is_empty() {
            return Ok(None);
        }
        let offset = offset as i64;
        let pointer = match self.pointer(instruction.operands[0])? {
            Pointer::Frame(disp) => Pointer::Frame(disp + offset),
            Pointer::Based { base, offset: was } => Pointer::Based { base, offset: was + offset },
        };
        self.pointers.insert(value, pointer);
        Ok(Some(pointer))
    }

    /// A GEP's indices, each a constant or `None`.
    fn indices(&self, inst: InstId) -> Vec<Option<i128>> {
        let context = &self.module.context;
        self.function.instruction(inst).operands[1..]
            .iter()
            .map(|&one| {
                let bits = self.function.operand_type(context, one).and_then(|ty| self.types().int_bits(ty)).unwrap_or(64);
                self.constant(one, bits.div_ceil(8)).map(i128::from)
            })
            .collect()
    }

    /// A GEP with a variable index, computed into a register: each index
    /// taken to the pointer's index width, as LLVM sign-extends or
    /// truncates it, scaled, and added to the base.
    fn indexed(&mut self, inst: InstId, source: TypeId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let function = self.function;
        let instruction = function.instruction(inst);
        let width = self.width(instruction.ty)?;
        let (offset, variable) = self.layout.collect_offset(self.types(), source, &self.indices(inst));
        let mut sum: Option<Held> = None;
        for (position, scale) in variable {
            let index = instruction.operands[1 + position];
            let index_ty = function.operand_type(&self.module.context, index).expect("a typed index");
            let mut held = self.held(index, index_ty, at, out)?;
            if held.width > width {
                held.width = width;
            } else if held.width < width {
                let wide = Held { value: self.fresh(), width };
                out.push(insn(at, semantics(Operation::Extend, "movsx", vec![Loc::Held(wide)], vec![Loc::Held(held)])));
                held = wide;
            }
            if scale != 1 {
                let scaled = Held { value: self.fresh(), width };
                let what = if scale.is_power_of_two() {
                    let shift = Loc::Imm(Imm { value: i64::from(scale.trailing_zeros()), width: 1, address: None });
                    semantics(Operation::Binary, "shl", vec![Loc::Held(scaled)], vec![Loc::Held(held), shift])
                } else {
                    let factor = Loc::Imm(Imm { value: scale as i64, width, address: None });
                    semantics(Operation::Multiply, "imul", vec![Loc::Held(scaled)], vec![Loc::Held(held), factor])
                };
                out.push(insn(at, what));
                held = scaled;
            }
            sum = Some(match sum {
                None => held,
                Some(before) => {
                    let added = Held { value: self.fresh(), width };
                    out.push(insn(at, semantics(Operation::Binary, "add", vec![Loc::Held(added)], vec![Loc::Held(before), Loc::Held(held)])));
                    added
                }
            });
        }
        let start = match self.pointer(instruction.operands[0])? {
            Pointer::Based { base, offset: 0 } if offset == 0 => base,
            pointer => {
                let moved = match pointer {
                    Pointer::Frame(disp) => Pointer::Frame(disp + offset as i64),
                    Pointer::Based { base, offset: was } => Pointer::Based { base, offset: was + offset as i64 },
                };
                let start = Held { value: self.fresh(), width };
                out.push(insn(at, self.address(moved, start)));
                start
            }
        };
        let result = Held { value: self.value(instruction.result.expect("an address")), width };
        let sum = sum.expect("a variable index");
        out.push(insn(at, semantics(Operation::Binary, "add", vec![Loc::Held(result)], vec![Loc::Held(start), Loc::Held(sum)])));
        Ok(())
    }

    /// `div` and `idiv` divide dx:ax, the high word made by `cwd` or zero,
    /// and leave both quotient and remainder.
    fn divide(&mut self, op: BinaryOp, inst: InstId, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let instruction = self.function.instruction(inst);
        let at = self.ats[&inst];
        let ty = instruction.ty;
        let width = self.width(ty)?;
        if !matches!(width, 2 | 4) {
            return refuse("a byte division");
        }
        let dividend = self.held(instruction.operands[0], ty, at, out)?;
        let divisor = self.held(instruction.operands[1], ty, at, out)?;
        let signed = matches!(op, BinaryOp::SDiv | BinaryOp::SRem);
        let high = Held { value: self.fresh(), width };
        out.push(insn(
            at,
            if signed {
                semantics(Operation::Extend, if width == 2 { "cwd" } else { "cdq" }, vec![Loc::Held(high)], vec![Loc::Held(dividend)])
            } else {
                semantics(Operation::Move, "mov", vec![Loc::Held(high)], vec![Loc::Imm(Imm { value: 0, width, address: None })])
            },
        ));
        let (result, other) = (Held { value: self.value(instruction.result.expect("a result")), width }, Held { value: self.fresh(), width });
        let (quotient, remainder) = if matches!(op, BinaryOp::SDiv | BinaryOp::UDiv) { (result, other) } else { (other, result) };
        let what = semantics(
            Operation::Divide,
            if signed { "idiv" } else { "div" },
            vec![Loc::Held(quotient), Loc::Held(remainder)],
            vec![Loc::Held(high), Loc::Held(dividend), Loc::Held(divisor)],
        );
        out.push(insn(at, what));
        Ok(())
    }

    fn memory(pointer: Pointer, width: u32) -> Mem {
        match pointer {
            Pointer::Frame(disp) => frame(disp, width),
            Pointer::Based { base, offset } => Mem { base: Some(base), offset, ..Mem::new(None, width) },
        }
    }

    fn instruction(
        &mut self,
        inst: InstId,
        block_at: &IndexMap<BlockId, i64>,
        out: &mut Vec<Arc<Insn>>,
        convention: &Convention,
    ) -> Result<(), Unselected> {
        let function = self.function;
        let instruction = function.instruction(inst);
        let at = self.ats[&inst];
        let operands = &instruction.operands;
        let type_of = |operand: Operand| function.operand_type(&self.module.context, operand).expect("a typed operand");
        match &instruction.opcode {
            Opcode::Alloca { .. } => {}
            Opcode::GetElementPtr { source } => {
                if self.folded(instruction.result.expect("an address"))?.is_none() {
                    self.indexed(inst, *source, at, out)?;
                }
            }
            Opcode::Load { volatile, .. } => {
                let width = self.width(instruction.ty)?;
                let cell = Self::memory(self.pointer(operands[0])?, width);
                let held = Held { value: self.value(instruction.result.expect("a load's value")), width };
                let what = semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Mem(cell)]);
                out.push(Arc::new(Insn { volatile: *volatile, ..insn_of(at, what) }));
            }
            Opcode::Store { volatile, .. } => {
                let ty = type_of(operands[0]);
                let width = self.width(ty)?;
                let stored = self.source(operands[0], ty, at, out)?;
                let cell = Self::memory(self.pointer(operands[1])?, width);
                let what = semantics(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![stored]);
                out.push(Arc::new(Insn { volatile: *volatile, ..insn_of(at, what) }));
            }
            Opcode::Binary(op) => {
                let (operation, name, commutes) = match op {
                    BinaryOp::Add => (Operation::Binary, "add", true),
                    BinaryOp::Sub => (Operation::Binary, "sub", false),
                    BinaryOp::And => (Operation::Binary, "and", true),
                    BinaryOp::Or => (Operation::Binary, "or", true),
                    BinaryOp::Xor => (Operation::Binary, "xor", true),
                    BinaryOp::Mul => (Operation::Multiply, "imul", true),
                    BinaryOp::SDiv | BinaryOp::SRem | BinaryOp::UDiv | BinaryOp::URem => return self.divide(*op, inst, out),
                    BinaryOp::Shl => (Operation::Binary, "shl", false),
                    BinaryOp::LShr => (Operation::Binary, "shr", false),
                    BinaryOp::AShr => (Operation::Binary, "sar", false),
                    _ => return refuse(instruction.opcode.mnemonic()),
                };
                let ty = instruction.ty;
                let (mut a, mut b) = (operands[0], operands[1]);
                if commutes && matches!(a, Operand::Constant(_)) {
                    std::mem::swap(&mut a, &mut b);
                }
                let a = Loc::Held(self.held(a, ty, at, out)?);
                let b = match (self.source(b, ty, at, out)?, name) {
                    // A shift counts from cl: its count is a byte.
                    (Loc::Held(count), "shl" | "shr" | "sar") => Loc::Held(Held { width: 1, ..count }),
                    (b, _) => b,
                };
                let result = Held { value: self.value(instruction.result.expect("a result")), width: self.width(ty)? };
                out.push(insn(at, semantics(operation, name, vec![Loc::Held(result)], vec![a, b])));
            }
            Opcode::Cast(op) => {
                let from = type_of(operands[0]);
                let (to, from_width) = (self.width(instruction.ty)?, self.width(from)?);
                let result = Held { value: self.value(instruction.result.expect("a result")), width: to };
                let what = match op {
                    CastOp::Trunc => {
                        let source = self.held(operands[0], from, at, out)?;
                        semantics(Operation::Move, "mov", vec![Loc::Held(result)], vec![Loc::Held(Held { width: to, ..source })])
                    }
                    CastOp::SExt | CastOp::ZExt => {
                        let source = self.held(operands[0], from, at, out)?;
                        let name = if *op == CastOp::SExt { "movsx" } else { "movzx" };
                        semantics(Operation::Extend, name, vec![Loc::Held(result)], vec![Loc::Held(source)])
                    }
                    CastOp::PtrToInt | CastOp::IntToPtr if to == from_width => {
                        let source = self.source(operands[0], from, at, out)?;
                        semantics(Operation::Move, "mov", vec![Loc::Held(result)], vec![source])
                    }
                    _ => return refuse(instruction.opcode.mnemonic()),
                };
                out.push(insn(at, what));
            }
            Opcode::ICmp(_) if self.fused.contains(&inst) => {}
            Opcode::ICmp(_) => return refuse("a comparison as a value"),
            Opcode::Br => match operands[..] {
                [Operand::Block(target)] => {
                    out.push(insn(at, jump(block_at[&target])));
                }
                [Operand::Value(condition), Operand::Block(taken), Operand::Block(_)] => {
                    let Some(compare) = self.fused_compare(condition) else {
                        return refuse("a branch on a value no comparison made");
                    };
                    let compare_ty = type_of(function.instruction(compare).operands[0]);
                    let Opcode::ICmp(predicate) = function.instruction(compare).opcode else { unreachable!("fused") };
                    let (mut a, mut b) = (function.instruction(compare).operands[0], function.instruction(compare).operands[1]);
                    let mut predicate = predicate;
                    if matches!(a, Operand::Constant(_)) {
                        std::mem::swap(&mut a, &mut b);
                        predicate = swapped(predicate);
                    }
                    let a = Loc::Held(self.held(a, compare_ty, at, out)?);
                    let b = self.source(b, compare_ty, at, out)?;
                    out.push(insn(at, semantics(Operation::Compare, "cmp", vec![], vec![a, b])));
                    let branch = Semantics { target: Some(block_at[&taken]), ..semantics(Operation::Branch, condition_code(predicate), vec![], vec![]) };
                    out.push(insn(at, branch));
                }
                _ => return refuse("a branch on a constant"),
            },
            Opcode::Ret => {
                let what = semantics(Operation::Return, "", vec![], vec![]);
                let mut one = Insn { reads_complete: true, ..Insn::new(at, Some((at, at)), Some(what), vec![], vec![]) };
                if let Some(&value) = operands.first() {
                    let held = self.held(value, type_of(value), at, out)?;
                    let Some(&register) = convention.returns.first() else { return refuse("a result the convention has no register for") };
                    one.uses = vec![held.value];
                    one.requires = vec![(held, register)];
                }
                out.push(Arc::new(one));
            }
            _ => return refuse(instruction.opcode.mnemonic()),
        }
        Ok(())
    }

    fn fused_compare(&self, condition: ValueId) -> Option<InstId> {
        match self.function.value(condition).def {
            ValueDef::Instruction(inst) if self.fused.contains(&inst) => Some(inst),
            _ => None,
        }
    }
}

fn frame(disp: i64, width: u32) -> Mem {
    Mem { through: Register::BP, disp_width: 2, ..Mem::new(Some(Addr::new(Space::Frame, disp)), width) }
}

fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

fn jump(target: i64) -> Semantics {
    Semantics { target: Some(target), ..semantics(Operation::Jump, "jmp", vec![], vec![]) }
}

fn insn_of(at: i64, what: Semantics) -> Insn {
    let (defines, uses) = (_written(&what.dests), _read(&what));
    Insn::new(at, Some((at, at)), Some(what), defines, uses)
}

fn insn(at: i64, what: Semantics) -> Arc<Insn> {
    Arc::new(insn_of(at, what))
}

fn condition_code(predicate: IntPredicate) -> &'static str {
    match predicate {
        IntPredicate::Eq => "je",
        IntPredicate::Ne => "jne",
        IntPredicate::Slt => "jl",
        IntPredicate::Sle => "jle",
        IntPredicate::Sgt => "jg",
        IntPredicate::Sge => "jge",
        IntPredicate::Ult => "jb",
        IntPredicate::Ule => "jbe",
        IntPredicate::Ugt => "ja",
        IntPredicate::Uge => "jae",
    }
}

/// The predicate that holds of `b, a` when `predicate` holds of `a, b`.
fn swapped(predicate: IntPredicate) -> IntPredicate {
    use IntPredicate::*;
    match predicate {
        Eq | Ne => predicate,
        Slt => Sgt,
        Sle => Sge,
        Sgt => Slt,
        Sge => Sle,
        Ult => Ugt,
        Ule => Uge,
        Ugt => Ult,
        Uge => Ule,
    }
}
