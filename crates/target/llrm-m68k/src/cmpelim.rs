//! A `tst` of a register that the instruction before it just set the codes
//! for is dropped: `move.l (4,%sp), %d1; tst.l %d1; bne` is `move.l (4,%sp),
//! %d1; bne`.
//!
//! # How it works
//! For each `tst` of a virtual register, [`eliminate_tests`] finds the
//! instruction in the block that defines it with no other setter of the codes
//! between, and the readers of the codes that follow, up to the next setter or
//! the end of the block (the codes do not cross a block before the select
//! diamonds are made). The `tst` goes when the defining instruction leaves
//! every flag those readers test as `tst` of its result would ([`crate::ccr`]),
//! at the same operand size; a reader the pass cannot read keeps it.
//!
//! DEVIATE: the pin has no `optimizeCompareInstr` for m68k and keeps the
//! compare. Reason: best of gcc and clang is the goal, and both reuse the
//! codes of the instruction that made the value. Exit: none.
//!
//! DEVIATE ([`Options::hoist_flag_setters`], exit 2026-11-20): the walk back
//! from the `tst` also passes instructions that set the codes, when each is
//! independent of the definition and of what lies between, and hoists them
//! above it. The pin's walk stops at the first instruction that modifies
//! `EFLAGS` (X86InstrInfo.cpp L5557-5600, pin d1106deb71cc) and nothing there
//! reorders. Reason: `move.b (%a0)+,%d1; addq.l #1,%d0; tst.b %d1; bne` is one
//! instruction per iteration over gcc's `addq.l #1,%d0; tst.b (%a0,%d0.l);
//! jne` (strlen at 68060: 52 clocks against 38). The same option lets a
//! writeback load (`move.b (%a0)+,%d1`, `MOV8do_WB`) stand in for the `tst`,
//! which the instruction's name hid from the table. Off for `Options::PINNED`.
//!
//! The definition may not be one the allocator makes again where its register
//! is read and drops where none is: a rematerializable instruction, or a load
//! of a frame slot while [`Options::rematerialize_arguments`] is on. The `tst`
//! was the only reader (a `beq` on the codes of a load nothing emitted,
//! spill_store_order -O2 on the 68000: `pack_ref` returned at once for mode
//! 0x10).
//!
//! # See also
//! `X86InstrInfo::optimizeCompareInstr`.

use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::ccr;
use crate::dagisel::local;
use crate::mfunc::MFunction;
use crate::regalloc::Assignment;

/// The mnemonic and operand size letter of the instruction `name`, from its
/// assembly text: `move.l` is `("move", Some('l'))`.
fn mnemonic(name: &str) -> Option<(&'static str, Option<char>)> {
    let text = local::text(name)?.1.split('\t').next()?;
    Some(match text.split_once('.') {
        Some((mnemonic, size)) => (mnemonic, size.chars().next()),
        None => (text, None),
    })
}

/// The register a `tst` of a register reads.
pub(crate) fn tested(inst: &Inst) -> Option<(u32, Option<char>)> {
    let Kind::Op(name) = inst.kind else { return None };
    let (mnemonic, size) = mnemonic(name)?;
    match (mnemonic, &inst.operands[..]) {
        ("tst", [Operand::Use(Reg::Virt(v))]) => Some((*v, size)),
        _ => None,
    }
}

/// The flags a reader of the codes tests, or `None` when it is not one the
/// pass reads: a branch or set on a condition, or a `CMOV`.
fn read_by(inst: &Inst) -> Option<u32> {
    let Kind::Op(name) = inst.kind else { return None };
    if name.starts_with("CMOV") {
        return match inst.operands[..] {
            [.., Operand::Imm(code)] => ccr::tested_by(code),
            _ => None,
        };
    }
    let (mnemonic, _) = mnemonic(name)?;
    let condition = mnemonic.strip_prefix('b').or_else(|| mnemonic.strip_prefix('s'))?;
    ccr::tested_by_name(condition)
}

/// Whether `def` leaves the codes `tst` of its result at `size` would give,
/// for every flag in `needed`.
fn leaves_codes_of_test(
    def: &Inst,
    v: u32,
    size: Option<char>,
    needed: u32,
    writeback: bool,
) -> bool {
    let result = matches!(
        def.operands.first(),
        Some(Operand::Def(Reg::Virt(d))) if *d == v
    );
    leaves_codes(def, result, size, needed, writeback)
}

/// [`leaves_codes_of_test`] for a `def` already known to define (`result`) or
/// not the register tested.
fn leaves_codes(
    def: &Inst,
    result: bool,
    size: Option<char>,
    needed: u32,
    writeback: bool,
) -> bool {
    let Kind::Op(name) = def.kind else { return false };
    // a writeback (`MOV8do_WB`) prints as its base row (`MOV8do`)
    let name = if writeback { name.strip_suffix("_WB").unwrap_or(name) } else { name };
    // the 64-bit product sets Z and N from all 64 bits, not from the low half
    // it defines first (`mulu.l` prints as the 32-bit row's mnemonic)
    if crate::dagisel::widemul::code_of(name).is_some() {
        return false;
    }
    let Some((mnemonic, def_size)) = mnemonic(name) else { return false };
    // `swap` has no size letter and works on the long
    let same_size = def_size.or(Some('l')) == size;
    result && same_size && ccr::like_test(mnemonic).is_some_and(|same| needed & !same == 0)
}

/// Drops each `tst` the instruction that defined its register already stands
/// in for. With `hoist`, the instructions between them that set the codes
/// move above the definition first (see the module's second DEVIATE).
pub fn eliminate_tests(
    function: &mut MFunction,
    hoist: bool,
    argument_remat: bool,
) {
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            let Some((v, size)) = tested(&block.insts[at]) else {
                at += 1;
                continue;
            };
            let Some((def, setters)) = droppable(&block.insts, at, v, size, hoist, argument_remat) else {
                at += 1;
                continue;
            };
            block.insts.remove(at);
            let mut moved: Vec<Inst> = setters.iter().rev().map(|&one| block.insts.remove(one)).collect();
            moved.reverse();
            block.insts.splice(def..def, moved);
        }
    }
}

/// Whether `inst` loads a frame slot: the loads the allocator may make again
/// where they are read.
fn is_frame_load(inst: &Inst) -> bool {
    matches!(inst.kind, Kind::Op("MOV32rp")) && inst.operands.iter().any(|one| matches!(one, Operand::Frame(_)))
}

/// After allocation: a `tst` right after the instruction that set the codes
/// for its register goes. The allocator makes a frame load again where its
/// register is read, so the load now stands beside the `tst`
/// ([`eliminate_tests`] keeps the `tst` of such a register for that reason).
pub fn eliminate_adjacent_tests(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let physical = |register: Reg| match register {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(p) => p,
    };
    for block in &mut function.blocks {
        let mut at = 1;
        while at < block.insts.len() {
            let drop = (|| {
                let Kind::Op(name) = block.insts[at].kind else { return false };
                let (mnemonic, size) = mnemonic(name).unwrap_or(("", None));
                let [Operand::Use(tested)] = block.insts[at].operands[..] else { return false };
                let before = &block.insts[at - 1];
                let result = matches!(
                    before.operands.first(),
                    Some(Operand::Def(def)) if physical(*def) == physical(tested)
                );
                // a load of a frame slot the allocator made again is a `move`,
                // which sets the codes in hardware though the
                // allocator does not count it as the setter
                mnemonic == "tst"
                    && (before.sets_flags() || is_frame_load(before))
                    && flags_read_after(&block.insts, at)
                        .is_some_and(|needed| leaves_codes(before, result, size, needed, false))
            })();
            if drop {
                // the reader now reads the load's codes: it is the setter
                block.insts[at - 1].sets_flags = true;
                block.insts.remove(at);
            } else {
                at += 1;
            }
        }
    }
}

/// The definition of the tested register and the instructions between it and
/// the `tst` that set the codes, when the `tst` is redundant.
fn droppable(
    insts: &[Inst],
    at: usize,
    v: u32,
    size: Option<char>,
    hoist: bool,
    argument_remat: bool,
) -> Option<(usize, Vec<usize>)> {
    let mut def = None;
    let mut setters = Vec::new();
    for (index, before) in insts[..at].iter().enumerate().rev() {
        if before.operands.iter().any(|one| matches!(one, Operand::Def(Reg::Virt(d)) if *d == v)) {
            def = Some(index);
            break;
        }
        if before.call || before.terminator {
            return None;
        }
        if before.sets_flags() {
            if !hoist {
                return None;
            }
            setters.push(index);
        }
    }
    let def = def.filter(|&index| insts[index].sets_flags())?;
    if insts[def].rematerializable || (argument_remat && is_frame_load(&insts[def])) {
        return None;
    }
    setters.reverse();
    if !setters.is_empty() {
        setters = with_producers(insts, def, at, setters);
        if !hoistable(insts, def, at, &setters) {
            return None;
        }
    }
    flags_read_after(insts, at)
        .is_some_and(|needed| leaves_codes_of_test(&insts[def], v, size, needed, hoist))
        .then_some((def, setters))
}

/// `setters` and, in order, each instruction between the definition and the
/// `tst` that defines a register one of them reads (the copy of a phi's value
/// a counter's `addq` takes), since they move together.
fn with_producers(
    insts: &[Inst],
    def: usize,
    at: usize,
    setters: Vec<usize>,
) -> Vec<usize> {
    let mut moved = setters;
    loop {
        let wanted = (def + 1..at).find(|index| {
            !moved.contains(index)
                && moved.iter().any(|&later| {
                    *index < later && insts[*index].operands.iter().any(|one| {
                        matches!(
                            one,
                            Operand::Def(Reg::Virt(d)) if insts[later].operands.contains(&Operand::Use(Reg::Virt(*d)))
                        )
                    })
                })
        });
        let Some(index) = wanted else { break };
        moved.push(index);
    }
    moved.sort_unstable();
    moved
}

/// Whether each of `setters` (indices in `def..at`) can move above
/// `insts[def]`: nothing in the window reads the codes, and each is independent
/// of every other instruction it passes.
fn hoistable(
    insts: &[Inst],
    def: usize,
    at: usize,
    setters: &[usize],
) -> bool {
    if insts[def..at].iter().any(|one| one.reads_flags()) {
        return false;
    }
    setters.iter().all(|&setter| {
        (def..setter).filter(|index| !setters.contains(index)).all(|index| independent(&insts[setter], &insts[index]))
    })
}

/// Whether `later` can run before `earlier`: no register or memory dependence
/// either way. A physical register is not analysed (its views alias); a call, a
/// terminator or a side effect stays in order.
fn independent(
    later: &Inst,
    earlier: &Inst,
) -> bool {
    if [later, earlier].iter().any(|one| one.call || one.terminator || one.barrier || one.side_effects) {
        return false;
    }
    if (later.may_store && (earlier.may_load || earlier.may_store)) || (earlier.may_store && later.may_load) {
        return false;
    }
    let touched = |inst: &Inst| -> Option<(Vec<u32>, Vec<u32>)> {
        let (mut defs, mut uses) = (Vec::new(), Vec::new());
        for operand in &inst.operands {
            match operand {
                Operand::Def(Reg::Virt(one)) => defs.push(*one),
                Operand::Use(Reg::Virt(one)) => uses.push(*one),
                Operand::Def(Reg::Phys(_)) | Operand::Use(Reg::Phys(_)) => return None,
                _ => {}
            }
        }
        Some((defs, uses))
    };
    let (Some((later_defs, later_uses)), Some((earlier_defs, earlier_uses))) = (touched(later), touched(earlier))
    else {
        return false;
    };
    later_defs.iter().all(|one| !earlier_defs.contains(one) && !earlier_uses.contains(one))
        && later_uses.iter().all(|one| !earlier_defs.contains(one))
}

/// The flags the readers of the codes after `insts[at]` test, up to the next
/// setter; `None` when one is a reader the pass cannot read.
pub fn flags_read_after(
    insts: &[Inst],
    at: usize,
) -> Option<u32> {
    let mut needed = 0;
    for after in &insts[at + 1..] {
        // `subx` and `addx` read the extend flag and then set the codes
        if after.reads_flags() && !after.call {
            needed |= read_by(after)?;
        }
        if after.sets_flags() || after.call {
            break;
        }
    }
    Some(needed)
}
