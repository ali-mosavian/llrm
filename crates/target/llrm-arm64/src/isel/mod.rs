//! Instruction selection: one MIR function becomes arm64 instructions over
//! virtual registers, as LLVM's `AArch64DAGToDAGISel` and
//! `AArch64TargetLowering` make `MachineInstr`s. The selector walks the blocks
//! in reverse post-order and maps each MIR instruction to the cheapest sequence
//! its operands allow (an immediate form, a folded shift or extend, a fused
//! compare and branch); a comparison only a branch or select reads is selected
//! where it is read, and so are the comparisons of an `or`/`and` chain a
//! branch reads (`chain`: one branch each). A `tail` call that returns its own
//! value, with its arguments in registers, is a branch after the frame is taken
//! down (`Op::Return` with a `tail` callee). A constant shift only an
//! `add`, `sub`, `and`, `or` or `xor` reads is that instruction's shifted
//! operand (`shifted`), a load only a `sext` reads is a sign-extending load,
//! and a shift or 32-bit extension only a `getelementptr` index reads is part
//! of the addressing mode (`index`).

mod agg;
mod atomic;
mod call;
mod chain;
mod conjunction;
mod control;
mod divide;
mod fixedpoint;
mod fp;
mod index;
mod int;
mod mem;
mod memops;
mod shifted;
mod val;
mod widemul;

use std::collections::{HashMap, HashSet};

use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::{
    BlockId, ConstantId, ConstantKind, Function, GlobalId, InstId, Module, Opcode, Operand, Type, TypeId, ValueDef,
    ValueId,
};
use val::{AddrExpr, Base, G, Known, Val};

use crate::cc::Scalar;
use crate::machine::Platform;
use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::{MBlock, MFunc, Op, block_label};

/// What selection cannot do (yet): the construct, for the message.
pub type Refusal = String;

pub(crate) type Selected<T> = Result<T, Refusal>;

fn refuse<T>(what: impl Into<String>) -> Selected<T> {
    Err(what.into())
}

/// A source operand: a constant known at selection, or a value in registers.
#[derive(Clone, Debug)]
pub(crate) enum Src {
    Int { bits: u32, value: u128 },
    Float { size: FpSize, bits: u64 },
    Val(Val),
}

pub struct Selector<'a> {
    pub(crate) module: &'a Module,
    pub(crate) function: &'a Function,
    pub(crate) layout: &'a DataLayout,
    pub(crate) platform: Platform,
    pub(crate) mf: MFunc,
    pub(crate) vals: IdMap<ValueId, Val>,
    /// The blocks in reverse post-order: the order they are selected and laid
    /// out in.
    order: Vec<BlockId>,
    index: IdMap<BlockId, usize>,
    uses: IdMap<ValueId, usize>,
    /// Comparisons selected where their reader is.
    pub(crate) fused: IdSet<InstId>,
    /// The shifts and extensions left to the `getelementptr` that indexes by
    /// them (also in `fused`).
    pub(crate) indexed: IdSet<InstId>,
    /// Branches on chains of comparisons, split into one branch each.
    pub(crate) chains: chain::Chains,
    /// Selects and branches on `and`/`or` trees of comparisons, made one `cmp`
    /// and a `ccmp` each.
    pub(crate) conjunctions: conjunction::Conjunctions,
    /// The block being filled.
    pub(crate) current: usize,
    /// How many accesses name each global (by symbol): one that is named more
    /// than once has its address made once per block (`shared_global`)
    /// instead of an `adrp` at each access.
    global_uses: HashMap<String, usize>,
    /// The register holding a global's address in a block, by symbol and block.
    global_regs: HashMap<(String, usize), Gpr>,
    global_pages: HashMap<(String, usize), Gpr>,
    /// The globals something other than a plain load or store of the symbol
    /// itself reads: they need their full address in a register anyway, so
    /// no access folds the page offset (`isWorthFoldingADDlow`).
    global_needs_address: HashSet<String>,
    /// A variadic function's register save areas and where its unnamed
    /// arguments begin.
    pub(crate) va: Option<call::VaInfo>,
}

/// `function` of `module` selected for `platform`.
pub fn select(
    module: &Module,
    function: GlobalId,
    layout: &DataLayout,
    platform: Platform,
) -> Selected<MFunc> {
    let global = module.global(function);
    let name = global.name.owned().ok_or("an unnamed function")?;
    let body = global.function().ok_or("a variable is not selected")?;
    let mut selector = Selector::new(module, body, layout, platform, &name);
    selector.run()?;
    Ok(selector.finish())
}

impl<'a> Selector<'a> {
    fn new(
        module: &'a Module,
        function: &'a Function,
        layout: &'a DataLayout,
        platform: Platform,
        name: &str,
    ) -> Self {
        let order = reverse_post_order(function);
        let index = order.iter().enumerate().map(|(at, block)| (*block, at)).collect();
        let mut uses: IdMap<ValueId, usize> = IdMap::new();
        for (_, inst) in function.walk() {
            for operand in &function.instruction(inst).operands {
                if let Operand::Value(value) = operand {
                    *uses.get_or_insert_with(*value, || 0) += 1;
                }
            }
        }
        let mut mf = MFunc::new(name);
        mf.blocks = (0..order.len()).map(|at| MBlock { label: block_label(name, at), ops: Vec::new() }).collect();
        Self {
            module,
            function,
            layout,
            platform,
            mf,
            vals: IdMap::new(),
            order,
            index,
            uses,
            fused: IdSet::new(),
            indexed: IdSet::new(),
            chains: chain::Chains::default(),
            conjunctions: conjunction::Conjunctions::default(),
            current: 0,
            global_uses: HashMap::new(),
            global_regs: HashMap::new(),
            global_pages: HashMap::new(),
            global_needs_address: HashSet::new(),
            va: None,
        }
    }

    /// The global a constant addresses: itself, or the base of a constant gep.
    fn global_of_constant(
        &self,
        id: ConstantId,
    ) -> Option<GlobalId> {
        match &self.module.context.get(id).kind {
            ConstantKind::Global(global) => Some(*global),
            ConstantKind::Expr(llrm_mir::ConstantExpr::GetElementPtr { operands, .. }) => {
                self.global_of_constant(*operands.first()?)
            }
            _ => None,
        }
    }

    /// Counts the accesses that name each global: a gep's result counts once
    /// for each of its readers.
    fn count_global_uses(&mut self) {
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut needs_address = HashSet::new();
        for (_, inst) in self.function.walk() {
            let instruction = self.function.instruction(inst);
            for (at, operand) in instruction.operands.iter().enumerate() {
                let Operand::Constant(id) = operand else { continue };
                let Some(global) = self.global_of_constant(*id) else { continue };
                let Ok(name) = self.symbol(global) else { continue };
                let readers = match (&instruction.opcode, instruction.result) {
                    (Opcode::GetElementPtr { .. }, Some(result)) if at == 0 => {
                        self.uses.get(&result).copied().unwrap_or(1).max(1)
                    }
                    _ => 1,
                };
                let plain = matches!(self.module.context.get(*id).kind, ConstantKind::Global(_))
                    && matches!(
                        (&instruction.opcode, at),
                        (Opcode::Load { .. }, 0) | (Opcode::Store { .. }, 1)
                    );
                if !plain {
                    needs_address.insert(name.clone());
                }
                *counts.entry(name).or_default() += readers;
            }
        }
        self.global_uses = counts;
        self.global_needs_address = needs_address;
    }

    fn finish(mut self) -> MFunc {
        self.mf.declared = self
            .function
            .debug_records()
            .iter()
            .filter_map(|record| {
                let llrm_mir::DebugWhat::Declare(llrm_mir::Operand::Value(address)) = record.what else { return None };
                match self.vals.get(&address)? {
                    Val::Ptr(AddrExpr { base: Base::Slot(slot), offset: 0, index: None }) => {
                        Some((record.variable, *slot))
                    }
                    _ => None,
                }
            })
            .collect();
        self.mf
    }

    pub(crate) fn emit(
        &mut self,
        op: Op,
    ) {
        self.mf.blocks[self.current].ops.push(op);
    }

    pub(crate) fn inst(
        &mut self,
        inst: Inst,
    ) {
        self.emit(Op::I(inst));
    }

    pub(crate) fn types(&self) -> &llrm_mir::Types {
        &self.module.context.types
    }

    pub(crate) fn ty(
        &self,
        id: TypeId,
    ) -> &Type {
        self.types().get(id)
    }

    pub(crate) fn use_count(
        &self,
        value: ValueId,
    ) -> usize {
        self.uses.get(&value).copied().unwrap_or(0)
    }

    pub(crate) fn operand_type(
        &self,
        operand: Operand,
    ) -> Selected<TypeId> {
        self.function.operand_type(&self.module.context, operand).ok_or_else(|| "an operand with no type".to_owned())
    }

    fn run(&mut self) -> Selected<()> {
        if self.order.is_empty() {
            return refuse("a function with no body");
        }
        self.mf.dynamic = self.function.walk().any(|(_, inst)| {
            matches!(self.function.instruction(inst).opcode, Opcode::Alloca { .. }) && !self.is_static_alloca(inst)
        });
        self.count_global_uses();
        self.find_chains();
        self.find_conjunctions();
        self.phi_registers()?;
        self.current = 0;
        self.parameters()?;
        for at in 0..self.order.len() {
            self.current = at;
            let block = self.order[at];
            let insts: Vec<InstId> = self.function.iter_block(block).collect();
            for inst in insts {
                self.select_inst(inst)?;
            }
        }
        Ok(())
    }

    /// A register for each phi, made before any block is selected: a loop's phi
    /// is read in a block selected before the one that feeds it.
    fn phi_registers(&mut self) -> Selected<()> {
        for &block in &self.order.clone() {
            for inst in self.function.iter_block(block).collect::<Vec<_>>() {
                let instruction = self.function.instruction(inst);
                if instruction.opcode != Opcode::Phi {
                    break;
                }
                let result = instruction.result.expect("a phi has a result");
                let value = self.fresh(instruction.ty)?;
                self.vals.insert(result, value);
            }
        }
        Ok(())
    }

    /// A new register for a value of type `ty`.
    pub(crate) fn fresh(
        &mut self,
        ty: TypeId,
    ) -> Selected<Val> {
        Ok(match self.ty(ty).clone() {
            Type::Int(bits) if bits <= 64 => {
                let known = if bits == 32 { Known::Zero } else { Known::Hi32Zero };
                let known = if bits > 32 { Known::Nothing } else { known };
                Val::Int(G { reg: self.mf.gpr(), bits, known })
            }
            Type::Pointer(_) => Val::Ptr(AddrExpr::of(Base::Reg(self.mf.gpr()))),
            Type::Float(llrm_mir::FloatKind::Float) => Val::Float { reg: self.mf.fpr(), size: FpSize::S },
            Type::Float(llrm_mir::FloatKind::Double) => Val::Float { reg: self.mf.fpr(), size: FpSize::D },
            Type::Array { element, count } if count <= 64 => {
                let members = (0..count).map(|_| self.fresh(element)).collect::<Selected<Vec<_>>>()?;
                Val::Agg(members)
            }
            Type::Struct { .. } | Type::Named(_) => {
                let fields = self.types().fields(ty).ok_or("an opaque struct")?.to_vec();
                Val::Agg(fields.into_iter().map(|field| self.fresh(field)).collect::<Selected<Vec<_>>>()?)
            }
            other => return refuse(format!("a value of type {other:?}")),
        })
    }

    /// The scalar types an aggregate (or a scalar) is made of, in order.
    pub(crate) fn leaf_types(
        &self,
        ty: TypeId,
    ) -> Selected<Vec<TypeId>> {
        Ok(match self.ty(ty).clone() {
            Type::Array { element, count } if count <= 64 => {
                let leaf = self.leaf_types(element)?;
                (0..count).flat_map(|_| leaf.clone()).collect()
            }
            Type::Struct { .. } | Type::Named(_) => {
                let fields = self.types().fields(ty).ok_or("an opaque struct")?.to_vec();
                let mut out = Vec::new();
                for field in fields {
                    out.extend(self.leaf_types(field)?);
                }
                out
            }
            Type::Int(_) | Type::Pointer(_) | Type::Float(_) => vec![ty],
            other => return refuse(format!("a value of type {other:?}")),
        })
    }

    /// The convention's scalar class of an argument or result of type `ty`.
    pub(crate) fn scalar(
        &self,
        ty: TypeId,
    ) -> Selected<Scalar> {
        Ok(match self.ty(ty) {
            Type::Int(bits) if *bits <= 32 => Scalar::I32,
            Type::Int(bits) if *bits <= 64 => Scalar::I64,
            Type::Pointer(_) => Scalar::I64,
            Type::Float(llrm_mir::FloatKind::Float) => Scalar::F32,
            Type::Float(llrm_mir::FloatKind::Double) => Scalar::F64,
            other => return refuse(format!("an argument of type {other:?}")),
        })
    }

    /// `operand` as a constant or a value.
    pub(crate) fn src(
        &mut self,
        operand: Operand,
    ) -> Selected<Src> {
        match operand {
            Operand::Value(id) => match self.vals.get(&id) {
                Some(value) => Ok(Src::Val(value.clone())),
                None => refuse(format!("value {} is read before it is selected", id.0)),
            },
            Operand::Constant(id) => self.constant(id),
            Operand::Block(_) => refuse("a block as a value"),
        }
    }

    fn constant(
        &mut self,
        id: ConstantId,
    ) -> Selected<Src> {
        let constant = self.module.context.get(id).clone();
        let ty = constant.ty;
        match (&constant.kind, self.ty(ty).clone()) {
            (ConstantKind::Int(value), Type::Int(bits)) => {
                Ok(Src::Int { bits, value: *value & llrm_mir::context::mask(bits) })
            }
            (ConstantKind::Zero | ConstantKind::Null | ConstantKind::Undef | ConstantKind::Poison, Type::Int(bits)) => {
                Ok(Src::Int { bits, value: 0 })
            }
            (
                ConstantKind::Null | ConstantKind::Zero | ConstantKind::Undef | ConstantKind::Poison,
                Type::Pointer(_),
            ) => Ok(Src::Int { bits: 64, value: 0 }),
            (ConstantKind::Float(bits), Type::Float(llrm_mir::FloatKind::Float)) => {
                Ok(Src::Float { size: FpSize::S, bits: *bits })
            }
            (ConstantKind::Float(bits), Type::Float(llrm_mir::FloatKind::Double)) => {
                Ok(Src::Float { size: FpSize::D, bits: *bits })
            }
            (ConstantKind::Zero | ConstantKind::Undef | ConstantKind::Poison, Type::Float(kind)) => {
                Ok(Src::Float { size: if kind == llrm_mir::FloatKind::Float { FpSize::S } else { FpSize::D }, bits: 0 })
            }
            (ConstantKind::Aggregate(members), Type::Array { .. } | Type::Struct { .. } | Type::Named(_)) => {
                let fields = match self.ty(ty) {
                    Type::Array { element, count } => vec![*element; *count as usize],
                    _ => self.types().fields(ty).ok_or("an opaque struct")?.to_vec(),
                };
                let mut out = Vec::new();
                for (member, _) in members.iter().zip(fields) {
                    let src = self.constant(*member)?;
                    out.push(self.materialize(src)?);
                }
                Ok(Src::Val(Val::Agg(out)))
            }
            (
                ConstantKind::Zero | ConstantKind::Undef | ConstantKind::Poison,
                Type::Array { .. } | Type::Struct { .. } | Type::Named(_),
            ) => {
                let mut value = self.fresh(ty)?;
                for leaf in value.leaves_mut() {
                    let zero = match leaf {
                        Val::Int(g) => Src::Int { bits: g.bits, value: 0 },
                        Val::Ptr(_) => Src::Int { bits: 64, value: 0 },
                        Val::Float { size, .. } => Src::Float { size: *size, bits: 0 },
                        Val::Agg(_) => unreachable!("leaves are scalars"),
                    };
                    let made = self.materialize(zero)?;
                    *leaf = made;
                }
                Ok(Src::Val(value))
            }
            (ConstantKind::Global(global), _) => {
                let got = self.needs_got(*global);
                let name = self.symbol(*global)?;
                Ok(Src::Val(Val::Ptr(AddrExpr::of(Base::Global { name, got }))))
            }
            (ConstantKind::Expr(expr), _) => self.constant_expression(expr.clone()),
            (kind, ty) => refuse(format!("the constant {kind:?} of type {ty:?}")),
        }
    }

    /// The linker's name of a global.
    pub(crate) fn symbol(
        &self,
        global: GlobalId,
    ) -> Selected<String> {
        let name = self.module.global(global).name.owned().ok_or("an unnamed global")?;
        Ok(crate::data::symbol_name(self.platform, &name))
    }

    /// Whether the global is reached through the GOT: it may be defined in
    /// another object (`AArch64Subtarget::ClassifyGlobalReference`).
    pub(crate) fn needs_got(
        &self,
        global: GlobalId,
    ) -> bool {
        use llrm_mir::{GlobalKind, Linkage};
        let global = self.module.global(global);
        let defined = match &global.kind {
            GlobalKind::Function(function) => !function.is_declaration(),
            GlobalKind::Variable(variable) => variable.initializer.is_some(),
        };
        let local = matches!(global.linkage, Linkage::Internal | Linkage::Private);
        let _ = defined;
        // Darwin reaches anything not defined here through the GOT; Linux
        // static code reaches all of it directly.
        self.platform == Platform::Darwin && !local && !defined
    }

    fn constant_expression(
        &mut self,
        expr: llrm_mir::ConstantExpr,
    ) -> Selected<Src> {
        match expr {
            llrm_mir::ConstantExpr::GetElementPtr { source, operands, .. } => {
                let base = self.src_constant(operands[0])?;
                let Src::Val(Val::Ptr(mut address)) = base else { return refuse("a constant gep on a non-address") };
                let mut indices = Vec::new();
                for &index in &operands[1..] {
                    match self.module.context.get(index).kind {
                        ConstantKind::Int(value) => {
                            let bits = self.types().int_bits(self.module.context.get(index).ty).unwrap_or(64);
                            indices.push(Some(llrm_mir::context::signed(value, bits)));
                        }
                        ConstantKind::Zero => indices.push(Some(0)),
                        _ => return refuse("a constant gep with a non-constant index"),
                    }
                }
                let (offset, variable) = self.layout.collect_offset(self.types(), source, &indices);
                if !variable.is_empty() {
                    return refuse("a constant gep with a variable index");
                }
                address.offset += offset as i64;
                Ok(Src::Val(Val::Ptr(address)))
            }
            llrm_mir::ConstantExpr::Cast { .. } => refuse("a constant cast"),
            llrm_mir::ConstantExpr::Binary { .. } => refuse("a constant binary operator"),
        }
    }

    fn src_constant(
        &mut self,
        id: ConstantId,
    ) -> Selected<Src> {
        self.constant(id)
    }

    fn select_inst(
        &mut self,
        inst: InstId,
    ) -> Selected<()> {
        if self.fused.contains(&inst) {
            return Ok(());
        }
        let instruction = self.function.instruction(inst).clone();
        let result = instruction.result;
        if result.is_some_and(|result| self.chains.contains(result) || self.conjunctions.contains(result)) {
            // Selected where the chain's branch, or the select that reads the
            // tree, is.
            return Ok(());
        }
        match &instruction.opcode {
            Opcode::Phi => Ok(()),
            Opcode::Binary(op) => self.binary(*op, &instruction),
            Opcode::Cast(op) => self.cast(*op, &instruction),
            Opcode::ICmp(predicate) => self.icmp_value(*predicate, &instruction),
            Opcode::FCmp(predicate) => self.fcmp_value(*predicate, &instruction),
            Opcode::FNeg => self.fneg(&instruction),
            Opcode::Select => self.select(&instruction),
            Opcode::ExtractValue(path) => self.extract_value(&instruction, path),
            Opcode::InsertValue(path) => self.insert_value(&instruction, path),
            Opcode::Freeze => {
                let value = self.src(instruction.operands[0])?;
                self.define(result, value)
            }
            Opcode::Alloca { allocated, align, .. } => self.alloca(inst, &instruction, *allocated, *align),
            Opcode::Load { .. } => self.load(&instruction),
            Opcode::Store { .. } => self.store(&instruction),
            Opcode::GetElementPtr { source } => self.gep(&instruction, *source),
            Opcode::Call(info) => self.call(inst, &instruction, info),
            // A call, then a branch to the normal block. The unwind edge is
            // left unselected: nothing unwinds, and QB's landing pad is entered
            // by the runtime, not by an unwinder.
            Opcode::Invoke(info) => {
                let [.., Operand::Block(normal), Operand::Block(_), callee] = instruction.operands[..] else {
                    return refuse("an invoke without its blocks");
                };
                let mut call = instruction.clone();
                call.operands.truncate(instruction.operands.len() - 3);
                call.operands.push(callee);
                self.call(inst, &call, info)?;
                let mut jump = instruction.clone();
                jump.opcode = Opcode::Br;
                jump.operands = vec![Operand::Block(normal)];
                jump.result = None;
                self.br(self.current, &jump)
            }
            // The runtime enters QB's landing pad with the frame put back, not
            // an unwinder with an exception: the pad's value is zero, and what
            // it reads of the error is the global ehprepare made.
            Opcode::LandingPad { .. } => {
                let mut value = self.fresh(instruction.ty)?;
                for leaf in value.leaves_mut() {
                    let zero = match leaf {
                        Val::Int(g) => Src::Int { bits: g.bits, value: 0 },
                        Val::Ptr(_) => Src::Int { bits: 64, value: 0 },
                        Val::Float { size, .. } => Src::Float { size: *size, bits: 0 },
                        Val::Agg(_) => unreachable!("leaves are scalars"),
                    };
                    *leaf = self.materialize(zero)?;
                }
                self.define(result, Src::Val(value))
            }
            Opcode::Ret => self.ret(&instruction),
            Opcode::Br => self.br(self.current, &instruction),
            Opcode::Switch => self.switch(self.current, &instruction),
            Opcode::Unreachable => {
                self.inst(Inst::Brk { imm16: 1 });
                Ok(())
            }
            other => refuse(format!("the instruction `{}`", other.mnemonic())),
        }
    }

    /// Names `src` as the value of `result`.
    pub(crate) fn define(
        &mut self,
        result: Option<ValueId>,
        src: Src,
    ) -> Selected<()> {
        let Some(result) = result else { return Ok(()) };
        let value = match src {
            Src::Val(value) => value,
            constant => self.materialize(constant)?,
        };
        self.vals.insert(result, value);
        Ok(())
    }

    /// `src` in registers.
    pub(crate) fn materialize(
        &mut self,
        src: Src,
    ) -> Selected<Val> {
        match src {
            Src::Val(value) => Ok(value),
            Src::Int { bits, value } => {
                let reg = self.mf.gpr();
                self.load_immediate(reg, bits, value as u64);
                Ok(Val::Int(G { reg, bits, known: if bits <= 32 { Known::Zero } else { Known::Nothing } }))
            }
            Src::Float { size, bits } => {
                let reg = self.mf.fpr();
                self.load_float(reg, size, bits);
                Ok(Val::Float { reg, size })
            }
        }
    }

    /// `reg = value` as a register of `bits`.
    pub(crate) fn load_immediate(
        &mut self,
        reg: Gpr,
        bits: u32,
        value: u64,
    ) {
        let register_bits = if bits <= 32 { 32 } else { 64 };
        let value = if register_bits == 32 { value & 0xFFFF_FFFF } else { value };
        let insts = crate::expand_imm::mov_imm(value, register_bits, reg);
        // A chain of `movz`/`movk` is several writes of one register: kept as
        // one definition (expanded after allocation) so CSE and
        // hoisting can move it.
        if insts.len() > 1 {
            self.emit(Op::Imm { dst: reg, bits: register_bits, value });
        } else {
            for inst in insts {
                self.inst(inst);
            }
        }
    }

    fn load_float(
        &mut self,
        reg: Vreg,
        size: FpSize,
        bits: u64,
    ) {
        let double = match size {
            FpSize::S => f64::from(f32::from_bits(bits as u32)),
            _ => f64::from_bits(bits),
        };
        if let Some(imm8) = crate::mc::immediate::fp_immediate(double) {
            self.inst(Inst::FpMoveImm { size, rd: reg, imm8 });
            return;
        }
        // Through an integer register (the zero register for 0.0): LLVM loads
        // from a constant pool.
        let (width, register_bits) = if size == FpSize::S { (Width::W, 32) } else { (Width::X, 64) };
        let scratch = if bits == 0 {
            Gpr::R31
        } else {
            let scratch = self.mf.gpr();
            self.load_immediate(scratch, register_bits, bits);
            scratch
        };
        self.inst(Inst::FpInt { op: FpConvert::Bits { to_fp: true }, width, size, gpr: scratch, fpr: reg });
    }

    /// A register holding the integer or pointer `src`, at its own width.
    pub(crate) fn gpr_of(
        &mut self,
        src: Src,
    ) -> Selected<G> {
        match self.materialize(src)? {
            Val::Int(g) => Ok(g),
            Val::Ptr(address) => {
                let reg = self.pointer_register(&address)?;
                Ok(G { reg, bits: 64, known: Known::Nothing })
            }
            Val::Float { .. } => refuse("a float used as an integer"),
            Val::Agg(_) => refuse("an aggregate used as an integer"),
        }
    }

    pub(crate) fn operand_g(
        &mut self,
        operand: Operand,
    ) -> Selected<G> {
        let src = self.src(operand)?;
        self.gpr_of(src)
    }
}

/// The blocks reachable from the entry, in reverse post-order.
fn reverse_post_order(function: &Function) -> Vec<BlockId> {
    let Some(entry) = function.entry() else { return Vec::new() };
    let mut seen = HashSet::new();
    let mut post = Vec::new();
    // An explicit stack of (block, next successor to visit).
    let mut stack = vec![(entry, function.successors(entry).into_iter().collect::<Vec<_>>(), 0usize)];
    seen.insert(entry);
    while let Some((block, successors, next)) = stack.last_mut() {
        if *next < successors.len() {
            let successor = successors[*next];
            *next += 1;
            if seen.insert(successor) {
                let following = function.successors(successor).into_iter().collect::<Vec<_>>();
                stack.push((successor, following, 0));
            }
        } else {
            post.push(*block);
            stack.pop();
        }
    }
    post.reverse();
    post
}

pub(crate) fn value_def(
    function: &Function,
    id: ValueId,
) -> Option<InstId> {
    match function.value(id).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

impl Selector<'_> {
    pub(crate) fn block_index(
        &self,
        block: BlockId,
    ) -> Option<usize> {
        self.index.get(&block).copied()
    }

    pub(crate) fn block_order(&self) -> &[BlockId] {
        &self.order
    }

    /// Whether `label` is a block made for an edge: those come after the blocks
    /// of the function.
    pub(crate) fn is_edge_label(
        &self,
        label: &str,
    ) -> bool {
        self.mf.blocks.iter().skip(self.order.len()).any(|one| one.label == label)
    }

    pub(crate) fn reg_of(
        &self,
        value: &Val,
    ) -> Option<Reg> {
        match value {
            Val::Int(g) => Some(Reg::Gpr(g.reg)),
            Val::Float { reg, .. } => Some(Reg::Fpr(*reg)),
            Val::Ptr(AddrExpr { base: Base::Reg(reg), offset: 0, index: None }) => Some(Reg::Gpr(*reg)),
            Val::Ptr(_) | Val::Agg(_) => None,
        }
    }
}
