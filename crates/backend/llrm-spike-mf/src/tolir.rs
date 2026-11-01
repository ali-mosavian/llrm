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
//! DEVIATE 2: the `MFunction` states no block odds. A branch with one edge
//! that stays in its loop and one that leaves gets 31 in 32 to stay (LLVM's
//! loop branch heuristic); every other branch is even. Exit: the odds the
//! front end knows (the IR's branch weights).

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::Arc;

use llrm_codegen::backend::allocresult::Error;
use llrm_codegen::backend::frame::{FrameInfo, Refused};
use llrm_codegen::greedy::shadow::{Decided, ShadowInputs};
use llrm_codegen::greedy::targetinfo::TargetRegisterInfo;
use llrm_codegen::greedy::tii::TargetInstrInfo;
use llrm_codegen::model::lir::{Block as LirBlockRef, Insn, LirBlock, LirBody};
use llrm_lir::registers::{RegId, Regs};
use llrm_lir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space};
use llrm_support::hash::IndexMap;

use crate::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::{Assignment, Target, successors};

/// The name of a copy: `COPY dst, src`.
const COPY: &str = "COPY";
/// A flag-setting instruction and the one that reads its flags, as one.
const BUNDLE: &str = "BUNDLE";

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
    /// Whether a path from the entry reaches each block.
    pub reachable: Vec<bool>,
    pub tii: SpikeInstrInfo<T>,
    pub tri: SpikeRegisterInfo,
}

impl<T: Target> Converted<T> {
    /// The body with its copies joined by the translated `RegisterCoalescer`
    /// (a join narrows the survivor's registers to those both could take).
    /// Run before [`Converted::inputs`]; a copy to or from a pinned value
    /// joins into it, and [`from_lir`] writes that value as the register.
    pub fn coalesce(
        &mut self,
        registers: Regs,
        size: bool,
    ) {
        let palettes = self
            .orders
            .iter()
            .map(|(value, order)| (*value, order.iter().copied().collect::<BTreeSet<RegId>>()))
            .collect();
        let (body, palettes) =
            llrm_codegen::coalescer::env::coalesce(&self.body, palettes, self.body.pins.clone(), size, registers);
        for (value, order) in self.orders.iter_mut() {
            if let Some(allowed) = palettes.get(value) {
                order.retain(|register| allowed.contains(register));
            }
        }
        self.body = body;
    }

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
    let reachable = reachable(&successors);
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
    // every physical register is a value before the first instruction, so a
    // call (which writes the ones it clobbers) can say so
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        let (reads, writes) = physical::<T>(inst);
        for register in reads.chain(writes) {
            made.pinned(register);
        }
    }
    let mut foldable: Vec<Option<(&'static str, usize)>> = Vec::new();
    let mut blocks = Vec::with_capacity(function.blocks.len());
    for (at, block) in function.blocks.iter().enumerate() {
        let mut insns = Vec::with_capacity(block.insts.len());
        // DEVIATE 2b: greedy is not given a block no path reaches (LLVM
        // removes them first); `from_lir` returns it as it was.
        let mut position = 0;
        while reachable[at] && position < block.insts.len() {
            let inst = &block.insts[position];
            let ordinal = made.origin.len() as i64;
            made.origin.push((at, position));
            foldable.push(foldable_source::<T>(inst));
            // A copy, spill or reload overwrites the flags a compare set, and
            // greedy puts them where a value is read: the compare and the
            // instruction that reads its flags are one instruction to it.
            let reader = block
                .insts
                .get(position + 1)
                .filter(
                    |next| inst.sets_flags
                        && !inst.terminator
                        && !inst.call
                        && next.reads_flags
                        && T::reads_flags_set_just_before(next),
                );
            match reader {
                Some(next) if made.independent(inst, next) => {
                    made.origin.push((at, position + 1));
                    foldable.push(None);
                    insns.push(Arc::new(made.bundle(inst, next, 2 * ordinal)));
                    position += 2;
                }
                _ => {
                    insns.push(Arc::new(made.insn(inst, 2 * ordinal)));
                    position += 1;
                }
            }
        }
        let mut lir = LirBlock::new(at as i64, insns);
        if reachable[at] {
            lir.succ = successors[at].iter().map(|to| *to as i64).collect();
        }
        blocks.push(LirBlockRef::from(lir));
    }
    let entry = if function.blocks.is_empty() { 0 } else { 0 };
    let mut body = LirBody::new(function.name.clone(), entry, blocks, IndexMap::default(), made.pins.clone());
    loop_branch_odds(&mut body);
    body.inputs = entry_live::<T>(function, &successors).iter().filter_map(|r| made.values.get(r).copied()).collect();
    let mut orders: IndexMap<u32, Vec<RegId>> = IndexMap::default();
    let clobbered_by_spills = T::spill_clobbers();
    for (v, class) in function.classes.iter().enumerate() {
        let mut order = T::candidates(*class);
        order.retain(|register| !clobbered_by_spills.contains(register));
        orders.insert(v as u32 + 1, order);
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
        reachable,
        tii: SpikeInstrInfo { classes: function.classes.clone(), registers, foldable, marker: PhantomData },
        tri: SpikeRegisterInfo {
            copy_cost: 1,
            csr_first_use_cost: T::csr_first_use_cost(),
            csr_cost_scale: T::csr_cost_scale(),
            csr_spill_first: T::csr_spill_first(),
            register_cost: T::register_cost,
        },
    })
}

/// The edge odds of a branch with one edge that stays in its innermost loop
/// and one that leaves it: 31 in 32 to stay (`calcLoopBranchHeuristics`,
/// weights 124 and 4). The `MFunction` states no odds, so the other branches
/// keep greedy's even split.
fn loop_branch_odds(body: &mut LirBody) {
    use llrm_codegen::model::lir::BlockOdds;
    let loops = llrm_codegen::analysis::loops::loops(&body.blocks, Some(body.entry));
    let mut taken = Vec::new();
    for block in body.blocks.iter() {
        let mut distinct: Vec<i64> = Vec::new();
        for to in block.succ.iter() {
            if !distinct.contains(to) {
                distinct.push(*to);
            }
        }
        let [first, second] = distinct.as_slice() else { continue };
        let Some(holding) = loops.iter().find(|one| one.body.contains(&block.at)) else { continue };
        let stays = [holding.body.contains(first), holding.body.contains(second)];
        if stays[0] == stays[1] {
            continue;
        }
        for (to, stay) in [(*first, stays[0]), (*second, stays[1])] {
            let odds = if stay { 31.0 / 32.0 } else { 1.0 / 32.0 };
            taken.push(((block.at, to), (odds * BlockOdds::CERTAIN).round() as u32));
        }
    }
    body.odds.taken.extend(taken);
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

    /// Whether `next` reads no virtual register `first` sets: one instruction
    /// cannot read what it also defines. A physical register it reads from
    /// `first` (the flags) is passed inside the bundle.
    fn independent(
        &self,
        first: &Inst,
        next: &Inst,
    ) -> bool {
        !first
            .operands
            .iter()
            .any(
                |set| matches!(
                    set,
                    Operand::Def(register @ Reg::Virt(_)) if next.operands.iter().any(|read| *read == Operand::Use(*register))
                ),
            )
    }

    /// `first` and `next` as one instruction: the operands of the two, in
    /// order, which [`from_lir`] takes apart again.
    fn bundle(
        &mut self,
        first: &Inst,
        next: &Inst,
        at: i64,
    ) -> Insn {
        let (one, two) = (self.insn(first, at), self.insn(next, at + 2));
        let (a, b) = (one.what.as_ref().expect("converted"), two.what.as_ref().expect("converted"));
        let what = Semantics {
            name: Some(BUNDLE.into()),
            dests: a.dests.iter().chain(&b.dests).cloned().collect(),
            sources: a.sources.iter().chain(&b.sources).cloned().collect(),
            target: b.target,
            ..Semantics::new(b.op)
        };
        let join = |x: &[u32], y: &[u32]| {
            let mut all = x.to_vec();
            for value in y {
                if !all.contains(value) {
                    all.push(*value);
                }
            }
            all
        };
        // what `next` reads of what `first` sets is read inside the bundle
        let inside = |value: &u32| one.defines.contains(value);
        let reads: Vec<u32> = two.uses.iter().copied().filter(|value| !inside(value)).collect();
        let mut insn =
            Insn::new(at, Some((at, at + 3)), Some(what), join(&one.defines, &two.defines), join(&one.uses, &reads));
        insn.requires =
            one.requires.iter().chain(two.requires.iter().filter(|(held, _)| !inside(&held.value))).cloned().collect();
        insn.delivers = one.delivers.iter().chain(&two.delivers).cloned().collect();
        insn.clobbers = one.clobbers.union(&two.clobbers).copied().collect();
        insn.symbol = if one.symbol == Some(true) || two.symbol == Some(true) { Some(true) } else { None };
        insn.volatile = one.volatile || two.volatile;
        insn
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
            // The call writes each clobbered register it names: a value that
            // lives across it cannot be the register, and a read after it
            // sees what the call left.
            for (register, value) in &self.values {
                let clobbered =
                    insn.clobbers.contains(register) || insn.clobbers.contains(&self.registers.root(*register));
                if clobbered && !insn.defines.contains(value) {
                    insn.defines.push(*value);
                }
            }
        }
        insn.symbol = symbol.then_some(true);
        insn.volatile = inst.side_effects;
        insn
    }
}

/// The name of `inst` and the index, among the sources the converter gives it,
/// of the read the target can take from a frame slot.
fn foldable_source<T: Target>(inst: &Inst) -> Option<(&'static str, usize)> {
    let Kind::Op(name) = inst.kind else { return None };
    let mut source = 0;
    for operand in &inst.operands {
        match operand {
            Operand::Use(Reg::Virt(v)) => {
                if T::fold_spill(inst, Reg::Virt(*v), 0).is_some() {
                    return Some((name, source));
                }
                source += 1;
            }
            Operand::Use(Reg::Phys(r)) if !T::reserved(T::root(*r)) => source += 1,
            Operand::Imm(_) | Operand::Symbol(..) | Operand::Frame(_) => source += 1,
            _ => {}
        }
    }
    None
}

/// What greedy asks of the spike target's instructions, answered from the
/// instruction itself: its name and operands, and the target's own spill forms.
pub struct SpikeInstrInfo<T: Target> {
    classes: Vec<T::Class>,
    registers: Regs,
    /// For the instruction numbered `at / 2`: its name and which of its
    /// sources the target can read from memory (`Target::fold_spill`).
    foldable: Vec<Option<(&'static str, usize)>>,
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
        let width = held_width(beside, value).unwrap_or_else(|| class_width::<T>(self.registers, class));
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
                // what the form clobbers is put back by the way back
                // ([`from_lir`]), which asks the target for the form again
                Operand::Def(Reg::Phys(_)) => {}
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

/// How wide `insn` holds `value`.
fn held_width(
    insn: &Insn,
    value: u32,
) -> Option<u32> {
    let what = insn.what.as_ref()?;
    what.dests
        .iter()
        .chain(&what.sources)
        .find_map(
            |loc| match loc {
                Loc::Held(held) if held.value == value => Some(held.width),
                _ => None,
            },
        )
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

    /// The read the target takes from memory (`Target::fold_spill`) becomes a
    /// memory source; `from_lir` writes it as the target's own form.
    fn fold_memory_operand(
        &self,
        one: &Arc<Insn>,
        reg: u32,
        cell: &Mem,
    ) -> Option<Arc<Insn>> {
        let what = one.what.as_ref()?;
        let (name, at) = self.foldable.get((one.at / 2) as usize).copied().flatten()?;
        if what.name.as_deref() != Some(name) || one.group.is_some() {
            return None;
        }
        let read = |loc: &Loc| matches!(loc, Loc::Held(held) if held.value == reg);
        let named = what.dests.iter().chain(&what.sources).filter(|loc| read(loc)).count();
        let Some(Loc::Held(held)) = what.sources.get(at).filter(|loc| read(loc)) else { return None };
        if named != 1 || held.width as u32 > cell.width {
            return None;
        }
        let mut sources = what.sources.clone();
        sources[at] = Loc::Mem(Mem { width: held.width as u32, ..cell.clone() });
        let mut out = (**one).clone();
        out.what = Some(Semantics { sources, ..what.clone() });
        out.uses.retain(|value| *value != reg);
        Some(Arc::new(out))
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
    pub csr_cost_scale: u32,
    pub csr_spill_first: bool,
    pub register_cost: fn(RegId) -> u8,
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

    fn csr_cost_scale(&self) -> u32 {
        self.csr_cost_scale
    }

    fn csr_spill_first(&self) -> bool {
        self.csr_spill_first
    }

    fn reg_cost(
        &self,
        phys: RegId,
    ) -> u8 {
        (self.register_cost)(phys)
    }

    fn lanes_saved_apart(&self) -> bool {
        true
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

/// The blocks a path from block 0 reaches.
fn reachable(successors: &[Vec<usize>]) -> Vec<bool> {
    let mut seen = vec![false; successors.len()];
    let mut work = if successors.is_empty() { Vec::new() } else { vec![0] };
    while let Some(at) = work.pop() {
        if !std::mem::replace(&mut seen[at], true) {
            work.extend(successors[at].iter().copied().filter(|to| !seen[*to]));
        }
    }
    seen
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

/// `source` with the registers greedy chose: its definitions take the
/// destinations in order, its reads the sources.
fn substitute<'a, T: Target>(
    source: &Inst,
    dests: &mut impl Iterator<Item = &'a Loc>,
    sources: &mut impl Iterator<Item = &'a Loc>,
    number: &mut dyn FnMut(u32) -> Reg,
    folds: &mut Vec<(Reg, i32)>,
) -> Result<Inst, String> {
    let mut made = source.clone();
    for operand in &mut made.operands {
        *operand = match operand.clone() {
            Operand::Def(Reg::Virt(_)) => match dests.next() {
                Some(Loc::Held(held)) => Operand::Def(number(held.value)),
                _ => return Err("a definition that is not a register".into()),
            },
            Operand::Def(Reg::Phys(r)) => {
                if !T::reserved(T::root(r)) {
                    dests.next();
                }
                Operand::Def(Reg::Phys(r))
            }
            Operand::Use(Reg::Virt(_)) => match sources.next() {
                Some(Loc::Held(held)) => Operand::Use(number(held.value)),
                // a read taken from its frame slot: a register stands in for
                // it until the target writes the folded form
                Some(Loc::Mem(cell)) => {
                    let stand_in = number(u32::MAX - folds.len() as u32);
                    folds.push((stand_in, slot_of(cell)));
                    Operand::Use(stand_in)
                }
                _ => return Err("a use that is not a register".into()),
            },
            Operand::Use(Reg::Phys(r)) => {
                if !T::reserved(T::root(r)) {
                    sources.next();
                }
                Operand::Use(Reg::Phys(r))
            }
            one @ (Operand::Imm(_) | Operand::Symbol(..) | Operand::Frame(_)) => {
                sources.next();
                one
            }
            other => other,
        };
    }
    Ok(made)
}

/// What greedy's edited function is as a machine function again.
pub struct Rewritten<C> {
    pub function: MFunction<C>,
    /// The register of each virtual register of `function`.
    pub assignment: Assignment,
    /// The frame slots the spills use, each with the class of what it holds.
    pub slots: Vec<(i32, C)>,
}

/// The machine function greedy's `Mode::Inline` result stands for, the way
/// `allocate_spilling` leaves one: the original instructions with the
/// registers greedy chose (`instruction.at / 2` names the original), the
/// spills and reloads as the target writes them, a clone's class its
/// parent's, and the register of each value.
pub fn from_lir<T: Target>(
    original: &MFunction<T::Class>,
    converted: &Converted<T>,
    decided: &Decided,
    registers: Regs,
) -> Result<Rewritten<T::Class>, String> {
    let edited = decided.edited.as_ref().ok_or_else(|| format!("{}: greedy returned no edited body", original.name))?;
    let known = original.classes.len() as u32;
    let mut classes = original.classes.clone();
    // A value of the function keeps its register number; a clone takes the
    // next one and its parent's class.
    let mut numbers: IndexMap<u32, u32> = IndexMap::default();
    let pinned = &converted.body.pins;
    let mut numbered = |value: u32, classes: &mut Vec<T::Class>| -> u32 {
        if value <= known {
            return value - 1;
        }
        *numbers.entry(value).or_insert_with(|| {
            let mut root = value;
            while root > known {
                match decided.parents.get(&root) {
                    Some(parent) => root = *parent,
                    None => break,
                }
            }
            classes.push(if root >= 1 && root <= known { original.classes[root as usize - 1] } else { T::any() });
            classes.len() as u32 - 1
        })
    };
    let mut number = |value: u32, classes: &mut Vec<T::Class>| -> Reg {
        match pinned.get(&value) {
            Some(register) => Reg::Phys(*register),
            None => Reg::Virt(numbered(value, classes)),
        }
    };
    // The literal and address definitions of the function by what they set:
    // the spiller makes one again where it is read, or puts it in place of a
    // copy of the value, and that instruction stands at another one's place.
    let mut definitions: IndexMap<String, (usize, usize)> = IndexMap::default();
    for block in &converted.body.blocks {
        for insn in block.insns.iter() {
            let (Some(what), Some(origin)) = (&insn.what, converted.origin.get((insn.at / 2) as usize)) else {
                continue;
            };
            if insn.defines.len() == 1 && insn.uses.is_empty() {
                definitions.entry(format!("{:?}{:?}", what.name, what.sources)).or_insert(*origin);
            }
        }
    }
    let mut slots: Vec<(i32, T::Class)> = Vec::new();
    let mut blocks = Vec::with_capacity(edited.blocks.len());
    for (at, block) in edited.blocks.iter().enumerate() {
        if !converted.reachable[at] {
            blocks.push(original.blocks[at].clone());
            continue;
        }
        let mut insts = Vec::with_capacity(block.insns.len());
        for insn in block.insns.iter() {
            let what = insn.what.as_ref().ok_or("an instruction with no semantics")?;
            let held = |locs: &[Loc]| -> Vec<u32> {
                locs.iter().filter_map(|loc| if let Loc::Held(held) = loc { Some(held.value) } else { None }).collect()
            };
            if (insn.spill_store || insn.spill_reload) && !insn.rematerialized {
                let value =
                    held(&what.dests).into_iter().chain(held(&what.sources)).next().ok_or("a spill of nothing")?;
                let cell = what
                    .dests
                    .iter()
                    .chain(&what.sources)
                    .find_map(|loc| if let Loc::Mem(cell) = loc { Some(cell) } else { None });
                let slot = slot_of(cell.ok_or_else(|| format!("a spill with no cell: {what:?}"))?);
                let register = number(value, &mut classes);
                let class = classes[match register {
                    Reg::Virt(v) => v as usize,
                    Reg::Phys(_) => 0,
                }];
                let made = if insn.spill_store {
                    T::spill_store(class, register, slot)
                } else {
                    T::spill_load(class, register, slot)
                };
                insts.push(made.ok_or("the target states no spill form")?);
                if !slots.iter().any(|(one, _)| *one == slot) {
                    slots.push((slot, class));
                }
                continue;
            }
            if what.name.as_deref() == Some(BUNDLE) {
                let (mut dests, mut sources) = (what.dests.iter(), what.sources.iter());
                for ordinal in [(insn.at / 2) as usize, (insn.at / 2) as usize + 1] {
                    let (from, inst) = *converted.origin.get(ordinal).ok_or("a bundle from nowhere")?;
                    let source = &original.blocks[from].insts[inst];
                    let mut folds = Vec::new();
                    insts.push(substitute::<T>(
                        source,
                        &mut dests,
                        &mut sources,
                        &mut |value| number(value, &mut classes),
                        &mut folds,
                    )?);
                    if !folds.is_empty() {
                        return Err("a bundle with a memory source".into());
                    }
                }
                continue;
            }
            let (mut from, mut inst) =
                *converted.origin.get((insn.at / 2) as usize).ok_or("an instruction from nowhere")?;
            let name = what.name.as_deref();
            let agrees = match original.blocks[from].insts[inst].kind {
                Kind::Copy => name == Some(COPY),
                Kind::Op(op) => name == Some(op),
            };
            // a literal made again stands beside any instruction: its position
            // says nothing about which literal it is
            if (insn.rematerialized || !agrees) && name != Some(COPY) {
                (from, inst) = *definitions
                    .get(&format!("{:?}{:?}", what.name, what.sources))
                    .ok_or_else(|| format!("an instruction that is no definition of the function: {what:?}"))?;
            }
            let source = &original.blocks[from].insts[inst];
            // a copy a split made stands beside an instruction and claims none
            // of its bytes, whatever that instruction is
            let made_by_split = insn.covers.is_some_and(|(from, to)| from == to);
            if name == Some(COPY) && (source.kind != Kind::Copy || made_by_split) {
                let (Some(to), Some(from)) = (held(&what.dests).first().copied(), held(&what.sources).first().copied())
                else {
                    return Err("a copy that does not move a value".into());
                };
                insts.push(Inst::new(
                    Kind::Copy,
                    vec![Operand::Def(number(to, &mut classes)), Operand::Use(number(from, &mut classes))],
                ));
                continue;
            }
            let (mut dests, mut sources) = (what.dests.iter(), what.sources.iter());
            let mut folds = Vec::new();
            let mut made = substitute::<T>(
                source,
                &mut dests,
                &mut sources,
                &mut |value| number(value, &mut classes),
                &mut folds,
            )?;
            for (stand_in, slot) in folds {
                made = T::fold_spill(&made, stand_in, slot).ok_or("a memory source the target has no form for")?;
            }
            insts.push(made);
        }
        blocks.push(Block { id: original.blocks[at].id, insts, phis: Vec::new() });
    }
    let mut assignment = vec![None; classes.len()];
    for (value, register) in &decided.placed {
        if converted.body.pins.contains_key(value) {
            continue;
        }
        let v = numbered(*value, &mut classes) as usize;
        if v >= assignment.len() {
            assignment.resize(v + 1, None);
        }
        let class = classes[v];
        assignment[v] =
            Some(if registers.root(*register) == *register { T::view(*register, class) } else { *register });
    }
    // A register nothing reads is anywhere its class allows.
    let assignment = assignment
        .iter()
        .enumerate()
        .map(|(v, one)| {
            one.or_else(|| T::candidates(classes[v]).first().copied()).ok_or("a register with no candidates")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let function = MFunction { blocks, classes, ..original.clone() };
    Ok(Rewritten { function, assignment: Assignment(assignment), slots })
}
