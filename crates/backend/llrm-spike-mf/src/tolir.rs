//! `MFunction` -> `LirBody`: the spike machine function as the translated
//! greedy allocator (`llrm-codegen::greedy`) reads its input.
//!
//! Run after [`eliminate_phis`](crate::regalloc::eliminate_phis) and
//! [`two_address`](crate::regalloc::two_address): greedy takes no phis, and a
//! tied operand is then one value both defined and used.
//!
//! A virtual register `v` is the value `v + 1`. Each physical operand of an
//! allocatable register is a short value of its own, `delivers` at its
//! definition and `requires` at each read, pinned to the register; a reserved
//! register (stack and frame pointer) is dropped, as no allocator hands it
//! out. An instruction's `at` is `2 * ordinal`, the key back to the
//! instruction it came from ([`Converted::origin`]).
//!
//! A physical register is one value for the whole function, so greedy's
//! liveness carries it across blocks; a register read before any definition
//! reaches it is an input (`LirBody::inputs`), found by backward liveness.
//!
//! DEVIATE 1: a physical register and one it overlaps (a pair and one of its
//! bytes) are two values, with no sub-register index to say one is part of
//! the other. Greedy refuses a function in which their live ranges meet
//! (`decide` returns None); a pair that arrives and a byte pair that leaves
//! at another time are fine. Exit: D-6502, if a 6502 function needs both live
//! together.
//!
//! DEVIATE 2: no block odds. The `MFunction` states none, so greedy's block
//! frequencies come from its loop estimate. Exit: D-6502, with the odds the
//! front end knows.

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::Arc;

use llrm_codegen::backend::allocresult::Error;
use llrm_codegen::backend::frame::{FrameInfo, Refused};
use llrm_codegen::greedy::shadow::ShadowInputs;
use llrm_codegen::greedy::targetinfo::TargetRegisterInfo;
use llrm_codegen::greedy::tii::TargetInstrInfo;
use llrm_codegen::model::lir::{Block as LirBlockRef, Insn, LirBlock, LirBody};
use llrm_lir::registers::{RegId, Regs};
use llrm_lir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space};
use llrm_support::hash::IndexMap;

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::{Target, successors};

/// The name of a copy: `COPY dst, src`.
const COPY: &str = "COPY";

/// A converted function and what greedy asks of the target around it.
pub struct Converted<T: Target> {
    pub body: LirBody,
    /// The registers a value may take, most preferred first.
    orders: IndexMap<u32, Vec<RegId>>,
    /// Values pinned to a register: they cannot be spilled.
    pub unspillable: BTreeSet<u32>,
    /// The roots a call keeps.
    pub saved: Vec<RegId>,
    /// `(block, instruction)` of the instruction numbered `at / 2`.
    pub origin: Vec<(usize, usize)>,
    pub tii: SpikeInstrInfo<T>,
    pub tri: SpikeRegisterInfo,
}

impl<T: Target> Converted<T> {
    /// The registers `value` may take.
    pub fn order(
        &self,
        value: u32,
    ) -> Vec<RegId> {
        self.orders.get(&value).cloned().unwrap_or_default()
    }

    /// What greedy runs on. `order` is `|value| self.order(value)`, which the
    /// caller owns so the inputs can borrow it.
    pub fn inputs<'a>(
        &'a self,
        registers: Regs,
        order: &'a dyn Fn(u32) -> Vec<RegId>,
    ) -> ShadowInputs<'a> {
        ShadowInputs {
            body: &self.body,
            registers,
            order,
            pins: &self.body.pins,
            unspillable: &self.unspillable,
            data: None,
            sets: None,
            stable_loads: false,
            saved: &self.saved,
            tii: &self.tii,
            tri: &self.tri,
            lane_pins: true,
        }
    }
}

/// `function` as greedy reads it. `registers` is the target's register file:
/// it gives each class its width.
pub fn to_lir<T: Target>(
    function: &MFunction<T::Class>,
    registers: Regs,
) -> Result<Converted<T>, String> {
    if function.blocks.iter().any(|block| !block.phis.is_empty()) {
        return Err(format!("{}: a phi reached the converter", function.name));
    }
    let widths: Vec<u32> = function.classes.iter().map(|class| class_width::<T>(registers, *class)).collect();
    let successors = successors(function);
    let mut made = Made {
        function,
        registers,
        widths: &widths,
        next: function.classes.len() as u32 + 1,
        pins: IndexMap::default(),
        values: IndexMap::default(),
        origin: Vec::new(),
        marker: PhantomData::<T>,
    };
    let mut blocks = Vec::with_capacity(function.blocks.len());
    for (at, block) in function.blocks.iter().enumerate() {
        let mut insns = Vec::with_capacity(block.insts.len());
        for (position, inst) in block.insts.iter().enumerate() {
            let ordinal = made.origin.len() as i64;
            made.origin.push((at, position));
            insns.push(Arc::new(made.insn(inst, 2 * ordinal)));
        }
        let mut lir = LirBlock::new(at as i64, insns);
        lir.succ = successors[at].iter().map(|to| *to as i64).collect();
        blocks.push(LirBlockRef::from(lir));
    }
    let entry = if function.blocks.is_empty() { 0 } else { 0 };
    let mut body = LirBody::new(function.name.clone(), entry, blocks, IndexMap::default(), made.pins.clone());
    body.inputs = entry_live::<T>(function, &successors).iter().filter_map(|r| made.values.get(r).copied()).collect();
    let mut orders: IndexMap<u32, Vec<RegId>> = IndexMap::default();
    for (v, class) in function.classes.iter().enumerate() {
        orders.insert(v as u32 + 1, T::candidates(*class));
    }
    for (value, register) in &made.pins {
        orders.insert(*value, vec![*register]);
    }
    let caller_saved: BTreeSet<RegId> = T::caller_saved().into_iter().collect();
    let mut saved: Vec<RegId> = Vec::new();
    for class in &function.classes {
        for root in T::candidates(*class) {
            if !caller_saved.contains(&T::root(root)) && !saved.contains(&root) {
                saved.push(root);
            }
        }
    }
    let unspillable = made.pins.keys().copied().collect();
    Ok(Converted {
        body,
        orders,
        unspillable,
        saved,
        origin: made.origin,
        tii: SpikeInstrInfo { classes: function.classes.clone(), registers, marker: PhantomData },
        tri: SpikeRegisterInfo { copy_cost: 1, csr_first_use_cost: 0 },
    })
}

/// The bytes a value of `class` takes: its first register's view.
fn class_width<T: Target>(
    registers: Regs,
    class: T::Class,
) -> u32 {
    T::candidates(class).first().and_then(|root| registers.bytes(T::view(*root, class))).map_or(1, |bytes| bytes as u32)
}

struct Made<'f, T: Target> {
    function: &'f MFunction<T::Class>,
    registers: Regs,
    widths: &'f [u32],
    /// The next value a pinned register gets.
    next: u32,
    pins: IndexMap<u32, RegId>,
    values: IndexMap<RegId, u32>,
    origin: Vec<(usize, usize)>,
    marker: PhantomData<T>,
}

impl<T: Target> Made<'_, T> {
    /// The value of a physical register: one for the whole function, so
    /// greedy's liveness follows it across blocks and through every rewrite.
    fn pinned(
        &mut self,
        register: RegId,
    ) -> (u32, u32) {
        let value = *self
            .values
            .entry(register)
            .or_insert_with(
                || {
                    let value = self.next;
                    self.next += 1;
                    self.pins.insert(value, register);
                    value
                },
            );
        (value, self.registers.bytes(register).map_or(1, |bytes| bytes as u32))
    }

    fn insn(
        &mut self,
        inst: &Inst,
        at: i64,
    ) -> Insn {
        let (mut defines, mut uses) = (Vec::new(), Vec::new());
        let (mut dests, mut sources) = (Vec::new(), Vec::new());
        let (mut requires, mut delivers) = (Vec::new(), Vec::new());
        let (mut target, mut symbol) = (None, false);
        // Reads before writes: a physical register read and rewritten by one
        // instruction is read as it was.
        for operand in &inst.operands {
            match operand {
                Operand::Use(Reg::Virt(v)) => {
                    let held = Held { value: v + 1, width: self.widths[*v as usize] };
                    uses.push(held.value);
                    sources.push(Loc::Held(held));
                }
                Operand::Use(Reg::Phys(register)) => {
                    if T::reserved(T::root(*register)) {
                        continue;
                    }
                    let value = self.pinned(*register).0;
                    let width = self.registers.bytes(*register).map_or(1, |bytes| bytes as u32);
                    let held = Held { value, width };
                    uses.push(value);
                    requires.push((held, *register));
                    sources.push(Loc::Held(held));
                }
                Operand::Imm(value) => sources.push(Loc::Imm(Imm { value: *value, width: 0, address: None })),
                Operand::Symbol(_, offset) => {
                    symbol = true;
                    sources.push(Loc::Imm(Imm { value: *offset, width: 0, address: None }));
                }
                Operand::Frame(slot) => {
                    sources.push(Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, i64::from(*slot))), 0)));
                }
                Operand::Block(id) => {
                    target = self.function.blocks.iter().position(|block| block.id == *id).map(|at| at as i64);
                }
                Operand::Def(_) => {}
            }
        }
        for operand in &inst.operands {
            match operand {
                Operand::Def(Reg::Virt(v)) => {
                    let held = Held { value: v + 1, width: self.widths[*v as usize] };
                    defines.push(held.value);
                    dests.push(Loc::Held(held));
                }
                Operand::Def(Reg::Phys(register)) => {
                    if T::reserved(T::root(*register)) {
                        continue;
                    }
                    let (value, width) = self.pinned(*register);
                    let held = Held { value, width };
                    defines.push(value);
                    delivers.push((held, *register));
                    dests.push(Loc::Held(held));
                }
                _ => {}
            }
        }
        // An immediate or a frame cell is as wide as the value it meets.
        let width = dests
            .iter()
            .chain(&sources)
            .find_map(|loc| if let Loc::Held(held) = loc { Some(held.width) } else { None })
            .unwrap_or(1);
        for loc in &mut sources {
            match loc {
                Loc::Imm(imm) => imm.width = width,
                Loc::Mem(mem) => mem.width = width,
                _ => {}
            }
        }
        let has_block = inst.operands.iter().any(|operand| matches!(operand, Operand::Block(_)));
        let op = if inst.call {
            Operation::Call
        } else if inst.terminator && inst.barrier {
            if has_block { Operation::Jump } else { Operation::Return }
        } else if inst.terminator {
            Operation::Branch
        } else if inst.kind == Kind::Copy {
            Operation::Move
        } else if inst.rematerializable {
            Operation::Move
        } else if defines.is_empty() && inst.sets_flags {
            Operation::Compare
        } else if !defines.is_empty() && !uses.is_empty() {
            Operation::Binary
        } else if !defines.is_empty() {
            Operation::Unary
        } else {
            Operation::Nothing
        };
        let name = match inst.kind {
            Kind::Copy => COPY,
            Kind::Op(name) => name,
        };
        let what = Semantics { name: Some(name.into()), dests, sources, target, ..Semantics::new(op) };
        let mut insn = Insn::new(at, Some((at, at + 1)), Some(what), defines, uses);
        insn.requires = requires;
        insn.delivers = delivers;
        if inst.call {
            insn.clobbers = T::caller_saved().into_iter().collect();
        }
        insn.symbol = symbol.then_some(true);
        insn.volatile = inst.side_effects;
        insn
    }
}

/// What greedy asks of the spike target's instructions, answered from the
/// instruction itself: its name and operands, and the target's own spill forms.
pub struct SpikeInstrInfo<T: Target> {
    classes: Vec<T::Class>,
    registers: Regs,
    marker: PhantomData<T>,
}

impl<T: Target> SpikeInstrInfo<T> {
    /// The class of `value`, a register of the function or one a split made.
    fn class_of(
        &self,
        value: u32,
    ) -> T::Class {
        value.checked_sub(1).and_then(|v| self.classes.get(v as usize)).copied().unwrap_or_else(T::any)
    }

    /// The instruction `made` stands beside `beside`, claiming none of its
    /// bytes.
    fn beside(
        &self,
        beside: &Insn,
        made: &Inst,
        value: u32,
        class: T::Class,
    ) -> Option<Insn> {
        let width = class_width::<T>(self.registers, class);
        let (mut defines, mut uses) = (Vec::new(), Vec::new());
        let (mut dests, mut sources) = (Vec::new(), Vec::new());
        for operand in &made.operands {
            match operand {
                Operand::Def(Reg::Virt(_)) => {
                    defines.push(value);
                    dests.push(Loc::Held(Held { value, width }));
                }
                Operand::Use(Reg::Virt(_)) => {
                    uses.push(value);
                    sources.push(Loc::Held(Held { value, width }));
                }
                Operand::Frame(slot) => {
                    let cell = Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, i64::from(*slot))), width));
                    if defines.is_empty() { dests.push(cell) } else { sources.push(cell) }
                }
                _ => return None,
            }
        }
        let name = match made.kind {
            Kind::Copy => COPY,
            Kind::Op(name) => name,
        };
        let at = beside.covers.map_or(beside.at, |covers| covers.0);
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(Operation::Move) };
        Some(Insn::new(beside.at, Some((at, at)), Some(what), defines, uses))
    }
}

/// The frame index a spill cell names.
fn slot_of(cell: &Mem) -> i32 {
    cell.addr.map_or(0, |addr| addr.disp as i32)
}

impl<T: Target> TargetInstrInfo for SpikeInstrInfo<T> {
    fn is_plain_move(
        &self,
        what: &Semantics,
    ) -> bool {
        what.op == Operation::Move && what.name.as_deref() == Some(COPY)
    }

    fn copy_semantics(
        &self,
        to: u32,
        from: u32,
        width: u32,
    ) -> Semantics {
        Semantics {
            name: Some(COPY.into()),
            dests: vec![Loc::Held(Held { value: to, width })],
            sources: vec![Loc::Held(Held { value: from, width })],
            ..Semantics::new(Operation::Move)
        }
    }

    /// One register written by a move of a literal: what the front end marked
    /// `rematerializable` converts to.
    fn remat_shape(
        &self,
        one: &Insn,
    ) -> bool {
        let Some(what) = one.what.as_ref() else { return false };
        one.defines.len() == 1
            && one.uses.is_empty()
            && one.group.is_none()
            && one.requires.is_empty()
            && one.delivers.is_empty()
            && one.clobbers.is_empty()
            && one.symbol != Some(true)
            && what.op == Operation::Move
            && matches!(
                what.dests.as_slice(),
                [Loc::Held(held)] if held.value == one.defines[0]
            )
            && matches!(what.sources.as_slice(), [Loc::Imm(_)])
    }

    fn is_trivially_re_materializable(
        &self,
        def: &Insn,
        _invariant: bool,
    ) -> bool {
        self.remat_shape(def)
    }

    fn argument_loads(
        &self,
        _body: &LirBody,
    ) -> BTreeSet<u32> {
        BTreeSet::new()
    }

    /// DEVIATE 3: no memory operand folds; `Target::fold_spill` is read by the
    /// spike allocator only. Exit: D-6502, which writes them as this hook.
    fn fold_memory_operand(
        &self,
        _one: &Arc<Insn>,
        _reg: u32,
        _cell: &Mem,
    ) -> Option<Arc<Insn>> {
        None
    }

    fn fold_copy_member(
        &self,
        _one: &Insn,
        _values: &BTreeSet<u32>,
        _frame: &mut dyn FrameInfo,
        _home: &dyn Fn(u32) -> u32,
    ) -> Result<Option<Arc<Insn>>, Error> {
        Ok(None)
    }

    fn load_reg_from_stack_slot(
        &self,
        beside: &Insn,
        into: u32,
        cell: &Mem,
    ) -> Arc<Insn> {
        let class = self.class_of(into);
        let made = T::spill_load(class, Reg::Virt(into.saturating_sub(1)), slot_of(cell))
            .and_then(|inst| self.beside(beside, &inst, into, class))
            .expect("the target states a load for a spilled register");
        let mut made = made;
        made.spill_reload = true;
        Arc::new(made)
    }

    fn store_reg_to_stack_slot(
        &self,
        beside: &Insn,
        out_of: u32,
        cell: &Mem,
    ) -> Arc<Insn> {
        let class = self.class_of(out_of);
        let made = T::spill_store(class, Reg::Virt(out_of.saturating_sub(1)), slot_of(cell))
            .and_then(|inst| self.beside(beside, &inst, out_of, class))
            .expect("the target states a store for a spilled register");
        let mut made = made;
        made.spill_store = true;
        Arc::new(made)
    }

    fn needs_data_group(
        &self,
        _one: &Insn,
    ) -> bool {
        false
    }
}

/// What greedy asks of the spike target's register file.
pub struct SpikeRegisterInfo {
    pub copy_cost: u32,
    pub csr_first_use_cost: u32,
}

impl TargetRegisterInfo for SpikeRegisterInfo {
    fn copy_cost(&self) -> u32 {
        self.copy_cost
    }

    fn csr_first_use_cost(
        &self,
        any_saved: bool,
    ) -> u32 {
        if any_saved { self.csr_first_use_cost } else { 0 }
    }
}

/// Spill slots for a function: frame indices above the ones it names.
pub struct SpikeFrame {
    slots: IndexMap<u32, i64>,
    next: i64,
}

impl SpikeFrame {
    /// A frame whose first spill slot is the frame index after every one the
    /// function names.
    pub fn of<C>(function: &MFunction<C>) -> SpikeFrame {
        let named = function
            .fixed
            .keys()
            .copied()
            .chain(function.locals.iter().map(|(index, ..)| *index))
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.insts)
                    .flat_map(|inst| &inst.operands)
                    .filter_map(|operand| if let Operand::Frame(slot) = operand { Some(*slot) } else { None }),
            )
            .max();
        SpikeFrame { slots: IndexMap::default(), next: named.map_or(0, |slot| i64::from(slot) + 1) }
    }
}

impl FrameInfo for SpikeFrame {
    fn home(
        &self,
        key: u32,
    ) -> Option<i64> {
        self.slots.get(&key).copied()
    }

    fn cell(
        &mut self,
        key: u32,
        width: u32,
    ) -> Result<Mem, Refused> {
        let next = &mut self.next;
        let slot = *self
            .slots
            .entry(key)
            .or_insert_with(
                || {
                    *next += 1;
                    *next - 1
                },
            );
        Ok(Mem::new(Some(Addr::new(Space::Frame, slot)), width))
    }

    fn set_stacks(
        &mut self,
        _stacks: IndexMap<i64, Vec<(i64, i64)>>,
    ) {
    }
}

/// The allocatable physical registers `inst` reads and writes.
fn physical<T: Target>(inst: &Inst) -> (impl Iterator<Item = RegId> + '_, impl Iterator<Item = RegId> + '_) {
    let of = |operand: &Operand, def: bool| match operand {
        Operand::Use(Reg::Phys(r)) if !def && !T::reserved(T::root(*r)) => Some(*r),
        Operand::Def(Reg::Phys(r)) if def && !T::reserved(T::root(*r)) => Some(*r),
        _ => None,
    };
    (inst.operands.iter().filter_map(move |o| of(o, false)), inst.operands.iter().filter_map(move |o| of(o, true)))
}

/// The physical registers read on some path from the entry before any
/// definition: backward liveness over the registers the function names, one
/// bit each. Cost: O(registers x (instructions + edges)), registers being
/// the target's constant.
fn entry_live<T: Target>(
    function: &MFunction<T::Class>,
    successors: &[Vec<usize>],
) -> Vec<RegId> {
    let mut names: Vec<RegId> = Vec::new();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        let (reads, writes) = physical::<T>(inst);
        for register in reads.chain(writes) {
            if !names.contains(&register) {
                names.push(register);
            }
        }
    }
    let words = names.len().div_ceil(64).max(1);
    let bit = |register: RegId| names.iter().position(|one| *one == register).unwrap();
    let (mut exposed, mut killed) =
        (vec![vec![0u64; words]; function.blocks.len()], vec![vec![0u64; words]; function.blocks.len()]);
    for (at, block) in function.blocks.iter().enumerate() {
        for inst in &block.insts {
            let (reads, writes) = physical::<T>(inst);
            for register in reads {
                let n = bit(register);
                if killed[at][n / 64] >> (n % 64) & 1 == 0 {
                    exposed[at][n / 64] |= 1 << (n % 64);
                }
            }
            for register in writes {
                let n = bit(register);
                killed[at][n / 64] |= 1 << (n % 64);
            }
        }
    }
    let mut live_in = exposed.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for at in (0..function.blocks.len()).rev() {
            for w in 0..words {
                let out = successors[at].iter().fold(0u64, |all, to| all | live_in[*to][w]);
                let now = exposed[at][w] | (out & !killed[at][w]);
                if now != live_in[at][w] {
                    live_in[at][w] = now;
                    changed = true;
                }
            }
        }
    }
    names.iter().enumerate().filter(|(n, _)| live_in[0][n / 64] >> (n % 64) & 1 == 1).map(|(_, r)| *r).collect()
}
