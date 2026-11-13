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
//! at another time are fine. Exit: the lane model (sub-register lanes in
//! live intervals), 2026-11-30, with DEVIATE 3.
//!
//! DEVIATE 3 (beyond the pin): a part of a lane-built value
//! (`Target::lane_moves`: z80 `PACK`, 6502 `PACK`/`UNPACK`) is hinted the lane
//! of the register its whole is copied to or from. The pin places the lanes by
//! coalescing the copies `eliminateRegSequence` makes
//! (RegisterCoalescer.cpp#L464). Exit: the lane model, 2026-11-30.
//!
//! DEVIATE 2: a target whose `MFunction` states no odds (`odds` is `None`)
//! gets 31 in 32 to stay on a branch with one edge that stays in its loop and
//! one that leaves (LLVM's loop branch heuristic); every other branch is
//! even. Exit: each target states the IR's edge probabilities.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use llrm_codegen::backend::allocresult::Error;
use llrm_codegen::backend::frame::{FrameInfo, Refused};
use llrm_codegen::greedy::shadow::{Decided, ShadowInputs};
use llrm_codegen::greedy::targetinfo::TargetRegisterInfo;
use llrm_codegen::greedy::tii::TargetInstrInfo;
use llrm_codegen::model::lir::{Block as LirBlockRef, Insn, LirBlock, LirBody};
use llrm_lir::registers::{RegId, Regs};
use llrm_lir::valuelanes::ValueLanes;
use llrm_lir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space};
use llrm_support::hash::IndexMap;

use crate::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::{Assignment, Join, Side, Target, successors};

/// The name of a copy: `COPY dst, src`.
const COPY: &str = "COPY";
/// A flag-setting instruction and the one that reads its flags, as one.
const BUNDLE: &str = "BUNDLE";

/// The number the converter gave the instruction `one` was made from: the key
/// into [`Converted::origin`] and the target's per-instruction answers. It is
/// `numbered`, which a pass that moves an instruction keeps, and `at / 2` for
/// one nothing numbered. `at` is not it: `MachineSink` sets the `at` of an
/// instruction it moves to its new neighbour's.
pub fn ordinal_of(one: &Insn) -> usize {
    one.numbered.map_or((one.at / 2) as usize, |n| n as usize)
}

/// For each instruction of `block`: whether a later one reads the target's flags
/// register by name before another writes it (a `sets_flags` instruction or a
/// named write). Empty answers where there is no flags register or the mode
/// models none.
fn flags_read_after<T: Target>(
    block: &crate::mfunc::Block,
    ssa_pins: bool,
) -> Vec<bool> {
    let mut wanted = vec![false; block.insts.len()];
    let Some(flags) = T::flags_register().filter(|_| ssa_pins) else { return wanted };
    let root = T::root(flags);
    let named = |reg: &Reg| matches!(reg, Reg::Phys(r) if T::root(*r) == root);
    let mut later = false;
    for (at, inst) in block.insts.iter().enumerate().rev() {
        wanted[at] = later;
        if inst.sets_flags() || inst.operands.iter().any(|operand| matches!(operand, Operand::Def(reg) if named(reg))) {
            later = false;
        }
        if inst.operands.iter().any(|operand| matches!(operand, Operand::Use(reg) if named(reg))) {
            later = true;
        }
    }
    wanted
}

fn numbered(
    mut one: Insn,
    ordinal: i64,
) -> Insn {
    one.numbered = Some(ordinal as u32);
    one
}

/// A converted function and what greedy asks of the target around it.
pub struct Converted<T: Target> {
    pub body: LirBody,
    /// The registers a value may take, most preferred first.
    orders: IndexMap<u32, Vec<RegId>>,
    /// The value each one a shared phase made is a copy of (`TwoAddress`'s
    /// fresh register for a tied source): its order and class are its parent's.
    derived: IndexMap<u32, u32>,
    /// Values pinned to a register: they cannot be spilled.
    pub unspillable: BTreeSet<u32>,
    /// The roots a call keeps.
    pub saved: Vec<RegId>,
    /// `(block, instruction)` of the instruction numbered `at / 2`.
    pub origin: Vec<(usize, usize)>,
    /// Whether a path from the entry reaches each block.
    pub reachable: Vec<bool>,
    /// Where each value [`Converted::coalesce`] absorbed went.
    joined: llrm_codegen::coalescer::env::JoinMap,
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
        self.coalesce_pinned(registers, size, T::joins_pinned_copies());
    }

    /// [`Converted::coalesce`], with `joins_pinned` stating whether a copy
    /// to or from a pinned value joins (`Target::joins_pinned_copies`).
    pub fn coalesce_pinned(
        &mut self,
        registers: Regs,
        size: bool,
        joins_pinned: bool,
    ) {
        let infos = self.tii.infos.clone();
        let kind_infos = infos.clone();
        let kinds = Rc::new(RefCell::new(UseKindTable::default()));
        let priced = Rc::clone(&kinds);
        let palettes = shared_palettes(&self.orders);
        let originals = palettes.clone();
        let (body, palettes, joined) = llrm_codegen::coalescer::env::coalesce(
            &self.body,
            palettes,
            self.body.pins.clone(),
            size,
            T::joins_global_copies(),
            joins_pinned,
            T::joins_sub_registers(),
            T::joins_lanes(),
            T::joins_loop_carried_first(),
            T::joins_unnarrowed_for_free(),
            registers,
            Some(Box::new(move |join| {
                let joined: Vec<RegId> = join.joined.iter().copied().collect();
                let side = |side: &llrm_codegen::coalescer::env::JoinSide| {
                    let before: Vec<RegId> = side.palette.iter().copied().collect();
                    // a kind is one instruction's operands that name the value:
                    // priced once however many of them there are
                    let worst = |operands: &[OperandInfo]| {
                        operands
                            .iter()
                            .filter_map(|info| {
                                let (was, will) = (info.cheapest(&before)?, info.cheapest(&joined)?);
                                Some(will.saturating_sub(was))
                            })
                            .max()
                            .unwrap_or(0)
                    };
                    let kinds = priced.borrow();
                    let extra = side
                        .kinds
                        .iter()
                        .map(|(kind, freq)| *freq * worst(&kinds.operands[*kind as usize]) as f32)
                        .sum::<f32>();
                    if !side.uses.is_empty() {
                        // LLRM_CHECK_USE_SUMMARY: the list of every instruction
                        // prices the same
                        let slow = side
                            .uses
                            .iter()
                            .map(|(one, freq)| {
                                let named: Vec<OperandInfo> = operands_in(&infos, one)
                                    .into_iter()
                                    .filter(|(value, _)| *value == side.value)
                                    .map(|(_, info)| info.clone())
                                    .collect();
                                *freq * worst(&named) as f32
                            })
                            .sum::<f32>();
                        assert!(
                            (slow - extra).abs() <= 1e-3 * (1.0 + slow.abs()),
                            "value {}: the instructions price {slow}, the summary {extra}",
                            side.value
                        );
                    }
                    (before, extra)
                };
                let ((source, source_extra), (destination, destination_extra)) = (side(&join.one), side(&join.other));
                T::should_coalesce(&Join {
                    source: Side { registers: &source, extra_cost: source_extra },
                    destination: Side { registers: &destination, extra_cost: destination_extra },
                    joined: &joined,
                    copy_frequency: join.copy_frequency,
                })
            })),
            Some(Box::new(|one: &Insn| is_cheap_move::<T>(one))),
            Some(Box::new(|literal, source, destination, size, alone| {
                let (source, destination): (Vec<RegId>, Vec<RegId>) =
                    (source.iter().copied().collect(), destination.iter().copied().collect());
                literal_remat_wins::<T>(literal, &source, &destination, size, alone)
            })),
            Box::new(move |one, value| {
                // which of the instruction's operand slots name the value: the
                // same instruction shape and slots are one kind, however many
                // instructions have them
                let shape = ordinal_of(one);
                let (_, slots) = kind_infos.get(shape)?;
                let what = one.what.as_ref()?;
                if what.name.as_deref() != Some(kind_infos[shape].0) {
                    return None;
                }
                let mut named = 0u64;
                for (at, info) in slots.iter().enumerate().take(64) {
                    let loc = if info.def { what.dests.get(info.index) } else { what.sources.get(info.index) };
                    if matches!(loc, Some(Loc::Held(held)) if held.value == value) {
                        named |= 1 << at;
                    }
                }
                if named == 0 {
                    return None;
                }
                let mut table = kinds.borrow_mut();
                if let Some(id) = table.by_shape.get(&(shape, named)) {
                    return Some(*id);
                }
                let mut operands: Vec<OperandInfo> = slots
                    .iter()
                    .enumerate()
                    .filter(|(at, _)| named & (1 << at) != 0)
                    .map(|(_, info)| info.clone())
                    .collect();
                operands.sort_by(|a, b| (&a.order, &a.costs).cmp(&(&b.order, &b.costs)));
                operands.dedup_by(|a, b| (&a.order, &a.costs) == (&b.order, &b.costs));
                let id = table.intern(operands);
                table.by_shape.insert((shape, named), id);
                Some(id)
            }),
        );
        for (value, order) in self.orders.iter_mut() {
            // A set no join narrowed is the one it started as.
            if let Some(allowed) =
                palettes.get(value).filter(|now| !originals.get(value).is_some_and(|was| Rc::ptr_eq(now, was)))
            {
                order.retain(|register| allowed.contains(register));
            }
        }
        self.body = body;
        self.joined = joined;
    }

    /// The body ordered by the translated pre-RA `MachineScheduler`
    /// (`machinesched::scheduled`): each value counts against the pressure set
    /// of its class, whose limit is the registers the class may take.
    ///
    /// Memory is ordered by what each access says it reaches (`Inst::memory`,
    /// `Insn::may_alias`); an access that says nothing stays ordered with all
    /// (was DEVIATE MS6, retired 2026-11-09).
    /// DEVIATE MS7: the graph's latencies are the shared defaults, not the
    /// CPU's scheduling model (the pin's generic scheduler reads
    /// `SchedModel`). Exit: an arm64 `SchedModel`.
    pub fn schedule(
        &mut self,
        function: &MFunction<T::Class>,
        registers: Regs,
    ) {
        let sets = pressure_sets::<T>(&function.classes);
        let limit = T::candidates(T::any()).len();
        let known = function.classes.len() as u32;
        let pins = self.body.pins.clone();
        // The lanes of the file are the ones read while the scheduler runs.
        let _lanes = llrm_codegen::backend::lanes::enter(registers);
        // The scheduler reads the register file off the body; allocation does
        // not.
        let before = self.body.registers.replace(registers);
        let allocatable = self.body.clone();
        self.body = self.with_effects(function, registers);
        let scheduled = llrm_codegen::backend::machinesched::scheduled(&self.body, limit, &*T::scheduling_model(), |_, counted| {
            let mut model =
                llrm_codegen::backend::regpressure::Model::new(sets.iter().map(|set| set.len() as u32).collect());
            for value in 1..=known {
                if pins.contains_key(&value) || !counted(value) {
                    continue;
                }
                let class = function.classes[value as usize - 1];
                model.count(value, 1, std::rc::Rc::from(sets_of::<T>(&sets, class)));
            }
            model
        });
        // What allocation reads is the original instruction, in the order the
        // scheduler chose.
        // By identity: the coalescer's clones of an instruction share its `at`.
        let original: llrm_support::hash::HashMap<u64, Arc<Insn>> = allocatable
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter().map(|one| (one.id(), Arc::clone(one))))
            .collect();
        let blocks = scheduled
            .blocks
            .iter()
            .map(|block| block.with_insns(block.insns.iter().map(|one| Arc::clone(&original[&one.id()])).collect()))
            .collect();
        self.body = allocatable.with_blocks(blocks);
        self.body.registers = before;
    }

    /// A copy of the body whose instructions state their effects, for the
    /// scheduler: the shared decoder knows no arm64 name. An instruction reads
    /// and writes the registers its pinned values stand for, the status flags
    /// (one lane) where it reads or sets them, and memory (one more lane)
    /// where it loads or stores; one with side effects is a barrier.
    /// Gives each copy a shared phase made its own instruction number
    /// ([`is_shared_copy`]): it stood on its neighbour's, so the target's
    /// answers about operands, memory forms and effects were the neighbour's.
    /// A copy has no operand info, as the converter's own copies have none.
    pub fn adopt(&mut self) {
        let blocks = self
            .body
            .blocks
            .iter()
            .map(|block| {
                if !block.insns.iter().any(|one| is_shared_copy(one)) {
                    return block.clone();
                }
                let insns: Vec<_> = block
                    .insns
                    .iter()
                    .map(|one| {
                        if !is_shared_copy(one) {
                            return Arc::clone(one);
                        }
                        let ordinal = self.origin.len() as i64;
                        let beside = self.origin.get(ordinal_of(one)).copied().unwrap_or((0, 0));
                        self.origin.push(beside);
                        self.tii.foldable.push(None);
                        self.tii.infos.push(("", Vec::new()));
                        let mut made = (**one).clone();
                        made.at = 2 * ordinal;
                        made.numbered = Some(ordinal as u32);
                        Arc::new(made)
                    })
                    .collect();
                block.with_insns(insns)
            })
            .collect();
        self.body = self.body.with_blocks(blocks);
        // a value `TwoAddress` made is held to what the value it copies is
        let known = self.tii.classes.len();
        let copies: Vec<(u32, u32)> = self
            .body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|one| is_shared_copy(one))
            .filter_map(|one| match (one.defines.first(), one.uses.first()) {
                (Some(&to), Some(&from)) => Some((to, from)),
                _ => None,
            })
            .collect();
        let mut changed = true;
        while changed {
            changed = false;
            for &(to, from) in &copies {
                if self.orders.contains_key(&to) || self.body.pins.contains_key(&to) {
                    continue;
                }
                let Some(order) = self.orders.get(&from).cloned() else { continue };
                self.orders.insert(to, order);
                self.derived.insert(to, from);
                let wanted = (to as usize).max(self.tii.classes.len());
                self.tii.classes.resize(wanted, T::any());
                let class =
                    from.checked_sub(1).and_then(|v| self.tii.classes.get(v as usize)).copied().unwrap_or_else(T::any);
                self.tii.classes[to as usize - 1] = class;
                changed = true;
            }
        }
        debug_assert!(self.tii.classes.len() >= known);
    }

    pub fn with_effects(
        &self,
        function: &MFunction<T::Class>,
        registers: Regs,
    ) -> LirBody {
        use llrm_codegen::backend::lanes::Lanes;
        let bits = self.body.bits;
        let flags = Lanes::flags(1);
        let memory = Lanes::flags(Lanes::MEMORY);
        let blocks = self
            .body
            .blocks
            .iter()
            .map(|block| {
                let insns = block
                    .insns
                    .iter()
                    .map(|one| {
                        let Some(&(from, at)) = self.origin.get(ordinal_of(one)) else { return Arc::clone(one) };
                        if is_shared_copy(one) {
                            return Arc::clone(one);
                        }
                        let inst = &function.blocks[from].insts[at];
                        let named = |values: &[u32]| -> Lanes {
                            values
                                .iter()
                                .filter_map(|value| self.body.pins.get(value))
                                // the zero register is no lane: nothing writes
                                // it
                                .filter(|register| registers.known(**register))
                                // the stack pointer a call rewrites has no
                                // lanes
                                .filter(|register| Lanes::exists(**register))
                                .map(|register| Lanes::span(*register, 0, 1))
                                .fold(Lanes::new(), |all, one| all.or(&one))
                        };
                        let (mut reads, mut writes) = (named(&one.uses), named(&one.defines));
                        let bundle = one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some(BUNDLE));
                        if inst.reads_flags() || bundle {
                            reads.extend(flags.iter().copied());
                        }
                        if inst.sets_flags() || bundle {
                            writes.extend(flags.iter().copied());
                        }
                        if inst.may_load {
                            reads.extend(memory.iter().copied());
                        }
                        if inst.may_store {
                            writes.extend(memory.iter().copied());
                            reads.extend(memory.iter().copied());
                        }
                        let mut made = (**one).clone().standing_for(one);
                        // The form names register 31 as the zero register
                        // wherever it is the stack pointer; the decoded
                        // semantics name `sp`, which `boundary` needs
                        // (`sub sp, sp, #16` moved below the stores it makes
                        // room for: `vla_call` printed 918, not 1213).
                        let writes_sp = one
                            .what
                            .as_ref()
                            .is_some_and(
                                |what| what.dests
                                    .iter()
                                    .any(|place| matches!(place, Loc::Reg(reg) if registers.is_stack(reg.register))),
                            );
                        if let Some(form) = inst
                            .form
                            .as_ref()
                            .filter(|_| !writes_sp)
                            .and_then(|form| in_body_values::<T>(form, inst, one))
                        {
                            made.what = Some(form);
                        }
                        made.volatile = made.volatile || inst.side_effects;
                        made.memory = inst.memory;
                        if let Some(what) = made.what.as_mut() {
                            if what.op == Operation::Nothing && inst.may_store {
                                what.op = Operation::Binary;
                            }
                        }
                        for slot in &made.register_effects {
                            slot.get_or_init(|| (bits, Some((reads.clone(), writes.clone()))));
                        }
                        Arc::new(made)
                    })
                    .collect();
                block.with_insns(insns)
            })
            .collect();
        self.body.with_blocks(blocks)
    }

    /// Where each value the coalescer absorbed went: `survivor(value)` is the
    /// value that stands for it in [`Converted::body`].
    pub fn joined(&self) -> &llrm_codegen::coalescer::env::JoinMap {
        &self.joined
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
            size: false,
            carry_slot_indexes: true,
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
    convert(function, registers, false)
}

/// [`to_lir`] for a function still in SSA form, phis kept: what the shared
/// machine stages before `PHIElimination` read. [`from_ssa`] reads the result
/// back.
pub fn to_ssa_lir<T: Target>(
    function: &MFunction<T::Class>,
    registers: Regs,
) -> Result<Converted<T>, String> {
    convert(function, registers, true)
}

/// `ssa_pins`: each write of a physical register makes a value of its own,
/// pinned to it, and a read takes the last write in its block (the value the
/// function enters with, in the entry block): the body is SSA, as the pin's
/// is, whose physical registers live only between a copy and its user. A
/// physical register read in a block that did not write it is refused.
///
/// DEVIATE SP1: the pin's isel (`InstrEmitter::EmitCopyToReg` /
/// `EmitCopyFromReg`) makes a COPY between a physical register and a virtual
/// one; here the physical operand stays and each write is a value pinned to
/// it. Exit: the m68k emitter makes those copies (2026-12-04).
fn convert<T: Target>(
    function: &MFunction<T::Class>,
    registers: Regs,
    ssa_pins: bool,
) -> Result<Converted<T>, String> {
    // A class's width once, not once per register of it.
    let mut width_of: Vec<(T::Class, u32)> = Vec::new();
    let widths: Vec<u32> = function
        .classes
        .iter()
        .map(|class| match width_of.iter().find(|(known, _)| known == class) {
            Some((_, width)) => *width,
            None => {
                let width = class_width::<T>(registers, *class);
                width_of.push((*class, width));
                width
            }
        })
        .collect();
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
        symbols: Vec::new(),
        block_at: block_positions(function),
        ssa_pins,
        block: 0,
        current: IndexMap::default(),
        inputs: IndexMap::default(),
        named: Vec::new(),
        crossed: None,
        flags_wanted: false,
        marker: PhantomData::<T>,
    };
    // every physical register is a value before the first instruction, so a
    // call (which writes the ones it clobbers) can say so
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        let (reads, writes) = physical::<T>(inst);
        for register in reads.chain(writes) {
            if !ssa_pins {
                made.pinned(register);
            } else if !made.named.contains(&register) {
                made.named.push(register);
            }
        }
    }
    let whole_homes = whole_homes::<T>(function);
    let mut foldable: Vec<Option<(&'static str, usize)>> = Vec::new();
    let clobbered_by_spills = T::spill_clobbers();
    let mut orders_by_class = Orders::<T>::new(&clobbered_by_spills);
    let mut infos: Vec<(&'static str, Vec<OperandInfo>)> = Vec::new();
    let mut blocks = Vec::with_capacity(function.blocks.len());
    // the converted predecessors' last values of the physical registers
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); function.blocks.len()];
    for (from, to) in successors.iter().enumerate().filter(|(from, _)| reachable[*from]) {
        for to in to {
            predecessors[*to].push(from);
        }
    }
    let mut exits: Vec<Option<IndexMap<RegId, u32>>> = vec![None; function.blocks.len()];
    for (at, block) in function.blocks.iter().enumerate() {
        let mut insns = Vec::with_capacity(block.insts.len());
        made.block = at;
        made.current.clear();
        let flag_reads = flags_read_after::<T>(block, ssa_pins);
        // A physical register no instruction of this block has written is the
        // value every predecessor leaves it, if they agree and all are
        // converted already (an unwritten join of a split block: the low word
        // of an i64 result copied before the select, read by the `RET`).
        // Otherwise a read of it is refused (a merge needs a phi).
        if ssa_pins && at != 0 {
            let before: Vec<&IndexMap<RegId, u32>> = predecessors[at].iter().filter_map(|p| exits[*p].as_ref()).collect();
            if before.len() == predecessors[at].len() {
                if let Some((first, rest)) = before.split_first() {
                    made.current = first
                        .iter()
                        .filter(|(register, value)| rest.iter().all(|one| one.get(*register) == Some(value)))
                        .map(|(register, value)| (*register, *value))
                        .collect();
                }
            }
        }
        // DEVIATE 2b: greedy is not given a block no path reaches (LLVM
        // removes them first); `from_lir` returns it as it was.
        let mut position = 0;
        while reachable[at] && position < block.insts.len() {
            let inst = &block.insts[position];
            made.flags_wanted = flag_reads[position];
            if let Some(lanes) = made.pack_lanes(inst) {
                // one copy per part, each with the origin of the `PACK`; the
                // part is still hinted the lane of the register its whole is
                // copied to (the join may not reach it: a loop's phi)
                let (_, mut own, ..) = operand_infos::<T>(inst, &function.classes, &mut orders_by_class);
                hint_lanes::<T>(inst, &whole_homes, registers, &mut own);
                for (k, lane) in lanes.into_iter().enumerate() {
                    let ordinal = made.origin.len() as i64;
                    made.origin.push((at, position));
                    foldable.push(None);
                    // the pair it is copied into (its class) and the part it
                    // copies; a clone of the pair that only these name is
                    // otherwise any register
                    let part = own
                        .iter()
                        .filter(|info| info.def || info.index == k)
                        .cloned()
                        .map(|info| OperandInfo { index: 0, lane: Some((lane.bytes, lane.offset)), ..info })
                        .collect();
                    infos.push((COPY, part));
                    insns.push(Arc::new(numbered(made.lane_copy(lane, 2 * ordinal), ordinal)));
                }
                position += 1;
                continue;
            }
            let ordinal = made.origin.len() as i64;
            made.origin.push((at, position));
            foldable.push(foldable_source::<T>(inst));
            let (name, mut own, sources, dests) = operand_infos::<T>(inst, &function.classes, &mut orders_by_class);
            hint_lanes::<T>(inst, &whole_homes, registers, &mut own);
            infos.push((name, own));
            // A copy, spill or reload overwrites the flags a compare set, and
            // greedy puts them where a value is read: the compare and the
            // instruction that reads its flags are one instruction to it.
            let reader = block
                .insts
                .get(position + 1)
                .filter(
                    |next| inst.sets_flags()
                        && !inst.terminator
                        && !inst.call
                        && next.reads_flags()
                        && T::reads_flags_set_just_before(next),
                );
            match reader {
                Some(next) if made.independent(inst, next) => {
                    made.origin.push((at, position + 1));
                    foldable.push(None);
                    // the bundle is one instruction to greedy: both ask for it
                    // (the compare's operands come first in it, then the
                    // reader's, after the compare's sources and dests)
                    let (_, read, ..) = operand_infos::<T>(next, &function.classes, &mut orders_by_class);
                    let compare = infos.last_mut().expect("the compare's own");
                    compare.0 = BUNDLE;
                    compare
                        .1
                        .extend(
                            read.into_iter().map(|info| OperandInfo {
                                index: info.index + if info.def { dests } else { sources },
                                ..info
                            }),
                        );
                    infos.push(("", Vec::new()));
                    insns.push(Arc::new(numbered(made.bundle(inst, next, 2 * ordinal), ordinal)));
                    position += 2;
                }
                _ => {
                    insns.push(Arc::new(numbered(made.insn(inst, 2 * ordinal), ordinal)));
                    position += 1;
                }
            }
        }
        exits[at] = reachable[at].then(|| made.current.clone());
        let mut lir = LirBlock::new(at as i64, insns);
        if reachable[at] {
            lir.succ = successors[at].iter().map(|to| *to as i64).collect();
            lir.phis = block.phis.iter().map(|phi| made.phi(phi)).collect::<Result<Vec<_>, _>>()?.into();
        }
        blocks.push(LirBlockRef::from(lir));
    }
    if let Some(why) = made.crossed.take() {
        return Err(why);
    }
    let entry = if function.blocks.is_empty() { 0 } else { 0 };
    let mut body = LirBody::new(function.name.clone(), entry, blocks, IndexMap::default(), made.pins.clone());
    body.returns_twice = function.returns_twice;
    body.registers = Some(registers);
    match &function.odds {
        Some(odds) => stated_odds(&mut body, function, odds),
        None => loop_branch_odds(&mut body),
    }
    body.inputs = if ssa_pins {
        made.inputs.values().copied().collect()
    } else {
        entry_live::<T>(function, &successors).iter().filter_map(|r| made.values.get(r).copied()).collect()
    };
    let mut orders: IndexMap<u32, Vec<RegId>> = IndexMap::default();
    for (v, class) in function.classes.iter().enumerate() {
        orders.insert(v as u32 + 1, orders_by_class.of(*class).clone());
    }
    for (value, register) in &made.pins {
        orders.insert(*value, vec![*register]);
    }
    let caller_saved: BTreeSet<RegId> = T::caller_saved().into_iter().collect();
    let mut saved: Vec<RegId> = Vec::new();
    // Each class once: a class seen again adds nothing the first time did not.
    let mut seen: Vec<T::Class> = Vec::new();
    for class in &function.classes {
        if seen.contains(class) {
            continue;
        }
        seen.push(*class);
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
        derived: IndexMap::default(),
        reachable,
        joined: Default::default(),
        tii: SpikeInstrInfo {
            classes: function.classes.clone(),
            registers,
            foldable,
            infos,
            clobbered: clobbered_by_spills,
            buffers: call_result_buffers::<T>(function),
            marker: PhantomData,
        },
        tri: SpikeRegisterInfo {
            copy_cost: 1,
            csr_first_use_cost: T::csr_first_use_cost(),
            size_bytes: T::size_bytes(),
            csr_cost_scale: T::csr_cost_scale(),
            csr_spill_first: T::csr_spill_first(),
            register_cost: T::register_cost,
        },
    })
}

/// Where each block id is, read once: a branch names its target by id.
fn block_positions<C>(function: &MFunction<C>) -> llrm_support::hash::HashMap<u32, i64> {
    BLOCK_WALKS.with(|walks| walks.set(walks.get() + function.blocks.len()));
    let mut at = llrm_support::hash::HashMap::default();
    for (position, block) in function.blocks.iter().enumerate() {
        at.entry(block.id).or_insert(position as i64);
    }
    at
}

/// The odds the `MFunction` states, by block number in `body`. An edge with
/// none shares what the others leave (`getSuccProbability`).
fn stated_odds<C>(
    body: &mut LirBody,
    function: &MFunction<C>,
    odds: &crate::placement::Odds,
) {
    for block in body.blocks.iter() {
        let from = function.blocks[block.at as usize].id;
        for &to in block.succ.iter() {
            let id = function.blocks[to as usize].id;
            if let Some((probability, _)) = odds.get(&(from, id)) {
                body.odds.taken.insert((block.at, to), probability.get_numerator());
            }
        }
    }
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

/// The registers greedy may give a virtual register of each class, minus the
/// ones a spill clobbers: the target is asked once per class, not once per
/// register or operand.
struct Orders<T: Target> {
    clobbered: Vec<RegId>,
    by_class: Vec<(T::Class, Vec<RegId>)>,
}

impl<T: Target> Orders<T> {
    fn new(clobbered: &[RegId]) -> Self {
        Self { clobbered: clobbered.to_vec(), by_class: Vec::new() }
    }

    /// The order for `class`; the classes in the order they were first asked
    /// for.
    fn of(
        &mut self,
        class: T::Class,
    ) -> &Vec<RegId> {
        let at = match self.by_class.iter().position(|(known, _)| *known == class) {
            Some(at) => at,
            None => {
                let mut order = T::candidates(class);
                order.retain(|register| !self.clobbered.contains(register));
                self.by_class.push((class, order));
                self.by_class.len() - 1
            }
        };
        &self.by_class[at].1
    }
}

struct Made<'f, T: Target> {
    function: &'f MFunction<T::Class>,
    registers: Regs,
    widths: &'f [u32],
    /// The next value a pinned register gets.
    next: u32,
    pins: IndexMap<u32, RegId>,
    values: IndexMap<RegId, u32>,
    /// See [`convert`]'s `ssa_pins`: the mode, the block being converted, the
    /// value each register holds in it, the values the entry block starts
    /// with, the registers the function names, and the first read refused.
    ssa_pins: bool,
    block: usize,
    current: IndexMap<RegId, u32>,
    inputs: IndexMap<RegId, u32>,
    named: Vec<RegId>,
    crossed: Option<String>,
    /// Whether an instruction after the one being converted reads the flags
    /// register by name before another writes it (`flags_read_after`).
    flags_wanted: bool,
    origin: Vec<(usize, usize)>,
    /// The symbols the function's operands name, in order of first use: the
    /// number a symbol's `Imm` carries as its address.
    symbols: Vec<String>,
    /// The position of the block each id names (the first, if one repeats).
    block_at: llrm_support::hash::HashMap<u32, i64>,
    marker: PhantomData<T>,
}

impl<T: Target> Made<'_, T> {
    /// `PHI result, (value, predecessor)..` in the body's numbering: a virtual
    /// register `v` is the value `v + 1`, a predecessor its block's position.
    fn phi(
        &self,
        phi: &crate::mfunc::Phi,
    ) -> Result<llrm_codegen::model::lir::Phi, String> {
        let value = |register: &Reg| match register {
            Reg::Virt(v) => Ok(v + 1),
            Reg::Phys(_) => Err(format!("{}: a phi of a physical register", self.function.name)),
        };
        Ok(llrm_codegen::model::lir::Phi {
            result: value(&phi.result)?,
            incoming: phi
                .incoming
                .iter()
                .map(|(pred, from)| Ok((*pred as i64, value(from)?)))
                .collect::<Result<_, String>>()?,
        })
    }

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

    /// The value a read of `register` sees (see [`convert`]'s `ssa_pins`).
    fn pinned_read(
        &mut self,
        register: RegId,
    ) -> (u32, u32) {
        if !self.ssa_pins {
            return self.pinned(register);
        }
        let width = self.registers.bytes(register).map_or(1, |bytes| bytes as u32);
        if let Some(value) = self.current.get(&register) {
            return (*value, width);
        }
        if self.block != 0 && self.crossed.is_none() {
            self.crossed = Some(format!(
                "{}: physical register {} is read in block {} without a write in it",
                self.function.name,
                register.index(),
                self.block
            ));
        }
        let value = self.fresh(register);
        self.inputs.insert(register, value);
        self.current.insert(register, value);
        (value, width)
    }

    /// The value a write of `register` makes (see [`convert`]'s `ssa_pins`).
    fn pinned_write(
        &mut self,
        register: RegId,
    ) -> (u32, u32) {
        if !self.ssa_pins {
            return self.pinned(register);
        }
        let value = self.fresh(register);
        self.current.insert(register, value);
        (value, self.registers.bytes(register).map_or(1, |bytes| bytes as u32))
    }

    fn fresh(
        &mut self,
        register: RegId,
    ) -> u32 {
        let value = self.next;
        self.next += 1;
        self.pins.insert(value, register);
        value
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

    /// The copies a lane-built instruction is converted to, where the target
    /// lowers lanes: one per part, into its lane of the whole.
    fn pack_lanes(
        &self,
        inst: &Inst,
    ) -> Option<Vec<LaneCopy>> {
        if !T::lowers_lanes() {
            return None;
        }
        let (whole, parts) = T::lane_moves(inst)?;
        let Some(Operand::Def(Reg::Virt(to))) = inst.operands.get(whole) else { return None };
        let mut copies = Vec::new();
        for (k, (part, offset, bytes)) in parts.into_iter().enumerate() {
            let Some(Operand::Use(Reg::Virt(from))) = inst.operands.get(part) else { return None };
            copies.push(LaneCopy {
                copy: Inst::new(Kind::Copy, vec![Operand::Def(Reg::Virt(*to)), Operand::Use(Reg::Virt(*from))]),
                whole: *to + 1,
                offset,
                bytes,
                first: k == 0,
            });
        }
        Some(copies)
    }

    /// `undef whole:lo = COPY lo`, then `whole:hi = COPY hi`: the lane the copy
    /// defines is its destination's `offset`, and every one after the first
    /// keeps the lanes before it.
    fn lane_copy(
        &mut self,
        lane: LaneCopy,
        at: i64,
    ) -> Insn {
        let mut insn = self.insn(&lane.copy, at);
        if let Some(Loc::Held(held)) = insn.what.as_mut().and_then(|what| what.dests.first_mut()) {
            held.width = lane.bytes;
            held.offset = lane.offset;
        }
        insn.def_lanes = vec![(lane.whole, ValueLanes::at(lane.offset, lane.bytes))];
        if lane.first {
            insn.undef_super = true;
        } else {
            insn.partial_defs = vec![lane.whole];
        }
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
        // where each operand stands in `sources` / `dests`, for the pairs it ties
        let mut source_at: Vec<Option<usize>> = vec![None; inst.operands.len()];
        let mut dest_at: Vec<Option<usize>> = vec![None; inst.operands.len()];
        for (index, operand) in inst.operands.iter().enumerate() {
            let before = sources.len();
            match operand {
                Operand::Use(Reg::Virt(v)) => {
                    let held = Held { value: v + 1, width: self.widths[*v as usize], offset: 0 };
                    uses.push(held.value);
                    sources.push(Loc::Held(held));
                }
                Operand::Use(Reg::Phys(register)) => {
                    if T::reserved(T::root(*register)) {
                        continue;
                    }
                    let value = self.pinned_read(*register).0;
                    let width = self.registers.bytes(*register).map_or(1, |bytes| bytes as u32);
                    let held = Held { value, width, offset: 0 };
                    uses.push(value);
                    requires.push((held, *register));
                    sources.push(Loc::Held(held));
                }
                Operand::Imm(value) => sources.push(Loc::Imm(Imm { value: *value, width: 0, address: None })),
                Operand::Symbol(name, offset) => {
                    symbol = true;
                    // The symbol is part of the value: `_s` and `0` are two
                    // literals, which `from_lir` must not take for one.
                    let at = self
                        .symbols
                        .iter()
                        .position(|one| one == name)
                        .unwrap_or_else(
                            || {
                                self.symbols.push(name.clone());
                                self.symbols.len() - 1
                            },
                        );
                    let address = Some(Addr::new(Space::External, at as i64));
                    sources.push(Loc::Imm(Imm { value: *offset, width: 0, address }));
                }
                Operand::Frame(slot) => {
                    sources.push(Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, i64::from(*slot))), 0)));
                }
                Operand::Block(id) => {
                    target = self.block_at.get(id).copied();
                }
                Operand::Def(_) => {}
            }
            if sources.len() > before {
                source_at[index] = Some(before);
            }
        }
        for (index, operand) in inst.operands.iter().enumerate() {
            let before = dests.len();
            match operand {
                Operand::Def(Reg::Virt(v)) => {
                    let held = Held { value: v + 1, width: self.widths[*v as usize], offset: 0 };
                    defines.push(held.value);
                    dests.push(Loc::Held(held));
                }
                Operand::Def(Reg::Phys(register)) => {
                    if T::reserved(T::root(*register)) {
                        // The stack pointer has no value to hold, but a write
                        // of it is named so `boundary`
                        // sees it
                        // (`modifiesRegister(SP)`): without it `sub sp, sp,
                        // #16` moved below the stores
                        // it makes room for.
                        if self.registers.is_stack(*register) {
                            let width = self.registers.bytes(*register).map_or(1, |bytes| bytes as u32);
                            dests.push(Loc::Reg(llrm_lir::Reg { register: *register, width }));
                        }
                        continue;
                    }
                    let (value, width) = self.pinned_write(*register);
                    let held = Held { value, width, offset: 0 };
                    defines.push(value);
                    delivers.push((held, *register));
                    dests.push(Loc::Held(held));
                }
                _ => {}
            }
            if dests.len() > before {
                dest_at[index] = Some(before);
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
        } else if defines.is_empty() && inst.sets_flags() {
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
        // `implicit-def $ccr`: the flags a later instruction of the block reads by
        // name (`CCR_SAVE`) are the value this one leaves. Last in the lists, so
        // the operands before it keep their numbers.
        if self.ssa_pins
            && self.flags_wanted
            && inst.sets_flags()
            && let Some(flags) = T::flags_register()
        {
            let (value, width) = self.pinned_write(flags);
            let held = Held { value, width, offset: 0 };
            defines.push(value);
            delivers.push((held, flags));
            dests.push(Loc::Held(held));
        }
        // the machine instruction's own tied operands, in the lists' numbering: a
        // dropped operand (a reserved register) moves the ones after it
        let tied = (!inst.ties.is_empty()).then(|| {
            inst.ties.iter().filter_map(|&(def, used)| Some((source_at[used]?, dest_at[def]?))).collect::<Vec<_>>()
        });
        let what = Semantics { name: Some(name.into()), dests, sources, target, tied, ..Semantics::new(op) };
        let mut insn = Insn::new(at, Some((at, at + 1)), Some(what), defines, uses);
        insn.requires = requires;
        insn.delivers = delivers;
        if T::greedy_early_clobber() && inst.early_clobber {
            insn.early_clobbers = insn.defines.clone();
        }
        if inst.call {
            insn.clobbers = match &inst.call_clobbers {
                Some(known) => known.as_ref().clone(),
                None => T::caller_saved().into_iter().collect(),
            };
            // The call writes each clobbered register it names: a value that
            // lives across it cannot be the register, and a read after it
            // sees what the call left.
            if self.ssa_pins {
                let written: Vec<RegId> = physical::<T>(inst).1.collect();
                for register in self.named.clone() {
                    let clobbered =
                        insn.clobbers.contains(&register) || insn.clobbers.contains(&self.registers.root(register));
                    if clobbered && !written.contains(&register) {
                        let value = self.pinned_write(register).0;
                        insn.defines.push(value);
                    }
                }
            }
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
        if T::lowers_lanes()
            && let Some((operand, offset, bytes)) = T::read_lanes(inst)
            && let Some(Operand::Use(Reg::Virt(from))) = inst.operands.get(operand)
        {
            insn.use_lanes.push((from + 1, ValueLanes::at(offset, bytes)));
        }
        // the lanes of the whole each part is copied into
        if let Some((whole, parts)) = T::lane_moves(inst)
            && let Some(Operand::Def(Reg::Virt(to)) | Operand::Use(Reg::Virt(to))) = inst.operands.get(whole)
        {
            for (part, offset, bytes) in parts {
                if let Some(Operand::Def(Reg::Virt(from)) | Operand::Use(Reg::Virt(from))) = inst.operands.get(part) {
                    insn.lane_copies.push((from + 1, to + 1, ValueLanes::at(offset, bytes)));
                }
            }
        }
        insn
    }
}

/// One part of a lane-built value, copied into its lane of the whole.
struct LaneCopy {
    copy: Inst,
    whole: u32,
    offset: u32,
    bytes: u32,
    first: bool,
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

thread_local! {
    static OPERAND_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static BLOCK_WALKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many blocks this thread has walked to resolve branch targets, for a
/// test to bound it.
pub fn block_walks() -> usize {
    BLOCK_WALKS.with(std::cell::Cell::get)
}

/// How many instructions this thread has read the whole operand table of (the
/// scan the coalescer's join pricing made of every use of a value at every
/// attempt), for a test to bound it.
pub fn operand_visits() -> usize {
    OPERAND_VISITS.with(std::cell::Cell::get)
}

/// The operand sets the instructions of a body name a value by, one id each
/// (a kind of use the coalescer sums).
#[derive(Default)]
struct UseKindTable {
    ids: std::collections::BTreeMap<Vec<(Vec<RegId>, Vec<u32>)>, u32>,
    /// The kind of an instruction shape (its index in the target's operand
    /// table) and the slots of it that name the value.
    by_shape: llrm_support::hash::HashMap<(usize, u64), u32>,
    operands: Vec<Vec<OperandInfo>>,
}

impl UseKindTable {
    /// The id of `operands`, sorted and without repeats.
    fn intern(
        &mut self,
        operands: Vec<OperandInfo>,
    ) -> u32 {
        let key: Vec<(Vec<RegId>, Vec<u32>)> =
            operands.iter().map(|one| (one.order.clone(), one.costs.clone())).collect();
        if let Some(id) = self.ids.get(&key) {
            return *id;
        }
        let id = self.operands.len() as u32;
        self.operands.push(operands);
        self.ids.insert(key, id);
        id
    }
}

/// What the target says of one virtual-register operand of an instruction:
/// the `index`th source (or destination, where `def`) the converter gave it.
/// Named by position, not by value, because the coalescer renames values.
#[derive(Clone)]
struct OperandInfo {
    def: bool,
    index: usize,
    /// The register it is cheaper with: the first of the cheapest in `order`
    /// when they differ (`getRegAllocationHints`).
    hint: Option<RegId>,
    /// The registers its class allows, in order.
    order: Vec<RegId>,
    /// What the instruction costs in each, aligned with `order`
    /// (`Target::operand_cost`).
    costs: Vec<u32>,
    /// For the part of a lane-built pair: the bytes and offset of the lane the
    /// copy writes. A copy greedy makes beside it, of the whole pair, is
    /// another instruction.
    lane: Option<(u32, u32)>,
}

impl OperandInfo {
    fn new<T: Target>(
        inst: &Inst,
        at: usize,
        def: bool,
        index: usize,
        order: Vec<RegId>,
    ) -> Self {
        let costs: Vec<u32> = order.iter().map(|register| T::operand_cost(inst, at, *register)).collect();
        let (least, most) = (costs.iter().min().copied(), costs.iter().max().copied());
        let hint = match (least, most) {
            (Some(least), Some(most)) if least < most => {
                costs.iter().position(|cost| *cost == least).map(|at| order[at])
            }
            _ => None,
        };
        Self { def, index, hint, order, costs, lane: None }
    }

    /// The cheapest this instruction is in any of `registers` it prices.
    fn cheapest(
        &self,
        registers: &[RegId],
    ) -> Option<u32> {
        registers
            .iter()
            .filter_map(|register| self.order.iter().position(|one| one == register))
            .map(|at| self.costs[at])
            .min()
    }
}

/// The virtual operands the target described for `one` in `infos` (indexed by
/// the instruction's number), with the value each holds now (the coalescer
/// renames values, not positions). An instruction greedy made (a copy, a
/// spill) has none.
fn operands_in<'a>(
    infos: &'a [(&'static str, Vec<OperandInfo>)],
    one: &Insn,
) -> Vec<(u32, &'a OperandInfo)> {
    OPERAND_VISITS.with(|visits| visits.set(visits.get() + 1));
    let Some((name, infos)) = infos.get(ordinal_of(one)) else { return Vec::new() };
    let Some(what) = one.what.as_ref() else { return Vec::new() };
    if what.name.as_deref() != Some(name) {
        return Vec::new();
    }
    let lane_dest = |bytes: u32, offset: u32| {
        matches!(
            what.dests.first(),
            Some(Loc::Held(held)) if held.width == bytes && held.offset == offset
        )
    };
    infos
        .iter()
        .filter(|info| info.lane.is_none_or(|(bytes, offset)| lane_dest(bytes, offset)))
        .filter_map(|info| {
            let loc = if info.def { what.dests.get(info.index) } else { what.sources.get(info.index) };
            match loc {
                Some(Loc::Held(held)) => Some((held.value, info)),
                _ => None,
            }
        })
        .collect()
}

/// The name of `inst` and what the target says of its virtual operands.
/// The register each value is copied to or from, through the chain of copies
/// between values, where the chain meets one register only: where a
/// lane-built value ends up whole.
fn whole_homes<T: Target>(function: &MFunction<T::Class>) -> llrm_support::hash::HashMap<u32, RegId> {
    let mut parent: Vec<u32> = (0..=function.classes.len() as u32).collect();
    fn find(
        parent: &mut [u32],
        mut v: u32,
    ) -> u32 {
        while parent[v as usize] != v {
            parent[v as usize] = parent[parent[v as usize] as usize];
            v = parent[v as usize];
        }
        v
    }
    let mut touched: Vec<(u32, RegId)> = Vec::new();
    for inst in function.blocks.iter().flat_map(|block| &block.insts).filter(|inst| inst.kind == Kind::Copy) {
        match inst.operands.as_slice() {
            [
                Operand::Def(Reg::Virt(a)) | Operand::Use(Reg::Virt(a)),
                Operand::Def(Reg::Virt(b)) | Operand::Use(Reg::Virt(b)),
            ] => {
                let (x, y) = (find(&mut parent, *a), find(&mut parent, *b));
                parent[x as usize] = y;
            }
            [
                Operand::Def(Reg::Phys(p)) | Operand::Use(Reg::Phys(p)),
                Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)),
            ]
            | [
                Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)),
                Operand::Def(Reg::Phys(p)) | Operand::Use(Reg::Phys(p)),
            ] if !T::reserved(T::root(*p)) => {
                touched.push((*v, *p));
            }
            _ => {}
        }
    }
    let mut homes: llrm_support::hash::HashMap<u32, Option<RegId>> = Default::default();
    for (v, p) in touched {
        let root = find(&mut parent, v);
        homes
            .entry(root)
            .and_modify(|home| {
                if *home != Some(p) {
                    *home = None
                }
            })
            .or_insert(Some(p));
    }
    (0..=function.classes.len() as u32)
        .filter_map(|v| homes.get(&find(&mut parent, v)).copied().flatten().map(|home| (v, home)))
        .collect()
}

/// DEVIATE (`Target::lane_moves`): the part of a lane-built value takes as its
/// hint the lane of the register its whole is copied to or from.
fn hint_lanes<T: Target>(
    inst: &Inst,
    homes: &llrm_support::hash::HashMap<u32, RegId>,
    registers: Regs,
    infos: &mut [OperandInfo],
) {
    let Some((whole, parts)) = T::lane_moves(inst) else { return };
    let Some(Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v))) = inst.operands.get(whole) else { return };
    let Some(home) = homes.get(v) else { return };
    let base = registers.lanes(*home).trailing_zeros();
    for (part, offset, bytes) in parts {
        let wanted = ((1_i64 << bytes) - 1) << (base + offset);
        let ordinal = inst.operands[..part]
            .iter()
            .filter(|o| matches!(o, Operand::Def(Reg::Virt(_)) | Operand::Use(Reg::Virt(_))))
            .count();
        let Some(info) = infos.get_mut(ordinal) else { continue };
        if let Some(lane) = info
            .order
            .iter()
            .copied()
            .find(|register| T::root(*register) == T::root(*home) && registers.lanes(*register) == wanted)
        {
            info.hint = Some(lane);
        }
    }
}

fn operand_infos<T: Target>(
    inst: &Inst,
    classes: &[T::Class],
    orders: &mut Orders<T>,
) -> (&'static str, Vec<OperandInfo>, usize, usize) {
    let Kind::Op(name) = inst.kind else { return ("", Vec::new(), 0, 0) };
    let (mut sources, mut dests) = (0, 0);
    let mut infos = Vec::new();
    for (at, operand) in inst.operands.iter().enumerate() {
        match operand {
            Operand::Use(Reg::Virt(v)) => {
                infos.push(OperandInfo::new::<T>(inst, at, false, sources, orders.of(classes[*v as usize]).clone()));
                sources += 1;
            }
            Operand::Use(Reg::Phys(register)) if !T::reserved(T::root(*register)) => sources += 1,
            Operand::Imm(_) | Operand::Symbol(..) | Operand::Frame(_) => sources += 1,
            Operand::Def(Reg::Virt(v)) => {
                infos.push(OperandInfo::new::<T>(inst, at, true, dests, orders.of(classes[*v as usize]).clone()));
                dests += 1;
            }
            Operand::Def(Reg::Phys(register)) if !T::reserved(T::root(*register)) => dests += 1,
            _ => {}
        }
    }
    (name, infos, sources, dests)
}

/// What greedy asks of the spike target's instructions, answered from the
/// instruction itself: its name and operands, and the target's own spill forms.
pub struct SpikeInstrInfo<T: Target> {
    classes: Vec<T::Class>,
    registers: Regs,
    /// For the instruction numbered `at / 2`: its name and which of its
    /// sources the target can read from memory (`Target::fold_spill`).
    foldable: Vec<Option<(&'static str, usize)>>,
    /// For the instruction numbered `at / 2`: its name and what the target
    /// says of its virtual operands.
    infos: Vec<(&'static str, Vec<OperandInfo>)>,
    /// `Target::spill_clobbers`: no value is held in one.
    clobbered: Vec<RegId>,
    /// `call_result_buffers`: frame objects a call fills and loads read back.
    buffers: BTreeSet<i64>,
    marker: PhantomData<T>,
}

impl<T: Target> SpikeInstrInfo<T> {
    /// A load of one of `buffers`: any other load of a frame slot is a local
    /// variable that other instructions write.
    fn is_known_buffer_load(
        &self,
        one: &Insn,
    ) -> bool {
        is_buffer_load::<T>(one)
            && one.what.as_ref().and_then(frame_cell).is_some_and(|addr| self.buffers.contains(&addr.disp))
    }

    fn operands_of(
        &self,
        one: &Insn,
    ) -> Vec<(u32, &OperandInfo)> {
        operands_in(&self.infos, one)
    }

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
                    dests.push(Loc::Held(Held { value, width, offset: 0 }));
                }
                Operand::Use(Reg::Virt(_)) => {
                    uses.push(value);
                    sources.push(Loc::Held(Held { value, width, offset: 0 }));
                }
                Operand::Frame(slot) => {
                    let cell = Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, i64::from(*slot))), width));
                    if defines.is_empty() { dests.push(cell) } else { sources.push(cell) }
                }
                // what the form clobbers is put back by the way back
                // ([`from_lir`]), which asks the target for the form again
                Operand::Def(Reg::Phys(_)) => {}
                // an offset the form carries (m68k's `MOVEM` pseudo): the way
                // back writes it again
                Operand::Imm(_) => {}
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

/// Whether the coalescer makes the literal `literal` again for a copy from a
/// value in `source` to one in `destination`. DEVIATE C10
/// (register_coalescer.rs): at -Os a constant that stays read (`!alone`) is not
/// made again unless the target says the move is no longer than the copy; the
/// target's price decides the rest.
pub fn literal_remat_wins<T: Target>(
    literal: &Insn,
    source: &[RegId],
    destination: &[RegId],
    size: bool,
    alone: bool,
) -> bool {
    (!size || alone || T::literal_move_is_copy_sized(literal))
        && T::literal_move_wins(literal, source, destination, size)
}

/// [`is_literal_move`], or the cheap move that reads registers
/// ([`Target::remat_reads_registers`]): the pin's `isReMaterializable` takes a
/// virtual-register use and `allUsesAvailableAt` asks at the copy whether it
/// still holds (the callers do).
fn is_cheap_move<T: Target>(one: &Insn) -> bool {
    is_literal_move(one)
        || (T::remat_reads_registers() && is_move_with_reads(one, false))
        || (T::remats_symbol_addresses() && one.uses.is_empty() && is_move_with_reads(one, true))
}

/// One register written by a move of a literal, whatever the target names it:
/// the target's `isAsCheapAsAMove && isReMaterializable` for a constant.
fn is_literal_move(one: &Insn) -> bool {
    one.uses.is_empty() && is_move_with_reads(one, false)
}

/// [`is_literal_move`] that may read registers: the registers it reads, then
/// the identity of the operation.
fn is_move_with_reads(
    one: &Insn,
    symbol: bool,
) -> bool {
    let Some(what) = one.what.as_ref() else { return false };
    one.defines.len() == 1
        && one.group.is_none()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty()
        && (symbol || one.symbol != Some(true))
        // a lane def makes part of its value: making it again does not
        && one.def_lanes.is_empty()
        && one.partial_defs.is_empty()
        && !one.undef_super
        && what.op == Operation::Move
        && matches!(
            what.dests.as_slice(),
            [Loc::Held(held)] if held.value == one.defines[0]
        )
        && matches!(
            what.sources.as_slice(),
            [reads @ .., Loc::Imm(_)] if reads.iter().all(|loc| matches!(loc, Loc::Held(held) if one.uses.contains(&held.value)))
        )
}

/// One register written by a load of a frame slot of an incoming argument.
fn is_frame_load<T: Target>(one: &Insn) -> bool {
    is_named_frame_load(one, T::is_invariant_argument_load)
}

/// One register written by a load of a frame object a call fills
/// (`call_result_buffers`).
fn is_buffer_load<T: Target>(one: &Insn) -> bool {
    is_named_frame_load(one, T::is_buffer_load)
}

fn is_named_frame_load(
    one: &Insn,
    named: fn(&str) -> bool,
) -> bool {
    let Some(what) = one.what.as_ref() else { return false };
    what.name.as_deref().is_some_and(named)
        && one.defines.len() == 1
        && one.uses.is_empty()
        && one.group.is_none()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty()
        && one.def_lanes.is_empty()
        && one.partial_defs.is_empty()
        && !one.undef_super
        && !one.volatile
        && matches!(
            what.dests.as_slice(),
            [Loc::Held(held)] if held.value == one.defines[0]
        )
        && frame_cell(what).is_some()
}

/// The frame slot a load reads: its one memory source, the rest immediates (the
/// displacement a pin row names beside it).
fn frame_cell(what: &Semantics) -> Option<Addr> {
    let mut found = None;
    for place in &what.sources {
        match place {
            Loc::Mem(cell) if found.is_none() => found = cell.addr.filter(|addr| addr.space == Space::Frame),
            Loc::Imm(imm) if imm.address.is_none() => {}
            _ => return None,
        }
    }
    found
}

/// The values defined once by a load of an incoming stack argument (a frame
/// slot below zero) that nothing in `body` writes or takes the address of.
fn incoming_argument_loads<T: Target>(body: &LirBody) -> BTreeSet<u32> {
    let incoming = |addr: Addr| addr.space == Space::Frame && addr.disp < 0;
    let mut written: BTreeSet<i64> = BTreeSet::new();
    let mut definitions: llrm_support::hash::HashMap<u32, usize> = Default::default();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        for value in &one.defines {
            *definitions.entry(*value).or_default() += 1;
        }
        let Some(what) = one.what.as_ref() else { continue };
        for place in what.dests.iter().chain(&what.sources) {
            match place {
                Loc::Address(a) if a.addr.is_some_and(incoming) => return BTreeSet::new(),
                Loc::Imm(imm) if imm.address.is_some_and(incoming) => return BTreeSet::new(),
                _ => {}
            }
        }
        for place in &what.dests {
            if let Loc::Mem(cell) = place {
                if let Some(addr) = cell.addr.filter(|addr| incoming(*addr)) {
                    written.insert(addr.disp);
                }
            }
        }
        if what.op == Operation::Address {
            if let Some(addr) = what.sources.iter().find_map(|place| match place {
                Loc::Mem(cell) => cell.addr.filter(|addr| incoming(*addr)),
                _ => None,
            }) {
                written.insert(addr.disp);
            }
        }
    }
    body.blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| is_frame_load::<T>(one))
        .filter_map(|one| {
            let Some(what) = one.what.as_ref() else { return None };
            let addr = frame_cell(what).filter(|addr| incoming(*addr))?;
            (!written.contains(&addr.disp) && definitions.get(&one.defines[0]) == Some(&1)).then_some(one.defines[0])
        })
        .collect()
}

/// The values defined once by a load of a buffer, with the cell each reads.
fn buffer_loads<T: Target>(
    body: &LirBody,
    buffers: &BTreeSet<i64>,
) -> Vec<(u32, (i64, u32))> {
    if buffers.is_empty() {
        return Vec::new();
    }
    let mut definitions: llrm_support::hash::HashMap<u32, usize> = Default::default();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        for value in &one.defines {
            *definitions.entry(*value).or_default() += 1;
        }
    }
    body.blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| is_buffer_load::<T>(one))
        .filter_map(|one| {
            let what = one.what.as_ref()?;
            let addr = frame_cell(what).filter(|addr| buffers.contains(&addr.disp))?;
            let Some(Loc::Held(held)) = what.dests.first() else { return None };
            (definitions.get(&one.defines[0]) == Some(&1)).then(|| (one.defines[0], (addr.disp, held.width)))
        })
        .collect()
}

/// DEVIATE R-BUF1 (owner greedy-subreg, exit 2026-11-20): the frame objects a
/// call fills and only loads read, each load straight after that call. The
/// pin remakes a load only from an immutable object. Here the address of the
/// object is stored once into the arguments of one call (copies aside) and
/// reaches nothing else, so that call is the one write to the object; every
/// load of it follows that call in its block with no call between, so a value
/// loaded is the object's content wherever the value lives, and a load made
/// again where it is read gives the same value.
///
/// Worse row (owner greedy-subreg, exit 2026-11-28): z80 `float_in_memory_loop`
/// -Os 326 -> 332 B. A remade load splits one value into two pieces and the
/// allocation after it shifts; no single defect.
/// z80 `fcmp_all` -O2 23661 -> 23670 B, clocks unpriced.
fn call_result_buffers<T: Target>(function: &MFunction<T::Class>) -> BTreeSet<i64> {
    let mut found = BTreeSet::new();
    let mut frames: BTreeSet<i32> = BTreeSet::new();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        if let Some(frame) = T::frame_address(inst) {
            frames.insert(frame);
        }
    }
    // uses of each virtual register in the whole function
    let mut uses: llrm_support::hash::HashMap<u32, usize> = Default::default();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        for operand in &inst.operands {
            if let Operand::Use(Reg::Virt(v)) = operand {
                *uses.entry(*v).or_default() += 1;
            }
        }
    }
    'frames: for frame in frames {
        let mut taker: Option<(usize, usize)> = None;
        let mut loads: Vec<(usize, usize)> = Vec::new();
        for (b, block) in function.blocks.iter().enumerate() {
            for (i, inst) in block.insts.iter().enumerate() {
                if !inst.operands.iter().any(|o| matches!(o, Operand::Frame(f) if *f == frame)) {
                    continue;
                }
                if T::frame_address(inst) == Some(frame) {
                    if taker.replace((b, i)).is_some() {
                        continue 'frames;
                    }
                } else if let Kind::Op(name) = inst.kind
                    && T::is_buffer_load(name)
                    && inst.operands.iter().any(|o| matches!(o, Operand::Def(_)))
                {
                    loads.push((b, i));
                } else {
                    continue 'frames;
                }
            }
        }
        let Some((tb, ti)) = taker else { continue };
        // the address, from its definition to the first call
        let block = &function.blocks[tb];
        let mut holders: BTreeSet<u32> = BTreeSet::new();
        let mut in_window = 0usize;
        let mut call = None;
        let mut spreads = false;
        let takes = &block.insts[ti];
        let mut phys_holders = false;
        for operand in &takes.operands {
            match operand {
                Operand::Def(Reg::Virt(v)) => {
                    holders.insert(*v);
                }
                Operand::Def(Reg::Phys(_)) => phys_holders = true,
                _ => {}
            }
        }
        for (i, inst) in block.insts.iter().enumerate().skip(ti + 1) {
            if inst.call {
                call = Some(i);
                break;
            }
            let reads: Vec<(usize, Reg)> = inst
                .operands
                .iter()
                .enumerate()
                .filter_map(|(at, o)| if let Operand::Use(r) = o { Some((at, *r)) } else { None })
                .collect();
            let reads_address = reads
                .iter()
                .any(
                    |(_, r)| match r {
                        Reg::Virt(v) => holders.contains(v),
                        Reg::Phys(_) => phys_holders,
                    },
                );
            if !reads_address {
                continue;
            }
            let is_copy = inst.kind == Kind::Copy;
            let is_store = inst.may_store && !inst.operands.iter().any(|o| matches!(o, Operand::Def(_)));
            if !is_copy && !is_store {
                spreads = true;
            }
            for (_, r) in &reads {
                if let Reg::Virt(v) = r {
                    if holders.contains(v) {
                        in_window += 1;
                    }
                }
            }
            if is_copy {
                for operand in &inst.operands {
                    if let Operand::Def(Reg::Virt(v)) = operand {
                        holders.insert(*v);
                    }
                }
            }
        }
        let Some(call) = call else { continue };
        let total: usize = holders.iter().map(|v| uses.get(v).copied().unwrap_or(0)).sum();
        if spreads || total != in_window {
            continue;
        }
        // every load follows the call in its block, with no call between
        for (b, i) in &loads {
            let ok = *b == tb && *i > call && !function.blocks[*b].insts[call + 1..*i].iter().any(|inst| inst.call);
            if !ok {
                continue 'frames;
            }
        }
        found.insert(i64::from(frame));
    }
    found
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
            dests: vec![Loc::Held(Held { value: to, width, offset: 0 })],
            sources: vec![Loc::Held(Held { value: from, width, offset: 0 })],
            ..Semantics::new(Operation::Move)
        }
    }

    /// One register written by a move of a literal: what the front end marked
    /// `rematerializable` converts to.
    fn remat_shape(
        &self,
        one: &Insn,
    ) -> bool {
        is_cheap_move::<T>(one) || is_frame_load::<T>(one) || self.is_known_buffer_load(one)
    }

    fn is_trivially_re_materializable(
        &self,
        def: &Insn,
        invariant: bool,
    ) -> bool {
        let r = is_cheap_move::<T>(def) || (invariant && (is_frame_load::<T>(def) || self.is_known_buffer_load(def)));
        r
    }

    fn argument_loads(
        &self,
        body: &LirBody,
    ) -> BTreeSet<u32> {
        let mut found = incoming_argument_loads::<T>(body);
        found.extend(buffer_loads::<T>(body, &self.buffers).into_iter().map(|(value, _)| value));
        found
    }

    fn argument_load_defs(
        &self,
        body: &LirBody,
    ) -> BTreeSet<(i64, u32)> {
        buffer_loads::<T>(body, &self.buffers).into_iter().map(|(_, cell)| cell).collect()
    }

    fn remat_load_cell(
        &self,
        one: &Insn,
    ) -> Option<(i64, u32)> {
        let addr = frame_cell(one.what.as_ref()?).filter(|addr| self.buffers.contains(&addr.disp))?;
        self.is_known_buffer_load(one).then(|| {
            (
                addr.disp,
                one.what.as_ref().and_then(|w| w.dests.first()).map_or(0, |d| match d {
                    Loc::Held(held) => held.width,
                    _ => 0,
                }),
            )
        })
    }

    /// The read the target takes from memory (`Target::fold_spill`) becomes a
    /// memory source; `from_lir` writes it as the target's own form.
    fn preferred_registers(
        &self,
        one: &Insn,
    ) -> Vec<(u32, RegId)> {
        self.operands_of(one).into_iter().filter_map(|(value, info)| Some((value, info.hint?))).collect()
    }

    fn copy_register_cost(
        &self,
        to: RegId,
        from: RegId,
        size: bool,
    ) -> u32 {
        T::copy_register_cost(to, from, size)
    }

    fn operand_orders(
        &self,
        one: &Insn,
    ) -> Vec<(u32, Vec<RegId>)> {
        self.operands_of(one).into_iter().map(|(value, info)| (value, info.order.clone())).collect()
    }

    fn spill_weight_scale(
        &self,
        order: &[RegId],
    ) -> f32 {
        T::spill_weight_scale(order)
    }

    fn larger_order(
        &self,
        order: &[RegId],
    ) -> Option<Vec<RegId>> {
        let mut larger = T::larger_order(order)?;
        larger.retain(|register| !self.clobbered.contains(register));
        Some(larger)
    }

    fn fold_memory_operand(
        &self,
        one: &Arc<Insn>,
        reg: u32,
        cell: &Mem,
    ) -> Option<Arc<Insn>> {
        let what = one.what.as_ref()?;
        // A straight COPY folds as a load or a store
        // (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetInstrInfo.cpp#L794>):
        // the spilled end is the slot, the other end stays a register.
        // A pinned end (`requires`/`delivers`) is a register this converter
        // numbers, not the allocator: 6502 copied `rs0` into a slot as
        // `lda rc18446744073709551603`.
        if one.group.is_none()
            && one.def_lanes.is_empty()
            && one.requires.is_empty()
            && one.delivers.is_empty()
            && self.is_plain_move(what)
            && let ([Loc::Held(dest)], [Loc::Held(source)]) = (what.dests.as_slice(), what.sources.as_slice())
            && dest.width == source.width
            && dest.width as u32 == cell.width
            && dest.value != source.value
            && self.class_of(dest.value) == self.class_of(source.value)
        {
            if dest.value == reg {
                return Some(self.store_reg_to_stack_slot(one, source.value, cell));
            }
            if source.value == reg {
                return Some(self.load_reg_from_stack_slot(one, dest.value, cell));
            }
        }
        if one.group.is_none()
            && !what.sources.iter().any(|loc| matches!(loc, Loc::Held(_)))
            && what.name.as_deref().is_some_and(T::is_zero_def)
            && let [Loc::Held(held)] = what.dests.as_slice()
            && held.value == reg
            && held.width as u32 <= cell.width
        {
            // A spilled zero is stored as the zero register: the definition
            // becomes the store.
            let mut out = (**one).clone();
            out.what = Some(Semantics {
                dests: vec![Loc::Mem(Mem { width: held.width as u32, ..cell.clone() })],
                sources: Vec::new(),
                ..what.clone()
            });
            out.defines.retain(|value| *value != reg);
            out.spill_store = true;
            // The store is not the definition it was cloned from: `from_lir`
            // must not look it up among the function's definitions.
            out.rematerialized = false;
            return Some(Arc::new(out));
        }
        let (name, at) = self.foldable.get(ordinal_of(one)).copied().flatten()?;
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

    fn stores_lanes(&self) -> bool {
        T::spills_lanes()
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
    pub size_bytes: Option<llrm_codegen::greedy::targetinfo::SizeBytes>,
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

    fn size_bytes(&self) -> Option<&llrm_codegen::greedy::targetinfo::SizeBytes> {
        self.size_bytes.as_ref()
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

/// The register of `wide`'s root that is the `bytes` bytes `offset` above
/// `wide`'s own lowest.
fn lane_register(
    registers: &Regs,
    wide: RegId,
    offset: u32,
    bytes: u32,
) -> Option<RegId> {
    registers.lane_register(wide, offset, bytes)
}

/// `source` with the registers greedy chose: its definitions take the
/// destinations in order, its reads the sources.
fn substitute<'a, T: Target>(
    source: &Inst,
    dests: &mut impl Iterator<Item = &'a Loc>,
    sources: &mut impl Iterator<Item = &'a Loc>,
    number: &mut dyn FnMut(&Held) -> Reg,
    folds: &mut Vec<(Reg, i32)>,
) -> Result<Inst, String> {
    let mut made = source.clone();
    for operand in &mut made.operands {
        *operand = match operand.clone() {
            Operand::Def(Reg::Virt(_)) => match dests.next() {
                Some(Loc::Held(held)) => Operand::Def(number(held)),
                _ => return Err("a definition that is not a register".into()),
            },
            Operand::Def(Reg::Phys(r)) => {
                if !T::reserved(T::root(r)) {
                    dests.next();
                }
                Operand::Def(Reg::Phys(r))
            }
            Operand::Use(Reg::Virt(_)) => match sources.next() {
                Some(Loc::Held(held)) => Operand::Use(number(held)),
                // a read taken from its frame slot: a register stands in for
                // it until the target writes the folded form
                Some(Loc::Mem(cell)) => {
                    let stand_in = number(&Held { value: u32::MAX - folds.len() as u32, width: 0, offset: 0 });
                    folds.push((stand_in, slot_of(cell)));
                    Operand::Use(stand_in)
                }
                other => return Err(format!("a use that is not a register: {:?} reads {other:?}", source.kind)),
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

/// A shared machine stage's result as a machine function again: the stage ran
/// on [`to_ssa_lir`]'s body and may have removed instructions, moved them
/// between blocks and renamed the values they read, which is all it is given
/// back as. An instruction it made, a block it added and a value it created
/// are refused: a stage hosted here must not make them (each later stage
/// extends this when it needs to).
pub fn from_ssa<T: Target>(
    original: &MFunction<T::Class>,
    converted: &Converted<T>,
    body: &LirBody,
) -> Result<MFunction<T::Class>, String> {
    let name = &original.name;
    if body.blocks.len() != original.blocks.len() {
        return Err(format!("{name}: a stage changed the number of blocks"));
    }
    let known = original.classes.len() as u32;
    // what each instruction read before the stage, by its place
    let before: llrm_support::hash::HashMap<i64, &Insn> =
        converted.body.blocks.iter().flat_map(|block| block.insns.iter()).map(|insn| (ordinal_of(insn) as i64, &**insn)).collect();
    // a value another replaces, found from the readers the stage rewrote
    let mut renamed: IndexMap<u32, u32> = IndexMap::default();
    let mut note = |from: u32, to: u32| -> Result<(), String> {
        match renamed.insert(from, to) {
            Some(held) if held != to => Err(format!("{name}: a stage renamed value {from} to {held} and to {to}")),
            _ => Ok(()),
        }
    };
    for block in body.blocks.iter() {
        for insn in block.insns.iter() {
            let Some(old) = before.get(&(ordinal_of(insn) as i64)) else {
                return Err(format!("{name}: a stage made an instruction"));
            };
            if old.uses.len() != insn.uses.len() {
                return Err(format!("{name}: a stage changed what an instruction reads"));
            }
            for (a, b) in old.uses.iter().zip(&insn.uses) {
                if a != b {
                    note(*a, *b)?;
                }
            }
        }
    }
    let rename = |value: u32| *renamed.get(&value).unwrap_or(&value);
    let register = |value: u32| -> Result<Reg, String> {
        match converted.body.pins.get(&value) {
            Some(register) => Ok(Reg::Phys(*register)),
            None if value >= 1 && value <= known => Ok(Reg::Virt(value - 1)),
            None => Err(format!("{name}: a stage made value {value}")),
        }
    };
    let mut blocks = Vec::with_capacity(body.blocks.len());
    for (at, lir) in body.blocks.iter().enumerate() {
        if lir.at != at as i64 {
            return Err(format!("{name}: a stage reordered the blocks"));
        }
        if !converted.reachable[at] {
            blocks.push(original.blocks[at].clone());
            continue;
        }
        let mut insts = Vec::with_capacity(lir.insns.len());
        for insn in lir.insns.iter() {
            // a bundle is two instructions
            let places =
                if insn.what.as_ref().is_some_and(|what| what.name.as_deref() == Some(BUNDLE)) { 2 } else { 1 };
            for k in 0..places {
                let (b, i) = *converted
                    .origin
                    .get(ordinal_of(insn) + k)
                    .ok_or_else(|| format!("{name}: an instruction with no origin"))?;
                let mut inst = original.blocks[b].insts[i].clone();
                for operand in &mut inst.operands {
                    if let Operand::Use(Reg::Virt(v)) = operand {
                        *v = rename(*v + 1) - 1;
                    }
                }
                insts.push(inst);
            }
        }
        let mut phis = Vec::with_capacity(lir.phis.len());
        for phi in lir.phis.iter() {
            phis.push(crate::mfunc::Phi {
                result: register(phi.result)?,
                incoming: phi
                    .incoming
                    .iter()
                    .map(|(pred, value)| Ok((*pred as usize, register(rename(*value))?)))
                    .collect::<Result<_, String>>()?,
            });
        }
        blocks.push(Block { id: original.blocks[at].id, insts, phis });
    }
    let mut function = original.clone();
    function.blocks = blocks;
    Ok(function)
}

/// A stage of the shared flow run on `function` in SSA form
/// ([`LIRTransform`](llrm_codegen::model::passes::LIRTransform)), and the
/// result back as a machine function. `stage` is made with the oracle that
/// holds the register classes: a value may stand for another only where the
/// target's classes of the two meet, and then is held to that class
/// (`constrainRegClass`).
pub fn host_ssa<T: Target>(
    function: &MFunction<T::Class>,
    registers: Regs,
    stage: impl FnOnce(llrm_codegen::backend::precse::Constrain) -> Box<dyn llrm_codegen::model::passes::LIRTransform>,
) -> Result<MFunction<T::Class>, String>
where
    T::Class: 'static,
{
    let converted = to_ssa_lir::<T>(function, registers)?;
    let known = function.classes.len() as u32;
    let classes = Rc::new(RefCell::new(function.classes.clone()));
    let oracle: llrm_codegen::backend::precse::Constrain = {
        let classes = Rc::clone(&classes);
        Rc::new(move |from: u32, to: u32| {
            // a pinned value has no class to constrain
            if from == 0 || to == 0 || from > known || to > known {
                return true;
            }
            let mut held = classes.borrow_mut();
            match T::meet(held[from as usize - 1], held[to as usize - 1]) {
                Some(both) => {
                    held[to as usize - 1] = both;
                    true
                }
                None => false,
            }
        })
    };
    let mut stage = stage(oracle);
    // The shared stages read an instruction's effects (the flags it sets, the
    // memory it touches) off the instruction; the spike target states them
    // here, as the pre-RA scheduler's input has them.
    let _lanes = llrm_codegen::backend::lanes::enter(registers);
    let input = converted.with_effects(function, registers);
    let body = stage.transform(input)?;
    let mut out = from_ssa(function, &converted, &body)?;
    out.classes = classes.borrow().clone();
    Ok(out)
}

/// A copy `PhiElimination` or `TwoAddress` made: a move of a value into a
/// value, named `mov`, that stands beside an instruction and claims none of
/// its bytes. A move of a literal is a definition made again, not this.
fn is_shared_copy(insn: &Insn) -> bool {
    insn.what.as_ref().is_some_and(|what| {
        what.op == Operation::Move
            && what.name.as_deref() == Some("mov")
            && matches!(what.dests.as_slice(), [Loc::Held(_)])
            && matches!(what.sources.as_slice(), [Loc::Held(_)])
    }) && insn.covers.is_some_and(|(from, to)| from == to)
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
                match decided.parents.get(&root).or_else(|| converted.derived.get(&root)) {
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
    // The values some instruction defines by lane (a pair built from its
    // bytes) are named by lane, in the register of that lane, wherever an
    // instruction names them.
    let lane_values: llrm_support::hash::HashSet<u32> = edited
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .flat_map(|insn| insn.def_lanes.iter().map(|(value, _)| *value))
        .collect();
    let placed_at: IndexMap<u32, RegId> = decided.placed.iter().copied().collect();
    let lane_failed: std::cell::RefCell<Option<String>> = Default::default();
    let lane_or_number = |held: &Held,
                          classes: &mut Vec<T::Class>,
                          number: &mut dyn FnMut(u32, &mut Vec<T::Class>) -> Reg|
     -> Reg {
        // A clone of such a value (a reload of the pair) has no lane def of
        // its own: a byte of a wider register is named by lane the same way.
        let narrower = placed_at
            .get(&held.value)
            .is_some_and(|wide| registers.bytes(*wide).is_some_and(|bytes| bytes > i64::from(held.width)));
        if (lane_values.contains(&held.value) || narrower) && !pinned.contains_key(&held.value) {
            let found =
                placed_at.get(&held.value).and_then(|wide| lane_register(&registers, *wide, held.offset, held.width));
            match found {
                Some(register) => return Reg::Phys(register),
                None => {
                    lane_failed.borrow_mut().get_or_insert(format!("no register for a lane of value {}", held.value));
                }
            }
        }
        number(held.value, classes)
    };
    // The literal and address definitions of the function by what they set:
    // the spiller makes one again where it is read, or puts it in place of a
    // copy of the value, and that instruction stands at another one's place.
    let mut definitions: IndexMap<String, (usize, usize)> = IndexMap::default();
    for block in &converted.body.blocks {
        for insn in block.insns.iter() {
            let (Some(what), Some(origin)) = (&insn.what, converted.origin.get(ordinal_of(insn))) else {
                continue;
            };
            // A definition the target marks rematerializable may read a value
            // (arm64's `add x, y, #n`): the spiller makes it again where it is
            // read, and it is no less a definition for the register it uses.
            let remakable = original.blocks[origin.0].insts[origin.1].rematerializable;
            if insn.defines.len() == 1 && (insn.uses.is_empty() || remakable) {
                definitions.entry(format!("{:?}{:?}", what.name, what.sources)).or_insert(*origin);
            }
        }
    }
    let mut slots: Vec<(i32, T::Class)> = Vec::new();
    let mut blocks = Vec::with_capacity(edited.blocks.len());
    // A block a shared phase made on an edge is labelled above every block of
    // the function (`(entry + 1) << 32`, `PhiElimination`) and takes the next
    // block id; the blocks stay in the layout order the body gives.
    let floor = (edited.entry + 1) << 32;
    let mut fresh = original.blocks.iter().map(|block| block.id + 1).max().unwrap_or(0);
    let mut ids: IndexMap<i64, u32> = IndexMap::default();
    for block in &edited.blocks {
        let id = if block.at >= floor {
            fresh += 1;
            fresh - 1
        } else {
            original.blocks.get(block.at as usize).ok_or("a block from nowhere")?.id
        };
        ids.insert(block.at, id);
    }
    for block in edited.blocks.iter() {
        let at = block.at as usize;
        if block.at < floor && !converted.reachable[at] {
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
                let value = held(&what.dests).into_iter().chain(held(&what.sources)).next();
                let cell = what
                    .dests
                    .iter()
                    .chain(&what.sources)
                    .find_map(|loc| if let Loc::Mem(cell) = loc { Some(cell) } else { None });
                let slot = slot_of(cell.ok_or_else(|| format!("a spill with no cell: {what:?}"))?);
                let Some(value) = value else {
                    // A zero stored from the zero register
                    // (`fold_memory_operand`).
                    let made = T::zero_store(T::any(), slot).ok_or("the target states no zero store")?;
                    place_spill(&mut insts, made)?;
                    if !slots.iter().any(|(one, _)| *one == slot) {
                        slots.push((slot, T::any()));
                    }
                    continue;
                };
                // named as every other instruction names the value (by lane
                // for a byte of a wider register): `drop_self_copies` counts
                // the readers of a register by name
                let named = what
                    .dests
                    .iter()
                    .chain(&what.sources)
                    .find_map(|loc| if let Loc::Held(held) = loc { Some(*held) } else { None })
                    .ok_or("a spill of no value")?;
                let numbered = number(value, &mut classes);
                // A pinned value is no virtual register: its class is one that
                // holds its register (vreg 0's class made a
                // spill of `hl` a byte store, `ld -4(ix), hl`).
                let class = match numbered {
                    Reg::Virt(v) => classes[v as usize],
                    Reg::Phys(register) => classes
                        .iter()
                        .copied()
                        .find(|class| T::candidates(*class).contains(&register))
                        .unwrap_or_else(T::any),
                };
                let register = if T::spills_named_by_lane() {
                    lane_or_number(&named, &mut classes, &mut number)
                } else {
                    numbered
                };
                let lane = cell.filter(|_| insn.spill_store).and_then(llrm_codegen::greedy::tii::lane_offset);
                let made = if let Some(offset) = lane {
                    T::spill_store_lane(class, register, slot, offset)
                } else if insn.spill_store {
                    T::spill_store(class, register, slot)
                } else {
                    T::spill_load(class, register, slot)
                };
                place_spill(&mut insts, made.ok_or("the target states no spill form")?)?;
                // a lane store says nothing of the slot's size
                if lane.is_none() && !slots.iter().any(|(one, _)| *one == slot) {
                    slots.push((slot, class));
                }
                continue;
            }
            if what.name.as_deref() == Some(BUNDLE) {
                let (mut dests, mut sources) = (what.dests.iter(), what.sources.iter());
                for ordinal in [ordinal_of(insn), ordinal_of(insn) + 1] {
                    let (from, inst) = *converted.origin.get(ordinal).ok_or("a bundle from nowhere")?;
                    let source = &original.blocks[from].insts[inst];
                    let mut folds = Vec::new();
                    insts.push(substitute::<T>(
                        source,
                        &mut dests,
                        &mut sources,
                        &mut |held| lane_or_number(held, &mut classes, &mut number),
                        &mut folds,
                    )?);
                    if !folds.is_empty() {
                        return Err("a bundle with a memory source".into());
                    }
                }
                continue;
            }
            // what is left where a jump to the next block was removed
            if what.op == Operation::Nothing
                && what.name.as_deref().is_none_or(str::is_empty)
                && insn.defines.is_empty()
                && insn.uses.is_empty()
            {
                continue;
            }
            // a jump `PhiElimination` made, onto an edge's block or where a
            // branch fell through
            if what.op == Operation::Jump
                && what.name.as_deref() == Some("jmp")
                && !what.indirect
                && insn.covers.is_some_and(|(from, to)| from == to)
            {
                let target = what.target.and_then(|label| ids.get(&label)).ok_or("a jump to a block from nowhere")?;
                insts.push(T::branch_to(*target).ok_or("the target has no jump to put on an edge")?);
                continue;
            }
            // a literal made on an edge stands at the edge's label: it is
            // found by what it sets, as a rematerialized one is
            let (mut from, mut inst) = match converted.origin.get(ordinal_of(insn)).filter(|_| insn.at < floor) {
                Some(&place) => place,
                None => *definitions.get(&format!("{:?}{:?}", what.name, what.sources)).ok_or_else(|| {
                    format!(
                        "an instruction from nowhere: at {} of {} origins, {what:?}",
                        insn.at,
                        converted.origin.len()
                    )
                })?,
            };
            // `PhiElimination` and `TwoAddress` name their copies `mov`, and
            // place them beside an instruction claiming none of its
            // bytes: copies, as a split's are. A move of a literal
            // is a definition made again, found below as a
            // rematerialized one is.
            let name = if is_shared_copy(insn) { Some(COPY) } else { what.name.as_deref() };
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
            // a copy into a lane of a pair: a move into the register of that
            // lane
            if name == Some(COPY) && !insn.def_lanes.is_empty() {
                // a source that is no value (a physical register) stays as
                // written
                if let Some(Loc::Held(source)) = what.sources.first() {
                    let Some(Loc::Held(to)) = what.dests.first() else {
                        return Err("a lane copy that does not move a value".into());
                    };
                    let dest = lane_or_number(to, &mut classes, &mut number);
                    let from = lane_or_number(source, &mut classes, &mut number);
                    insts.push(Inst::new(Kind::Copy, vec![Operand::Def(dest), Operand::Use(from)]));
                    continue;
                }
            }
            // a copy a split made stands beside an instruction and claims none
            // of its bytes, whatever that instruction is
            let made_by_split = insn.covers.is_some_and(|(from, to)| from == to);
            if name == Some(COPY) && (source.kind != Kind::Copy || made_by_split) {
                let (Some(to), Some(from)) = (held(&what.dests).first().copied(), held(&what.sources).first().copied())
                else {
                    return Err(format!(
                        "a copy that does not move a value: {what:?} covers {:?} of {:?}",
                        insn.covers, source.kind
                    ));
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
                &mut |held| lane_or_number(held, &mut classes, &mut number),
                &mut folds,
            )
            .map_err(|why| {
                format!("{why}: {what:?} uses {:?} defines {:?} of {:?}", insn.uses, insn.defines, source.operands)
            })?;
            for (stand_in, slot) in folds {
                made = T::fold_spill(&made, stand_in, slot).ok_or("a memory source the target has no form for")?;
            }
            if insn.rematerialized && T::remade_sets_no_flags() {
                made.remade_sets_flags = made.sets_flags();
                made.sets_flags = false;
            }
            // a branch `PhiElimination` pointed at the block of an edge
            if let (Some(label), false) = (what.target, what.indirect)
                && let Some(&id) = ids.get(&label)
            {
                let named: Vec<&mut u32> = made
                    .operands
                    .iter_mut()
                    .filter_map(|operand| if let Operand::Block(one) = operand { Some(one) } else { None })
                    .collect();
                if let Ok([one]) = <[&mut u32; 1]>::try_from(named) {
                    *one = id;
                }
            }
            insts.push(made);
        }
        blocks.push(Block { id: ids[&block.at], insts, phis: Vec::new() });
    }
    if let Some(failed) = lane_failed.take() {
        return Err(failed);
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
    let mut function = MFunction { blocks, classes, ..original.clone() };
    // an edge's block takes the odds of the edge it stands on
    for edge in edited.blocks.iter().filter(|block| block.at >= floor) {
        let Some(&to) = edge.succ.iter().next().and_then(|label| ids.get(label)) else { continue };
        if let Some(from) = edited.blocks.iter().find(|block| block.succ.contains(&edge.at)) {
            function.split_odds(ids[&from.at], to, ids[&edge.at]);
        }
    }
    Ok(Rewritten { function, assignment: Assignment(assignment), slots })
}

/// The pressure sets of a function: each distinct register list a class of
/// `classes` may take, sorted. The pin counts register units; classes over the
/// same registers (Dr8, Dr16 and Dr32) are one set, not one each.
pub fn pressure_sets<T: Target>(classes: &[T::Class]) -> Vec<Vec<RegId>> {
    let mut sets: Vec<Vec<RegId>> = Vec::new();
    for class in classes {
        let mut registers = T::candidates(*class);
        registers.sort();
        if !sets.contains(&registers) {
            sets.push(registers);
        }
    }
    sets
}

/// The indexes in `sets` a value of `class` counts against: every set that
/// shares a register with it (an Xr32 value lies in the data, the address and
/// the whole set).
pub fn sets_of<T: Target>(
    sets: &[Vec<RegId>],
    class: T::Class,
) -> Vec<usize> {
    let own = T::candidates(class);
    (0..sets.len()).filter(|at| sets[*at].iter().any(|one| own.contains(one))).collect()
}

/// `function` allocated by greedy: converted, its copies joined by the
/// coalescer (`size` joins what saves bytes first), allocated with the spills
/// written into it, their slots shared, and written back.
///
/// DEVIATE PJ1: a target that joins pinned copies (`joins_pinned_copies`; the
/// pin never joins into an allocatable register) is allocated again without
/// them when greedy finds no register, as the pin's allocator would have
/// spilled the pinned value. Exit: the target states false.
pub fn allocate_greedy<T: Target>(
    function: &mut MFunction<T::Class>,
    registers: Regs,
    size: bool,
) -> Result<(Assignment, Vec<(i32, T::Class)>), String> {
    allocate_greedy_after::<T>(function, registers, size, None)
}

/// The shared phases that stand between the SSA function and the allocator
/// (`PhiElimination`, `TwoAddress`): the body in, the body out.
pub type Prelude<'a> = &'a dyn Fn(LirBody) -> Result<LirBody, String>;

/// [`allocate_greedy`], where `prelude` takes the function still in SSA form
/// ([`to_ssa_lir`]) to the body greedy reads, in place of the private phi
/// elimination and two-address passes the caller then leaves out. The copies
/// it makes stand on their neighbour's `at` and claim no bytes
/// (`covers == (x, x)`), which `from_lir` rebuilds as plain copies.
pub fn allocate_greedy_after<T: Target>(
    function: &mut MFunction<T::Class>,
    registers: Regs,
    size: bool,
    prelude: Option<Prelude>,
) -> Result<(Assignment, Vec<(i32, T::Class)>), String> {
    let name = function.name.clone();
    let first = allocate_greedy_pinned::<T>(function, registers, size, T::joins_pinned_copies(), prelude);
    let result = match first {
        Err(why) if T::joins_pinned_copies() && why.ends_with("greedy could not allocate") => {
            let second = allocate_greedy_pinned::<T>(function, registers, size, false, prelude);
            if second.is_ok() {
                record_refusal(&name, &why, true);
            }
            second.map_err(|_| why)
        }
        other => other,
    };
    if let Err(why) = &result {
        record_refusal(&name, why, false);
    }
    result
}

/// A function greedy would not take as it was, for the audit of silent
/// fallbacks: a target that answers a refusal with another allocator, or with
/// a second try, shows nothing else.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refusal {
    pub function: String,
    pub why: String,
    /// Allocated on a second try (DEVIATE PJ1) rather than refused.
    pub retried: bool,
}

static REFUSALS: std::sync::Mutex<Vec<Refusal>> = std::sync::Mutex::new(Vec::new());

fn record_refusal(
    function: &str,
    why: &str,
    retried: bool,
) {
    if llrm_support::env_flag!("LLRM_GREEDY_REFUSALS") {
        eprintln!("greedy refusal: @{function}: {why}{}", if retried { " (allocated on the second try)" } else { "" });
    }
    let one = Refusal { function: function.to_owned(), why: why.to_owned(), retried };
    REFUSALS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(one);
}

/// The refusals since the last call, oldest first.
pub fn take_refusals() -> Vec<Refusal> {
    std::mem::take(&mut *REFUSALS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()))
}

fn allocate_greedy_pinned<T: Target>(
    function: &mut MFunction<T::Class>,
    registers: Regs,
    size: bool,
    joins_pinned: bool,
    prelude: Option<Prelude>,
) -> Result<(Assignment, Vec<(i32, T::Class)>), String> {
    use llrm_codegen::greedy::shadow::{Mode, decide_with_split};
    let mut converted = match prelude {
        None => to_lir::<T>(function, registers)?,
        Some(prelude) => {
            let mut converted = to_ssa_lir::<T>(function, registers)?;
            let _lanes = llrm_codegen::backend::lanes::enter(registers);
            let input = converted.with_effects(function, registers);
            converted.body = prelude(input)?;
            converted.adopt();
            converted
        }
    };
    if !llrm_support::env_flag!("LLRM_NO_COALESCE") {
        converted.coalesce_pinned(registers, size, joins_pinned);
    }
    if T::schedules_before_allocation() && !llrm_support::env_flag!("LLRM_NO_PRESCHED") {
        converted.schedule(function, registers);
    }
    let order = |value: u32| converted.order(value);
    let inputs = ShadowInputs { size, ..converted.inputs(registers, &order) };
    let mut frame = SpikeFrame::of(function);
    let decided = decide_with_split(&inputs, Mode::Inline { frame: &mut frame }, true)
        .ok_or_else(|| format!("{}: greedy refused", function.name))?;
    if decided.spill_failed || decided.report.stats.failed > 0 {
        return Err(format!("{}: greedy could not allocate", function.name));
    }
    let mut back = from_lir::<T>(function, &converted, &decided, registers)?;
    // `StackSlotColoring`: greedy gave each spilled value a slot of its own
    crate::spill::share_slots(&mut back.function, &mut back.slots);
    *function = back.function;
    Ok((back.assignment, back.slots))
}

/// The registers each value may take as sets, one set for all the values
/// that start with the same order (a class's values do): a set is built once
/// per distinct order, not once per value (arm64 shellsort: 700 values, 8
/// classes).
fn shared_palettes(orders: &IndexMap<u32, Vec<RegId>>) -> IndexMap<u32, Rc<BTreeSet<RegId>>> {
    let mut interned: llrm_support::hash::HashMap<&[RegId], Rc<BTreeSet<RegId>>> = Default::default();
    orders
        .iter()
        .map(|(value, order)| {
            let set = interned.entry(order.as_slice()).or_insert_with(|| Rc::new(order.iter().copied().collect()));
            (*value, Rc::clone(set))
        })
        .collect()
}

#[cfg(test)]
mod lane_def_tests {
    use super::*;

    fn literal(lane: bool) -> Insn {
        let what = Semantics {
            dests: vec![Loc::Held(Held { value: 5, width: 1, offset: 0 })],
            sources: vec![Loc::Imm(Imm { value: 0, width: 1, address: None })],
            ..Semantics::new(Operation::Move)
        };
        let mut one = Insn::new(0, Some((0, 0)), Some(what), vec![5], Vec::new());
        if lane {
            one.def_lanes = vec![(5, ValueLanes::at(0, 1))];
            one.partial_defs = vec![5];
        }
        one
    }

    /// The low byte of a pair, written alone, was taken for the pair: the
    /// spiller made it again where the pair was read and dropped the high
    /// byte written beside it (`mul_constants` -O2 summed a stale byte).
    #[test]
    fn a_byte_written_into_a_pair_is_not_a_literal_to_make_again() {
        assert!(is_literal_move(&literal(false)));
        assert!(!is_literal_move(&literal(true)));
    }
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    /// A set was built for every value, a few hundred times the sets a
    /// function's classes need, and every join cloned them: 4 M of the
    /// 64 M arm64 shellsort's allocation costs. Values of one order share one
    /// set; an order of its own gets its own.
    #[test]
    fn test_values_of_one_order_share_one_palette() {
        let first = vec![RegId::AX, RegId::BX];
        let second = vec![RegId::AX];
        let mut orders: IndexMap<u32, Vec<RegId>> = IndexMap::default();
        for value in 1..=700u32 {
            orders.insert(value, if value % 100 == 0 { second.clone() } else { first.clone() });
        }
        let palettes = shared_palettes(&orders);
        let distinct: std::collections::HashSet<*const BTreeSet<RegId>> = palettes.values().map(Rc::as_ptr).collect();
        assert_eq!(distinct.len(), 2, "one set per distinct order");
        assert_eq!(palettes[&1].iter().copied().collect::<Vec<_>>(), first);
        assert_eq!(palettes[&100].iter().copied().collect::<Vec<_>>(), second);
    }
}

/// A spill goes in before the terminators the block has so far. A def that
/// shares a bundle with its terminator (`LDI` and the `BEQ` that reads its
/// flags) is one instruction to greedy, which stores it after the bundle; the
/// pin stores after the def inside the bundle, before the terminator
/// (`InlineSpiller::insertSpill`, `std::next(MI)`; SplitKit
/// `findInsertLocation` likewise stays before the first terminator).
///
/// DEVIATE (row S2-BUNDLE-SPILL): an LIR bundle has no position inside it, so
/// the store is moved here instead of made inside. Exit: 2026-12-01, or when
/// bundles carry inside positions.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InlineSpiller.cpp#L1277-L1299>
fn place_spill(
    insts: &mut Vec<Inst>,
    spill: Inst,
) -> Result<(), String> {
    let at = insts.iter().rposition(|inst| !inst.terminator).map_or(0, |last| last + 1);
    if at < insts.len() && spill.sets_flags() && insts[at..].iter().any(|inst| inst.reads_flags()) {
        return Err("a spill between a flag setter and the terminator that reads the flags".into());
    }
    insts.insert(at, spill);
    Ok(())
}

/// `form` with its values the body's: the k-th def operand of `inst` is the
/// k-th value `one` defines, the k-th use operand its k-th use (a register the
/// target reserves is no value). The form names what the target's selector
/// chose (the scheduler's model reads it); the body's values are what its
/// graph and pressure read. None where the counts differ or a value has no
/// operand.
fn in_body_values<T: Target>(
    form: &Semantics,
    inst: &Inst,
    one: &Insn,
) -> Option<Semantics> {
    // The form names the registers `form_operands` had; the body's values
    // stand at the same positions in the instruction as it is now.
    let registers = |operands: &[Operand]| {
        let (mut defs, mut uses) = (Vec::new(), Vec::new());
        for operand in operands {
            let (list, register) = match operand {
                Operand::Def(register) => (&mut defs, register),
                Operand::Use(register) => (&mut uses, register),
                _ => continue,
            };
            match register {
                Reg::Virt(n) => list.push(*n),
                Reg::Phys(register) if !T::reserved(T::root(*register)) => list.push(u32::MAX),
                Reg::Phys(_) => {}
            }
        }
        (defs, uses)
    };
    let ((defs, uses), (now_defs, now_uses)) = (registers(&inst.form_operands), registers(&inst.operands));
    if defs.len() != one.defines.len()
        || uses.len() != one.uses.len()
        || now_defs.len() != defs.len()
        || now_uses.len() != uses.len()
    {
        return None;
    }
    renumbered(form, defs, uses, &one.defines, &one.uses)
}

/// `form` with each register number `defs`/`uses` list (as the instruction's
/// operands named them, `u32::MAX` for a physical one) standing for the value
/// at the same position of `defines`/`uses`.
fn renumbered(
    form: &Semantics,
    defs: Vec<u32>,
    uses: Vec<u32>,
    defines: &[u32],
    reads: &[u32],
) -> Option<Semantics> {
    let numbered = |numbers: Vec<u32>, values: &[u32]| -> std::collections::HashMap<u32, u32> {
        numbers.into_iter().zip(values.iter().copied()).filter(|(number, _)| *number != u32::MAX).collect()
    };
    let (def_map, use_map) = (numbered(defs, defines), numbered(uses, reads));
    let held = |held: &Held, map: &std::collections::HashMap<u32, u32>| -> Option<Held> {
        Some(Held { value: *map.get(&held.value)?, ..*held })
    };
    let remap = |loc: &Loc, map: &std::collections::HashMap<u32, u32>| -> Option<Loc> {
        Some(match loc {
            Loc::Held(each) => Loc::Held(held(each, map)?),
            Loc::Mem(cell) => {
                let mut cell = cell.clone();
                for slot in [&mut cell.base, &mut cell.index, &mut cell.selector] {
                    if let Some(each) = slot.as_ref() {
                        *slot = Some(held(each, map)?);
                    }
                }
                Loc::Mem(cell)
            }
            other => other.clone(),
        })
    };
    let dests = form.dests.iter().map(|loc| remap(loc, &def_map)).collect::<Option<Vec<_>>>()?;
    let sources = form.sources.iter().map(|loc| remap(loc, &use_map)).collect::<Option<Vec<_>>>()?;
    Some(form.with_operands(form.name.clone(), dests, sources))
}

#[cfg(test)]
mod form_tests {
    use super::*;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4, offset: 0 })
    }

    /// A pass rewrote a read after the form was made (`fadd v3, v1, v2` became
    /// `fadd v3, v1, v9`): the form still names `v2`, and the instruction was
    /// left without it, so the scheduler priced `FADDSrr` and `FDIVSrr` of
    /// arm64 `nbody_single` -O3 by the default latency (14 of its 53 float
    /// ops). The form's numbers are those of the operands it was made from.
    #[test]
    fn a_read_renamed_after_the_form_was_made_keeps_its_form() {
        let form = Semantics {
            dests: vec![held(3)],
            sources: vec![held(1), held(2)],
            ..Semantics::new(Operation::Binary)
        };
        // Operands when the form was made: defs [3], uses [1, 2]; the body's
        // values are one more than the registers, the read now being v9's.
        let got = renumbered(&form, vec![3], vec![1, 2], &[4], &[2, 10]).expect("the form stays");
        assert_eq!(got.dests, vec![held(4)]);
        assert_eq!(got.sources, vec![held(2), held(10)]);
    }
}
