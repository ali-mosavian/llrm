//! Instruction selection from MIR: each function's instructions as LIR over
//! virtual registers, as LLVM's SelectionDAG makes MachineInstrs. It chooses
//! instructions and addressing only; two-address form, registers and the
//! frame's size are the machine phases'.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;
use llrm_analysis::cfg;
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::ranges::{self, Facts};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand, ValueDef, ValueId};
use llrm_mir::intrinsics::{FloatFunction, Intrinsic};
use llrm_mir::{BinaryOp, CastOp, ConstantKind, FloatKind, FloatPredicate, GlobalId, IntPredicate, Module, Opcode, Type, TypeId};

use crate::abi::runtime::Contract;
use crate::backend::constpool::{self, Pool};
use crate::backend::cpu::Profile;
use crate::backend::target::Segments;
use crate::backend::{addressforms, arithmetic, division};
use crate::backend::lower::{_read, _written, call_clobbered_high, call_clobbers};
use crate::model::ir::{Addr, Address, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi};
use crate::model::passes::AddressForm;
use crate::support::hash::IndexMap;

mod combined;
mod wide;

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

/// The registers a result of `width` leaves in: a dword in DX:AX, an i64
/// in EDX:EAX.
fn returned(width: u32) -> Vec<Register> {
    if matches!(width, 4 | 8) { vec![Register::EAX, Register::EDX] } else { vec![Register::EAX] }
}

/// Whether a function's code is far: in addrspace(1), entered by a far call.
pub fn far(global: &GlobalValue) -> Result<bool, Unselected> {
    match global.address_space {
        0 => Ok(false),
        1 => Ok(true),
        other => refuse(format!("code in address space {other}")),
    }
}

/// Refuses an argument passed in the frame's own bytes: its slot would be
/// addressable, which `sealed_arguments` denies, and the bytes are not what
/// a push of the pointer passes.
fn in_the_frame(attributes: &[Vec<llrm_mir::Attribute>]) -> Result<(), Unselected> {
    for attribute in attributes.iter().flatten() {
        if let llrm_mir::Attribute::Type(name, _) = attribute {
            if matches!(name.as_str(), "byval" | "inalloca" | "byref") {
                return refuse(format!("a {name} argument"));
            }
        }
    }
    Ok(())
}

/// `function`'s convention: the return address, BP, then its arguments,
/// the last pushed nearest.
fn convention(module: &Module, layout: &DataLayout, global: GlobalId) -> Result<Convention, Unselected> {
    let global = module.global(global);
    let Some(function) = global.function() else { return refuse("a variable has no convention") };
    in_the_frame(&function.parameter_attrs)?;
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
    /// The code laid down in place of each call to an inline helper.
    pub inline: IndexMap<i64, Vec<u8>>,
    pub far: BTreeSet<i64>,
    /// The bytes below BP its allocas and stack temporaries take: an
    /// indexed access names no frame slot the frame could find it by.
    pub depth: i64,
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

/// The conditions of the flags that answer a comparison: one, both of two,
/// or either of two.
#[derive(Clone, Copy, Debug)]
enum Test {
    One(&'static str),
    Both(&'static str, &'static str),
    Either(&'static str, &'static str),
}

/// How `fcom`'s answer, in the flags as sahf leaves it, decides each float
/// predicate, as LLVM's x87 lowering does: whether the operands are
/// compared the other way, and the conditions. Its flags are an unsigned
/// compare's, and unordered sets ZF, PF and CF: `a > b` is `ja`, false
/// when unordered, and a less-than compares the other way to stay false.
/// An unordered predicate is the carry or zero that unordered also sets:
/// `a < b` or unordered is `jb`, as the old route branched on each BASIC
/// comparison (`lower::_unordered`).
const FLOAT_CONDITIONS: [(FloatPredicate, bool, Test); 12] = [
    (FloatPredicate::Ogt, false, Test::One("ja")),
    (FloatPredicate::Oge, false, Test::One("jae")),
    (FloatPredicate::Olt, true, Test::One("ja")),
    (FloatPredicate::Ole, true, Test::One("jae")),
    (FloatPredicate::Oeq, false, Test::Both("je", "jnp")),
    (FloatPredicate::Une, false, Test::Either("jne", "jp")),
    (FloatPredicate::Ult, false, Test::One("jb")),
    (FloatPredicate::Ule, false, Test::One("jbe")),
    (FloatPredicate::Ugt, true, Test::One("jb")),
    (FloatPredicate::Uge, true, Test::One("jbe")),
    (FloatPredicate::Ueq, false, Test::One("je")),
    (FloatPredicate::One, false, Test::One("jne")),
];

fn float_conditions(predicate: FloatPredicate) -> Option<(bool, Test)> {
    FLOAT_CONDITIONS.iter().find(|(one, _, _)| *one == predicate).map(|&(_, swapped, test)| (swapped, test))
}

/// The x87 operation on st(0) each float function is. One no x87
/// instruction is -- `fatan`, `flog2`, `fexp2` -- a frontend's finalizer
/// spells as the sequence that computes it.
const FLOAT_FUNCTIONS: [(FloatFunction, &str); 8] = [
    (FloatFunction::Fabs, "fabs"),
    (FloatFunction::Sqrt, "fsqrt"),
    (FloatFunction::Rint, "frndint"),
    (FloatFunction::Sin, "fsin"),
    (FloatFunction::Cos, "fcos"),
    (FloatFunction::Atan, "fatan"),
    (FloatFunction::Log2, "flog2"),
    (FloatFunction::Exp2, "fexp2"),
];

/// The width of a float value in LIR: x87's extended precision.
const FLOAT: u32 = 10;

/// The most stores a memset expands to, as LLVM's x86 MaxStoresPerMemset.
const MEMSET_STORES: i64 = 16;

/// The bytes a value of `ty` takes in memory or on the stack: a far
/// pointer is its offset and selector, in two registers.
fn size_of(module: &Module, layout: &DataLayout, ty: TypeId) -> Result<u32, Unselected> {
    match module.context.types.get(ty) {
        Type::Pointer(1) => Ok(4),
        Type::Int(64) => Ok(8),
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
///
/// An `index` is a register the address adds, `[bp+si+disp]` or
/// `[bx+si+disp]`, where only accesses read the address: see `indexed`;
/// `scale` multiplies it in the 67h form, `[ebx+esi*4]`.
enum Pointer {
    Frame { disp: i64, index: Option<Held> },
    Based { base: Held, index: Option<Held>, scale: i64, offset: i64 },
    /// A near global's symbol, a displacement from it, and a register
    /// holding a variable one.
    Global { space: Space, index: i64, offset: i64, base: Option<Held> },
    /// A far pointer's selector and offset, and a displacement from it; no
    /// offset register is offset 0, as a segment's pointer has.
    Far { selector: Held, base: Option<Held>, index: Option<Held>, scale: i64, offset: i64 },
}

pub fn selected<'c>(module: &Module, name: &str, contracts: Contracts<'c>, pool: &mut Pool, cpu: &'c Profile, segments: &'c Segments) -> Result<Selected, Unselected> {
    let Some(global) = module.named(name) else { return refuse(format!("no function @{name}")) };
    let Some(function) = module.global(global).function().filter(|one| !one.is_declaration()) else {
        return refuse(format!("@{name} has no body"));
    };
    let Some(layout) = module.datalayout.as_deref() else { return refuse("a module with no datalayout") };
    let layout = DataLayout::parse(layout).map_err(Unselected)?;
    let convention = convention(module, &layout, global)?;
    let unit = Unit::of(module, &layout, function);
    let exact = ranges::exact_offsets(&unit).map_err(Unselected)?;
    let secondary = cpu.address_forms.iter().find(|form| form.secondary && form.index_width == 4).filter(|form| form.before_spill(&cpu.operations));
    let (facts, typed) = match secondary {
        None => (Facts::default(), BTreeSet::new()),
        Some(_) => {
            let typed = function.walk().map(|(_, inst)| inst).filter(|&inst| MemRef::of(&unit, inst).is_some_and(|one| one.typed.is_some())).collect();
            (ranges::scoped(&unit).map_err(Unselected)?, typed)
        }
    };
    let mut selector = Selector {
        module,
        function,
        layout,
        values: IndexMap::default(),
        next: 0,
        pointers: IndexMap::default(),
        fars: IndexMap::default(),
        wides: IndexMap::default(),
        halves: BTreeSet::new(),
        depth: 0,
        ats: IndexMap::default(),
        fused: BTreeSet::new(),
        cells: BTreeSet::new(),
        stored: BTreeSet::new(),
        words: IndexMap::default(),
        joins: IndexMap::default(),
        tested: BTreeSet::new(),
        consumed: BTreeSet::new(),
        paired: IndexMap::default(),
        callees: llrm_mir::memory::callees(module),
        private: Vec::new(),
        cpu,
        segments,
        exact,
        exact_sums: BTreeSet::new(),
        secondary,
        facts,
        typed,
        promoted: BTreeSet::new(),
        pending: IndexMap::default(),
        phi_inputs: IndexMap::default(),
        pair_inputs: IndexMap::default(),
        pair_phi_inputs: IndexMap::default(),
        stand_ins: IndexMap::default(),
        current: None,
        far_globals: IndexMap::default(),
        materialized: Vec::new(),
        edges: IndexMap::default(),
        chains: IndexMap::default(),
        pins: IndexMap::default(),
        inputs: BTreeSet::new(),
        contracts,
        calls: IndexMap::default(),
        inline: IndexMap::default(),
        far: BTreeSet::new(),
        reachable: BTreeSet::new(),
        flagged: BTreeSet::new(),
        pool,
    };
    let body = selector.body(name, &convention)?;
    Ok(Selected { body, convention, calls: selector.calls, inline: selector.inline, far: selector.far, depth: selector.depth })
}

struct Selector<'m, 'c, 'p> {
    module: &'m Module,
    function: &'m Function,
    layout: DataLayout,
    values: IndexMap<ValueId, u32>,
    next: u32,
    pointers: IndexMap<ValueId, Pointer>,
    /// Each far pointer value's offset and selector, as LLVM's type
    /// legalizer expands a value no register holds into two; no offset
    /// register is offset 0.
    fars: IndexMap<ValueId, (Option<Held>, Held)>,
    /// Each i64 value's low and high dwords, expanded likewise.
    wides: IndexMap<ValueId, (Held, Held)>,
    /// The registers holding those halves.
    halves: BTreeSet<u32>,
    depth: i64,
    ats: IndexMap<InstId, i64>,
    /// Comparisons a branch reads as flags, made beside it.
    fused: BTreeSet<InstId>,
    /// Loads whose only reader takes their cell as its memory operand.
    cells: BTreeSet<InstId>,
    /// Conversions whose only reader is a store: x87 stores them itself.
    stored: BTreeSet<InstId>,
    /// Dword loads read only as words: each word's offset and the value it is.
    words: IndexMap<InstId, Vec<(i64, ValueId)>>,
    /// A dword joined from two words: those words, low then high.
    joins: IndexMap<u32, (Held, Held)>,
    /// ANDs only a comparison with zero reads: a `test`.
    tested: BTreeSet<InstId>,
    /// What another instruction's selection made: a narrowed load's
    /// readers, the second of a quotient and remainder.
    consumed: BTreeSet<InstId>,
    /// A division whose remainder, or remainder whose quotient, a later
    /// instruction of its block is: that instruction's value.
    paired: IndexMap<InstId, ValueId>,
    /// What each callee does to memory.
    callees: llrm_mir::memory::Callees,
    /// The frame bytes no exposed alloca occupies, which no call reaches.
    private: Vec<(crate::model::ir::Addr, u32)>,
    /// What each instruction costs, where a choice depends on it.
    cpu: &'c Profile,
    /// Which segment registers the machine's program model leaves free.
    segments: &'c Segments,
    /// Index values every access names exactly at any wider width.
    exact: BTreeSet<ValueId>,
    /// The registers indexing cells `exact` proves: each such cell is
    /// `Mem::exact`, for `exactaddress`.
    exact_sums: BTreeSet<u32>,
    /// The 67h form, where the target prices it below a spill.
    secondary: Option<&'c AddressForm>,
    /// Each block's interval facts, where `secondary` asks them.
    facts: Facts,
    /// The accesses of a typed lvalue.
    typed: BTreeSet<InstId>,
    /// Word values a scaled address reads as dwords, widened where defined.
    promoted: BTreeSet<u32>,
    /// What each block computes for its successors' phis, before its terminator.
    pending: IndexMap<BlockId, Vec<Arc<Insn>>>,
    /// The register each (phi, predecessor) reads, where that block made it.
    phi_inputs: IndexMap<(InstId, BlockId), u32>,
    /// A split phi's halves each predecessor brings, and the value it
    /// copies into them before its terminator: as the old route split a far
    /// pointer into two words and an i64 into two dwords, each its own phi.
    pair_inputs: IndexMap<BlockId, Vec<(Operand, Held, Held)>>,
    pair_phi_inputs: IndexMap<(InstId, BlockId), (u32, u32)>,
    /// What each split phi input's stand-in is.
    stand_ins: IndexMap<u32, u32>,
    /// The block being selected.
    current: Option<BlockId>,
    /// Each far global's offset and selector, where a block made them.
    far_globals: IndexMap<(BlockId, GlobalId), (Held, Held)>,
    /// What `global` made for the instruction being selected, placed before it.
    materialized: Vec<Arc<Insn>>,
    /// The LIR blocks each MIR edge leaves from: a switch's cases leave
    /// from blocks of their own.
    edges: IndexMap<(BlockId, BlockId), Vec<i64>>,
    /// The blocks a switch's compare chain adds after its own.
    chains: IndexMap<InstId, Vec<i64>>,
    pins: IndexMap<u32, Register>,
    inputs: BTreeSet<u32>,
    contracts: Contracts<'c>,
    calls: IndexMap<i64, String>,
    /// The code laid down in place of each call to an inline helper.
    inline: IndexMap<i64, Vec<u8>>,
    far: BTreeSet<i64>,
    /// The blocks execution can reach.
    reachable: BTreeSet<BlockId>,
    /// Calls whose result is the flags their contract says they leave.
    flagged: BTreeSet<ValueId>,
    /// Where a float constant is loaded from, as LLVM's constant pool.
    pool: &'p mut Pool,
}

impl Selector<'_, '_, '_> {
    fn body(&mut self, name: &str, convention: &Convention) -> Result<LirBody, Unselected> {
        let function = self.function;
        // Only what execution can reach is selected, as LLVM's code generator
        // drops unreachable blocks.
        let mut work = vec![function.entry().expect("a body")];
        while let Some(block) = work.pop() {
            if self.reachable.insert(block) {
                work.extend(self.successors(block));
            }
        }
        let layout: Vec<BlockId> = function.layout().iter().copied().filter(|block| self.reachable.contains(block)).collect();
        let layout = &layout[..];
        let mut at = 0;
        let mut block_at = IndexMap::default();
        let mut reach = BTreeSet::new();
        for &block in layout {
            block_at.insert(block, at);
            for &inst in function.block(block).instructions() {
                self.ats.insert(inst, at);
                at += 1;
                if let Opcode::Alloca { allocated, .. } = function.instruction(inst).opcode {
                    let size = self.layout.alloc_size(self.types(), allocated) as i64;
                    self.depth += size + size % 2;
                    let address = function.instruction(inst).result.expect("an address");
                    self.pointers.insert(address, Pointer::Frame { disp: -self.depth, index: None });
                    if llrm_analysis::frameescape::exposes(function, address) {
                        reach.insert((-self.depth, -self.depth + size));
                    }
                }
            }
        }
        self.private = crate::model::mir::outside(&reach);
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
                for successor in self.successors(block) {
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
                self.float_loaded(held, "fld", Pointer::Frame { disp, index: None }, size, false, block_at[&entry], &mut prologue);
                continue;
            }
            if self.is_far(function.value(parameter).ty) {
                let (offset, selector) = self.far_loaded(Pointer::Frame { disp, index: None }, false, block_at[&entry], &mut prologue);
                self.fars.insert(parameter, (Some(offset), selector));
                continue;
            }
            // Its low dword at the lower address.
            if self.is_wide(ty) {
                let (low, high) = (self.fresh_held(4), self.fresh_held(4));
                for (held, by) in [(low, 0), (high, 4)] {
                    prologue.push(insn(block_at[&entry], semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Mem(frame(disp + by, 4))])));
                }
                self.wides.insert(parameter, (low, high));
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
                self.fold(inst);
                self.narrowed(inst);
                self.test(inst);
                self.pair(inst);
            }
            self.phis_from(block)?;
        }
        // Selected in reverse postorder, so every definition before its
        // readers, as a folded address must be; laid out as the function is.
        let mut order = Vec::new();
        let (mut seen, mut stack) = (BTreeSet::new(), vec![(entry, 0)]);
        seen.insert(entry);
        while let Some((block, next)) = stack.pop() {
            let successors = self.successors(block);
            match successors.get(next) {
                Some(&one) => {
                    stack.push((block, next + 1));
                    if seen.insert(one) {
                        stack.push((one, 0));
                    }
                }
                None => order.push(block),
            }
        }
        order.reverse();
        let mut made: IndexMap<BlockId, Vec<LirBlock>> = IndexMap::default();
        for &block in &order {
            self.current = Some(block);
            let blocks = made.entry(block).or_default();
            let mut insns = if block == entry { std::mem::take(&mut prologue) } else { Vec::new() };
            let mut phis = Vec::new();
            for &inst in function.block(block).instructions() {
                let instruction = function.instruction(inst);
                if instruction.opcode == Opcode::Phi && (self.is_far(instruction.ty) || self.is_wide(instruction.ty)) {
                    let result = instruction.result.expect("a phi's value");
                    let (low, high) = match self.fars.get(&result) {
                        Some(&(Some(offset), selector)) => (offset, selector),
                        _ => self.wides[&result],
                    };
                    for (half, one) in [(low, 0), (high, 1)] {
                        let incoming = self.incoming_by(inst, |selector, inst, from, _| {
                            let pair = selector.pair_phi_inputs[&(inst, from)];
                            if one == 0 { pair.0 } else { pair.1 }
                        })?;
                        phis.push(Phi { result: half.value, incoming });
                    }
                    continue;
                }
                if instruction.opcode == Opcode::Phi {
                    let result = instruction.result.expect("a phi's value");
                    let incoming = self.incoming(inst)?;
                    self.width(instruction.ty)?;
                    phis.push(Phi { result: self.value(result), incoming });
                    continue;
                }
                if instruction.opcode.is_terminator() {
                    insns.extend(self.pending.shift_remove(&block).unwrap_or_default());
                    let start = insns.len();
                    self.pair_copies(block, self.ats[&inst], &mut insns)?;
                    insns.splice(start..start, std::mem::take(&mut self.materialized));
                }
                if instruction.opcode == Opcode::Switch {
                    self.switch(inst, &block_at, block_at[&block], std::mem::take(&mut insns), std::mem::take(&mut phis), blocks)?;
                    break;
                }
                let start = insns.len();
                self.instruction(inst, &block_at, &mut insns, convention)?;
                insns.splice(start..start, std::mem::take(&mut self.materialized));
            }
            if function.instruction(function.terminator(block).expect("a terminator")).opcode == Opcode::Switch {
                continue;
            }
            let mut succ: Vec<i64> = Vec::new();
            for one in self.successors(block) {
                if !succ.contains(&block_at[&one]) {
                    succ.push(block_at[&one]);
                }
            }
            blocks.push(LirBlock { succ, phis, ..LirBlock::new(block_at[&block], insns) });
        }
        for block in made.values_mut().flatten() {
            for phi in &mut block.phis {
                for (_, value) in &mut phi.incoming {
                    *value = self.stand_ins.get(value).copied().unwrap_or(*value);
                }
            }
        }
        let cold = self.cold(&order);
        let blocks = layout
            .iter()
            .flat_map(|block| {
                let chain = made.shift_remove(block).unwrap_or_default();
                chain.into_iter().map(|one| LirBlock { cold: cold.contains(block), ..one })
            })
            .collect();
        let blocks = self.widen(combined::combined(self.unread_halves_dropped(blocks)))?;
        let mut body = LirBody::new(name, block_at[&entry], blocks, IndexMap::default(), self.pins.clone());
        body.sealed_arguments = true;
        body.inputs = self.inputs.clone();
        body.ordered = true;
        body.loop_trip_counts = self.trip_counts(&block_at);
        Ok(body)
    }

    /// Each loop's header and constant trips, as `induction` proves them.
    fn trip_counts(&self, block_at: &IndexMap<BlockId, i64>) -> Vec<(i64, i64)> {
        let unit = Unit::of(self.module, &self.layout, self.function);
        let facts = unit.registers();
        let mut counts: Vec<(i64, i64)> = unit
            .shape()
            .loops
            .iter()
            .filter_map(|one| {
                let header = block_at.get(&cfg::block(one.header))?;
                let count = llrm_analysis::induction::trip_count(&unit, one, &facts)?;
                Some((*header, i64::try_from(count).expect("a trip count fits an int64")))
            })
            .collect();
        counts.sort();
        counts
    }

    /// Blocks from which every path ends in `unreachable`, as the old
    /// route's noreturn::cold finds those ending in a terminal call: a least
    /// fixed point, so a loop that never exits is not cold, and none is
    /// when the entry is.
    fn cold(&self, blocks: &[BlockId]) -> BTreeSet<BlockId> {
        let function = self.function;
        let mut found = BTreeSet::new();
        let mut changed = true;
        while changed {
            changed = false;
            for &block in blocks {
                let ends = match function.instruction(function.terminator(block).expect("a terminator")).opcode {
                    Opcode::Unreachable => true,
                    Opcode::Ret => false,
                    _ => {
                        let successors = self.successors(block);
                        !successors.is_empty() && successors.iter().all(|one| found.contains(one))
                    }
                };
                if ends && found.insert(block) {
                    changed = true;
                }
            }
        }
        if found.contains(&function.entry().expect("a body")) { BTreeSet::new() } else { found }
    }

    /// A comparison whose only reader is its block's branch stays flags.
    fn fuse(&mut self, block: BlockId, inst: InstId) {
        let function = self.function;
        let instruction = function.instruction(inst);
        match instruction.opcode {
            Opcode::ICmp(_) => {}
            // A branch reads one condition of the flags.
            Opcode::FCmp(predicate) if float_conditions(predicate).is_some_and(|(_, test)| matches!(test, Test::One(_))) => {}
            _ => return,
        }
        let result = instruction.result.expect("a comparison's value");
        if let [only] = function.users(result)
            && function.terminator(block) == Some(only.user)
            && function.instruction(only.user).opcode == Opcode::Br
        {
            self.fused.insert(inst);
        }
    }

    /// Where x87 reads or writes only memory, the cell a value comes from or
    /// goes to is the operand, as the old route placed a MIR cell operand
    /// (`lower::_place`): an integer load only `sitofp` reads is `fild`'s
    /// cell, a float load only a comparison reads second is `fcom`'s, and a
    /// conversion only a store reads is stored by `fistp`, `fisttp` or
    /// `fstp` into the store's cell.
    fn fold(&mut self, inst: InstId) {
        let function = self.function;
        let instruction = function.instruction(inst);
        match (&instruction.opcode, instruction.operands.first()) {
            (Opcode::Cast(CastOp::SIToFP), Some(&Operand::Value(value))) => {
                let ValueDef::Instruction(load) = function.value(value).def else { return };
                let fild = matches!(self.types().int_bits(function.value(value).ty), Some(16 | 32));
                if fild && matches!(function.instruction(load).opcode, Opcode::Load { volatile: false, .. }) && self.only_reader(value, inst) && self.unwritten(load, inst) {
                    self.cells.insert(load);
                }
            }
            (Opcode::FCmp(predicate), Some(_)) => {
                let Some((swapped, _)) = float_conditions(*predicate) else { return };
                let Operand::Value(value) = instruction.operands[usize::from(!swapped)] else { return };
                let ValueDef::Instruction(load) = function.value(value).def else { return };
                // A comparison a branch reads is made beside the branch.
                let Some(block) = function.parent(inst) else { return };
                let at = if self.fused.contains(&inst) { function.terminator(block).expect("a terminator") } else { inst };
                if matches!(function.instruction(load).opcode, Opcode::Load { volatile: false, .. }) && self.only_reader(value, inst) && self.unwritten(load, at) {
                    self.cells.insert(load);
                }
            }
            (Opcode::Store { .. }, Some(&Operand::Value(value))) => {
                let ValueDef::Instruction(conversion) = function.value(value).def else { return };
                let converting = function.instruction(conversion);
                let by_x87 = match &converting.opcode {
                    Opcode::Cast(CastOp::FPToSI) => matches!(self.types().int_bits(converting.ty), Some(16 | 32)),
                    Opcode::Cast(CastOp::FPTrunc) => true,
                    Opcode::Call(_) => self.lrint(conversion) && matches!(self.types().int_bits(converting.ty), Some(16 | 32)),
                    _ => false,
                };
                if by_x87 && self.only_reader(value, inst) {
                    self.stored.insert(conversion);
                }
            }
            _ => {}
        }
    }

    /// A dword load read only through its words is those words loaded, as
    /// the old route's narrow selects: `trunc` reads the low word, `trunc`
    /// of a shift right by 16 the high.
    fn narrowed(&mut self, inst: InstId) {
        let function = self.function;
        let instruction = function.instruction(inst);
        if !matches!(instruction.opcode, Opcode::Load { volatile: false, .. }) || self.types().int_bits(instruction.ty) != Some(32) {
            return;
        }
        let Some(loaded) = instruction.result else { return };
        let word = |reader: InstId| {
            let reading = function.instruction(reader);
            (reading.opcode == Opcode::Cast(CastOp::Trunc) && self.types().int_bits(reading.ty) == Some(16)).then(|| reading.result).flatten()
        };
        let mut halves = Vec::new();
        let mut readers = Vec::new();
        for one in function.users(loaded) {
            let reading = function.instruction(one.user);
            if let Some(result) = word(one.user) {
                halves.push((0, result));
                readers.push(one.user);
                continue;
            }
            let high = matches!(reading.opcode, Opcode::Binary(BinaryOp::LShr | BinaryOp::AShr)) && one.index == 0 && self.constant(reading.operands[1], 4) == Some(16);
            let Some(shifted) = reading.result.filter(|_| high) else { return };
            let [only] = function.users(shifted) else { return };
            let Some(result) = word(only.user) else { return };
            halves.push((2, result));
            readers.extend([one.user, only.user]);
        }
        if !halves.is_empty() {
            self.words.insert(inst, halves);
            self.consumed.extend(readers);
        }
    }

    /// An AND only a comparison with zero reads is `test`: its result is
    /// dead, and its flags are the comparison's, as the old route's
    /// _flag_test selects.
    fn test(&mut self, inst: InstId) {
        let function = self.function;
        let instruction = function.instruction(inst);
        if !matches!(instruction.opcode, Opcode::ICmp(_)) {
            return;
        }
        let (a, b) = (instruction.operands[0], instruction.operands[1]);
        let Some(Operand::Value(value)) = [(a, b), (b, a)].into_iter().find(|&(_, zero)| self.constant(zero, 4) == Some(0)).map(|(value, _)| value) else { return };
        let ValueDef::Instruction(and) = function.value(value).def else { return };
        let anding = function.instruction(and);
        let registers = anding.operands.iter().all(|one| matches!(one, Operand::Value(_)));
        if anding.opcode == Opcode::Binary(BinaryOp::And) && registers && matches!(self.types().int_bits(anding.ty), Some(16 | 32)) && self.only_reader(value, inst) {
            self.tested.insert(and);
        }
    }

    /// A quotient and a remainder of the same operands in one block are
    /// one division, which leaves both, as the old route's divmod is.
    fn pair(&mut self, inst: InstId) {
        let function = self.function;
        let instruction = function.instruction(inst);
        let partner = match instruction.opcode {
            Opcode::Binary(BinaryOp::SDiv) => BinaryOp::SRem,
            Opcode::Binary(BinaryOp::SRem) => BinaryOp::SDiv,
            Opcode::Binary(BinaryOp::UDiv) => BinaryOp::URem,
            Opcode::Binary(BinaryOp::URem) => BinaryOp::UDiv,
            _ => return,
        };
        if self.consumed.contains(&inst) || !matches!(self.types().int_bits(instruction.ty), Some(16 | 32 | 64)) {
            return;
        }
        let Some(block) = function.parent(inst) else { return };
        let later = function.block(block).instructions().iter().skip_while(|&&one| one != inst).skip(1);
        let found = later.copied().find(|&one| {
            let other = function.instruction(one);
            other.opcode == Opcode::Binary(partner) && other.operands == instruction.operands && !self.consumed.contains(&one)
        });
        if let Some(one) = found {
            self.paired.insert(inst, function.instruction(one).result.expect("a result"));
            self.consumed.insert(one);
        }
    }

    /// Whether `value` is read once, by `user` in the block that defines it.
    fn only_reader(&self, value: ValueId, user: InstId) -> bool {
        let function = self.function;
        let ValueDef::Instruction(definition) = function.value(value).def else { return false };
        matches!(function.users(value), [only] if only.user == user) && function.parent(definition) == function.parent(user)
    }

    /// Whether nothing after `from` and before `to`, in one block, may write memory.
    fn unwritten(&self, from: InstId, to: InstId) -> bool {
        let function = self.function;
        let Some(block) = function.parent(from) else { return false };
        let instructions = function.block(block).instructions();
        let (Some(start), Some(end)) = (instructions.iter().position(|&one| one == from), instructions.iter().position(|&one| one == to)) else { return false };
        start < end && instructions[start + 1..end].iter().all(|&one| !llrm_mir::memory::of(&self.module.context, &self.callees, function, one).writes)
    }

    /// Whether `inst` is a call of `llvm.lrint`.
    fn lrint(&self, inst: InstId) -> bool {
        let Some(&Operand::Constant(callee)) = self.function.instruction(inst).operands.last() else { return false };
        let ConstantKind::Global(global) = self.module.context.get(callee).kind else { return false };
        self.module.global(global).name.as_deref().is_some_and(|name| llrm_mir::intrinsics::is_reserved(name) && Intrinsic::named(name) == Some(Intrinsic::LRint))
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
            if self.is_far(instruction.ty) || self.is_wide(instruction.ty) {
                self.pair_phi(inst);
                continue;
            }
            for pair in instruction.operands.chunks(2) {
                let [value, Operand::Block(from)] = *pair else { unreachable!("a phi's pairs") };
                if !self.reachable.contains(&from) || !self.successors(from).contains(&block) {
                    continue;
                }
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

    /// A far or i64 phi as two, one per half: each input a pair its
    /// predecessor makes before its terminator, when the value it copies is
    /// selected.
    fn pair_phi(&mut self, inst: InstId) {
        let function = self.function;
        let instruction = function.instruction(inst);
        let block = function.parent(inst).expect("a placed phi");
        let width = if self.is_far(instruction.ty) { 2 } else { 4 };
        let (low, high) = (self.fresh_held(width), self.fresh_held(width));
        let result = instruction.result.expect("a phi's value");
        if width == 2 {
            self.fars.insert(result, (Some(low), high));
        } else {
            self.wides.insert(result, (low, high));
        }
        for pair in instruction.operands.chunks(2) {
            let [value, Operand::Block(from)] = *pair else { unreachable!("a phi's pairs") };
            if !self.reachable.contains(&from) || !self.successors(from).contains(&block) || self.pair_phi_inputs.contains_key(&(inst, from)) {
                continue;
            }
            let (low, high) = (self.fresh_held(width), self.fresh_held(width));
            self.pair_inputs.entry(from).or_default().push((value, low, high));
            self.pair_phi_inputs.insert((inst, from), (low.value, high.value));
        }
    }

    /// The halves split phis' inputs from `block` are, once selected: what
    /// `block` makes of a constant before its terminator, and each input's
    /// stand-in renamed to them.
    fn pair_copies(&mut self, block: BlockId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        for (value, low, high) in self.pair_inputs.shift_remove(&block).unwrap_or_default() {
            let (from_low, from_high) = if low.width == 2 { self.far(value, at, out)? } else { self.wide(value, at, out)? };
            self.stand_ins.extend([(low.value, from_low.value), (high.value, from_high.value)]);
        }
        Ok(())
    }

    /// Each LIR edge into the phi's block, and the register it brings.
    fn incoming(&mut self, inst: InstId) -> Result<Vec<(i64, u32)>, Unselected> {
        self.incoming_by(inst, |selector, inst, from, value| match (selector.phi_inputs.get(&(inst, from)), value) {
            (Some(&made), _) => made,
            (None, Operand::Value(one)) => selector.value(one),
            (None, _) => unreachable!("phis_from made every other input"),
        })
    }

    /// Each LIR edge into the phi's block, and the register `input` says
    /// the edge from `from` brings.
    fn incoming_by(&mut self, inst: InstId, input: impl Fn(&mut Self, InstId, BlockId, Operand) -> u32) -> Result<Vec<(i64, u32)>, Unselected> {
        let function = self.function;
        let instruction = function.instruction(inst);
        let block = function.parent(inst).expect("a placed phi");
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for pair in instruction.operands.chunks(2) {
            let [value, Operand::Block(from)] = *pair else { unreachable!("a phi's pairs") };
            // A block that reaches this one by two edges is listed once per edge.
            // An edge execution never takes brings nothing.
            let Some(edges) = self.edges.get(&(from, block)).cloned() else { continue };
            if !seen.insert(from) {
                continue;
            }
            let held = input(self, inst, from, value);
            out.extend(edges.iter().map(|&at| (at, held)));
        }
        Ok(out)
    }

    /// The blocks `block` can leave for: a branch on a constant takes one.
    fn successors(&self, block: BlockId) -> Vec<BlockId> {
        let function = self.function;
        let terminator = function.instruction(function.terminator(block).expect("a terminator"));
        match terminator.operands[..] {
            [condition @ Operand::Constant(_), Operand::Block(taken), Operand::Block(otherwise)] if terminator.opcode == Opcode::Br => {
                vec![if self.constant(condition, 1) == Some(0) { otherwise } else { taken }]
            }
            _ => function.successors(block),
        }
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
        if self.is_float(ty) {
            return self.float(operand, at, out);
        }
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
            Pointer::Frame { index: Some(_), .. } | Pointer::Based { index: Some(_), .. } | Pointer::Far { index: Some(_), .. } => {
                unreachable!("an indexed address is read only by accesses")
            }
            Pointer::Frame { disp, index: None } => {
                let address = Address { through: Register::BP, disp_width: 2, ..Address::new(Some(Addr::new(Space::Frame, disp))) };
                semantics(Operation::Address, "lea", vec![Loc::Held(held)], vec![Loc::Address(address)])
            }
            // A far pointer's offset.
            Pointer::Based { base, offset, .. } | Pointer::Far { base: Some(base), offset, .. } => {
                let step = Loc::Imm(Imm { value: offset, width: held.width, address: None });
                semantics(Operation::Binary, "add", vec![Loc::Held(held)], vec![Loc::Held(base), step])
            }
            Pointer::Far { base: None, offset, .. } => {
                semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Imm(Imm { value: offset, width: held.width, address: None })])
            }
            Pointer::Global { space, index, offset, base } => {
                let symbol = Loc::Imm(Imm { value: 0, width: held.width, address: Some(Addr { index, ..Addr::new(space, offset) }) });
                match base {
                    None => semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![symbol]),
                    Some(base) => semantics(Operation::Binary, "add", vec![Loc::Held(held)], vec![Loc::Held(base), symbol]),
                }
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
            return Ok(Pointer::Far { selector, base, index: None, scale: 1, offset: 0 });
        }
        if !matches!(self.types().get(ty), Type::Pointer(0)) {
            return refuse(format!("an access through a {}", self.types().display(ty)));
        }
        let width = self.width(ty)?;
        Ok(Pointer::Based { base: Held { value: self.value(value), width }, index: None, scale: 1, offset: 0 })
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

    /// A global's address, and a constant displacement from it. A far
    /// global's selector and offset are made once in each block that
    /// reads them, as a DAG materializes a constant per block, before the
    /// instruction being selected.
    fn global(&mut self, operand: Operand) -> Result<Pointer, Unselected> {
        let Operand::Constant(id) = operand else { unreachable!("a constant") };
        let (global, offset) = crate::backend::globals::target(self.module, &self.layout, id).map_err(|error| Unselected(format!("{error}: {:?}", self.module.context.get(id).kind)))?;
        let space = crate::backend::globals::space(self.module, global);
        if self.module.global(global).address_space != 0 {
            let Some(block) = self.current else { return refuse("a far global outside a block") };
            let (base, selector) = match self.far_globals.get(&(block, global)) {
                Some(&pair) => pair,
                None => {
                    let (base, selector) = (self.fresh_held(2), self.fresh_held(2));
                    let at = self.ats[&self.function.block(block).instructions()[0]];
                    let symbol = |space, index| Loc::Imm(Imm { value: 0, width: 2, address: Some(Addr { index, ..Addr::new(space, 0) }) });
                    for (into, from) in [(selector, symbol(Space::Group, crate::backend::globals::segment_of(global))), (base, symbol(space, i64::from(global.0)))] {
                        self.materialized.push(insn(at, semantics(Operation::Move, "mov", vec![Loc::Held(into)], vec![from])));
                    }
                    self.far_globals.insert((block, global), (base, selector));
                    (base, selector)
                }
            };
            return Ok(Pointer::Far { selector, base: Some(base), index: None, scale: 1, offset });
        }
        Ok(Pointer::Global { space, index: i64::from(global.0), offset, base: None })
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
        let pointer = self.pointer(instruction.operands[0])?;
        let address = instruction.result.expect("an address");
        let one = match variable[..] {
            [(position, scale)] => Some((instruction.operands[1 + position], scale as i64)),
            _ => None,
        };
        if let Some(scaled) = one.and_then(|(index, scale)| self.widened(inst, index, pointer.moved(offset as i64), scale)) {
            self.pointers.insert(address, scaled);
            return Ok(());
        }
        let exact = one.is_some_and(|(index, _)| matches!(index, Operand::Value(index) if self.exact.contains(&index)));
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
        let sum = sum.expect("a variable index");
        if exact {
            self.exact_sums.insert(sum.value);
        }
        // A global's symbol is the displacement of the register holding the
        // index, [index+symbol], and the address as a value their sum.
        if let Pointer::Global { space, index, offset: start, base: None } = pointer {
            let indexed = Pointer::Global { space, index, offset: start + offset as i64, base: Some(sum) };
            if self.only_addressed(address) {
                self.pointers.insert(address, indexed);
            } else {
                let held = Held { value: self.value(address), width };
                out.push(insn(at, self.address(indexed, held)));
            }
            return Ok(());
        }
        // An address only accesses read is their base plus the sum as an
        // index, as the old route's addressforms folds `b + (c << k)` read
        // only by cells: the add goes. Word addressing has no scale.
        if self.only_addressed(address) {
            let indexed = match pointer.moved(offset as i64) {
                Pointer::Frame { disp, index: None } => Some(Pointer::Frame { disp, index: Some(sum) }),
                Pointer::Based { base, index: None, offset, .. } => Some(Pointer::Based { base, index: Some(sum), scale: 1, offset }),
                Pointer::Far { selector, base: Some(base), index: None, offset, .. } => Some(Pointer::Far { selector, base: Some(base), index: Some(sum), scale: 1, offset }),
                Pointer::Far { selector, base: None, index: None, offset, .. } => Some(Pointer::Far { selector, base: Some(sum), index: None, scale: 1, offset }),
                _ => None,
            };
            if let Some(indexed) = indexed {
                self.pointers.insert(address, indexed);
                return Ok(());
            }
        }
        let start = match pointer {
            Pointer::Based { base, index: None, offset: 0, .. } | Pointer::Far { base: Some(base), index: None, offset: 0, .. } if offset == 0 => Some(base),
            Pointer::Far { base: None, index: None, offset: 0, .. } if offset == 0 => None,
            pointer => {
                let moved = pointer.moved(offset as i64);
                let start = Held { value: self.fresh(), width };
                out.push(insn(at, self.address(moved, start)));
                Some(start)
            }
        };
        let result = match (pointer, start) {
            // Offset 0 plus the index is the index.
            (Pointer::Far { selector, .. }, None) => {
                self.fars.insert(address, (Some(sum), selector));
                return Ok(());
            }
            (Pointer::Far { selector, .. }, Some(_)) if far => {
                let offset = self.fresh_held(2);
                self.fars.insert(address, (Some(offset), selector));
                offset
            }
            _ => Held { value: self.value(address), width },
        };
        let start = start.expect("a near pointer's register");
        out.push(insn(at, semantics(Operation::Binary, "add", vec![Loc::Held(result)], vec![Loc::Held(start), Loc::Held(sum)])));
        Ok(())
    }

    /// The shifts and adds a multiply by `factor` is, where the target
    /// prices them below `imul`: the old route's `_scaled`.
    fn scaled(&self, factor: Operand, ty: TypeId) -> Result<Option<Vec<(&'static str, i64)>>, Unselected> {
        let width = self.width(ty)?;
        let Some(n) = self.constant(factor, width).filter(|&n| matches!(width, 2 | 4) && 1 < n) else { return Ok(None) };
        let chain = arithmetic::scale(n, self.cpu).map_err(Unselected)?;
        Ok(chain.filter(|chain| chain.iter().all(|&(name, count)| name != "shl" || count < i64::from(width) * 8)))
    }

    /// Whether every reader of `value` takes it as the address of a load or
    /// a store, directly or through a constant offset.
    fn only_addressed(&self, value: ValueId) -> bool {
        let function = self.function;
        let users = function.users(value);
        !users.is_empty()
            && users.iter().all(|one| match &function.instruction(one.user).opcode {
                Opcode::Load { .. } => one.index == 0,
                Opcode::Store { .. } => one.index == 1,
                Opcode::GetElementPtr { source } => {
                    let constant = self.layout.collect_offset(self.types(), *source, &self.indices(one.user)).1.is_empty();
                    one.index == 0 && constant && function.instruction(one.user).result.is_some_and(|result| self.only_addressed(result))
                }
                _ => false,
            })
    }

    /// `pointer` indexed by `index` times `scale` in the 67h form, as the old
    /// route's addressforms folds a word product only cells read:
    /// `[ebx+esi*4]`. The index and the base are widened where a load
    /// defines them (`addressforms::promote`), so no register is added. The
    /// wider sum names the same byte where the index is a non-negative word
    /// at every access, and the access is typed or its offset exact.
    fn widened(&mut self, inst: InstId, index: Operand, pointer: Pointer, scale: i64) -> Option<Pointer> {
        let form = self.secondary?;
        let Operand::Value(index) = index else { return None };
        let function = self.function;
        let address = function.instruction(inst).result?;
        if scale <= 1 || !form.scales.contains(&scale) || !self.only_addressed(address) || !self.promotable(index) {
            return None;
        }
        let proven = self.accesses(address).into_iter().all(|access| {
            let fact = function.parent(access).and_then(|block| self.facts.get(&cfg::id(block))).and_then(|known| known.get(&index));
            fact.is_some_and(|fact| fact.width == 16 && fact.low >= 0.into()) && (self.typed.contains(&access) || self.exact.contains(&index))
        });
        if !proven {
            return None;
        }
        let root = match self.root(function.instruction(inst).operands[0]) {
            Operand::Value(root) => Some(root),
            _ => None,
        };
        let wide = Held { value: self.value(index), width: 4 };
        let (scaled, base) = match pointer {
            Pointer::Based { base, index: None, offset, .. } if root.is_some_and(|root| self.promotable(root)) => {
                (Pointer::Based { base: Held { width: 4, ..base }, index: Some(wide), scale, offset }, Some(base))
            }
            Pointer::Far { selector, base: Some(base), index: None, offset, .. } if root.is_some_and(|root| self.promotable(root)) => {
                (Pointer::Far { selector, base: Some(Held { width: 4, ..base }), index: Some(wide), scale, offset }, Some(base))
            }
            // No base: twice the index is the index added to itself.
            Pointer::Far { selector, base: None, index: None, offset, .. } if scale == 2 => {
                (Pointer::Far { selector, base: Some(wide), index: Some(wide), scale: 1, offset }, None)
            }
            Pointer::Far { selector, base: None, index: None, offset, .. } => (Pointer::Far { selector, base: None, index: Some(wide), scale, offset }, None),
            _ => return None,
        };
        self.promoted.extend(base.map(|base| base.value).into_iter().chain([wide.value]));
        Some(scaled)
    }

    /// The pointer `operand` offsets by constants.
    fn root(&self, mut operand: Operand) -> Operand {
        while let Operand::Value(value) = operand {
            let ValueDef::Instruction(inst) = self.function.value(value).def else { break };
            let instruction = self.function.instruction(inst);
            let Opcode::GetElementPtr { source } = instruction.opcode else { break };
            if !self.layout.collect_offset(self.types(), source, &self.indices(inst)).1.is_empty() {
                break;
            }
            operand = instruction.operands[0];
        }
        operand
    }

    /// Whether `value` is a word a load or a parameter defines: its
    /// definition can be `movzx`, as `addressforms::promote` rewrites it.
    fn promotable(&self, value: ValueId) -> bool {
        let function = self.function;
        let ty = function.value(value).ty;
        let word = self.is_far(ty) || (!self.is_float(ty) && matches!(self.width(ty), Ok(2)));
        word && match function.value(value).def {
            ValueDef::Argument(_) => true,
            ValueDef::Instruction(inst) => {
                matches!(function.instruction(inst).opcode, Opcode::Load { .. })
                    && !self.cells.contains(&inst)
                    && !self.consumed.contains(&inst)
                    && !self.words.contains_key(&inst)
            }
        }
    }

    /// The loads and stores through `address`, directly or at a constant
    /// offset.
    fn accesses(&self, address: ValueId) -> Vec<InstId> {
        let function = self.function;
        function
            .users(address)
            .iter()
            .flat_map(|one| match function.instruction(one.user).opcode {
                Opcode::GetElementPtr { .. } => function.instruction(one.user).result.map_or_else(Vec::new, |result| self.accesses(result)),
                _ => vec![one.user],
            })
            .collect()
    }

    /// `blocks` with each cell `exact_sums` indexes marked exact, and each
    /// word value a scaled address reads widened where it is defined.
    fn widen(&mut self, blocks: Vec<LirBlock>) -> Result<Vec<LirBlock>, Unselected> {
        let marked = |what: &Semantics| {
            let exact = |operand: &Loc| match operand {
                Loc::Mem(cell) if [cell.base, cell.index].iter().flatten().any(|one| self.exact_sums.contains(&one.value)) => {
                    Loc::Mem(Mem { exact: true, ..cell.clone() })
                }
                _ => operand.clone(),
            };
            Semantics { dests: what.dests.iter().map(exact).collect(), sources: what.sources.iter().map(exact).collect(), ..what.clone() }
        };
        let blocks: Vec<LirBlock> = blocks
            .into_iter()
            .map(|block| {
                let insns = block.insns.iter().map(|one| Arc::new(Insn { what: one.what.as_ref().map(marked), ..(**one).clone() })).collect();
                block.with_insns(insns)
            })
            .collect();
        if self.promoted.is_empty() {
            return Ok(blocks);
        }
        let by_at: IndexMap<i64, Vec<Arc<Insn>>> = blocks.iter().map(|block| (block.at, block.insns.clone())).collect();
        assert_eq!(by_at.len(), blocks.len(), "a block per address");
        let mut next = self.next;
        let mut fresh = || {
            next += 1;
            next
        };
        let mut promoted = addressforms::promote(&by_at, &self.promoted, &mut fresh).map_err(Unselected)?;
        self.next = next;
        Ok(blocks.into_iter().map(|block| block.with_insns(promoted.shift_remove(&block.at).expect("each block"))).collect())
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
        let signed = matches!(op, BinaryOp::SDiv | BinaryOp::SRem);
        let other = match self.paired.get(&inst) {
            Some(&partner) => self.value(partner),
            None => self.fresh(),
        };
        let (result, other) = (Held { value: self.value(instruction.result.expect("a result")), width }, Held { value: other, width });
        let (quotient, remainder) = if matches!(op, BinaryOp::SDiv | BinaryOp::UDiv) { (result, other) } else { (other, result) };
        // A signed division by a constant is a multiply by its reciprocal
        // where the target prices that cheaper, as the old route's
        // division::reciprocal selects.
        if let Some(constant) = self.constant(instruction.operands[1], width).filter(|_| signed) {
            let mut next = self.next;
            let mut fresh = || {
                next += 1;
                next
            };
            let reciprocal = division::reciprocal(dividend, constant, &[quotient, remainder], &mut fresh, self.cpu, remainder == result || self.paired.contains_key(&inst)).map_err(Unselected)?;
            self.next = next;
            if let Some(parts) = reciprocal {
                out.extend(parts.into_iter().map(|what| insn(at, what)));
                return Ok(());
            }
        }
        let divisor = self.held(instruction.operands[1], ty, at, out)?;
        let high = Held { value: self.fresh(), width };
        out.push(insn(
            at,
            if signed {
                semantics(Operation::Extend, if width == 2 { "cwd" } else { "cdq" }, vec![Loc::Held(high)], vec![Loc::Held(dividend)])
            } else {
                semantics(Operation::Move, "mov", vec![Loc::Held(high)], vec![Loc::Imm(Imm { value: 0, width, address: None })])
            },
        ));
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
            Pointer::Frame { disp, index: None } => frame(disp, width),
            // As addressforms spells an indexed frame cell: BP and the index,
            // the displacement a literal no relocation owns, through SS.
            Pointer::Frame { disp, index: Some(index) } => Mem {
                through: Register::BP,
                disp_width: 2,
                index: Some(index),
                ..Mem::new(Some(Addr { segment: Register::SS, ..Addr::new(Space::Literal, disp) }), width)
            },
            Pointer::Global { space, index, offset, base } => Mem { disp_width: 2, base, ..Mem::new(Some(Addr { index, ..Addr::new(space, offset) }), width) },
            Pointer::Based { base, index: None, offset, .. } => Mem { base: Some(base), offset, ..Mem::new(None, width) },
            // An indexed cell's displacement is a literal, as a based cell's is in addressforms.
            Pointer::Based { base, index: Some(index), scale, offset } => {
                Mem { base: Some(base), index: Some(index), scale, offset, disp_width: 2, ..Mem::new(Some(Addr::new(Space::Literal, offset)), width) }
            }
            Pointer::Far { selector, base, index, scale, offset } => Mem {
                offset,
                scale,
                disp_width: 2,
                base,
                index,
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
        // A numeric constant, null among them: selector:offset as one dword.
        if let Some(bits) = self.constant(operand, 4) {
            let halves = [bits & 0xFFFF, (bits >> 16) & 0xFFFF].map(|value| {
                let held = self.fresh_held(2);
                let word = Loc::Imm(Imm { value, width: 2, address: None });
                out.push(insn(at, semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![word])));
                held
            });
            return Ok((halves[0], halves[1]));
        }
        match self.pointer(operand)? {
            Pointer::Far { selector, base: Some(base), index: None, offset: 0, .. } => Ok((base, selector)),
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
                    (Some(offset), selector)
                }
                (CastOp::AddrSpaceCast, Type::Pointer(2)) => (None, self.held(operand, from, at, out)?),
                (CastOp::IntToPtr, Type::Int(32)) => {
                    let dword = self.held(operand, from, at, out)?;
                    let top = self.fresh_held(4);
                    let sixteen = Loc::Imm(Imm { value: 16, width: 1, address: None });
                    out.push(insn(at, semantics(Operation::Binary, "shr", vec![Loc::Held(top)], vec![Loc::Held(dword), sixteen])));
                    (Some(Held { width: 2, ..dword }), Held { width: 2, ..top })
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
            // Truncated, as LLVM's is: segment:offset's low word.
            (CastOp::PtrToInt, Type::Int(16)) => out.push(mov(Held { value: self.value(result), width: 2 }, Loc::Held(offset))),
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
                let held = self.float(operand, at, out)?;
                self.values.insert(result, held.value);
            }
            CastOp::FPTrunc => {
                let held = self.float(operand, at, out)?;
                let cell = self.float_stored(held, "fstp", 4, at, out);
                let into = Held { value: self.value(result), width: FLOAT };
                self.float_loaded(into, "fld", cell, 4, false, at, out);
            }
            CastOp::SIToFP if self.cell(operand).is_some() => {
                let load = self.cell(operand).expect("a folded load");
                let (pointer, width) = (self.pointer(self.function.instruction(load).operands[0])?, self.width(from)?);
                let into = Held { value: self.value(result), width: FLOAT };
                self.float_loaded(into, "fild", pointer, width, false, at, out);
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
            CastOp::FPToSI => self.float_to_integer(operand, "fisttp", result, to, false, at, out)?,
            CastOp::FPToUI => self.float_to_integer(operand, "fisttp", result, to, true, at, out)?,
            _ => return refuse(format!("{op:?} of a float")),
        }
        Ok(())
    }

    /// A float stored as an integer of `to` by `name`, and loaded back. x87
    /// stores only signed integers: an unsigned one is stored twice as
    /// wide, and its low part read.
    fn float_to_integer(&mut self, operand: Operand, name: &str, result: ValueId, to: TypeId, unsigned: bool, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let width = self.width(to)?;
        if width == 1 {
            return refuse("a float to a byte");
        }
        let held = self.float(operand, at, out)?;
        let cell = self.float_stored(held, name, if unsigned { 2 * width } else { width }, at, out);
        let into = Held { value: self.value(result), width };
        out.push(insn(at, semantics(Operation::Move, "mov", vec![Loc::Held(into)], vec![Loc::Mem(Self::memory(cell, width))])));
        Ok(())
    }

    /// The load `operand` is, if its cell is its reader's operand.
    fn cell(&self, operand: Operand) -> Option<InstId> {
        let Operand::Value(value) = operand else { return None };
        match self.function.value(value).def {
            ValueDef::Instruction(load) if self.cells.contains(&load) => Some(load),
            _ => None,
        }
    }

    /// The conversion `value` is, if the store reading it stores it.
    fn converted(&self, value: ValueId) -> Option<InstId> {
        match self.function.value(value).def {
            ValueDef::Instruction(conversion) if self.stored.contains(&conversion) => Some(conversion),
            _ => None,
        }
    }

    /// A conversion stored by x87 into the cell at `pointer`: `fisttp` toward
    /// zero, `fistp` as lrint rounds, `fstp` narrowing to a float.
    fn store_converted(&mut self, conversion: InstId, pointer: Pointer, volatile: bool, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let instruction = self.function.instruction(conversion);
        let (name, operand) = match instruction.opcode {
            Opcode::Cast(CastOp::FPToSI) => ("fisttp", instruction.operands[0]),
            Opcode::Cast(CastOp::FPTrunc) => ("fstp", instruction.operands[0]),
            _ => ("fistp", instruction.operands[0]),
        };
        let size = self.size(instruction.ty)?;
        let held = self.float(operand, at, out)?;
        let what = semantics(Operation::FloatStore, name, vec![Loc::Mem(Self::memory(pointer, size))], vec![Loc::Held(held)]);
        out.push(Arc::new(Insn { volatile, ..insn_of(at, what) }));
        Ok(())
    }

    /// `into` made of a low and a high word, and dropped if only its words
    /// are read: they are remembered.
    fn joined(&mut self, into: Held, low: Held, high: Held, at: i64, out: &mut Vec<Arc<Insn>>) {
        let (wide_low, wide_high, shifted) = (self.half(), self.half(), self.half());
        self.halves.insert(into.value);
        self.joins.insert(into.value, (low, high));
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
            // Selected where its reader is: see fold.
            _ if self.cells.contains(&inst) || self.stored.contains(&inst) || self.tested.contains(&inst) || self.consumed.contains(&inst) => {}
            Opcode::Load { .. } if self.words.contains_key(&inst) => {
                let pointer = self.pointer(operands[0])?;
                let mut made: IndexMap<i64, Held> = IndexMap::default();
                let mut halves = self.words[&inst].clone();
                halves.sort_by_key(|&(offset, _)| offset);
                for (offset, result) in halves {
                    let held = Held { value: self.value(result), width: 2 };
                    let source = match made.get(&offset) {
                        Some(&earlier) => Loc::Held(earlier),
                        None => Loc::Mem(Self::memory(pointer.moved(offset), 2)),
                    };
                    made.entry(offset).or_insert(held);
                    out.push(insn(at, semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![source])));
                }
            }
            // What a poison store leaves may be anything, so it stays as is.
            Opcode::Store { volatile: false, .. } if matches!(operands[0], Operand::Constant(one) if self.module.context.get(one).kind == ConstantKind::Poison) => {}
            Opcode::Store { volatile, .. } if matches!(operands[0], Operand::Value(value) if self.converted(value).is_some()) => {
                let Operand::Value(value) = operands[0] else { unreachable!("a converted value") };
                let conversion = self.converted(value).expect("a stored conversion");
                let pointer = self.pointer(operands[1])?;
                self.store_converted(conversion, pointer, *volatile, at, out)?;
            }
            Opcode::GetElementPtr { source } => {
                if self.folded(instruction.result.expect("an address"))?.is_none() {
                    self.indexed(inst, *source, at, out)?;
                }
            }
            Opcode::Cast(op) if self.is_wide(instruction.ty) || self.is_wide(type_of(operands[0])) => self.wide_cast(*op, inst, at, out)?,
            Opcode::Binary(op) if self.is_wide(instruction.ty) => self.wide_binary(*op, inst, at, out)?,
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
                        let held = self.float(value, at, out)?;
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
                let (a, b) = (self.float(operands[0], at, out)?, self.float(operands[1], at, out)?);
                let result = Held { value: self.value(instruction.result.expect("a result")), width: FLOAT };
                out.push(insn(at, semantics(Operation::FloatArith, name, vec![Loc::Held(result)], vec![Loc::Held(a), Loc::Held(b)])));
            }
            Opcode::FNeg => {
                let a = self.float(operands[0], at, out)?;
                let result = Held { value: self.value(instruction.result.expect("a result")), width: FLOAT };
                out.push(insn(at, semantics(Operation::FloatUnary, "fchs", vec![Loc::Held(result)], vec![Loc::Held(a)])));
            }
            Opcode::Cast(op) if self.is_float(instruction.ty) || self.is_float(type_of(operands[0])) => self.float_cast(*op, inst, at, out)?,
            Opcode::Load { volatile, .. } if self.is_far(instruction.ty) => {
                let pointer = self.pointer(operands[0])?;
                let (offset, selector) = self.far_loaded(pointer, *volatile, at, out);
                self.fars.insert(instruction.result.expect("a load's value"), (Some(offset), selector));
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
                if *op == BinaryOp::Mul
                    && let Some(chain) = self.scaled(b, ty)?
                {
                    // Shifts and adds of the source, as the old route's _scaled selects.
                    let result = Held { value: self.value(instruction.result.expect("a result")), width: self.width(ty)? };
                    let mut current = a.clone();
                    for (index, &(name, count)) in chain.iter().enumerate() {
                        let into = if index == chain.len() - 1 { result } else { self.fresh_held(result.width) };
                        let other = if name == "shl" { Loc::Imm(Imm { value: count, width: 1, address: None }) } else { a.clone() };
                        out.push(insn(at, semantics(Operation::Binary, name, vec![Loc::Held(into)], vec![current, other])));
                        current = Loc::Held(into);
                    }
                    return Ok(());
                }
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
                let test = self.compare(inst, at, out)?;
                let result = Held { value: self.value(instruction.result.expect("a result")), width: 1 };
                let set = |code: &str, into: Held| insn(at, semantics(Operation::Unary, &format!("set{}", &code[1..]), vec![Loc::Held(into)], vec![]));
                match test {
                    Test::One(code) => out.push(set(code, result)),
                    Test::Both(a, b) | Test::Either(a, b) => {
                        let (first, second) = (self.fresh_held(1), self.fresh_held(1));
                        out.extend([set(a, first), set(b, second)]);
                        let join = if matches!(test, Test::Both(..)) { "and" } else { "or" };
                        out.push(insn(at, semantics(Operation::Binary, join, vec![Loc::Held(result)], vec![Loc::Held(first), Loc::Held(second)])));
                    }
                }
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
                        Some(compare) => match self.compare(compare, at, out)? {
                            Test::One(code) => code,
                            _ => unreachable!("only a one-condition compare is fused"),
                        },
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
                [Operand::Constant(_), ..] => {
                    let block = function.parent(inst).expect("a placed branch");
                    out.push(insn(at, jump(block_at[&self.successors(block)[0]])));
                }
                _ => return refuse("a branch of no form"),
            },
            Opcode::Ret => {
                let what = semantics(Operation::Return, "", vec![], vec![]);
                let mut one = Insn { reads_complete: true, ..Insn::new(at, Some((at, at)), Some(what), vec![], vec![]) };
                if let Some(&value) = operands.first().filter(|&&one| self.is_float(type_of(one))) {
                    // Left in st(0).
                    let held = self.float(value, at, out)?;
                    out.push(insn(at, semantics(Operation::FloatStore, "", vec![], vec![Loc::Held(held)])));
                } else if let Some(&value) = operands.first().filter(|&&one| self.is_wide(type_of(one))) {
                    // edx:eax.
                    let (low, high) = self.wide(value, at, out)?;
                    one.requires = vec![(low, Register::EAX), (high, Register::EDX)];
                    one.uses = vec![low.value, high.value];
                } else if let Some(&value) = operands.first().filter(|&&one| self.is_far(type_of(one))) {
                    let (offset, selector) = self.far(value, at, out)?;
                    let [low, high] = convention.returns[..] else { return refuse("a far result the convention has no pair for") };
                    one.requires = vec![(offset, low), (selector, high)];
                    one.uses = vec![offset.value, selector.value];
                } else if let Some(&value) = operands.first() {
                    let held = self.held(value, type_of(value), at, out)?;
                    one.requires = match convention.returns[..] {
                        [register] => vec![(held, register)],
                        // A dword result in a word pair: the words it was joined from.
                        [low, high] if held.width == 4 && self.joins.contains_key(&held.value) => {
                            let (low_word, high_word) = self.joins[&held.value];
                            vec![(low_word, low), (high_word, high)]
                        }
                        // Else its low word, and its high word shifted down.
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
            Opcode::Call(info) => {
                in_the_frame(&info.argument_attrs)?;
                self.call(inst, info.calling_convention, at, out)?;
            }
            // Nothing runs after it: the block ends with what came before.
            Opcode::Unreachable => {}
            _ => return refuse(instruction.opcode.mnemonic()),
        }
        Ok(())
    }

    /// A direct call: its arguments pushed as its convention orders them,
    /// its result delivered in ax or dx:ax, and what its contract says it
    /// destroys and who pops.
    /// A call's MIR operation, listing what it may read and write as the
    /// old route's `frame_bounded` did: anything but the frame bytes no
    /// exposed alloca occupies.
    fn listed(&self, at: i64, effects: llrm_mir::memory::Effects) -> Arc<crate::model::mir::Op> {
        use crate::model::mir;
        let reference = mir::MemRef { excludes: self.private.clone(), ..mir::MemRef::new(None, 4) };
        let mut op = mir::Op::new(at, mir::OpCode::nothing(), "call", vec![], vec![]);
        op.kind = mir::Kind::Call;
        op.memory_complete = true;
        if effects.reads {
            op.loads = vec![reference.clone()];
        }
        if effects.writes {
            op.stores = vec![reference];
        }
        Arc::new(op)
    }

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
                    let Some(&(_, name)) = FLOAT_FUNCTIONS.iter().find(|(one, _)| *one == function) else { return refuse(format!("{function:?}")) };
                    let a = self.float(arguments[0], at, out)?;
                    let result = Held { value: self.value(instruction.result.expect("a result")), width: FLOAT };
                    out.push(insn(at, semantics(Operation::FloatUnary, name, vec![Loc::Held(result)], vec![Loc::Held(a)])));
                    Ok(())
                }
                // Rounds as the machine's default mode does: fistp.
                Some(Intrinsic::LRint) => {
                    let result = instruction.result.expect("lrint's value");
                    self.float_to_integer(arguments[0], "fistp", result, instruction.ty, false, at, out)
                }
                Some(intrinsic @ (Intrinsic::PortIn | Intrinsic::PortOut)) => self.port(intrinsic == Intrinsic::PortIn, inst, arguments, at, out),
                Some(Intrinsic::Fixed { divide }) => self.fixed(divide, inst, arguments, at, out),
                _ => refuse(format!("@{name}")),
            };
        }
        if let Some(declared) = global.function() {
            in_the_frame(&declared.parameter_attrs)?;
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
                let held = self.float(argument, at, out)?;
                let cell = self.float_stored(held, "fstp", size, at, out);
                for by in (0..i64::from(size) / 4).rev().map(|dword| dword * 4) {
                    out.push(insn(at, semantics(Operation::Push, "push", vec![], vec![Loc::Mem(Self::memory(cell.moved(by), 4))])));
                }
                pushed += i64::from(size);
                continue;
            }
            if self.is_wide(ty) {
                // Its low dword at the lower address: the high pushed first.
                let (low, high) = self.wide(argument, at, out)?;
                for held in [high, low] {
                    out.push(insn(at, semantics(Operation::Push, "push", vec![], vec![Loc::Held(held)])));
                }
                pushed += 8;
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
        if contract.flags_result {
            if let Some(value) = instruction.result {
                if !function.users(value).iter().all(|one| matches!(function.instruction(one.user).opcode, Opcode::ICmp(_))) {
                    return refuse(format!("@{name}'s flags read other than by a comparison"));
                }
                self.flagged.insert(value);
            }
        } else if let Some(value) = instruction.result.filter(|_| self.is_float(instruction.ty)) {
            float = Some(Held { value: self.value(value), width: FLOAT });
        } else if let Some(value) = instruction.result.filter(|_| self.is_wide(instruction.ty)) {
            let (low, high) = (self.fresh_held(4), self.fresh_held(4));
            delivers = vec![(low, Register::EAX), (high, Register::EDX)];
            self.wides.insert(value, (low, high));
        } else if let Some(value) = instruction.result.filter(|_| self.is_far(instruction.ty)) {
            let (offset, selector) = (self.fresh_held(2), self.fresh_held(2));
            delivers = vec![(offset, Register::EAX), (selector, Register::EDX)];
            self.fars.insert(value, (Some(offset), selector));
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
        let effects = llrm_mir::memory::of(&self.module.context, &self.callees, function, inst);
        out.push(Arc::new(Insn {
            op: Some(self.listed(at, effects)),
            clobbers: call_clobbers(&contract, self.segments),
            clobbers_high: call_clobbered_high(&contract, self.segments),
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

    /// A memset, as LLVM's getMemset lowers one: a constant byte over a
    /// constant length in at most `MEMSET_STORES` stores, widest first, or
    /// else `rep stos` through es:di, as the old route's `_fill` makes it:
    /// a constant byte's dwords first, then its tail's bytes. A far
    /// destination's selector is ES; a near one's segment is set in ES
    /// around the fill.
    fn memset(&mut self, arguments: &[Operand], at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let &[destination, value, length, volatile] = arguments else { return refuse("a memset of other than four operands") };
        let volatile = self.constant(volatile, 1) != Some(0);
        let byte = self.constant(value, 1);
        let pattern = |byte: i64, width: u32| (0..width).fold(0i64, |word, _| (word << 8) | (byte & 0xFF));
        let pointer = self.pointer(destination)?;
        let mut put = |what: Semantics, out: &mut Vec<Arc<Insn>>| out.push(Arc::new(Insn { volatile, ..insn_of(at, what) }));
        let imm = |value: i64, width: u32| Loc::Imm(Imm { value, width, address: None });
        let constant = byte.zip(self.constant(length, 2));
        if let Some((byte, length)) = constant
            && length / 4 + (length % 4).count_ones() as i64 <= MEMSET_STORES
        {
            let mut offset = 0;
            for width in [4, 2, 1] {
                while length - offset >= i64::from(width) {
                    let cell = Self::memory(pointer.moved(offset), width);
                    put(semantics(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![imm(pattern(byte, width), width)]), out);
                    offset += i64::from(width);
                }
            }
            return Ok(());
        }
        // Each part: the value stored, how many (none for one store), its width.
        let mut parts: Vec<(Held, Option<Loc>, u32)> = Vec::new();
        match (byte, constant) {
            (Some(byte), Some((_, length))) => {
                let stored = self.fresh_held(4);
                put(semantics(Operation::Move, "mov", vec![Loc::Held(stored)], vec![imm(pattern(byte, 4), 4)]), out);
                parts.push((stored, Some(imm(length / 4, 2)), 4));
                let mut tail = length % 4;
                for width in [2, 1] {
                    if tail >= width {
                        parts.push((stored, None, width as u32));
                        tail -= width;
                    }
                }
            }
            (Some(byte), None) => {
                let counted = self.held(length, self.function.operand_type(&self.module.context, length).expect("a typed length"), at, out)?;
                let (bulk, tail, stored) = (self.fresh_held(2), self.fresh_held(2), self.fresh_held(4));
                put(semantics(Operation::Binary, "shr", vec![Loc::Held(bulk)], vec![Loc::Held(counted), imm(2, 1)]), out);
                put(semantics(Operation::Binary, "and", vec![Loc::Held(tail)], vec![Loc::Held(counted), imm(3, 2)]), out);
                put(semantics(Operation::Move, "mov", vec![Loc::Held(stored)], vec![imm(pattern(byte, 4), 4)]), out);
                parts.extend([(stored, Some(Loc::Held(bulk)), 4), (stored, Some(Loc::Held(tail)), 1)]);
            }
            (None, _) => {
                let ty = self.function.operand_type(&self.module.context, value).expect("a typed byte");
                let stored = self.held(value, ty, at, out)?;
                let counted = self.held(length, self.function.operand_type(&self.module.context, length).expect("a typed length"), at, out)?;
                parts.push((stored, Some(Loc::Held(counted)), 1));
            }
        }
        let mut through = match pointer {
            Pointer::Based { base, index: None, offset: 0, .. } | Pointer::Far { base: Some(base), index: None, offset: 0, .. } => base,
            _ => {
                let through = self.fresh_held(2);
                put(self.address(pointer, through), out);
                through
            }
        };
        let segment = match pointer {
            Pointer::Far { selector, .. } => Loc::Held(selector),
            _ => {
                let segment = Loc::Reg(Reg { register: Register::ES, width: 2 });
                let source = if matches!(pointer, Pointer::Frame { .. }) { Register::SS } else { Register::DS };
                put(semantics(Operation::Push, "push", vec![], vec![segment.clone()]), out);
                put(semantics(Operation::Push, "push", vec![], vec![Loc::Reg(Reg { register: source, width: 2 })]), out);
                put(semantics(Operation::Pop, "pop", vec![segment.clone()], vec![]), out);
                segment
            }
        };
        for (stored, count, width) in parts {
            let name = match width {
                1 => "stosb",
                2 => "stosw",
                _ => "stosd",
            };
            let stepped = self.fresh_held(2);
            let what = match count {
                // One store needs neither a count nor REP.
                None => semantics(Operation::Fill, name, vec![Loc::Mem(Mem::new(None, 0)), Loc::Held(stepped)], vec![Loc::Held(stored), Loc::Held(through), segment.clone()]),
                Some(count) => {
                    let count = match count {
                        Loc::Held(held) => held,
                        other => {
                            let held = self.fresh_held(2);
                            put(semantics(Operation::Move, "mov", vec![Loc::Held(held)], vec![other]), out);
                            held
                        }
                    };
                    let emptied = self.fresh_held(2);
                    semantics(
                        Operation::Fill,
                        name,
                        vec![Loc::Mem(Mem::new(None, 0)), Loc::Held(stepped), Loc::Held(emptied)],
                        vec![Loc::Held(stored), Loc::Held(count), Loc::Held(through), segment.clone()],
                    )
                }
            };
            put(what, out);
            through = stepped;
        }
        if !matches!(pointer, Pointer::Far { .. }) {
            put(semantics(Operation::Pop, "pop", vec![Loc::Reg(Reg { register: Register::ES, width: 2 })], vec![]), out);
        }
        Ok(())
    }

    /// `cmp` of a comparison's operands, a constant second; the predicate
    /// that holds of them as ordered.
    fn compare(&mut self, inst: InstId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<Test, Unselected> {
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
        // A call's flags are its result compared with zero.
        if let Operand::Value(value) = a
            && self.flagged.contains(&value)
        {
            if self.constant(b, 2) != Some(0) || !self.flags_reach(value, inst) {
                return refuse("a call's flags read apart from its compare with zero");
            }
            return Ok(Test::One(condition_code(predicate)));
        }
        if let Operand::Value(value) = a
            && let ValueDef::Instruction(and) = self.function.value(value).def
            && self.tested.contains(&and)
        {
            let operands = self.function.instruction(and).operands.clone();
            let (x, y) = (Loc::Held(self.held(operands[0], ty, at, out)?), Loc::Held(self.held(operands[1], ty, at, out)?));
            out.push(insn(at, semantics(Operation::Compare, "test", vec![], vec![x, y])));
            return Ok(Test::One(condition_code(predicate)));
        }
        if self.is_wide(ty) {
            return self.wide_compare(predicate, a, b, at, out);
        }
        let a = Loc::Held(self.held(a, ty, at, out)?);
        let b = self.source(b, ty, at, out)?;
        out.push(insn(at, semantics(Operation::Compare, "cmp", vec![], vec![a, b])));
        Ok(Test::One(condition_code(predicate)))
    }

    /// Whether the flags `value`'s call leaves are still those the compare
    /// `inst` reads: it follows the call, and the branch reading it follows it.
    fn flags_reach(&self, value: ValueId, inst: InstId) -> bool {
        let function = self.function;
        let ValueDef::Instruction(call) = function.value(value).def else { return false };
        let Some(block) = function.parent(call) else { return false };
        let instructions = function.block(block).instructions();
        let Some(at) = instructions.iter().position(|&one| one == call) else { return false };
        instructions.get(at + 1) == Some(&inst) && (!self.fused.contains(&inst) || instructions.get(at + 2) == function.terminator(block).as_ref())
    }

    /// A port read or written: `in` or `out`, a port below 256 immediate,
    /// any other in a register, as the machine's constraints pin them.
    fn port(&mut self, reading: bool, inst: InstId, arguments: &[Operand], at: i64, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {
        let function = self.function;
        let type_of = |operand| function.operand_type(&self.module.context, operand).expect("a typed argument");
        let port = match self.constant(arguments[0], 2) {
            Some(port) if (0..256).contains(&port) => Loc::Imm(Imm { value: port, width: 1, address: None }),
            _ => Loc::Held(self.held(arguments[0], type_of(arguments[0]), at, out)?),
        };
        let what = if reading {
            let instruction = function.instruction(inst);
            let result = Held { value: self.value(instruction.result.expect("a port's value")), width: self.width(instruction.ty)? };
            semantics(Operation::Barrier, "in", vec![Loc::Held(result)], vec![port])
        } else {
            let value = self.held(arguments[1], type_of(arguments[1]), at, out)?;
            semantics(Operation::Barrier, "out", vec![], vec![port, Loc::Held(value)])
        };
        out.push(insn(at, what));
        Ok(())
    }

    /// `fcom` of a float comparison's operands, in the order its row in
    /// `FLOAT_CONDITIONS` compares them; the conditions that answer it.
    fn float_compare(&mut self, predicate: FloatPredicate, inst: InstId, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<Test, Unselected> {
        let instruction = self.function.instruction(inst);
        let Some((swapped, test)) = float_conditions(predicate) else { return refuse(format!("fcmp {predicate:?}")) };
        let (a, b) = if swapped { (instruction.operands[1], instruction.operands[0]) } else { (instruction.operands[0], instruction.operands[1]) };
        let b = match self.cell(b) {
            Some(load) => {
                let loaded = self.function.instruction(load);
                Loc::Mem(Self::memory(self.pointer(loaded.operands[0])?, self.size(loaded.ty)?))
            }
            None => Loc::Held(self.float(b, at, out)?),
        };
        let a = self.float(a, at, out)?;
        out.push(insn(at, semantics(Operation::Compare, "fcom", vec![], vec![Loc::Held(a), b])));
        Ok(test)
    }

    /// A float operand, in an x87 register.
    fn float(&mut self, operand: Operand, at: i64, out: &mut Vec<Arc<Insn>>) -> Result<Held, Unselected> {
        let id = match operand {
            Operand::Value(value) => return Ok(Held { value: self.value(value), width: FLOAT }),
            Operand::Constant(id) => id,
            Operand::Block(_) => unreachable!("a float is no block"),
        };
        // As LLVM's x87 lowering: +0 and +1 are `fldz` and `fld1`, and any
        // other constant loads from the pool.
        let ConstantKind::Float(bits) = self.module.context.get(id).kind else { return refuse("a float constant of no bits") };
        let size = self.size(self.module.context.get(id).ty)?;
        let held = self.fresh_held(FLOAT);
        let value = if size == 4 { f64::from(f32::from_bits(bits as u32)) } else { f64::from_bits(bits) };
        let (name, sources) = match value {
            0.0 if value.is_sign_positive() => ("fldz", vec![]),
            1.0 => ("fld1", vec![]),
            _ => ("fld", vec![Loc::Mem(self.pool.cell(constpool::narrowest(value)))]),
        };
        out.push(insn(at, semantics(Operation::FloatLoad, name, vec![Loc::Held(held)], sources)));
        Ok(held)
    }

    /// A fresh frame cell of `size` bytes, as a DAG's stack temporary.
    fn temporary(&mut self, size: i64) -> Pointer {
        self.depth += size + size % 2;
        Pointer::Frame { disp: -self.depth, index: None }
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
            Pointer::Frame { disp, index } => Pointer::Frame { disp: disp + by, index },
            Pointer::Based { base, index, scale, offset } => Pointer::Based { base, index, scale, offset: offset + by },
            Pointer::Global { space, index, offset, base } => Pointer::Global { space, index, offset: offset + by, base },
            Pointer::Far { selector, base, index, scale, offset } => Pointer::Far { selector, base, index, scale, offset: offset + by },
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
