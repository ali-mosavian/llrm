//! Instruction selection from MIR: each function's instructions as LIR over
//! virtual registers, as LLVM's SelectionDAG makes MachineInstrs. It chooses
//! instructions and addressing only; two-address form, registers and the
//! frame's size are the machine phases'.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand, ValueDef, ValueId};
use llrm_mir::intrinsics::{FloatFunction, Intrinsic};
use llrm_mir::{BinaryOp, CastOp, ConstantKind, FloatKind, FloatPredicate, GlobalId, IntPredicate, Module, Opcode, Type, TypeId};

use crate::abi::runtime::Contract;
use crate::backend::lower::{_read, _written, call_clobbered_high, call_clobbers};
use crate::model::ir::{Addr, Address, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi};
use crate::support::hash::IndexMap;

/// Where a function's parameters arrive and its result leaves, as its
/// calling convention and address space say: LLVM's CC_X86 for ia16.
#[derive(Clone, Debug)]
pub struct Convention {
    /// Each parameter's cell, by its displacement from BP.
    pub parameters: Vec<i64>,
    /// The registers a result leaves in, low part first.
    pub returns: Vec<Register>,
    /// The bytes the function pops as it returns.
    pub popped: i64,
}

/// How a calling convention pushes arguments and who pops them. C pushes
/// right to left and its caller pops; BASIC pushes left to right and pops
/// its own.
struct Passing {
    in_order: bool,
    pops: bool,
}

fn passing(convention: u32) -> Result<Passing, Unselected> {
    match convention {
        0 => Ok(Passing { in_order: false, pops: false }),
        llrm_mir::opcode::BASIC => Ok(Passing { in_order: true, pops: true }),
        other => refuse(format!("calling convention {other}")),
    }
}

/// The bytes an argument of `width` takes on the stack: a byte is pushed
/// as a word.
fn slot(width: u32) -> i64 {
    i64::from(width.max(2))
}

/// The registers a result of `width` leaves in: a dword in DX:AX.
fn returned(width: u32) -> Vec<Register> {
    if width == 4 { vec![Register::EAX, Register::EDX] } else { vec![Register::EAX] }
}

/// Whether a function's code is far: in addrspace(1), entered by a far call.
pub fn far(global: &GlobalValue) -> Result<bool, Unselected> {
    match global.address_space {
        0 => Ok(false),
        1 => Ok(true),
        other => refuse(format!("code in address space {other}")),
    }
}

/// `function`'s convention: the return address, BP, then its arguments,
/// the last pushed nearest.
fn convention(module: &Module, layout: &DataLayout, global: GlobalId) -> Result<Convention, Unselected> {
    let global = module.global(global);
    let Some(function) = global.function() else { return refuse("a variable has no convention") };
    let first = if far(global)? { 6 } else { 4 };
    let Passing { in_order, pops } = passing(function.calling_convention)?;
    let mut widths = function.parameters().iter().map(|&one| size_of(module, layout, function.value(one).ty).map(slot)).collect::<Result<Vec<_>, _>>()?;
    // The last pushed is nearest: C's first argument, BASIC's last.
    if in_order {
        widths.reverse();
    }
    let mut parameters = Vec::new();
    let mut cursor = first;
    for width in widths {
        parameters.push(cursor);
        cursor += width;
    }
    if in_order {
        parameters.reverse();
    }
    let types = &module.context.types;
    let (result, _, _) = module.signature(function.ty);
    // A float leaves in st(0), which no register names.
    let returns = if types.is_void(result) || matches!(types.get(result), Type::Float(_)) { Vec::new() } else { returned(size_of(module, layout, result)?) };
    let popped = if pops { cursor - first } else { 0 };
    Ok(Convention { parameters, returns, popped })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unselected(pub String);

/// A function's LIR, and the calls it makes: each call's callee by the
/// call's `at`, and which of them are far.
#[derive(Clone, Debug)]
pub struct Selected {
    pub body: LirBody,
    pub convention: Convention,
    pub calls: IndexMap<i64, String>,
    pub far: BTreeSet<i64>,
}

/// A call's contract, asked of the ABI that knows the callee: its name,
/// whether it pops its own arguments, and how many bytes were pushed.
pub type Contracts<'c> = &'c dyn Fn(&str, bool, i64) -> Result<Contract, String>;

/// The bytes a value of `ty` takes in a register.
fn width_of(module: &Module, layout: &DataLayout, ty: TypeId) -> Result<u32, Unselected> {
    let types = &module.context.types;
    match types.get(ty) {
        // An i1 is a byte holding 0 or 1, as LLVM stores one.
        Type::Int(1) => Ok(1),
        Type::Int(bits @ (8 | 16 | 32)) => Ok(bits / 8),
        Type::Pointer(space @ (0 | 2)) => Ok(layout.pointer(*space).bits / 8),
        // An x87 register holds any float, extended.
        Type::Float(FloatKind::Float | FloatKind::Double) => Ok(FLOAT),
        _ => refuse(format!("a {} value", types.display(ty))),
    }
}

/// The width of a float value in LIR: x87's extended precision.
const FLOAT: u32 = 10;

/// The most stores a memset expands to, as LLVM's x86 MaxStoresPerMemset.
const MEMSET_STORES: i64 = 16;

/// The bytes a value of `ty` takes in memory or on the stack: a far
/// pointer is its offset and selector, in two registers.
fn size_of(module: &Module, layout: &DataLayout, ty: TypeId) -> Result<u32, Unselected> {
    match module.context.types.get(ty) {
        Type::Pointer(1) => Ok(4),
        Type::Float(FloatKind::Float) => Ok(4),
        Type::Float(FloatKind::Double) => Ok(8),
        _ => width_of(module, layout, ty),
    }
}

fn refuse<T>(what: impl Into<String>) -> Result<T, Unselected> {
    Err(Unselected(what.into()))
}

/// Where a pointer points, when it need not be a register: a frame slot
/// is an addressing mode, and a constant offset is its displacement.
#[derive(Clone, Copy, Debug)]
enum Pointer {
    Frame(i64),
    Based { base: Held, offset: i64 },
    /// A near global's symbol, and a displacement from it.
    Global { space: Space, index: i64, offset: i64 },
    /// A far pointer's selector and offset, and a displacement from it.
    Far { selector: Held, base: Held, offset: i64 },
}

pub fn selected(module: &Module, name: &str, contracts: Contracts<'_>) -> Result<Selected, Unselected> {
    let Some(global) = module.named(name) else { return refuse(format!("no function @{name}")) };
    let Some(function) = module.global(global).function().filter(|one| !one.is_declaration()) else {
        return refuse(format!("@{name} has no body"));
    };
    let Some(layout) = module.datalayout.as_deref() else { return refuse("a module with no datalayout") };
    let layout = DataLayout::parse(layout).map_err(Unselected)?;
    let convention = convention(module, &layout, global)?;
    let mut selector = Selector {
        module,
        function,
        layout,
        values: IndexMap::default(),
        next: 0,
        pointers: IndexMap::default(),
        fars: IndexMap::default(),
        depth: 0,
        ats: IndexMap::default(),
        fused: BTreeSet::new(),
        pending: IndexMap::default(),
        phi_inputs: IndexMap::default(),
        edges: IndexMap::default(),
        chains: IndexMap::default(),
        pins: IndexMap::default(),
        inputs: BTreeSet::new(),
        contracts,
        calls: IndexMap::default(),
        far: BTreeSet::new(),
    };
    let body = selector.body(name, &convention)?;
    Ok(Selected { body, convention, calls: selector.calls, far: selector.far })
}

struct Selector<'m, 'c> {
    module: &'m Module,
    function: &'m Function,
    layout: DataLayout,
    values: IndexMap<ValueId, u32>,
    next: u32,
    pointers: IndexMap<ValueId, Pointer>,
    /// Each far pointer value's offset and selector, as LLVM's type
    /// legalizer expands a value no register holds into two.
    fars: IndexMap<ValueId, (Held, Held)>,
    depth: i64,
    ats: IndexMap<InstId, i64>,
    /// Comparisons a branch reads as flags, made beside it.
    fused: BTreeSet<InstId>,
    /// What each block computes for its successors' phis, before its terminator.
    pending: IndexMap<BlockId, Vec<Arc<Insn>>>,
    /// The register each (phi, predecessor) reads, where that block made it.
    phi_inputs: IndexMap<(InstId, BlockId), u32>,
    /// The LIR blocks each MIR edge leaves from: a switch's cases leave
    /// from blocks of their own.
    edges: IndexMap<(BlockId, BlockId), Vec<i64>>,
    /// The blocks a switch's compare chain adds after its own.
    chains: IndexMap<InstId, Vec<i64>>,
    pins: IndexMap<u32, Register>,
    inputs: BTreeSet<u32>,
    contracts: Contracts<'c>,
    calls: IndexMap<i64, String>,
    far: BTreeSet<i64>,
}

impl Selector<'_, '_> {
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
        for &block in layout {
            let from = block_at[&block];
            let terminator = function.terminator(block).expect("a terminator");
            if function.instruction(terminator).opcode == Opcode::Switch {
                let (default, cases) = self.cases(terminator);
                let mut leaving = from;
                for (index, &(_, target)) in cases.iter().enumerate() {
                    self.edge(block, target, leaving);
                    if index + 1 < cases.len() {
                        leaving = at;
                        at += 1;
                        self.chains.entry(terminator).or_default().push(leaving);
                    }
                }
                self.edge(block, default, leaving);
            } else {
                for successor in function.successors(block) {
                    self.edge(block, successor, from);
                }
            }
        }
        let entry = function.entry().expect("a body");
        let mut prologue = Vec::new();
        for (&parameter, &disp) in function.parameters().iter().zip(&convention.parameters) {
            // Only a used argument is loaded, as a DAG has no node for an unused one.
            if function.users(parameter).is_empty() {
                continue;
            }
            let ty = function.value(parameter).ty;
            if self.is_float(ty) {
                let (held, size) = (Held { value: self.value(parameter), width: FLOAT }, self.size(ty)?);
                self.float_loaded(held, "fld", Pointer::Frame(disp), size, false, block_at[&entry], &mut prologue);
                continue;
            }
            if self.is_far(function.value(parameter).ty) {
                let pair = self.far_loaded(Pointer::Frame(disp), false, block_at[&entry], &mut prologue);
                self.fars.insert(parameter, pair);
                continue;
            }
            let width = self.width(function.value(parameter).ty)?;
            let held = Held { value: self.value(parameter), width };
            let what = semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Mem(frame(disp, width))]);
            prologue.push(insn(block_at[&entry], what));
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
                    let incoming = self.incoming(inst)?;
                    self.width(instruction.ty)?;
                    phis.push(Phi { result: self.value(result), incoming });
                    continue;
                }
                if instruction.opcode.is_terminator() {
                    insns.extend(self.pending.shift_remove(&block).unwrap_or_default());
                }
                if instruction.opcode == Opcode::Switch {
                    self.switch(inst, &block_at, block_at[&block], std::mem::take(&mut insns), std::mem::take(&mut phis), &mut blocks)?;
                    break;
                }
                self.instruction(inst, &block_at, &mut insns, convention)?;
            }
            if function.instruction(function.terminator(block).expect("a terminator")).opcode == Opcode::Switch {
                continue;
            }
            let mut succ: Vec<i64> = Vec::new();
            for one in function.successors(block) {
                if !succ.contains(&block_at[&one]) {
                    succ.push(block_at[&one]);
                }
            }
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
        if !matches!(instruction.opcode, Opcode::ICmp(_) | Opcode::FCmp(_)) {
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

    /// Each LIR edge into the phi's block, and the register it brings.
    fn incoming(&mut self, inst: InstId) -> Result<Vec<(i64, u32)>, Unselected> {
        let function = self.function;
        let instruction = function.instruction(inst);
        let block = function.parent(inst).expect("a placed phi");
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for pair in instruction.operands.chunks(2) {
            let [value, Operand::Block(from)] = *pair else { unreachable!("a phi's pairs") };
            // A block that reaches this one by two edges is listed once per edge.
            if !seen.insert(from) {
                continue;
            }
            let held = match (self.phi_inputs.get(&(inst, from)), value) {
                (Some(&made), _) => made,
                (None, Operand::Value(one)) => self.value(one),
                (None, _) => unreachable!("phis_from made every other input"),
            };
            out.extend(self.edges[&(from, block)].iter().map(|&at| (at, held)));
        }
        Ok(out)
    }

    fn edge(&mut self, from: BlockId, to: BlockId, leaving: i64) {
        let ats = self.edges.entry((from, to)).or_default();
        if !ats.contains(&leaving) {
            ats.push(leaving);
        }
    }

    /// A switch's default, and each case that goes elsewhere.
    fn cases(&self, inst: InstId) -> (BlockId, Vec<(Operand, BlockId)>) {
        let operands = &self.function.instruction(inst).operands;
        let Operand::Block(default) = operands[1] else { unreachable!("a switch's default") };
        let cases = operands[2..]
            .chunks(2)
            .filter_map(|pair| match *pair {
                [value, Operand::Block(target)] if target != default => Some((value, target)),
                _ => None,
            })
            .collect();
        (default, cases)
    }

    /// A switch as a chain of compares, as SelectionDAGBuilder makes one
    /// short of a jump table: each case its own block, the last falling to
    /// the default.
    fn switch(
        &mut self,
        inst: InstId,
        block_at: &IndexMap<BlockId, i64>,
        from: i64,
        mut insns: Vec<Arc<Insn>>,
        mut phis: Vec<Phi>,
        blocks: &mut Vec<LirBlock>,
    ) -> Result<(), Unselected> {
        let at = self.ats[&inst];
        let operand = self.function.instruction(inst).operands[0];
        let ty = self.function.operand_type(&self.module.context, operand).expect("a typed value");
        let (default, cases) = self.cases(inst);
        if cases.is_empty() {
            insns.push(insn(at, jump(block_at[&default])));
            blocks.push(LirBlock { succ: vec![block_at[&default]], phis, ..LirBlock::new(from, insns) });
            return Ok(());
        }
        let value = Loc::Held(self.held(operand, ty, at, &mut insns)?);
        let chain: Vec<i64> = std::iter::once(from).chain(self.chains.get(&inst).cloned().unwrap_or_default()).collect();
        for (index, (case, target)) in cases.into_iter().enumerate() {
            let next = chain.get(index + 1).copied().unwrap_or(block_at[&default]);
            let case = self.source(case, ty, at, &mut insns)?;
            insns.push(insn(at, semantics(Operation::Compare, "cmp", vec![], vec![value.clone(), case])));
            let branch = Semantics { target: Some(block_at[&target]), ..semantics(Operation::Branch, "je", vec![], vec![]) };
            insns.push(insn(at, branch));
            let succ = vec![block_at[&target], next];
            blocks.push(LirBlock { succ, phis: std::mem::take(&mut phis), ..LirBlock::new(chain[index], std::mem::take(&mut insns)) });
        }
        Ok(())
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

    fn fresh_held(&mut self, width: u32) -> Held {
        Held { value: self.fresh(), width }
    }

    fn types(&self) -> &llrm_mir::Types {
        &self.module.context.types
    }

    /// A value's width in bytes, if a register holds it.
    fn width(&self, ty: TypeId) -> Result<u32, Unselected> {
        width_of(self.module, &self.layout, ty)
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
        let pointer = match operand {
            Operand::Value(value) => self.folded(value)?,
            _ => Some(self.global(operand)?),
        };
        match pointer {
            None => {
                let Operand::Value(value) = operand else { unreachable!("a constant is folded") };
                Ok(Held { value: self.value(value), width })
            }
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
            // A far pointer's offset.
            Pointer::Based { base, offset } | Pointer::Far { base, offset, .. } => {
                let step = Loc::Imm(Imm { value: offset, width: held.width, address: None });
                semantics(Operation::Binary, "add", vec![Loc::Held(held)], vec![Loc::Held(base), step])
            }
            Pointer::Global { space, index, offset } => {
                let symbol = Loc::Imm(Imm { value: 0, width: held.width, address: Some(Addr { index, ..Addr::new(space, offset) }) });
                semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![symbol])
            }
        }
    }

    /// Where a pointer operand points.
    fn pointer(&mut self, operand: Operand) -> Result<Pointer, Unselected> {
        let Operand::Value(value) = operand else { return self.global(operand) };
        if let Some(pointer) = self.folded(value)? {
            return Ok(pointer);
        }
        let ty = self.function.value(value).ty;
        if let Some(&(base, selector)) = self.fars.get(&value) {
            return Ok(Pointer::Far { selector, base, offset: 0 });
        }
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
        let pointer = self.pointer(instruction.operands[0])?.moved(offset);
        self.pointers.insert(value, pointer);
        Ok(Some(pointer))
    }

    /// A near global's address, and a constant displacement from it.
    fn global(&self, operand: Operand) -> Result<Pointer, Unselected> {
        let Operand::Constant(id) = operand else { unreachable!("a constant") };
        let (global, offset) = crate::backend::globals::target(self.module, &self.layout, id).map_err(Unselected)?;
        if self.module.global(global).address_space != 0 {
            return refuse("a far global");
        }
        Ok(Pointer::Global { space: crate::backend::globals::space(self.module, global), index: i64::from(global.0), offset })
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
        // A far pointer's index is its 16-bit offset, as `p1:32:16:16:16` says.
        let far = self.is_far(instruction.ty);
        let width = if far { 2 } else { self.width(instruction.ty)? };
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
        let pointer = self.pointer(instruction.operands[0])?;
        let start = match pointer {
            Pointer::Based { base, offset: 0 } | Pointer::Far { base, offset: 0, .. } if offset == 0 => base,
            pointer => {
                let moved = pointer.moved(offset as i64);
                let start = Held { value: self.fresh(), width };
                out.push(insn(at, self.address(moved, start)));
                start
            }
        };
        let address = instruction.result.expect("an address");
        let result = match pointer {
            Pointer::Far { selector, .. } if far => {
                let offset = self.fresh_held(2);
                self.fars.insert(address, (offset, selector));
                offset
            }
            _ => Held { value: self.value(address), width },
        };
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
            Pointer::Global { space, index, offset } => Mem { disp_width: 2, ..Mem::new(Some(Addr { index, ..Addr::new(space, offset) }), width) },
            Pointer::Based { base, offset } => Mem { base: Some(base), offset, ..Mem::new(None, width) },
            Pointer::Far { selector, base, offset } => Mem {
                offset,
                disp_width: 2,
                base: Some(base),
                selector: Some(selector),
                ..Mem::new(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, offset) }), width)
            },
        }
    }

    fn is_far(&self, ty: TypeId) -> bool {
        matches!(self.types().get(ty), Type::Pointer(1))
    }

    fn is_float(&self, ty: TypeId) -> bool {
        matches!(self.types().get(ty), Type::Float(_))
    }

    /// A value's bytes in memory.
    fn size(&self, ty: TypeId) -> Result<u32, Unselected> {
        size_of(self.module, &self.layout, ty)
    }

    /// A far pointer's two words at `pointer`, offset first.
    fn far_loaded(&mut self, pointer: Pointer, volatile: bool, at: i64, out: &mut Vec<Arc<Insn>>) -> (Held, Held) {
        let (offset, selector) = (self.fresh_held(2), self.fresh_held(2));
        for (held, by) in [(offset, 0), (selector, 2)] {
            let what = semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Mem(Self::memory(pointer.moved(by), 2))]);
            out.push(Arc::new(Insn { volatile, ..insn_of(at, what) }));
        }
        (offset, selector)
    }

    /// A far pointer operand's offset and selector, each in a register.
    fn far(&mut self, operand: Operand, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(Held, Held), Unselected> {
        match self.pointer(operand)? {
            Pointer::Far { selector, base, offset: 0 } => Ok((base, selector)),
            pointer @ Pointer::Far { selector, .. } => {
                let moved = self.fresh_held(2);
                out.push(insn(at, self.address(pointer, moved)));
                Ok((moved, selector))
            }
            _ => refuse("a far constant"),
        }
    }

    /// A cast to or from a far pointer: its halves taken apart or put
    /// together. A near pointer is into DGROUP; a segment is offset 0.
    fn far_cast(&mut self, op: CastOp, inst: InstId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let instruction = self.function.instruction(inst);
        let (operand, to) = (instruction.operands[0], instruction.ty);
        let from = self.function.operand_type(&self.module.context, operand).expect("a typed operand");
        let result = instruction.result.expect("a cast's value");
        let word = |value| Loc::Imm(Imm { value, width: 2, address: None });
        let mov = |into: Held, from: Loc| insn(at, semantics(Operation::Move, "mov", vec![Loc::Held(into)], vec![from]));
        if self.is_far(to) {
            let pair = match (op, self.types().get(from).clone()) {
                (CastOp::AddrSpaceCast, Type::Pointer(0)) => {
                    let offset = self.held(operand, from, at, out)?;
                    let selector = self.fresh_held(2);
                    let (space, index) = crate::hir::lower::DGROUP;
                    out.push(mov(selector, Loc::Imm(Imm { value: 0, width: 2, address: Some(Addr { index, ..Addr::new(space, 0) }) })));
                    (offset, selector)
                }
                (CastOp::AddrSpaceCast, Type::Pointer(2)) => {
                    let selector = self.held(operand, from, at, out)?;
                    let offset = self.fresh_held(2);
                    out.push(mov(offset, word(0)));
                    (offset, selector)
                }
                (CastOp::IntToPtr, Type::Int(32)) => {
                    let dword = self.held(operand, from, at, out)?;
                    let top = self.fresh_held(4);
                    let sixteen = Loc::Imm(Imm { value: 16, width: 1, address: None });
                    out.push(insn(at, semantics(Operation::Binary, "shr", vec![Loc::Held(top)], vec![Loc::Held(dword), sixteen])));
                    (Held { width: 2, ..dword }, Held { width: 2, ..top })
                }
                _ => return refuse(format!("{op:?} to a far pointer")),
            };
            self.fars.insert(result, pair);
            return Ok(());
        }
        let (offset, selector) = self.far(operand, at, out)?;
        match (op, self.types().get(to).clone()) {
            (CastOp::AddrSpaceCast, Type::Pointer(2)) => out.push(mov(Held { value: self.value(result), width: 2 }, Loc::Held(selector))),
            (CastOp::AddrSpaceCast, Type::Pointer(0)) => out.push(mov(Held { value: self.value(result), width: 2 }, Loc::Held(offset))),
            (CastOp::PtrToInt, Type::Int(32)) => {
                let joined = Held { value: self.value(result), width: 4 };
                self.joined(joined, offset, selector, at, out);
            }
            _ => return refuse(format!("{op:?} of a far pointer")),
        }
        Ok(())
    }

    /// A conversion to, from or between floats, through a stack temporary
    /// where x87 reads or writes only memory: `fild`, `fistp`, and a
    /// narrowing's `fstp`, as LLVM's x87 lowering goes through the stack.
    fn float_cast(&mut self, op: CastOp, inst: InstId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let instruction = self.function.instruction(inst);
        let (operand, to) = (instruction.operands[0], instruction.ty);
        let from = self.function.operand_type(&self.module.context, operand).expect("a typed operand");
        let result = instruction.result.expect("a cast's value");
        match op {
            // Exact: the register already holds it extended.
            CastOp::FPExt => {
                let held = self.float(operand)?;
                self.values.insert(result, held.value);
            }
            CastOp::FPTrunc => {
                let held = self.float(operand)?;
                let cell = self.float_stored(held, "fstp", 4, at, out);
                let into = Held { value: self.value(result), width: FLOAT };
                self.float_loaded(into, "fld", cell, 4, false, at, out);
            }
            CastOp::SIToFP => {
                let mut held = self.held(operand, from, at, out)?;
                // fild reads a word, a dword or a qword.
                if held.width == 1 {
                    let word = self.fresh_held(2);
                    out.push(insn(at, semantics(Operation::Extend, "movsx", vec![Loc::Held(word)], vec![Loc::Held(held)])));
                    held = word;
                }
                let cell = self.temporary(i64::from(held.width));
                out.push(insn(at, semantics(Operation::Move, "mov", vec![Loc::Mem(Self::memory(cell, held.width))], vec![Loc::Held(held)])));
                let into = Held { value: self.value(result), width: FLOAT };
                self.float_loaded(into, "fild", cell, held.width, false, at, out);
            }
            CastOp::FPToSI => self.float_to_integer(operand, "fisttp", result, to, at, out)?,
            _ => return refuse(format!("{op:?} of a float")),
        }
        Ok(())
    }

    /// A float stored as an integer of `to` by `name`, and loaded back.
    fn float_to_integer(&mut self, operand: Operand, name: &str, result: ValueId, to: TypeId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let width = self.width(to)?;
        if width == 1 {
            return refuse("a float to a byte");
        }
        let held = self.float(operand)?;
        let cell = self.float_stored(held, name, width, at, out);
        let into = Held { value: self.value(result), width };
        out.push(insn(at, semantics(Operation::Move, "mov", vec![Loc::Held(into)], vec![Loc::Mem(Self::memory(cell, width))])));
        Ok(())
    }

    /// `into` made of a low and a high word.
    fn joined(&mut self, into: Held, low: Held, high: Held, at: i64, out: &mut Vec<Arc<Insn>>) {
        let (wide_low, wide_high, shifted) = (self.fresh_held(4), self.fresh_held(4), self.fresh_held(4));
        let sixteen = Loc::Imm(Imm { value: 16, width: 1, address: None });
        for what in [
            semantics(Operation::Extend, "movzx", vec![Loc::Held(wide_low)], vec![Loc::Held(low)]),
            semantics(Operation::Extend, "movzx", vec![Loc::Held(wide_high)], vec![Loc::Held(high)]),
            semantics(Operation::Binary, "shl", vec![Loc::Held(shifted)], vec![Loc::Held(wide_high), sixteen]),
            semantics(Operation::Binary, "or", vec![Loc::Held(into)], vec![Loc::Held(shifted), Loc::Held(wide_low)]),
        ] {
            out.push(insn(at, what));
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
            Opcode::Load { volatile, .. } if self.is_float(instruction.ty) => {
                let (pointer, size) = (self.pointer(operands[0])?, self.size(instruction.ty)?);
                let held = Held { value: self.value(instruction.result.expect("a load's value")), width: FLOAT };
                self.float_loaded(held, "fld", pointer, size, *volatile, at, out);
            }
            Opcode::Store { volatile, .. } if self.is_float(type_of(operands[0])) => {
                let (pointer, size) = (self.pointer(operands[1])?, self.size(type_of(operands[0]))?);
                let what = match operands[0] {
                    Operand::Constant(id) => {
                        // A constant is its bits, stored as integers are.
                        let ConstantKind::Float(bits) = self.module.context.get(id).kind else { return refuse("a float constant of no bits") };
                        let bits = if size == 4 { u128::from(bits as u32) } else { u128::from(bits) };
                        let low = semantics(Operation::Move, "mov", vec![Loc::Mem(Self::memory(pointer, 4))], vec![Loc::Imm(Imm { value: bits as u32 as i64, width: 4, address: None })]);
                        if size == 8 {
                            out.push(Arc::new(Insn { volatile: *volatile, ..insn_of(at, low) }));
                            let high = Loc::Imm(Imm { value: (bits >> 32) as u32 as i64, width: 4, address: None });
                            semantics(Operation::Move, "mov", vec![Loc::Mem(Self::memory(pointer.moved(4), 4))], vec![high])
                        } else {
                            low
                        }
                    }
                    value => {
                        let held = self.float(value)?;
                        semantics(Operation::FloatStore, "fstp", vec![Loc::Mem(Self::memory(pointer, size))], vec![Loc::Held(held)])
                    }
                };
                out.push(Arc::new(Insn { volatile: *volatile, ..insn_of(at, what) }));
            }
            Opcode::Binary(op @ (BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv)) => {
                let name = match op {
                    BinaryOp::FAdd => "fadd",
                    BinaryOp::FSub => "fsub",
                    BinaryOp::FMul => "fmul",
                    _ => "fdiv",
                };
                let (a, b) = (self.float(operands[0])?, self.float(operands[1])?);
                let result = Held { value: self.value(instruction.result.expect("a result")), width: FLOAT };
                out.push(insn(at, semantics(Operation::FloatArith, name, vec![Loc::Held(result)], vec![Loc::Held(a), Loc::Held(b)])));
            }
            Opcode::FNeg => {
                let a = self.float(operands[0])?;
                let result = Held { value: self.value(instruction.result.expect("a result")), width: FLOAT };
                out.push(insn(at, semantics(Operation::FloatUnary, "fchs", vec![Loc::Held(result)], vec![Loc::Held(a)])));
            }
            Opcode::Cast(op) if self.is_float(instruction.ty) || self.is_float(type_of(operands[0])) => self.float_cast(*op, inst, at, out)?,
            Opcode::Load { volatile, .. } if self.is_far(instruction.ty) => {
                let pointer = self.pointer(operands[0])?;
                let pair = self.far_loaded(pointer, *volatile, at, out);
                self.fars.insert(instruction.result.expect("a load's value"), pair);
            }
            Opcode::Store { volatile, .. } if self.is_far(type_of(operands[0])) => {
                let (offset, selector) = self.far(operands[0], at, out)?;
                let pointer = self.pointer(operands[1])?;
                for (held, by) in [(offset, 0), (selector, 2)] {
                    let what = semantics(Operation::Move, "mov", vec![Loc::Mem(Self::memory(pointer.moved(by), 2))], vec![Loc::Held(held)]);
                    out.push(Arc::new(Insn { volatile: *volatile, ..insn_of(at, what) }));
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
                if self.types().int_bits(ty) == Some(1) && !matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) {
                    return refuse("arithmetic on an i1");
                }
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
            Opcode::Cast(op) if self.is_far(instruction.ty) || self.is_far(type_of(operands[0])) => self.far_cast(*op, inst, at, out)?,
            Opcode::Cast(op) => {
                let from = type_of(operands[0]);
                let (to, from_width) = (self.width(instruction.ty)?, self.width(from)?);
                let boolean = self.types().int_bits(from) == Some(1);
                let result = Held { value: self.value(instruction.result.expect("a result")), width: to };
                let what = match op {
                    CastOp::Trunc if self.types().int_bits(instruction.ty) == Some(1) => {
                        let source = self.held(operands[0], from, at, out)?;
                        let one = Loc::Imm(Imm { value: 1, width: 1, address: None });
                        semantics(Operation::Binary, "and", vec![Loc::Held(result)], vec![Loc::Held(Held { width: 1, ..source }), one])
                    }
                    CastOp::Trunc => {
                        let source = self.held(operands[0], from, at, out)?;
                        semantics(Operation::Move, "mov", vec![Loc::Held(result)], vec![Loc::Held(Held { width: to, ..source })])
                    }
                    // An i1's byte is already 0 or 1: its sign extension is its negation.
                    CastOp::SExt | CastOp::ZExt if boolean => {
                        let source = self.held(operands[0], from, at, out)?;
                        let widened = if *op == CastOp::SExt { Held { value: self.fresh(), width: to } } else { result };
                        let what = if to == 1 {
                            semantics(Operation::Move, "mov", vec![Loc::Held(widened)], vec![Loc::Held(source)])
                        } else {
                            semantics(Operation::Extend, "movzx", vec![Loc::Held(widened)], vec![Loc::Held(source)])
                        };
                        if *op == CastOp::ZExt {
                            what
                        } else {
                            out.push(insn(at, what));
                            semantics(Operation::Unary, "neg", vec![Loc::Held(result)], vec![Loc::Held(widened)])
                        }
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
            Opcode::ICmp(_) | Opcode::FCmp(_) if self.fused.contains(&inst) => {}
            // SETcc, as LLVM selects a comparison it keeps as a value.
            Opcode::ICmp(_) | Opcode::FCmp(_) => {
                let code = self.compare(inst, at, out)?;
                let result = Held { value: self.value(instruction.result.expect("a result")), width: 1 };
                let name = format!("set{}", &code[1..]);
                out.push(insn(at, semantics(Operation::Unary, &name, vec![Loc::Held(result)], vec![])));
            }
            Opcode::Br => match operands[..] {
                [Operand::Block(target)] => {
                    out.push(insn(at, jump(block_at[&target])));
                }
                [Operand::Value(_), Operand::Block(taken), Operand::Block(otherwise)] if taken == otherwise => {
                    out.push(insn(at, jump(block_at[&taken])));
                }
                [Operand::Value(condition), Operand::Block(taken), Operand::Block(_)] => {
                    let code = match self.fused_compare(condition) {
                        Some(compare) => self.compare(compare, at, out)?,
                        None => {
                            let tested = Loc::Held(Held { value: self.value(condition), width: 1 });
                            let zero = Loc::Imm(Imm { value: 0, width: 1, address: None });
                            out.push(insn(at, semantics(Operation::Compare, "cmp", vec![], vec![tested, zero])));
                            "jne"
                        }
                    };
                    let branch = Semantics { target: Some(block_at[&taken]), ..semantics(Operation::Branch, code, vec![], vec![]) };
                    out.push(insn(at, branch));
                }
                _ => return refuse("a branch on a constant"),
            },
            Opcode::Ret => {
                let what = semantics(Operation::Return, "", vec![], vec![]);
                let mut one = Insn { reads_complete: true, ..Insn::new(at, Some((at, at)), Some(what), vec![], vec![]) };
                if let Some(&value) = operands.first().filter(|&&one| self.is_float(type_of(one))) {
                    // Left in st(0).
                    let held = self.float(value)?;
                    out.push(insn(at, semantics(Operation::FloatStore, "", vec![], vec![Loc::Held(held)])));
                } else if let Some(&value) = operands.first().filter(|&&one| self.is_far(type_of(one))) {
                    let (offset, selector) = self.far(value, at, out)?;
                    let [low, high] = convention.returns[..] else { return refuse("a far result the convention has no pair for") };
                    one.requires = vec![(offset, low), (selector, high)];
                    one.uses = vec![offset.value, selector.value];
                } else if let Some(&value) = operands.first() {
                    let held = self.held(value, type_of(value), at, out)?;
                    one.requires = match convention.returns[..] {
                        [register] => vec![(held, register)],
                        // A dword result in a word pair: its low word, and its high word shifted down.
                        [low, high] if held.width == 4 => {
                            let top = Held { value: self.fresh(), width: 4 };
                            let sixteen = Loc::Imm(Imm { value: 16, width: 1, address: None });
                            out.push(insn(at, semantics(Operation::Binary, "shr", vec![Loc::Held(top)], vec![Loc::Held(held), sixteen])));
                            vec![(Held { width: 2, ..held }, low), (Held { width: 2, ..top }, high)]
                        }
                        _ => return refuse("a result the convention has no registers for"),
                    };
                    one.uses = one.requires.iter().map(|(held, _)| held.value).collect();
                }
                out.push(Arc::new(one));
            }
            Opcode::Call(info) => self.call(inst, info.calling_convention, at, out)?,
            // Nothing runs after it: the block ends with what came before.
            Opcode::Unreachable => {}
            _ => return refuse(instruction.opcode.mnemonic()),
        }
        Ok(())
    }

    /// A direct call: its arguments pushed as its convention orders them,
    /// its result delivered in ax or dx:ax, and what its contract says it
    /// destroys and who pops.
    fn call(&mut self, inst: InstId, convention: u32, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let function = self.function;
        let instruction = function.instruction(inst);
        let (callee, arguments) = instruction.operands.split_last().expect("a callee");
        let Operand::Constant(callee) = *callee else { return refuse("an indirect call") };
        let ConstantKind::Global(global) = self.module.context.get(callee).kind else { return refuse("a call of a constant") };
        let global = self.module.global(global);
        let name = global.name.clone().unwrap_or_default();
        if llrm_mir::intrinsics::is_reserved(&name) {
            return match Intrinsic::named(&name) {
                Some(Intrinsic::MemSet) => self.memset(arguments, at, out),
                Some(Intrinsic::Unary(function)) => {
                    let name = match function {
                        FloatFunction::Fabs => "fabs",
                        FloatFunction::Sqrt => "fsqrt",
                        FloatFunction::Rint => "frndint",
                        FloatFunction::Sin => "fsin",
                        FloatFunction::Cos => "fcos",
                        other => return refuse(format!("{other:?}")),
                    };
                    let a = self.float(arguments[0])?;
                    let result = Held { value: self.value(instruction.result.expect("a result")), width: FLOAT };
                    out.push(insn(at, semantics(Operation::FloatUnary, name, vec![Loc::Held(result)], vec![Loc::Held(a)])));
                    Ok(())
                }
                // Rounds as the machine's default mode does: fistp.
                Some(Intrinsic::LRint) => {
                    let result = instruction.result.expect("lrint's value");
                    self.float_to_integer(arguments[0], "fistp", result, instruction.ty, at, out)
                }
                _ => refuse(format!("@{name}")),
            };
        }
        let far = far(global)?;
        let Passing { in_order, pops } = passing(convention)?;
        let mut order: Vec<usize> = (0..arguments.len()).collect();
        if !in_order {
            order.reverse();
        }
        let mut pushed = 0;
        for index in order {
            let argument = arguments[index];
            let ty = function.operand_type(&self.module.context, argument).expect("a typed argument");
            if self.is_float(ty) {
                // Its bytes from a stack temporary, the high dword pushed first.
                let size = self.size(ty)?;
                let held = self.float(argument)?;
                let cell = self.float_stored(held, "fstp", size, at, out);
                for by in (0..i64::from(size) / 4).rev().map(|dword| dword * 4) {
                    out.push(insn(at, semantics(Operation::Push, "push", vec![], vec![Loc::Mem(Self::memory(cell.moved(by), 4))])));
                }
                pushed += i64::from(size);
                continue;
            }
            if self.is_far(ty) {
                // Its offset at the lower address: the selector pushed first.
                let (offset, selector) = self.far(argument, at, out)?;
                for held in [selector, offset] {
                    out.push(insn(at, semantics(Operation::Push, "push", vec![], vec![Loc::Held(held)])));
                }
                pushed += 4;
                continue;
            }
            let mut held = self.held(argument, ty, at, out)?;
            if held.width == 1 {
                let word = Held { value: self.fresh(), width: 2 };
                out.push(insn(at, semantics(Operation::Extend, "movzx", vec![Loc::Held(word)], vec![Loc::Held(held)])));
                held = word;
            }
            pushed += slot(held.width);
            out.push(insn(at, semantics(Operation::Push, "push", vec![], vec![Loc::Held(held)])));
        }
        let contract = (self.contracts)(&name, pops, pushed).map_err(Unselected)?;
        let mut delivers = Vec::new();
        let mut result = None;
        let mut float = None;
        if let Some(value) = instruction.result.filter(|_| self.is_float(instruction.ty)) {
            float = Some(Held { value: self.value(value), width: FLOAT });
        } else if let Some(value) = instruction.result.filter(|_| self.is_far(instruction.ty)) {
            let (offset, selector) = (self.fresh_held(2), self.fresh_held(2));
            delivers = vec![(offset, Register::EAX), (selector, Register::EDX)];
            self.fars.insert(value, (offset, selector));
        } else if let Some(value) = instruction.result {
            let width = self.width(instruction.ty)?;
            let held = Held { value: self.value(value), width };
            match returned(width)[..] {
                // dx:ax, joined into the dword register the value lives in.
                [low_register, high_register] => {
                    let (low, high) = (Held { value: self.fresh(), width: 2 }, Held { value: self.fresh(), width: 2 });
                    delivers = vec![(low, low_register), (high, high_register)];
                    result = Some((held, low, high));
                }
                ref registers => delivers = vec![(held, registers[0])],
            }
        }
        let what = semantics(Operation::Call, "call", vec![], vec![]);
        out.push(Arc::new(Insn {
            clobbers: call_clobbers(&contract),
            clobbers_high: call_clobbered_high(&contract),
            defines: delivers.iter().map(|(held, _)| held.value).collect(),
            delivers,
            ..Insn::new(at, Some((at, at)), Some(what), vec![], vec![])
        }));
        // A float result is in st(0).
        if let Some(held) = float {
            out.push(insn(at, semantics(Operation::FloatLoad, "", vec![Loc::Held(held)], vec![])));
        }
        self.calls.insert(at, name);
        if far {
            self.far.insert(at);
        }
        if contract.caller_cleanup > 0 {
            let sp = Loc::Reg(Reg { register: Register::SP, width: 2 });
            let count = Loc::Imm(Imm { value: contract.caller_cleanup, width: 2, address: None });
            out.push(insn(at, semantics(Operation::Binary, "add", vec![sp.clone()], vec![sp, count])));
        }
        if let Some((held, low, high)) = result {
            self.joined(held, low, high, at, out);
        }
        Ok(())
    }

    /// A memset of a constant byte over a constant length, as LLVM's
    /// getMemset lowers one: at most `MEMSET_STORES` stores, widest first,
    /// or `rep stosd` through es:di and stores for the tail.
    fn memset(&mut self, arguments: &[Operand], at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let &[destination, value, length, volatile] = arguments else { return refuse("a memset of other than four operands") };
        let (Some(byte), Some(length)) = (self.constant(value, 1), self.constant(length, 2)) else {
            return refuse("a memset of a variable byte or length");
        };
        let volatile = self.constant(volatile, 1) != Some(0);
        let pattern = |width: u32| (0..width).fold(0i64, |word, _| (word << 8) | (byte & 0xFF));
        let pointer = self.pointer(destination)?;
        let (bulk, tail) = if length / 4 + (length % 4).count_ones() as i64 > MEMSET_STORES { (length / 4, length % 4) } else { (0, length) };
        if bulk > 0 {
            let segment = Loc::Reg(Reg { register: Register::ES, width: 2 });
            let source = if matches!(pointer, Pointer::Frame(_)) { Register::SS } else { Register::DS };
            let (stored, count, through) = (self.fresh_held(4), self.fresh_held(2), self.fresh_held(2));
            let (stepped, emptied) = (self.fresh_held(2), self.fresh_held(2));
            for what in [
                semantics(Operation::Move, "mov", vec![Loc::Held(stored)], vec![Loc::Imm(Imm { value: pattern(4), width: 4, address: None })]),
                semantics(Operation::Move, "mov", vec![Loc::Held(count)], vec![Loc::Imm(Imm { value: bulk, width: 2, address: None })]),
                self.address(pointer, through),
                semantics(Operation::Push, "push", vec![], vec![segment.clone()]),
                semantics(Operation::Push, "push", vec![], vec![Loc::Reg(Reg { register: source, width: 2 })]),
                semantics(Operation::Pop, "pop", vec![segment.clone()], vec![]),
                semantics(
                    Operation::Fill,
                    "stosd",
                    vec![Loc::Mem(Mem::new(None, 0)), Loc::Held(stepped), Loc::Held(emptied)],
                    vec![Loc::Held(stored), Loc::Held(count), Loc::Held(through), segment.clone()],
                ),
                semantics(Operation::Pop, "pop", vec![segment], vec![]),
            ] {
                out.push(Arc::new(Insn { volatile, ..insn_of(at, what) }));
            }
        }
        let mut offset = length - tail;
        for width in [4, 2, 1] {
            while length - offset >= i64::from(width) {
                let cell = Self::memory(pointer.moved(offset), width);
                let what = semantics(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![Loc::Imm(Imm { value: pattern(width), width, address: None })]);
                out.push(Arc::new(Insn { volatile, ..insn_of(at, what) }));
                offset += i64::from(width);
            }
        }
        Ok(())
    }

    /// `cmp` of a comparison's operands, a constant second; the predicate
    /// that holds of them as ordered.
    fn compare(&mut self, inst: InstId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<&'static str, Unselected> {
        let instruction = self.function.instruction(inst);
        if let Opcode::FCmp(predicate) = instruction.opcode {
            return self.float_compare(predicate, inst, at, out);
        }
        let Opcode::ICmp(mut predicate) = instruction.opcode else { unreachable!("a comparison") };
        let (mut a, mut b) = (instruction.operands[0], instruction.operands[1]);
        let ty = self.function.operand_type(&self.module.context, a).expect("a typed operand");
        if matches!(a, Operand::Constant(_)) {
            std::mem::swap(&mut a, &mut b);
            predicate = swapped(predicate);
        }
        let a = Loc::Held(self.held(a, ty, at, out)?);
        let b = self.source(b, ty, at, out)?;
        out.push(insn(at, semantics(Operation::Compare, "cmp", vec![], vec![a, b])));
        Ok(condition_code(predicate))
    }

    /// `fcom`, its answer in the flags as sahf leaves it, as an unsigned
    /// compare's: `a > b` is `ja`, and false when unordered. A less-than
    /// compares the other way, as LLVM's x87 lowering does, so that it is
    /// false when unordered too.
    fn float_compare(&mut self, predicate: FloatPredicate, inst: InstId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<&'static str, Unselected> {
        let instruction = self.function.instruction(inst);
        let (a, b) = (instruction.operands[0], instruction.operands[1]);
        let ((a, b), code) = match predicate {
            FloatPredicate::Ogt => ((a, b), "ja"),
            FloatPredicate::Oge => ((a, b), "jae"),
            FloatPredicate::Olt => ((b, a), "ja"),
            FloatPredicate::Ole => ((b, a), "jae"),
            other => return refuse(format!("fcmp {other:?}")),
        };
        let (a, b) = (self.float(a)?, self.float(b)?);
        out.push(insn(at, semantics(Operation::Compare, "fcom", vec![], vec![Loc::Held(a), Loc::Held(b)])));
        Ok(code)
    }

    /// A float operand, in an x87 register.
    fn float(&mut self, operand: Operand) -> Result<Held, Unselected> {
        match operand {
            Operand::Value(value) => Ok(Held { value: self.value(value), width: FLOAT }),
            _ => refuse("a float constant operand"),
        }
    }

    /// A fresh frame cell of `size` bytes, as a DAG's stack temporary.
    fn temporary(&mut self, size: i64) -> Pointer {
        self.depth += size + size % 2;
        Pointer::Frame(-self.depth)
    }

    /// A float's `size` bytes stored to a stack temporary, where a push or
    /// an integer load reads them: `fstp`, or `fistp` as an integer.
    fn float_stored(&mut self, held: Held, name: &str, size: u32, at: i64, out: &mut Vec<Arc<Insn>>) -> Pointer {
        let cell = self.temporary(i64::from(size));
        out.push(insn(at, semantics(Operation::FloatStore, name, vec![Loc::Mem(Self::memory(cell, size))], vec![Loc::Held(held)])));
        cell
    }

    /// A float loaded from `size` bytes at `pointer`: `fld`, or `fild` of an integer.
    fn float_loaded(&mut self, into: Held, name: &str, pointer: Pointer, size: u32, volatile: bool, at: i64, out: &mut Vec<Arc<Insn>>) {
        let what = semantics(Operation::FloatLoad, name, vec![Loc::Held(into)], vec![Loc::Mem(Self::memory(pointer, size))]);
        out.push(Arc::new(Insn { volatile, ..insn_of(at, what) }));
    }

    fn fused_compare(&self, condition: ValueId) -> Option<InstId> {
        match self.function.value(condition).def {
            ValueDef::Instruction(inst) if self.fused.contains(&inst) => Some(inst),
            _ => None,
        }
    }
}

impl Pointer {
    fn moved(self, by: i64) -> Pointer {
        match self {
            Pointer::Frame(disp) => Pointer::Frame(disp + by),
            Pointer::Based { base, offset } => Pointer::Based { base, offset: offset + by },
            Pointer::Global { space, index, offset } => Pointer::Global { space, index, offset: offset + by },
            Pointer::Far { selector, base, offset } => Pointer::Far { selector, base, offset: offset + by },
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
