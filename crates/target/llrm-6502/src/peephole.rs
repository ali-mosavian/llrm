//! Redundant loads, compares, transfers and jumps removed from a function's
//! instructions.
//!
//! # How it works
//! Seven passes walk the lines of an [`Asm`] once each. What an instruction
//! does to `a` and to the zero and sign flags comes from
//! [`crate::asm::Op::flags`], so a pass asks "does this instruction set the
//! flags" and does not list mnemonics:
//!
//! - [`drop_reloads`]: `sta m; lda m` (likewise `x`, `y`) of a zero-page or
//!   spill cell, when the load's flags are unread. A carry flag op or a store
//!   to another cell in between leaves the register and the cell as they were.
//! - [`fold_known_cells`]: `lda rc8` (likewise the other loads and `cmp`,
//!   `adc`, `sbc`, `and`, `ora`, `eor`) of a cell the function stored an
//!   immediate into becomes `lda #n`: the value is known, and the passes after
//!   it can drop the load or the compare. A call keeps `rc8`..`rc15`
//!   (callee-saved); a label forgets everything.
//! - [`drop_known_loads`]: `ldx #n` of what the register already holds, and
//!   `lda m` (likewise `ldx`, `ldy`) of a cell the register was loaded from or
//!   stored to, tracked through immediates, cell stores and transfers,
//!   forgotten at a call or an instruction not listed. A cell is forgotten when
//!   written; an indexed or indirect store forgets every cell. At a label only
//!   branches here reach, the values every edge agrees on stay.
//! - [`zero_test_through_an_index`]: `cmp #0` of `a` before a branch on zero or
//!   sign is `tax` (`tay`) when that register and the carry are dead.
//! - [`compare_through_an_index`]: `txa; cmp m` is `cpx m` when `a` is dead
//!   after it (likewise `tya`, `cpy`).
//! - [`drop_zero_compares`]: `cmp #0` (likewise `cpx`, `cpy`) before a branch
//!   on zero or sign, after an instruction that set those flags from that
//!   register. Labels between the two do not matter: a path that jumps to one
//!   brings its own flags.
//! - [`load_straight_into_a`]: `ldx m; ...; txa` (likewise `ldy` and `tya`)
//!   with only loads and stores of other bytes between, and `x` dead after the
//!   transfer: `lda m`.
//! - [`store_before_transfer`]: `tax; ...; stx m` (likewise `tay` and `sty`)
//!   with only work on `a` between, `m` untouched and the index register dead
//!   after: `sta m` where the transfer was.
//! - [`drop_cancelling_transfers`]: `tya; tay` and its three twins: the second
//!   moves what the first just moved.
//! - [`branch_on_shift_carry`]: `ldx m; bpl` after `asl|rol c` where `c` held
//!   what `m` holds: the shift's carry is `m`'s top bit, so `bcc` replaces
//!   both.
//! - [`zero_cells_with_stz`]: 65C02 `lda #0; sta m` (a run of stores of the
//!   zero) with `a` dead after: `stz m` each.
//! - [`step_in_place`]: `lda #1; clc; adc m; sta m` (also `lda m; clc; adc #1;
//!   sta m`, `lda m; sec; sbc #1; sta m` and the 65C02's `lda m; inc a; sta m`,
//!   each a step of 1) of a byte in memory, with `a` and the carry dead after:
//!   `inc m` (`dec m`), which sets the sign and zero flags as the store left
//!   them.
//! - [`shift_in_place`]: `lda m; asl|lsr|rol|ror a; sta m` of a byte in memory
//!   with `a` dead after: `asl|lsr|rol|ror m`, the same carry and sign and zero
//!   flags (2 bytes and 3 clocks fewer on a zero-page cell).
//! - [`jump_to_return`]: `jmp l` where `l:` is a lone `rts` becomes `rts`.
//!
//! DEVIATE: LLVM's `PeepholeOptimizer` and `MachineCSE` work on allocated
//! machine instructions before they are expanded; these work on the expanded
//! 6502 instructions of one function, because the 6502 emitter expands as it
//! allocates. Exit: D-6502 of the greedy PR #6259 puts the 6502 on the shared
//! pipeline; the passes that need only registers and flags move there.
//!
//! # Example
//! ```text
//!   sta rc2          sta rc2
//!   lda rc2     =>   ora rc3
//!   ora rc3
//! ```
//!
//! # Runs
//! At the end of `alloc::emit_with`, before branch relaxation.

use llrm_support::hash::{HashMap, HashSet};

use crate::asm::{Asm, Flags, Index, Inst, Line, Op, Operand, Registers};

/// Whether the zero and sign flags are set again before anything reads them,
/// from `lines[from]` on.
pub(crate) fn flags_unread_after(
    lines: &[Line],
    from: usize,
) -> bool {
    for line in &lines[from..] {
        // control falling through a label carries these flags on; the other
        // ways in bring their own
        if matches!(line, Line::Label(_)) {
            continue;
        }
        let Line::Inst(inst) = line else { return false };
        // a jump leaves the block: no block reads the flags its predecessor
        // left (`bra` is the 65C02's)
        if matches!(inst.op, Op::Jmp | Op::Bra) {
            return true;
        }
        match inst.flags() {
            Flags::Keeps => {}
            Flags::Of(_) | Flags::Sets => return true,
            Flags::Reads => return false,
        }
    }
    false
}

/// Whether the emitted instruction leaves the zero and sign flags describing
/// what `a` now holds.
pub fn sets_nz_of_a(inst: &Inst) -> bool {
    sets_nz_of(inst, Registers::A)
}

/// Whether the instruction leaves the zero and sign flags describing what
/// `register` now holds.
fn sets_nz_of(
    inst: &Inst,
    register: Registers,
) -> bool {
    matches!(inst.flags(), Flags::Of(set) if set.contains(register))
}

/// The passes, in the order they feed each other.
pub fn run(asm: &mut Asm) {
    run_with(asm, true);
}

/// [`run`], the transfer of a known constant (`drop_known_loads`) left out when
/// `known_loads` is false: the text a layout choice is priced on.
pub fn run_with(
    asm: &mut Asm,
    known_loads: bool,
) {
    // DEVIATE 6502-RET-TAIL (owner 6502-selectjoin, exit 2026-11-20: the shared
    // post-allocation `latetaildup` on the 6502 flow, step 5): the pin's late
    // TailDuplicate copies a return block into the block that falls into it;
    // that block's code is only known here, so the copy is kept when the whole
    // pass list leaves fewer bytes.
    let mut copied = asm.clone();
    if duplicate_return_tail_into_fall_through(&mut copied) {
        run_passes(&mut copied, known_loads);
        run_passes(asm, known_loads);
        if code_bytes(&copied) < code_bytes(asm) {
            *asm = copied;
        }
        return;
    }
    run_passes(asm, known_loads);
}

/// `(run_with(asm, false), run_with(asm, true))` on one copy of `asm`: the
/// passes before the first that reads `known_loads` run once for both.
pub fn run_pair(asm: &Asm) -> (Asm, Asm) {
    let mut copied = asm.clone();
    let tail_copied = duplicate_return_tail_into_fall_through(&mut copied);
    let mut base = asm.clone();
    run_prefix(&mut base);
    if tail_copied {
        run_prefix(&mut copied);
    }
    let finish = |known_loads: bool| {
        let mut plain = base.clone();
        run_suffix(&mut plain, known_loads);
        if tail_copied {
            let mut with_copy = copied.clone();
            run_suffix(&mut with_copy, known_loads);
            if code_bytes(&with_copy) < code_bytes(&plain) {
                return with_copy;
            }
        }
        plain
    };
    (finish(false), finish(true))
}

fn code_bytes(asm: &Asm) -> usize {
    asm.lines.iter().filter_map(Line::inst).map(Inst::bytes).sum()
}

/// A label whose block is one straight instruction and an `rts`, reached by
/// falling into it: the block is copied ahead of the label, so the fall-through
/// path returns by itself. One instruction is the pin's size-optimising limit
/// (`MaxDuplicateCount = 1`,
/// [TailDuplicator.cpp#L582](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L582)).
fn duplicate_return_tail_into_fall_through(asm: &mut Asm) -> bool {
    let mut changed = false;
    let mut at = 1;
    while at + 2 < asm.lines.len() {
        let (Line::Label(_), Some(Line::Inst(first)), Some(Line::Inst(last))) =
            (&asm.lines[at], asm.lines.get(at + 1), asm.lines.get(at + 2))
        else {
            at += 1;
            continue;
        };
        let straight = matches!(
            first.op,
            Op::Tax | Op::Tay | Op::Txa | Op::Tya | Op::Lda | Op::Ldx | Op::Ldy
        );
        let falls_in = matches!(
            &asm.lines[at - 1],
            Line::Inst(before) if !matches!(before.op, Op::Jmp | Op::Bra | Op::Rts)
        );
        if straight && last.op == Op::Rts && falls_in {
            let tail = [asm.lines[at + 1].clone(), asm.lines[at + 2].clone()];
            asm.lines.splice(at..at, tail);
            at += 3;
            changed = true;
        } else {
            at += 1;
        }
    }
    changed
}

fn run_passes(
    asm: &mut Asm,
    known_loads: bool,
) {
    run_prefix(asm);
    run_suffix(asm, known_loads);
}

thread_local! {
    static PREFIX_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread ran the passes ahead of the first that reads
/// `known_loads` (what a test counts to see they ran once for a pair).
pub fn prefix_runs() -> usize {
    PREFIX_RUNS.with(std::cell::Cell::get)
}

/// The passes ahead of the first that reads `known_loads`: the same for the
/// priced text and the text kept, so a pair runs them once.
fn run_prefix(asm: &mut Asm) {
    PREFIX_RUNS.with(|runs| runs.set(runs.get() + 1));
    fold_known_cells(asm);
    hoist_common_successor_code(asm);
    branch_over_jump(asm);
    duplicate_return_tail(asm);
    drop_reloads(asm);
    drop_flag_reload(asm);
    drop_dead_repeat_load(asm);
    zero_cells_with_stz(asm);
    merge_saves_of_a(asm);
}

fn run_suffix(
    asm: &mut Asm,
    known_loads: bool,
) {
    if known_loads {
        drop_known_loads(asm);
    }
    // after the known loads: a register that holds the literal stores it
    // without a bracket; the loads the moved pair repeats are known in turn
    sink_save_bracket(asm);
    if known_loads {
        drop_known_loads(asm);
    }
    hoist_loop_constant(asm);
    sink_step_to_compare(asm);
    drop_zero_compares(asm);
    zero_test_through_an_index(asm);
    step_in_place(asm);
    shift_in_place(asm);
    load_straight_into_a(asm);
    compare_through_an_index(asm);
    store_before_transfer(asm);
    drop_cancelling_transfers(asm);
    crate::peephole_cells::carry_stays_in_the_flag(asm);
    crate::peephole_cells::fold_temp_operand(asm);
    crate::peephole_cells::fold_spill_copy(asm);
    crate::peephole_cells::arithmetic_chain_in_a(asm);
    crate::peephole_cells::chain_ends_in_a(asm);
    crate::peephole_cells::forward_cell_copy(asm);
    crate::peephole_cells::carry_holds_the_bit(asm);
    crate::peephole_cells::coalesce_copy(asm);
    // the chain's load can follow the store it was loaded from
    drop_reloads(asm);
    if crate::copyprop::enabled(std::env::var_os("LLRM_6502_MCP").as_deref()) {
        crate::copyprop::copy_propagation(asm);
    }
    crate::peephole_cells::drop_dead_stores(asm);
    drop_dead_writes(asm);
    // a transfer that fed an `or` of zero is gone: the load is next to its
    // store
    drop_reloads(asm);
    crate::peephole_cells::store_then_index_load(asm);
    // last of the cell passes: the choice is by the final size, after the
    // chain passes have taken what they take from the loop's neighbours
    crate::peephole_cells::shift_loop_in_a(asm);
    branch_on_shift_carry(asm);
    jump_to_return(asm);
}

/// How many lines name `label`: a branch or jump to it, or a directive.
fn references(
    lines: &[Line],
    label: &str,
) -> usize {
    lines
        .iter()
        .filter(|line| match line {
            Line::Inst(Inst { operand, .. }) => operand.name() == Some(label),
            Line::Raw(text) => text.contains(label),
            Line::Label(_) => false,
        })
        .count()
}

/// The flag a conditional branch tests: `c`, `z`, `n` or `v`.
fn tested_flag(op: Op) -> Option<char> {
    match op {
        Op::Bcc | Op::Bcs => Some('c'),
        Op::Beq | Op::Bne => Some('z'),
        Op::Bmi | Op::Bpl => Some('n'),
        Op::Bvc | Op::Bvs => Some('v'),
        _ => None,
    }
}

/// Whether `inst` can run before a branch that tests `flag` instead of after
/// it, when both paths start with it: it reads no flag and leaves that one
/// alone (`lda`, `ldx`, `ldy`, the transfers write only `n` and `z`; the
/// stores write none; `clc` and `sec` write `c`).
fn hoistable(
    inst: &Inst,
    flag: char,
) -> bool {
    match inst.op {
        Op::Sta | Op::Stx | Op::Sty | Op::Stz => true,
        Op::Lda | Op::Ldx | Op::Ldy | Op::Tax | Op::Tay | Op::Txa | Op::Tya => flag == 'c' || flag == 'v',
        Op::Clc | Op::Sec => flag != 'c',
        _ => false,
    }
}

/// The instructions both paths of a conditional branch start with run before
/// the branch, once: `bcs b20 / b23: lda #1; sta rc8; jmp b3 / b20: lda #1; sta
/// rc8` is `lda #1; sta rc8; bcs b20 / b23: jmp b3 / b20:`. Each path has only
/// the branch for a predecessor. The pin's `BranchFolder::HoistCommonCode`
/// ([BranchFolding.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1980)),
/// which also stops at an instruction that touches what the branch reads.
pub fn hoist_common_successor_code(asm: &mut Asm) {
    let mut at = 0;
    while at < asm.lines.len() {
        at += 1;
        let Some((hoisted, fall, target, insert)) = common_successor_code(&asm.lines, at - 1, Exits::of(asm)) else {
            continue;
        };
        let count = hoisted.len();
        let mut lines = Vec::with_capacity(asm.lines.len());
        for (index, line) in asm.lines.drain(..).enumerate() {
            if index == insert {
                lines.extend(hoisted.iter().cloned().map(Line::Inst));
            }
            if (fall..fall + count).contains(&index) || (target..target + count).contains(&index) {
                continue;
            }
            lines.push(line);
        }
        asm.lines = lines;
        at += count;
    }
}

/// The instructions to hoist above the branch at `at`, where the two copies
/// start (the fall-through path's first, the target's first), and where the
/// hoisted copy goes (the branch, or the flag setup before it).
fn common_successor_code(
    lines: &[Line],
    at: usize,
    exits: Exits,
) -> Option<(Vec<Inst>, usize, usize, usize)> {
    let Line::Inst(branch) = &lines[at] else { return None };
    let flag = tested_flag(branch.op)?;
    let Operand::Direct(label) = &branch.operand else { return None };
    let target_label = lines.iter().position(|line| matches!(line, Line::Label(name) if name == label))?;
    // reached by this branch alone
    let before = target_label.checked_sub(1).and_then(|index| lines[index].inst())?;
    if references(lines, label) != 1 || !matches!(before.op, Op::Jmp | Op::Bra | Op::Rts) {
        return None;
    }
    let mut fall = at + 1;
    while let Some(Line::Label(name)) = lines.get(fall) {
        if references(lines, name) != 0 {
            return None;
        }
        fall += 1;
    }
    let target = target_label + 1;
    let mut hoisted = Vec::new();
    while let (Some(Line::Inst(one)), Some(Line::Inst(other))) =
        (lines.get(fall + hoisted.len()), lines.get(target + hoisted.len()))
    {
        if one.op != other.op || one.operand != other.operand || !hoistable(one, flag) {
            break;
        }
        hoisted.push(one.clone());
    }
    if hoisted.is_empty() {
        let (copy, insert) = hoist_above_flag_setup(lines, at, fall, target, flag, exits)?;
        return Some((copy, fall, target, insert));
    }
    Some((hoisted, fall, target, at))
}

/// Whether `inst` reads only registers, an immediate or a named cell or global.
fn plain_operand(inst: &Inst) -> bool {
    matches!(inst.operand, Operand::Imm(_)) || matches!(inst.operand, Operand::Direct(_))
}

/// Both paths of a `beq`/`bne`/`bmi`/`bpl` start with `lda c; sta d` (the
/// loads write the flags the branch reads, so they cannot sit between the test
/// and the branch): the copy goes before the loads that make the flags
/// (`lda rc6; ora rc7; bne`), which read none of the cells the copy stores.
/// The copy leaves `a` and `n`/`z` set by its last load, and the setup then
/// sets both, so neither successor may read `a`, `n` or `z` before writing
/// them. Returns the copy and the index of the setup's first instruction.
fn hoist_above_flag_setup(
    lines: &[Line],
    at: usize,
    fall: usize,
    target: usize,
    flag: char,
    exits: Exits,
) -> Option<(Vec<Inst>, usize)> {
    if flag != 'z' && flag != 'n' {
        return None;
    }
    let mut copy = Vec::new();
    while let (Some(Line::Inst(one)), Some(Line::Inst(other))) =
        (lines.get(fall + copy.len()), lines.get(target + copy.len()))
    {
        let cell = matches!(one.op, Op::Sta) && one.operand.is_cell()
            || matches!(one.op, Op::Lda) && (one.operand.is_cell() || matches!(one.operand, Operand::Imm(_)));
        if one.op != other.op || one.operand != other.operand || !cell {
            break;
        }
        copy.push(one.clone());
    }
    if copy.first().is_none_or(|first| first.op != Op::Lda) {
        return None;
    }
    // the setup: plain loads and logic right before the branch, from its last
    // `lda`
    let mut start = at;
    while let Some(Line::Inst(inst)) = start.checked_sub(1).map(|b| &lines[b]) {
        if !matches!(inst.op, Op::Lda | Op::Ora | Op::And | Op::Eor) || !plain_operand(inst) {
            break;
        }
        start -= 1;
    }
    while start < at && !matches!(&lines[start], Line::Inst(Inst { op: Op::Lda, .. })) {
        start += 1;
    }
    if start == at {
        return None;
    }
    let stored: Vec<&Operand> = copy.iter().filter(|inst| inst.op == Op::Sta).map(|inst| &inst.operand).collect();
    if lines[start..at].iter().any(|line| line.inst().is_some_and(|inst| stored.contains(&&inst.operand))) {
        return None;
    }
    let count = copy.len();
    let leaves_a_and_flags_unread = |from: usize| {
        dead_from(lines, from, Registers::A, &mut HashSet::default(), exits) && !reads_nz_first(lines, from)
    };
    (leaves_a_and_flags_unread(fall + count) && leaves_a_and_flags_unread(target + count)).then_some((copy, start))
}

/// Whether a branch on `n` or `z` runs before anything writes them, straight on
/// from `from` (a jump, a call or raw text that is not preceded by a write
/// stops the search as unsafe).
fn reads_nz_first(
    lines: &[Line],
    from: usize,
) -> bool {
    for line in &lines[from..] {
        let inst = match line {
            Line::Inst(inst) => inst,
            // a label changes nothing on this path
            Line::Label(_) => continue,
            _ => return true,
        };
        match inst.op {
            Op::Beq | Op::Bne | Op::Bmi | Op::Bpl => return true,
            Op::Lda
            | Op::Ldx
            | Op::Ldy
            | Op::Tax
            | Op::Tay
            | Op::Txa
            | Op::Tya
            | Op::Cmp
            | Op::Cpx
            | Op::Cpy
            | Op::And
            | Op::Ora
            | Op::Eor
            | Op::Adc
            | Op::Sbc
            | Op::Inx
            | Op::Iny
            | Op::Dex
            | Op::Dey => return false,
            Op::Sta | Op::Stx | Op::Sty | Op::Stz | Op::Clc | Op::Sec => {}
            _ => return true,
        }
    }
    true
}

/// `bcs l / jmp m / l:` is `bcc m / l:` (labels nothing names between the
/// branch and the jump go).
pub fn branch_over_jump(asm: &mut Asm) {
    let mut at = 0;
    while at < asm.lines.len() {
        at += 1;
        let Line::Inst(branch) = &asm.lines[at - 1] else { continue };
        if branch.op == Op::Bra || branch.op.opposite().is_none() {
            continue;
        }
        let mut jump = at;
        while matches!(
            &asm.lines[jump..],
            [Line::Label(name), ..] if references(&asm.lines, name) == 0
        ) {
            jump += 1;
        }
        let (Some(Line::Inst(jmp)), Some(Line::Label(over))) = (asm.lines.get(jump), asm.lines.get(jump + 1)) else {
            continue;
        };
        if jmp.op != Op::Jmp || branch.operand != Operand::Direct(over.clone()) {
            continue;
        }
        let Operand::Direct(far) = &jmp.operand else { continue };
        let inverted = Inst::new(branch.op.opposite().unwrap(), Operand::Direct(far.clone()));
        asm.lines.splice(at - 1..=jump, [Line::Inst(inverted)]);
    }
}

/// `inc m` or `dec m` then `lda m` with `a` dead after it: the load only sets
/// the flags the step already set from the same result (a 16-bit down-counter's
/// `dec rc4; lda rc4; bne`).
pub fn drop_flag_reload(asm: &mut Asm) {
    let exits = Exits::of(asm);
    let keep: Vec<bool> = (0..asm.lines.len())
        .map(|at| {
            let (Some(Line::Inst(step)), Line::Inst(load)) = (at.checked_sub(1).map(|b| &asm.lines[b]), &asm.lines[at])
            else {
                return true;
            };
            let reload = load.op == Op::Lda
                && matches!(step.op, Op::Inc | Op::Dec)
                && load.operand.is_cell()
                && step.operand == load.operand
                && dead_from(&asm.lines, at + 1, Registers::A, &mut HashSet::default(), exits);
            !reload
        })
        .collect();
    retain(asm, &keep);
}

/// `ldx m; ldy m` (any two of `lda`, `ldx`, `ldy` of one cell) with the second
/// register dead after: the second load only repeats the flags the first set,
/// for a branch (`xs8`'s guard `n != 0` loaded the count twice).
pub fn drop_dead_repeat_load(asm: &mut Asm) {
    let exits = Exits::of(asm);
    let keep: Vec<bool> = (0..asm.lines.len())
        .map(|at| {
            let (Some(Line::Inst(first)), Line::Inst(second)) =
                (at.checked_sub(1).map(|b| &asm.lines[b]), &asm.lines[at])
            else {
                return true;
            };
            let register = |op: Op| match op {
                Op::Lda => Some(Registers::A),
                Op::Ldx => Some(Registers::X),
                Op::Ldy => Some(Registers::Y),
                _ => None,
            };
            let repeat = matches!(
                (register(first.op), register(second.op)),
                (Some(one), Some(two)) if one != two
            ) && first.operand.is_cell()
                && first.operand == second.operand
                && dead_from(&asm.lines, at + 1, register(second.op).unwrap(), &mut HashSet::default(), exits);
            !repeat
        })
        .collect();
    retain(asm, &keep);
}

/// `l: ldy #k` at the top of a loop that nothing else enters and that does not
/// write `y` again (no call, no other write): the load runs once, before the
/// loop (`x` the same). The pin's MachineLICM hoists the constant; here the
/// register keeps it.
pub fn hoist_loop_constant(asm: &mut Asm) {
    let mut at = 0;
    while at < asm.lines.len() {
        at += 1;
        let Line::Label(label) = &asm.lines[at - 1] else { continue };
        let (Some(Line::Inst(load)), Some(Line::Inst(next))) = (asm.lines.get(at), asm.lines.get(at + 1)) else {
            continue;
        };
        let writes = match load.op {
            Op::Ldy => [Op::Ldy, Op::Tay, Op::Iny, Op::Dey],
            Op::Ldx => [Op::Ldx, Op::Tax, Op::Inx, Op::Dex],
            _ => continue,
        };
        if !matches!(load.operand, Operand::Imm(_)) || !matches!(next.op.flags(&next.operand), Flags::Of(_)) {
            continue;
        }
        // the entry falls into the label, and every other reference is a branch
        // back from the loop
        let mut before = at - 1;
        while before > 0
            && matches!(
                &asm.lines[before - 1],
                Line::Label(name) if references(&asm.lines, name) == 0
            )
        {
            before -= 1;
        }
        let Some(Line::Inst(entry)) = before.checked_sub(1).map(|index| &asm.lines[index]) else { continue };
        if matches!(entry.op, Op::Jmp | Op::Bra | Op::Rts) {
            continue;
        }
        let mut last = at;
        let mut closed = true;
        for (index, line) in asm.lines.iter().enumerate() {
            match line {
                Line::Inst(Inst { operand, .. }) if operand.name() == Some(label.as_str()) => {
                    if index < at {
                        closed = false;
                    }
                    last = last.max(index);
                }
                Line::Raw(text) if text.contains(label.as_str()) => closed = false,
                _ => {}
            }
        }
        if !closed || last == at {
            continue;
        }
        let quiet = asm
            .lines[at + 1..=last]
            .iter()
            .all(
                |line| match line {
                    Line::Inst(inst) => !writes.contains(&inst.op) && inst.op != Op::Jsr,
                    Line::Label(_) => true,
                    Line::Raw(_) => false,
                },
            );
        if !quiet {
            continue;
        }
        let moved = asm.lines.remove(at);
        asm.lines.insert(before, moved);
    }
}

/// What a register-only instruction writes: a register, and which of the carry
/// and the zero and sign flags.
struct Writes {
    register: Option<Registers>,
    carry: bool,
    nz: bool,
}

/// The writes of `inst` if it does nothing else: no memory, no call, no branch,
/// no stack. A load reads a zero-page cell or an immediate, never a device.
fn pure_writes(inst: &Inst) -> Option<Writes> {
    use Op::*;
    let plain = matches!(inst.operand, Operand::Imm(_)) || inst.operand.is_cell();
    let (register, carry, nz) = match inst.op {
        Lda | And | Ora | Eor if plain => (Some(Registers::A), false, true),
        Ldx if plain => (Some(Registers::X), false, true),
        Ldy if plain => (Some(Registers::Y), false, true),
        Tax | Dex | Inx => (Some(Registers::X), false, true),
        Tay | Dey | Iny => (Some(Registers::Y), false, true),
        Txa | Tya => (Some(Registers::A), false, true),
        Asl | Lsr | Rol | Ror if inst.operand == Operand::Acc => (Some(Registers::A), true, true),
        Cmp | Cpx | Cpy if plain => (None, true, true),
        Clc | Sec => (None, true, false),
        _ => return None,
    };
    Some(Writes { register, carry, nz })
}

/// A register-only instruction all of whose results are dead goes: the register
/// is written before it is read, the carry and the zero and sign flags too,
/// along every path.
///
/// The pin's `DeadMachineInstructionElim`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/DeadMachineInstructionElim.cpp>)
/// removes a def nothing reads while the value is a virtual register; here the
/// last reader of a carry made into a byte (`lda #0; rol a; sta cell`) is the
/// store that `drop_dead_stores` removes after allocation, so the def is dead
/// only now. The pin has no post-allocation dead-def elimination:
/// `DeadMachineInstructionElim` is added only in `addMachineSSAOptimization`
/// ([TargetPassConfig.cpp#L1361](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetPassConfig.cpp#L1361),
/// [#L1376](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetPassConfig.cpp#L1376)),
/// and `addMachineLateOptimization`
/// ([#L1560](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetPassConfig.cpp#L1560))
/// runs `MachineLateInstrsCleanup`, `BranchFolder`, `TailDuplicate` and
/// `MachineCopyPropagation` only.
///
/// DEVIATE 6502-DEAD-WRITE (owner llrm-target-6502, exit 2026-11-20; 6502 only,
/// no other target runs this pass): the same rule over the registers and flags
/// the instruction writes, in the asm; exit: a post-allocation dead-def
/// elimination on machine IR.
pub fn drop_dead_writes(asm: &mut Asm) {
    let mut at = asm.lines.len();
    while at > 0 {
        at -= 1;
        let Line::Inst(inst) = &asm.lines[at] else { continue };
        let Some(writes) = pure_writes(inst) else { continue };
        let exits = Exits::of(asm);
        let dead = writes
            .register
            .is_none_or(|register| dead_from(&asm.lines, at + 1, register, &mut HashSet::default(), exits))
            && (!writes.carry || carry_dead_from(&asm.lines, at + 1, &mut HashSet::default()))
            && (!writes.nz || nz_dead_from(&asm.lines, at + 1, &mut HashSet::default()));
        if dead {
            asm.lines.remove(at);
        }
    }
}

/// Whether the zero and sign flags are written before they are read, along
/// every path from `lines[from]`. A call or a return counts as a write.
fn nz_dead_from(
    lines: &[Line],
    from: usize,
    seen: &mut HashSet<usize>,
) -> bool {
    let to =
        |inst: &Inst| inst.target().and_then(|label| lines.iter().position(|l| *l == Line::Label(label.to_owned())));
    let mut at = from;
    while let Some(line) = lines.get(at) {
        if !seen.insert(at) {
            return true;
        }
        at += 1;
        let inst = match line {
            Line::Inst(inst) => inst,
            Line::Label(_) => continue,
            Line::Raw(_) => return false,
        };
        match inst.op {
            Op::Beq | Op::Bne | Op::Bmi | Op::Bpl => return false,
            Op::Bcc | Op::Bcs | Op::Bvc | Op::Bvs => {
                if !to(inst).is_some_and(|target| nz_dead_from(lines, target, seen)) {
                    return false;
                }
            }
            Op::Jmp | Op::Bra => return to(inst).is_some_and(|target| nz_dead_from(lines, target, seen)),
            _ => match inst.flags() {
                Flags::Keeps => {}
                Flags::Of(_) | Flags::Sets => return true,
                Flags::Reads => return false,
            },
        }
    }
    false
}

/// The first instruction at or after `lines[from]`, past labels.
fn next_instruction(
    lines: &[Line],
    from: usize,
) -> Option<&Inst> {
    for line in &lines[from.min(lines.len())..] {
        match line {
            Line::Inst(inst) => return Some(inst),
            Line::Label(_) => {}
            Line::Raw(_) => return None,
        }
    }
    None
}

/// `dex` (likewise `inx`, `dey`, `iny`), then straight-line work that leaves
/// the register alone, then `cpx #0; bne`: the step moves down to the compare,
/// which its flags make redundant ([`drop_zero_compares`]). A count taken down
/// at the top of a rotated loop is tested at the bottom without a compare.
pub fn sink_step_to_compare(asm: &mut Asm) {
    let mut at = 0;
    while at < asm.lines.len() {
        let Line::Inst(step) = &asm.lines[at] else {
            at += 1;
            continue;
        };
        let (index, compare) = match step.op {
            Op::Dex | Op::Inx => (Index::X, Op::Cpx),
            Op::Dey | Op::Iny => (Index::Y, Op::Cpy),
            _ => {
                at += 1;
                continue;
            }
        };
        let mut end = at + 1;
        let found = loop {
            match asm.lines.get(end) {
                Some(Line::Inst(inst))
                    if inst.op == compare && matches!(&inst.operand, Operand::Imm(v) if v == "0") =>
                {
                    break matches!(
                        next_instruction(&asm.lines, end + 1),
                        Some(next) if matches!(next.op, Op::Beq | Op::Bne | Op::Bmi | Op::Bpl)
                    );
                }
                Some(Line::Inst(inst)) if leaves_index_alone(inst, index) => end += 1,
                _ => break false,
            }
        };
        if found && end > at + 1 {
            let step = asm.lines.remove(at);
            asm.lines.insert(end - 1, step);
        }
        at += 1;
    }
}

/// Whether `inst` is straight-line work that neither reads nor writes `index`.
fn leaves_index_alone(
    inst: &Inst,
    index: Index,
) -> bool {
    let plain = matches!(
        inst.op,
        Op::Lda
            | Op::Sta
            | Op::Adc
            | Op::Sbc
            | Op::And
            | Op::Ora
            | Op::Eor
            | Op::Asl
            | Op::Lsr
            | Op::Rol
            | Op::Ror
            | Op::Clc
            | Op::Sec
            | Op::Inc
            | Op::Dec
            | Op::Cmp
            | Op::Nop
    );
    let addressed = match &inst.operand {
        Operand::Indexed(_, used) => *used == index,
        Operand::IndirectX(_) => index == Index::X,
        Operand::IndirectY(_) => index == Index::Y,
        _ => false,
    };
    plain && !addressed
}

/// `cmp #0` before a branch on zero or sign, after an instruction that set
/// those flags from `a` (stores in between leave them): the branch reads them
/// as they are.
pub fn drop_zero_compares(asm: &mut Asm) {
    let lines = &asm.lines;
    let keep: Vec<bool> = (0..lines.len())
        .map(|at| {
            let compared = match &lines[at] {
                Line::Inst(Inst { op, operand: Operand::Imm(v), .. }) if v == "0" => match op {
                    Op::Cmp => Some(Registers::A),
                    Op::Cpx => Some(Registers::X),
                    Op::Cpy => Some(Registers::Y),
                    _ => None,
                },
                _ => None,
            };
            let branch = next_instruction(lines, at + 1)
                .is_some_and(|next| matches!(next.op, Op::Beq | Op::Bne | Op::Bmi | Op::Bpl));
            let tested = branch
                && compared.is_some_and(|register| {
                    lines[..at]
                        .iter()
                        .rev()
                        .find(|before| !before.inst().is_some_and(|i| i.flags() == Flags::Keeps))
                        .and_then(Line::inst)
                        .is_some_and(|before| sets_nz_of(before, register))
                });
            !tested
        })
        .collect();
    retain(asm, &keep);
}

/// `cmp #0` of `a` before a branch on zero or sign, with `x` (else `y`) and the
/// carry dead after it: `tax` (`tay`) sets the same flags in a byte less. A
/// byte argument arrives in `a` with no flags set.
pub fn zero_test_through_an_index(asm: &mut Asm) {
    for at in 0..asm.lines.len() {
        let Line::Inst(Inst { op: Op::Cmp, operand: Operand::Imm(zero), place }) = &asm.lines[at] else { continue };
        let place = *place;
        let branch = next_instruction(&asm.lines, at + 1)
            .is_some_and(|next| matches!(next.op, Op::Beq | Op::Bne | Op::Bmi | Op::Bpl));
        if zero != "0" || !branch || !carry_dead_from(&asm.lines, at + 1, &mut HashSet::default()) {
            continue;
        }
        let exits = Exits::of(asm);
        let free = [(Registers::X, Op::Tax), (Registers::Y, Op::Tay)]
            .into_iter()
            .find(|(register, _)| dead_from(&asm.lines, at + 1, *register, &mut HashSet::default(), exits));
        if let Some((_, transfer)) = free {
            asm.lines[at] = Line::Inst(Inst { op: transfer, operand: Operand::None, place });
        }
    }
}

/// `txa; cmp m` (likewise `tya`, `cpy`) with `a` dead after the compare:
/// `cpx m` sets the same flags in a byte less and 2 clocks fewer. The compare
/// of a value kept in an index register is staged through `a`, the one
/// register the instruction selection compares in.
///
/// DEVIATE (6502-CPX): the pin's compare takes its left side in any register
/// of the class the allocator gives it (X86 `CMP32rr`); here the left side is
/// fixed in `a` before allocation, so the register it lives in is known only
/// afterwards. Owner 6502, exit 2026-11-30: the left side of a compare in
/// `a`, `x` or `y` at selection.
pub fn compare_through_an_index(asm: &mut Asm) {
    let mut at = 1;
    while at < asm.lines.len() {
        at += 1;
        let (Line::Inst(transfer), Line::Inst(compare)) = (&asm.lines[at - 2], &asm.lines[at - 1]) else { continue };
        let index = match transfer.op {
            Op::Txa => Op::Cpx,
            Op::Tya => Op::Cpy,
            _ => continue,
        };
        // `cpx`/`cpy` have no indexed or indirect form
        if compare.op != Op::Cmp || !matches!(compare.operand, Operand::Imm(_)) && !compare.operand.is_cell() {
            continue;
        }
        let exits = Exits::of(asm);
        if !dead_from(&asm.lines, at, Registers::A, &mut HashSet::default(), exits) {
            continue;
        }
        let place = compare.place;
        let operand = compare.operand.clone();
        asm.lines[at - 1] = Line::Inst(Inst { op: index, operand, place });
        asm.lines.remove(at - 2);
        at -= 1;
    }
}

/// The lines of `asm` whose entry in `keep` is true.
fn retain(
    asm: &mut Asm,
    keep: &[bool],
) {
    let mut keep = keep.iter();
    asm.lines.retain(|_| *keep.next().unwrap());
}

/// Whether a call keeps the cell `name`: `rc8`..`rc15` are callee-saved.
fn kept_by_calls(name: &str) -> bool {
    name.strip_prefix("rc").and_then(|n| n.parse::<u8>().ok()).is_some_and(|n| (8..=15).contains(&n))
}

/// A load or an ALU operation of a cell that holds a known immediate reads the
/// immediate instead. The values known are the immediates loaded into a
/// register and stored from it (`sta`, `stx`, `sty`, `stz`) to a zero-page or
/// spill cell, forgotten when the cell is written another way, at a label, at
/// an instruction not listed, and at a call for every cell but `rc8`..`rc15`.
/// A store through a pointer or an index may write any cell.
pub fn fold_known_cells(asm: &mut Asm) {
    let mut registers: [Option<String>; 3] = [None, None, None];
    let mut cells: std::collections::HashMap<String, String> = Default::default();
    let register_of = |op: Op| match op {
        Op::Lda | Op::Sta | Op::Cmp => 0,
        Op::Ldx | Op::Stx | Op::Cpx => 1,
        _ => 2,
    };
    for line in &mut asm.lines {
        let Line::Inst(inst) = line else {
            registers = Default::default();
            cells.clear();
            continue;
        };
        let cell = match &inst.operand {
            Operand::Direct(name) if inst.operand.is_cell() => Some(name.clone()),
            _ => None,
        };
        let known = cell.as_ref().and_then(|name| cells.get(name)).cloned();
        match inst.op {
            Op::Lda | Op::Ldx | Op::Ldy => {
                if let Some(value) = known {
                    inst.operand = Operand::Imm(value);
                }
                registers[register_of(inst.op)] = match &inst.operand {
                    Operand::Imm(value) => Some(value.clone()),
                    _ => None,
                };
            }
            Op::Cmp | Op::Cpx | Op::Cpy => {
                if let Some(value) = known {
                    inst.operand = Operand::Imm(value);
                }
            }
            Op::Adc | Op::Sbc | Op::And | Op::Ora | Op::Eor => {
                if let Some(value) = known {
                    inst.operand = Operand::Imm(value);
                }
                registers[0] = None;
            }
            Op::Sta | Op::Stx | Op::Sty => match (&inst.operand, cell) {
                (_, Some(name)) => match &registers[register_of(inst.op)] {
                    Some(value) => drop(cells.insert(name, value.clone())),
                    None => drop(cells.remove(&name)),
                },
                // a global named directly is not a cell
                (Operand::Direct(_), None) => {}
                _ => cells.clear(),
            },
            Op::Stz => match (&inst.operand, cell) {
                (_, Some(name)) => drop(cells.insert(name, "0".to_owned())),
                (Operand::Direct(_), None) => {}
                _ => cells.clear(),
            },
            Op::Inc | Op::Dec | Op::Asl | Op::Lsr | Op::Rol | Op::Ror => match (&inst.operand, cell) {
                (Operand::Acc | Operand::None, _) => registers[0] = None,
                (_, Some(name)) => drop(cells.remove(&name)),
                (Operand::Direct(_), None) => {}
                _ => cells.clear(),
            },
            Op::Inx | Op::Dex => registers[1] = None,
            Op::Iny | Op::Dey => registers[2] = None,
            Op::Pla => registers[0] = None,
            Op::Txa => registers[0] = registers[1].clone(),
            Op::Tya => registers[0] = registers[2].clone(),
            Op::Tax => registers[1] = registers[0].clone(),
            Op::Tay => registers[2] = registers[0].clone(),
            Op::Clc | Op::Sec | Op::Pha | Op::Nop | Op::Bit => {}
            Op::Jsr => {
                registers = Default::default();
                match &inst.operand {
                    Operand::Direct(callee) if !callee.starts_with("__csave") && !callee.starts_with("__crest") => {
                        cells.retain(|name, _| kept_by_calls(name));
                    }
                    _ => cells.clear(),
                }
            }
            // a branch not taken falls through with all that is known here
            op if op.opposite().is_some() => {}
            _ => {
                registers = Default::default();
                cells.clear();
            }
        }
    }
}

/// What a register holds: an immediate, or the byte of a cell it was loaded
/// from or stored to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Value<'a> {
    Imm(&'a str),
    Cell(&'a str),
}

/// What `a`, `x` and `y` hold.
type Known<'a> = [Option<Value<'a>>; 3];

/// A register that held `cell` (any cell when `None`) no longer does: the cell
/// was written.
fn forget_cell(
    state: &mut Known<'_>,
    cell: Option<&str>,
) {
    for held in state.iter_mut() {
        if let Some(Value::Cell(name)) = held
            && cell.is_none_or(|written| written == *name)
        {
            *held = None;
        }
    }
}

/// The registers' values at a label, from every branch to it: a value both
/// edges agree on.
fn meet<'a>(
    into: &mut Known<'a>,
    other: &Known<'a>,
) {
    for (one, two) in into.iter_mut().zip(other) {
        if one != two {
            *one = None;
        }
    }
}

/// Whether the opcode jumps to the label it names, without a return.
fn jumps(op: Op) -> bool {
    op.is_branch() || op == Op::Jmp
}

/// The labels only branches in this text reach (a function's entry, anything
/// called, exported or named by a directive is not): a branch's target.
fn branch_targets(lines: &[Line]) -> (Vec<&str>, Vec<&str>) {
    let mut branched: Vec<&str> = Vec::new();
    let mut other: Vec<&str> = Vec::new();
    for line in lines {
        match line {
            Line::Inst(Inst { op, operand: Operand::Direct(name), .. }) if jumps(*op) => branched.push(name),
            Line::Inst(Inst { operand: Operand::Direct(name), .. }) => other.push(name),
            _ => {}
        }
    }
    let mut seen_label = false;
    let mut targets = Vec::new();
    let mut quiet = Vec::new();
    for line in lines {
        if let Line::Label(name) = line {
            let entry = !std::mem::replace(&mut seen_label, true);
            let named = lines.iter().any(|l| matches!(l, Line::Raw(text) if text.contains(name.as_str())));
            if !entry && !named && branched.contains(&name.as_str()) && !other.contains(&name.as_str()) {
                targets.push(name.as_str());
            } else if !entry && !named && !branched.contains(&name.as_str()) && !other.contains(&name.as_str()) {
                quiet.push(name.as_str());
            }
        }
    }
    (targets, quiet)
}

/// One walk over `lines` with the values known at each label in `at_label`
/// (a label no branch reached yet is not there): the loads to drop and to turn
/// into transfers, and the values each branch carries to its label.
fn walk<'a>(
    lines: &'a [Line],
    targets: &[&str],
    quiet: &[&str],
    at_label: &HashMap<&'a str, Known<'a>>,
) -> (Vec<bool>, Vec<(usize, Op)>, Vec<(usize, Inst, usize)>, HashMap<&'a str, Known<'a>>) {
    let mut known: Option<Known<'a>> = Some([None; 3]);
    let mut keep = vec![true; lines.len()];
    let mut transfers: Vec<(usize, Op)> = Vec::new();
    let mut carried: HashMap<&str, Known> = HashMap::default();
    let mut stores: Vec<(usize, Inst, usize)> = Vec::new();
    let mut skip = 0;
    for (at, line) in lines.iter().enumerate() {
        if at < skip {
            continue;
        }
        let Line::Inst(Inst { op, operand, .. }) = line else {
            known = Some(match line {
                Line::Label(name) if targets.contains(&name.as_str()) => {
                    // fallen into, and branched to
                    let mut here = at_label.get(name.as_str()).copied();
                    if let (Some(fall), Some(here)) = (known.as_ref(), here.as_mut()) {
                        meet(here, fall);
                    }
                    here.or(known).unwrap_or([None; 3])
                }
                // nothing names it: control only falls into it
                Line::Label(name) if quiet.contains(&name.as_str()) => known.unwrap_or([None; 3]),
                _ => [None; 3],
            });
            continue;
        };
        let mut state = known.unwrap_or([None; 3]);
        // `pha; lda #n; sta m; pla` (a literal made into a cell while `a` is
        // live) is one store when `x` or `y` already holds `n`
        if *op == Op::Pha
            && let [Some(Line::Inst(load)), Some(Line::Inst(store)), Some(Line::Inst(pull))] =
                [1, 2, 3].map(|n| lines.get(at + n))
            && (load.op, store.op, pull.op) == (Op::Lda, Op::Sta, Op::Pla)
            && let (Operand::Imm(value), Operand::Direct(_)) = (&load.operand, &store.operand)
            && flags_unread_after(lines, at + 4)
            && let Some(by) =
                [(1, Op::Stx), (2, Op::Sty)].into_iter().find(|(r, _)| state[*r] == Some(Value::Imm(value.as_str())))
        {
            stores.push((at, Inst { op: by.1, operand: store.operand.clone(), place: store.place }, 3));
            // the shortcut writes the cell the skipped `sta` wrote
            match &store.operand {
                Operand::Direct(name) => forget_cell(&mut state, Some(name)),
                _ => forget_cell(&mut state, None),
            }
            skip = at + 4;
            known = Some(state);
            continue;
        }
        let register = match op {
            Op::Lda => Some(0),
            Op::Ldx => Some(1),
            Op::Ldy => Some(2),
            _ => None,
        };
        let on_a = matches!(operand, Operand::None | Operand::Acc);
        if jumps(*op)
            && let Operand::Direct(name) = operand
            && targets.contains(&name.as_str())
        {
            carried.entry(name.as_str()).and_modify(|have| meet(have, &state)).or_insert(state);
        }
        // control does not run on past a jump or a return
        let ends = matches!(op, Op::Bra | Op::Jmp | Op::Rts);
        // a write of memory: a direct one changes its own cell, any other (an
        // indexed or indirect store, a read-modify-write through a pointer)
        // may change any
        if matches!(
            op,
            Op::Sta | Op::Stx | Op::Sty | Op::Stz | Op::Inc | Op::Dec | Op::Asl | Op::Lsr | Op::Rol | Op::Ror
        ) && !on_a
        {
            match operand {
                Operand::Direct(name) => forget_cell(&mut state, Some(name)),
                _ => forget_cell(&mut state, None),
            }
        }
        match (op, register, operand) {
            (_, Some(register), Operand::Imm(value)) => {
                if state[register] == Some(Value::Imm(value.as_str())) && flags_unread_after(lines, at + 1) {
                    keep[at] = false;
                    known = Some(state);
                    continue;
                }
                // `x` and `y` have no transfer between them
                let from = match register {
                    0 => [1, 2].into_iter().find(|&from| state[from] == Some(Value::Imm(value.as_str()))),
                    _ => (state[0] == Some(Value::Imm(value.as_str()))).then_some(0),
                };
                if let Some(from) = from {
                    let op = match (from, register) {
                        (1, 0) => Op::Txa,
                        (2, 0) => Op::Tya,
                        (0, 1) => Op::Tax,
                        _ => Op::Tay,
                    };
                    transfers.push((at, op));
                }
                state[register] = Some(Value::Imm(value));
            }
            // a load of a cell the register holds goes; a transfer from
            // another register that holds it is left: it would end the chain
            // folds below (`sta rc7 / ldx rc7` as `tax` hid the copy from them,
            // +384 instructions in `fill_nest`)
            (_, Some(register), Operand::Direct(name)) if operand.is_cell() => {
                let held = Some(Value::Cell(name.as_str()));
                if state[register] == held && flags_unread_after(lines, at + 1) {
                    keep[at] = false;
                    known = Some(state);
                    continue;
                }
                state[register] = held;
            }
            // a store leaves the register holding the cell it wrote (unless
            // it holds an immediate)
            (Op::Sta | Op::Stx | Op::Sty, _, Operand::Direct(name)) if operand.is_cell() => {
                let from = match op {
                    Op::Sta => 0,
                    Op::Stx => 1,
                    _ => 2,
                };
                // a register holds one fact: an immediate outranks the cell
                // (the load of a constant is the one that is dropped)
                if !matches!(state[from], Some(Value::Imm(_))) {
                    state[from] = Some(Value::Cell(name.as_str()));
                }
            }
            // `or` and `xor` of a zero are the load of the operand (the same
            // flags): `tya; ora m` with `y` = 0 is `lda m`, and the transfer
            // is then a dead write
            (Op::Ora | Op::Eor, _, Operand::Imm(_) | Operand::Direct(_))
                if state[0] == Some(Value::Imm("0")) && (operand.is_cell() || matches!(operand, Operand::Imm(_))) =>
            {
                stores.push((
                    at,
                    Inst { op: Op::Lda, operand: operand.clone(), place: line.inst().map_or(0, |inst| inst.place) },
                    0,
                ));
                state[0] = match operand {
                    Operand::Imm(value) => Some(Value::Imm(value.as_str())),
                    Operand::Direct(name) => Some(Value::Cell(name.as_str())),
                    _ => None,
                };
            }
            // the same through a pointer: `ora (p),y` of a zero is `lda (p),y`
            (Op::Ora | Op::Eor, _, Operand::IndirectY(_) | Operand::Indirect(_))
                if state[0] == Some(Value::Imm("0")) =>
            {
                let place = line.inst().map_or(0, |inst| inst.place);
                stores.push((at, Inst { op: Op::Lda, operand: operand.clone(), place }, 0));
                state[0] = None;
            }
            (Op::Lda | Op::Pla | Op::Adc | Op::Sbc | Op::And | Op::Ora | Op::Eor, ..) => state[0] = None,
            (Op::Asl | Op::Lsr | Op::Rol | Op::Ror | Op::Inc | Op::Dec, _, _) if on_a => state[0] = None,
            (Op::Ldx | Op::Inx | Op::Dex, ..) => state[1] = None,
            (Op::Ldy | Op::Iny | Op::Dey, ..) => state[2] = None,
            (Op::Txa, ..) => state[0] = state[1],
            // `x` holds the constant `y` does: the transfer from `x` leaves the
            // load of `y` dead
            (Op::Tya, ..) => {
                if matches!(state[2], Some(Value::Imm(_))) && state[1] == state[2] {
                    transfers.push((at, Op::Txa));
                }
                state[0] = state[2];
            }
            (Op::Tax, ..) => state[1] = state[0],
            (Op::Tay, ..) => state[2] = state[0],
            // a register's value survives what neither writes it nor leaves
            // the function: stores, compares, branches, memory
            // read-modify-write
            (Op::Cmp | Op::Cpx | Op::Cpy | Op::Bit | Op::Inc | Op::Dec | Op::Asl | Op::Lsr | Op::Rol | Op::Ror, ..) => {
            }
            _ if line.inst().is_some_and(|i| i.flags() == Flags::Keeps) => {}
            _ if op.is_branch() => {}
            _ => state = [None; 3],
        }
        known = (!ends).then_some(state);
    }
    (keep, transfers, stores, carried)
}

/// DEVIATE 6502-KNOWN-CELLS: the pin's MachineCopyPropagation erases a
/// redundant copy inside one block; the facts here cross labels (every edge
/// must agree), which the pin gets from register allocation keeping the value
/// in the register. Reason: the cells are this target's spill homes. Exit:
/// 2026-11-30, with the allocator-level fix. Owner: 6502.
///
/// `ldx #n` (likewise `lda` and `ldy`) of what the register already holds: it
/// goes, unless a branch reads the flags it sets. One that another register
/// holds is the 1-byte transfer (`txa` for `lda #0` when `x` is 0: the same
/// clocks, and the same `N` and `Z`; llvm-mos's
/// `MOSLateOptimization::combineLdImm` does the same, bar the 65816-only
/// `x`/`y` transfers). The values known are the immediates loaded, the cells
/// loaded or stored and the transfers between registers, forgotten at a call or
/// an instruction not listed. A load of a cell the register holds goes; one
/// another register holds stays (a `tax` there hid the copy from the folds
/// after this pass). At a label only branches in this text reach, they are the
/// values every incoming edge agrees on (a loop's back edge is found by walking
/// again until they hold); at any other label they are forgotten.
pub fn drop_known_loads(asm: &mut Asm) {
    let Some((mut keep, transfers, stores)) = decide_known_loads(&asm.lines) else { return };
    for (at, op) in transfers {
        asm.lines[at] = Line::Inst(Inst::new(op, Operand::None));
    }
    for (at, replacement, dropped) in stores {
        asm.lines[at] = Line::Inst(replacement);
        keep[at + 1..at + 1 + dropped].fill(false);
    }
    retain(asm, &keep);
}

/// The loads to keep and to turn into transfers, once the values at the labels
/// hold. A walk that does not settle them loses a value at some label (each
/// label starts absent, then holds three registers, and values are only lost),
/// so `4 * labels + 2` walks reach the fixed point; a cap of eight left a
/// 3800-line function (jpeg_single, nine walks) with every redundant load.
fn decide_known_loads(lines: &[Line]) -> Option<(Vec<bool>, Vec<(usize, Op)>, Vec<(usize, Inst, usize)>)> {
    let (targets, quiet) = branch_targets(lines);
    let mut at_label: HashMap<&str, Known> = HashMap::default();
    for _ in 0..4 * targets.len() + 2 {
        let (keep, transfers, stores, carried) = walk(lines, &targets, &quiet, &at_label);
        if carried == at_label {
            return Some((keep, transfers, stores));
        }
        at_label = carried;
    }
    None
}

/// Whether `line` changes neither a register nor the cell `load` reads: a
/// carry flag op, or a store to another zero-page or spill cell.
fn leaves_cell_and_registers(
    line: &Line,
    load: &Operand,
) -> bool {
    let Line::Inst(Inst { op, operand, .. }) = line else { return false };
    match op {
        Op::Clc | Op::Sec | Op::Nop => true,
        // a store changes no register, and no cell but its own: a global named
        // directly or indexed is not a cell, a zero-page indexed store or one
        // through a pointer may be
        Op::Sta | Op::Stx | Op::Sty | Op::Stz => match operand {
            Operand::Direct(_) => operand != load,
            Operand::Indexed(name, _) => !(name.starts_with("rc") || name.starts_with("s_")),
            _ => false,
        },
        _ => false,
    }
}

/// `sta m` or `lda m` then `lda m` (likewise `x` and `y`) of a zero-page or
/// spill cell: the register still holds it, so the load goes. It also set the
/// zero and sign flags, so it stays when a branch reads them before anything
/// sets them again. A carry flag op or a store to anything but `m` between the
/// two changes neither.
pub fn drop_reloads(asm: &mut Asm) {
    let lines = &asm.lines;
    let keep: Vec<bool> = (0..lines.len())
        .map(|at| {
            let Line::Inst(Inst { op: load, operand, .. }) = &lines[at] else { return true };
            let store = match load {
                Op::Lda => Op::Sta,
                Op::Ldx => Op::Stx,
                Op::Ldy => Op::Sty,
                _ => return true,
            };
            let reload = operand.is_cell()
                && lines[..at]
                    .iter()
                    .rev()
                    .find(|before| !leaves_cell_and_registers(before, operand))
                    .and_then(Line::inst)
                    .is_some_and(|before| (before.op == store || before.op == *load) && before.operand == *operand)
                && flags_unread_after(lines, at + 1);
            !reload
        })
        .collect();
    retain(asm, &keep);
}

/// `ldx m; ...; txa` (likewise `ldy m` and `tya`): `lda m`, where between the
/// two only `a` and the flags are worked on, nothing writes `m` or touches the
/// index register, every branch goes where the register is dead, and it is dead
/// after the transfer. The flags are those of `m` either way. (A word compare
/// that read a pointer's high half through `y` was `ldy rc7; lda rc6; cmp rc2;
/// bne; tya; cmp rc3`.)
pub fn load_straight_into_a(asm: &mut Asm) {
    // one load each round: the transfer of the first may stand between the
    // second and its own
    while load_straight_once(asm) {}
}

fn load_straight_once(asm: &mut Asm) -> bool {
    let lines = &asm.lines;
    let exits = Exits::of(asm);
    let label_at = |name: &str| lines.iter().position(|line| matches!(line, Line::Label(l) if l == name));
    let mut drops: Vec<(usize, usize)> = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let Line::Inst(Inst {
            op: load @ (Op::Ldx | Op::Ldy),
            operand: from @ (Operand::Direct(_) | Operand::Imm(_)),
            ..
        }) = line
        else {
            continue;
        };
        let (transfer, register) = if *load == Op::Ldx { (Op::Txa, Registers::X) } else { (Op::Tya, Registers::Y) };
        let mut to = None;
        // N and Z are still the load's: a branch on them would test the deleted
        // load
        let mut load_flags = true;
        for (next, line) in lines.iter().enumerate().skip(at + 1) {
            let Line::Inst(Inst { op, operand, .. }) = line else { break };
            if *op == transfer {
                to = Some(next);
                break;
            }
            let index = if register == Registers::X { Index::X } else { Index::Y };
            let touches_index = matches!(operand, Operand::Indexed(_, i) if *i == index)
                || matches!(
                    (operand, register),
                    (Operand::IndirectX(_), Registers::X) | (Operand::IndirectY(_), Registers::Y)
                );
            let quiet = !touches_index
                && match op {
                    Op::Clc
                    | Op::Sec
                    | Op::Nop
                    | Op::Lda
                    | Op::Cmp
                    | Op::Adc
                    | Op::Sbc
                    | Op::And
                    | Op::Ora
                    | Op::Eor => true,
                    Op::Sta => matches!(operand, Operand::Direct(_)) && operand != from,
                    Op::Beq | Op::Bne | Op::Bcc | Op::Bcs | Op::Bmi | Op::Bpl => match operand {
                        Operand::Direct(target) => {
                            let reads_load = load_flags && !matches!(op, Op::Bcc | Op::Bcs);
                            !reads_load
                                && label_at(target).is_some_and(|label| {
                                    dead_from(lines, label, register, &mut HashSet::default(), exits)
                                })
                        }
                        _ => false,
                    },
                    // the other index register: a load or store of another byte
                    Op::Ldx | Op::Stx if register == Registers::Y => {
                        matches!(operand, Operand::Direct(_) | Operand::Imm(_)) && (*op == Op::Ldx || operand != from)
                    }
                    Op::Ldy | Op::Sty if register == Registers::X => {
                        matches!(operand, Operand::Direct(_) | Operand::Imm(_)) && (*op == Op::Ldy || operand != from)
                    }
                    _ => false,
                };
            if !quiet {
                break;
            }
            if !matches!(op.flags(operand), Flags::Keeps) {
                load_flags = false;
            }
        }
        if let Some(to) = to
            && dead_from(lines, to + 1, register, &mut HashSet::default(), exits)
        {
            drops.push((at, to));
        }
    }
    let changed = !drops.is_empty();
    let mut keep = vec![true; asm.lines.len()];
    for (at, to) in drops {
        let Line::Inst(Inst { operand, .. }) = &asm.lines[at] else { continue };
        let operand = operand.clone();
        if let Line::Inst(inst) = &mut asm.lines[to] {
            inst.op = Op::Lda;
            inst.operand = operand;
        }
        keep[at] = false;
    }
    retain(asm, &keep);
    changed
}

/// `tax; ...; stx m` (likewise `tay` and `sty`): the byte is in `a` at the
/// transfer, so `sta m` there, when between the two only `a` is worked on, `m`
/// is not touched, nothing reads or writes the index register, and the register
/// is dead after the store. The flags the transfer set are set again before a
/// read. (A pointer plus a constant went `adc #3; tax; lda rc1; adc #0; tay;
/// stx rc6; sty rc7` where `adc #3; sta rc6; lda rc1; adc #0; sta rc7` does.)
pub fn store_before_transfer(asm: &mut Asm) {
    while store_before_once(asm) {}
}

fn store_before_once(asm: &mut Asm) -> bool {
    let lines = &asm.lines;
    for (at, line) in lines.iter().enumerate() {
        let Line::Inst(Inst { op: transfer @ (Op::Tax | Op::Tay), .. }) = line else { continue };
        let (store, register, other_transfer, other_load, other_store) = if *transfer == Op::Tax {
            (Op::Stx, Registers::X, Op::Tay, Op::Ldy, Op::Sty)
        } else {
            (Op::Sty, Registers::Y, Op::Tax, Op::Ldx, Op::Stx)
        };
        let index = if register == Registers::X { Index::X } else { Index::Y };
        let mut found = None;
        let mut target: Option<&Operand> = None;
        for (next, line) in lines.iter().enumerate().skip(at + 1) {
            let Line::Inst(Inst { op, operand, .. }) = line else { break };
            if *op == store {
                if matches!(operand, Operand::Direct(_)) {
                    found = Some(next);
                    target = Some(operand);
                }
                break;
            }
            let touches_index = matches!(operand, Operand::Indexed(_, i) if *i == index)
                || matches!(
                    (operand, register),
                    (Operand::IndirectX(_), Registers::X) | (Operand::IndirectY(_), Registers::Y)
                );
            let quiet = !touches_index
                && match op {
                    Op::Clc | Op::Sec | Op::Nop => true,
                    Op::Lda | Op::Cmp | Op::Adc | Op::Sbc | Op::And | Op::Ora | Op::Eor => {
                        matches!(operand, Operand::Direct(_) | Operand::Imm(_))
                    }
                    Op::Sta => matches!(operand, Operand::Direct(_)),
                    o if *o == other_transfer || *o == other_load => {
                        matches!(operand, Operand::Direct(_) | Operand::Imm(_) | Operand::None)
                    }
                    o if *o == other_store => matches!(operand, Operand::Direct(_)),
                    _ => false,
                };
            if !quiet {
                break;
            }
        }
        let (Some(to), Some(cell)) = (found, target) else { continue };
        // `m` is neither read nor written between the two
        let untouched = lines[at + 1..to].iter().all(|line| line.inst().is_none_or(|i| i.operand != *cell));
        if untouched
            && flags_unread_after(lines, at + 1)
            && dead_from(lines, to + 1, register, &mut HashSet::default(), Exits::default())
        {
            let cell = cell.clone();
            asm.lines[at] = Line::Inst(Inst { op: Op::Sta, operand: cell, place: 0 });
            asm.lines.remove(to);
            return true;
        }
    }
    false
}

/// `tya; tay`, `tay; tya`, `txa; tax` and `tax; txa`: the second transfer
/// moves what the first just moved and sets the flags the first set.
pub fn drop_cancelling_transfers(asm: &mut Asm) {
    let mut out: Vec<Line> = Vec::with_capacity(asm.lines.len());
    let mut cancelled = false;
    for line in asm.lines.drain(..) {
        let cancels = !cancelled
            && match (out.last().and_then(Line::inst), &line) {
                (Some(Inst { op: before, .. }), Line::Inst(Inst { op, .. })) => {
                    matches!(
                        (before, op),
                        (Op::Tya, Op::Tay) | (Op::Tay, Op::Tya) | (Op::Txa, Op::Tax) | (Op::Tax, Op::Txa)
                    )
                }
                _ => false,
            };
        // the pair is gone: the next line is not paired with its first half
        cancelled = cancels;
        if !cancels {
            out.push(line);
        }
    }
    asm.lines = out;
}

/// `pha; X; pla; pha; Y; pla` where `Y` is straight-line code that starts
/// with a load into `a`: `pha; X; Y; pla`. `Y` writes `a` before it reads it
/// and holds the stack as the second `pha` left it, so the `pla; pha` that
/// restored `a` for it was dead, as were the flags it set (`Y`'s load sets
/// them before anything reads). Two literals stored into cells while `a` is
/// live (`known_loads_walks` -O1: two brackets of 12 clocks, run 576 times)
/// are one bracket, and the second `lda #n` is then a known load.
///
/// DEVIATE (6502-A-BRACKET): not in the pin; llvm-mos parks a live A in an
/// imaginary register instead. Reason: the allocator's `pha`/`pla` save left
/// dead `pla; pha` pairs in 9 (O2) and 12 (Os) corpus programs. Exit: the
/// allocator parks A in a cell (2026-11-24).
pub fn merge_saves_of_a(asm: &mut Asm) {
    loop {
        let lines = &asm.lines;
        let found = (0..lines.len().saturating_sub(2)).find(|&at| {
            let op = |n: usize| lines.get(at + n).and_then(Line::inst).map(|inst| inst.op);
            if (op(0), op(1)) != (Some(Op::Pla), Some(Op::Pha)) || op(2) != Some(Op::Lda) {
                return false;
            }
            // the second bracket's body: straight-line up to its `pla`
            let mut n = 2;
            loop {
                let Some(Line::Inst(inst)) = lines.get(at + n) else { return false };
                match inst.op {
                    Op::Pla => return true,
                    Op::Pha
                    | Op::Php
                    | Op::Plp
                    | Op::Phx
                    | Op::Phy
                    | Op::Tsx
                    | Op::Txs
                    | Op::Jsr
                    | Op::Jmp
                    | Op::Rts => return false,
                    op if op.is_branch() => return false,
                    _ => {}
                }
                n += 1;
            }
        });
        let Some(at) = found else { return };
        asm.lines.drain(at..at + 2);
    }
}

/// `pha; lda #n; sta m; pla` (a literal made into a cell while `a` is live)
/// moved to where `a` is dead: just before the instruction that next loads `a`
/// without reading it, or the one that loaded the value now live, with only
/// straight-line code between that touches neither `m` nor the flags a branch
/// reads. The bracket is then `lda #n; sta m`: `idioms` -O2 made 78 of them
/// (12 clocks and 5 bytes each against 5 and 4).
///
/// The pin's post-RA scheduler (`PostRASchedulerList`) does not move a store
/// across the live range of a register; this target saves with `pha`/`pla`,
/// so the move is the pass.
///
/// DEVIATE (6502-A-BRACKET): same exit as `merge_saves_of_a`.
pub fn sink_save_bracket(asm: &mut Asm) {
    // an instruction that sets `a` from elsewhere than `a`, and so ends its
    // live range
    let overwrites = |inst: &Inst| match inst.op {
        Op::Lda => !matches!(inst.operand, Operand::Acc),
        Op::Txa | Op::Tya | Op::Pla => true,
        _ => false,
    };
    // straight-line code that is no memory access but a plain cell's, none of
    // `m`, and sets or keeps what a branch would read (there is no branch)
    let passable = |inst: &Inst, cell: &str| -> bool {
        use Op::*;
        let plain = match &inst.operand {
            Operand::None | Operand::Acc | Operand::Imm(_) => true,
            Operand::Direct(name) => name != cell,
            _ => false,
        };
        plain
            && matches!(
                inst.op,
                Lda | Ldx
                    | Ldy
                    | Sta
                    | Stx
                    | Sty
                    | Adc
                    | Sbc
                    | And
                    | Ora
                    | Eor
                    | Cmp
                    | Cpx
                    | Cpy
                    | Asl
                    | Lsr
                    | Rol
                    | Ror
                    | Inc
                    | Dec
                    | Tax
                    | Tay
                    | Txa
                    | Tya
                    | Inx
                    | Iny
                    | Dex
                    | Dey
                    | Clc
                    | Sec
            )
    };
    let mut at = 0;
    while at + 3 < asm.lines.len() {
        let [Line::Inst(push), Line::Inst(load), Line::Inst(store), Line::Inst(pull)] = &asm.lines[at..at + 4] else {
            at += 1;
            continue;
        };
        let bracket = (push.op, load.op, store.op, pull.op) == (Op::Pha, Op::Lda, Op::Sta, Op::Pla)
            && matches!(load.operand, Operand::Imm(_));
        let Operand::Direct(cell) = &store.operand else {
            at += 1;
            continue;
        };
        if !bracket || !cell.starts_with("rc") {
            at += 1;
            continue;
        }
        let (load, store, cell) = (load.clone(), store.clone(), cell.clone());
        // after the bracket: to the instruction that ends the live range
        let mut target = None;
        for look in at + 4..(at + 4 + 12).min(asm.lines.len()) {
            let Line::Inst(inst) = &asm.lines[look] else { break };
            if overwrites(inst) {
                target = Some((look, true));
                break;
            }
            if !passable(inst, &cell) {
                break;
            }
        }
        // else before it: back to the instruction that made the value live
        if target.is_none() {
            for look in (at.saturating_sub(12)..at).rev() {
                let Line::Inst(inst) = &asm.lines[look] else { break };
                if overwrites(inst) {
                    target = Some((look, false));
                    break;
                }
                if !passable(inst, &cell) {
                    break;
                }
            }
        }
        let Some((look, after)) = target else {
            at += 1;
            continue;
        };
        // `lda #n` changes the zero and sign flags, which the instruction at
        // `look` sets again; it leaves the carry
        let moved = [Line::Inst(load), Line::Inst(store)];
        asm.lines.drain(at..at + 4);
        let into = if after { look - 4 } else { look };
        asm.lines.splice(into..into, moved);
        at = 0;
    }
}

/// `jmp l` where `l:` starts a block of straight-line loads, stores and
/// transfers that ends in `rts`, no longer than the jump: the block itself. A
/// zero-trip path that stored its result and jumped to the return that loads
/// it returns the constant (`dotx`: `lda #0; sta rc7; jmp b3` and `b3: lda rc7;
/// rts` become `lda #0; rts` once the load and the store are dropped). The
/// pin's `TailDuplicator` copies a return block into its predecessors the
/// same way
/// ([TailDuplicator.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L545)).
/// A block nothing reaches afterwards is dropped.
pub fn duplicate_return_tail(asm: &mut Asm) {
    let mut at = 0;
    let mut changed = false;
    while at < asm.lines.len() {
        let Line::Inst(jump) = &asm.lines[at] else {
            at += 1;
            continue;
        };
        let Some(target) = jump.target().filter(|_| jump.op == Op::Jmp) else {
            at += 1;
            continue;
        };
        let Some(label) = asm.lines.iter().position(|line| matches!(line, Line::Label(name) if name == target)) else {
            at += 1;
            continue;
        };
        let mut tail = Vec::new();
        let mut bytes = 0;
        let mut returns = false;
        for line in &asm.lines[label + 1..] {
            let Line::Inst(inst) = line else { break };
            let straight = matches!(
                inst.op,
                Op::Lda
                    | Op::Ldx
                    | Op::Ldy
                    | Op::Sta
                    | Op::Stx
                    | Op::Sty
                    | Op::Tax
                    | Op::Tay
                    | Op::Txa
                    | Op::Tya
                    | Op::Clc
                    | Op::Sec
            );
            if inst.op == Op::Rts {
                returns = true;
                tail.push(line.clone());
                break;
            }
            bytes += inst.bytes();
            if !straight || bytes > jump.bytes() {
                break;
            }
            tail.push(line.clone());
        }
        if returns {
            asm.lines.splice(at..=at, tail.iter().cloned());
            at += tail.len();
            changed = true;
        } else {
            at += 1;
        }
    }
    if changed {
        drop_unreached_blocks(asm);
    }
}

/// The lines after an unconditional `jmp`, `bra` or `rts` up to the next label
/// that something names: nothing runs them.
fn drop_unreached_blocks(asm: &mut Asm) {
    let mut at = 1;
    while at < asm.lines.len() {
        let after_exit = matches!(
            &asm.lines[at - 1],
            Line::Inst(inst) if matches!(inst.op, Op::Jmp | Op::Bra | Op::Rts)
        );
        let unnamed = matches!(
            &asm.lines[at],
            Line::Label(name) if references(&asm.lines, name) == 0
        );
        if after_exit && unnamed {
            let mut end = at + 1;
            while matches!(asm.lines.get(end), Some(Line::Inst(_))) {
                end += 1;
            }
            asm.lines.drain(at..end);
        } else {
            at += 1;
        }
    }
}

/// `jmp l` where `l:` is directly followed by `rts` (past other labels): the
/// `rts` itself, one byte and one jump shorter.
pub fn jump_to_return(asm: &mut Asm) {
    let mut returns: HashSet<String> = HashSet::default();
    let mut returning = false;
    for line in asm.lines.iter().rev() {
        match line {
            Line::Inst(inst) => returning = inst.op == Op::Rts,
            Line::Label(label) => {
                if returning {
                    returns.insert(label.clone());
                }
            }
            Line::Raw(_) => returning = false,
        }
    }
    for line in &mut asm.lines {
        if matches!(
            line,
            Line::Inst(inst) if inst.op == Op::Jmp && inst.target().is_some_and(|l| returns.contains(l))
        ) {
            *line = Line::Inst(Inst::new(Op::Rts, Operand::None));
        }
    }
}

/// Whether the instruction writes the zero-page or spill cell `cell`.
fn writes_cell(
    inst: &Inst,
    cell: &Operand,
) -> bool {
    matches!(
        inst.op,
        Op::Sta | Op::Stx | Op::Sty | Op::Inc | Op::Dec | Op::Asl | Op::Lsr | Op::Rol | Op::Ror
    )
        && inst.operand == *cell
}

/// What the function lets `dead_from` assume about the instructions that leave
/// it or call out. The default assumes nothing: a return reads the register and
/// so does a call.
#[derive(Clone, Copy, Default)]
pub(crate) struct Exits<'a> {
    /// A return does not read `a`: the function leaves nothing there.
    no_a_result: bool,
    /// The routines called with an argument in `a`; a call to any other one
    /// writes `a` without reading it. `None`: every call reads it.
    a_callees: Option<&'a std::collections::BTreeSet<String>>,
    /// A return does not read `x`: the function leaves nothing there.
    no_x_result: bool,
    /// The routines called with an argument in `x`; a call to any other that is
    /// not a runtime routine (`__`) writes `x` without reading it. `None`:
    /// every call reads it.
    x_callees: Option<&'a std::collections::BTreeSet<String>>,
    /// The same for `y`.
    no_y_result: bool,
    y_callees: Option<&'a std::collections::BTreeSet<String>>,
}

impl<'a> Exits<'a> {
    pub(crate) fn of(asm: &'a Asm) -> Exits<'a> {
        Exits {
            no_a_result: asm.no_a_result,
            a_callees: Some(&asm.a_callees),
            no_x_result: asm.no_x_result,
            x_callees: Some(&asm.x_callees),
            no_y_result: asm.no_y_result,
            y_callees: Some(&asm.y_callees),
        }
    }
}

/// Whether register `bit` (A, X or Y) is written before it is read, along every
/// path from `lines[from]`. An instruction not listed counts as a read; a call
/// and a return do as `exits` says.
pub(crate) fn dead_from(
    lines: &[Line],
    from: usize,
    bit: Registers,
    seen: &mut HashSet<usize>,
    exits: Exits,
) -> bool {
    let mut at = from;
    while let Some(line) = lines.get(at) {
        if !seen.insert(at) {
            return true;
        }
        at += 1;
        let Line::Inst(inst) = line else {
            if matches!(line, Line::Raw(_)) {
                return false;
            }
            continue;
        };
        let indexed = |index| match (&inst.operand, index) {
            (Operand::Indexed(_, i), _) => *i == index,
            (Operand::IndirectX(_), Index::X) | (Operand::IndirectY(_), Index::Y) => true,
            _ => false,
        };
        let (reads, kills) = match (inst.op, bit) {
            (Op::Lda | Op::Pla, Registers::A) => (false, true),
            (Op::Ldx | Op::Plx, Registers::X) => (indexed(Index::X), true),
            (Op::Ldy | Op::Ply, Registers::Y) => (indexed(Index::Y), true),
            (Op::Tax | Op::Tay | Op::Txa | Op::Tya, _) => {
                let (from, to) = match inst.op {
                    Op::Tax => (Registers::A, Registers::X),
                    Op::Tay => (Registers::A, Registers::Y),
                    Op::Txa => (Registers::X, Registers::A),
                    _ => (Registers::Y, Registers::A),
                };
                (bit == from, bit == to)
            }
            (Op::Sta | Op::Stx | Op::Sty | Op::Clc | Op::Sec | Op::Nop, _) => {
                let stored = match inst.op {
                    Op::Sta => Some(Registers::A),
                    Op::Stx => Some(Registers::X),
                    Op::Sty => Some(Registers::Y),
                    _ => None,
                };
                (stored == Some(bit) || indexed(if bit == Registers::X { Index::X } else { Index::Y }), false)
            }
            (Op::Cpx | Op::Inx | Op::Dex, Registers::X) | (Op::Cpy | Op::Iny | Op::Dey, Registers::Y) => (true, false),
            // the other registers are left alone by an index register's own
            // steps and compares
            (Op::Cpx | Op::Inx | Op::Dex, Registers::Y | Registers::A)
            | (Op::Cpy | Op::Iny | Op::Dey, Registers::X | Registers::A) => (false, false),
            (Op::Adc | Op::And | Op::Ora | Op::Eor | Op::Sbc | Op::Cmp | Op::Bit | Op::Pha, Registers::A) => {
                (true, false)
            }
            (Op::Asl | Op::Lsr | Op::Rol | Op::Ror | Op::Inc | Op::Dec, Registers::A) => {
                (matches!(inst.operand, Operand::Acc | Operand::None), false)
            }
            (
                Op::Adc
                | Op::And
                | Op::Ora
                | Op::Eor
                | Op::Sbc
                | Op::Cmp
                | Op::Bit
                | Op::Asl
                | Op::Lsr
                | Op::Rol
                | Op::Ror
                | Op::Inc
                | Op::Dec
                | Op::Pha
                | Op::Php
                | Op::Cld
                | Op::Sed
                | Op::Clv,
                _,
            ) => (indexed(if bit == Registers::X { Index::X } else { Index::Y }), false),
            (Op::Lda | Op::Ldx | Op::Ldy, _) => (indexed(if bit == Registers::X { Index::X } else { Index::Y }), false),
            (Op::Rts, _) => {
                return bit == Registers::A && exits.no_a_result
                    || bit == Registers::X && exits.no_x_result
                    || bit == Registers::Y && exits.no_y_result;
            }
            (Op::Jsr, _) if bit == Registers::X || bit == Registers::Y => {
                // the callee writes `x` (a result) or `y` (scratch) unless it
                // was an argument; a runtime routine may take
                // one that no instruction lists
                let callee = match &inst.operand {
                    Operand::Direct(name) if !name.starts_with("__") => name,
                    _ => return false,
                };
                let callees = if bit == Registers::X { exits.x_callees } else { exits.y_callees };
                match callees {
                    Some(set) if !set.contains(callee) => return true,
                    _ => return false,
                }
            }
            (Op::Jsr, _) if bit == Registers::A => {
                // the callee clobbers `a`: it is dead unless it was an argument
                let callee = match &inst.operand {
                    Operand::Direct(name) => name,
                    _ => return false,
                };
                match exits.a_callees {
                    Some(set) if !set.contains(callee) => return true,
                    _ => return false,
                }
            }
            (Op::Jmp, _) => {
                return inst
                    .target()
                    .is_some_and(
                        |label| lines
                            .iter()
                            .position(|l| *l == Line::Label(label.to_owned()))
                            .is_some_and(|to| dead_from(lines, to, bit, seen, exits)),
                    );
            }
            (op, _) if op.opposite().is_some() || op == Op::Bra => {
                let target =
                    inst.target().and_then(|label| lines.iter().position(|l| *l == Line::Label(label.to_owned())));
                let taken = target.is_some_and(|to| dead_from(lines, to, bit, seen, exits));
                if op == Op::Bra {
                    return taken;
                }
                if !taken {
                    return false;
                }
                continue;
            }
            _ => return false,
        };
        if reads {
            return false;
        }
        if kills {
            return true;
        }
    }
    false
}

/// What a test reads: a cell, or a register.
#[derive(PartialEq)]
enum Source<'a> {
    Cell(&'a Operand),
    Register(Registers),
}

/// A shift that puts one bit of its operand in the carry, and the branch that
/// asks for that bit (from the instruction tables: `asl` and `rol` move bit 7
/// out, which `bpl`/`bmi` test as the sign; `lsr` and `ror` move bit 0 out,
/// which `and #1` and `beq`/`bne` test).
struct CarryOut {
    shifts: [Op; 2],
    /// `and #1` stands between the load and the branch.
    masked: bool,
    /// The branches for "the bit was clear" and "the bit was set".
    branches: [Op; 2],
}

const LEFT: CarryOut = CarryOut { shifts: [Op::Asl, Op::Rol], masked: false, branches: [Op::Bpl, Op::Bmi] };
const RIGHT: CarryOut = CarryOut { shifts: [Op::Lsr, Op::Ror], masked: true, branches: [Op::Beq, Op::Bne] };

/// Whether the instruction writes register `bit`.
fn writes_register(
    inst: &Inst,
    bit: Registers,
) -> bool {
    let acc = matches!(inst.operand, Operand::Acc | Operand::None);
    match inst.op {
        Op::Lda | Op::Pla | Op::Txa | Op::Tya | Op::Adc | Op::Sbc | Op::And | Op::Ora | Op::Eor => bit == Registers::A,
        Op::Asl | Op::Lsr | Op::Rol | Op::Ror | Op::Inc | Op::Dec => acc && bit == Registers::A,
        Op::Ldx | Op::Plx | Op::Tax | Op::Inx | Op::Dex | Op::Tsx => bit == Registers::X,
        Op::Ldy | Op::Ply | Op::Tay | Op::Iny | Op::Dey => bit == Registers::Y,
        _ => false,
    }
}

/// `asl|rol c` (`lsr|ror c`) and then, with only loads, transfers and logic
/// between, a branch on the top (bottom) bit of what `c` held before it: `c`
/// was a copy of a cell (`ldx m; stx c`) or of a register (`stx c`) that has
/// not changed since. The shift put that bit in the carry, so `bcc` replaces
/// `bpl` or `beq` (`bcs` for `bmi`, `bne`) and the test (`ldx m`, `txa`,
/// `and #1`, ...) goes, when the register it set is written again before
/// anything reads it. [`LEFT`] and [`RIGHT`] say which shifts and tests pair.
///
/// DEVIATE (6502-8): neither LLVM's pin nor llvm-mos folds a bit test of a
/// value into the carry of its own shift (the pin has ARM Thumb1's `LSLS`
/// compare and x86's carry of `add x,-1`, both narrower). Exit: a
/// target-independent shift-with-carry-out in the MIR, so the select that
/// tests a bit before `c << 1` or `c >> 1` branches on the shift's carry.
pub fn branch_on_shift_carry(asm: &mut Asm) {
    let mut at = 2;
    while at < asm.lines.len() {
        let rewrite = [&LEFT, &RIGHT].into_iter().find_map(|shape| carry_branch(asm, at, shape));
        if let Some((carry, from)) = rewrite {
            let Line::Inst(branch) = &mut asm.lines[at] else { unreachable!() };
            branch.op = carry;
            asm.lines.drain(from..at);
            at = from + 1;
        } else {
            at += 1;
        }
    }
}

/// The branch to use and the first line of the test to drop, if the branch
/// at `lines[at]` tests the bit `shape`'s shift moved into the carry.
fn carry_branch(
    asm: &Asm,
    at: usize,
    shape: &CarryOut,
) -> Option<(Op, usize)> {
    let Line::Inst(branch) = &asm.lines[at] else { return None };
    let carry = match shape.branches.iter().position(|op| *op == branch.op)? {
        0 => Op::Bcc,
        _ => Op::Bcs,
    };
    // what the test reads, the register it sets, and its first line
    let (source, tested, from) = if shape.masked {
        let Line::Inst(Inst { op: Op::And, operand: Operand::Imm(one), .. }) = &asm.lines[at - 1] else { return None };
        if one != "1" {
            return None;
        }
        match asm.lines.get(at - 2) {
            Some(Line::Inst(Inst { op: Op::Lda, operand, .. })) if operand.is_cell() => {
                (Source::Cell(operand), Registers::A, at - 2)
            }
            Some(Line::Inst(Inst { op: Op::Txa, .. })) => (Source::Register(Registers::X), Registers::A, at - 2),
            Some(Line::Inst(Inst { op: Op::Tya, .. })) => (Source::Register(Registers::Y), Registers::A, at - 2),
            // `and #1` alone tests the accumulator as it is
            _ => (Source::Register(Registers::A), Registers::A, at - 1),
        }
    } else {
        let Line::Inst(Inst { op: read, operand, .. }) = &asm.lines[at - 1] else { return None };
        match read {
            Op::Lda if operand.is_cell() => (Source::Cell(operand), Registers::A, at - 1),
            Op::Ldx if operand.is_cell() => (Source::Cell(operand), Registers::X, at - 1),
            Op::Ldy if operand.is_cell() => (Source::Cell(operand), Registers::Y, at - 1),
            Op::Tax => (Source::Register(Registers::A), Registers::X, at - 1),
            Op::Tay => (Source::Register(Registers::A), Registers::Y, at - 1),
            Op::Txa => (Source::Register(Registers::X), Registers::A, at - 1),
            Op::Tya => (Source::Register(Registers::Y), Registers::A, at - 1),
            _ => return None,
        }
    };
    let unchanged = |inst: &Inst| match &source {
        Source::Cell(cell) => !writes_cell(inst, cell),
        Source::Register(bit) => !writes_register(inst, *bit),
    };
    // the shift, past instructions that leave the carry and the source alone
    let mut shift = from;
    let copy = loop {
        shift = shift.checked_sub(1)?;
        let Line::Inst(before) = &asm.lines[shift] else { return None };
        match before.op {
            op if shape.shifts.contains(&op) && before.operand.is_cell() => break &before.operand,
            Op::Lda
            | Op::Ldx
            | Op::Ldy
            | Op::Tax
            | Op::Tay
            | Op::Txa
            | Op::Tya
            | Op::Eor
            | Op::And
            | Op::Ora
            | Op::Nop
            | Op::Inx
            | Op::Iny
            | Op::Dex
            | Op::Dey
            | Op::Sta
            | Op::Stx
            | Op::Sty
                if unchanged(before) => {}
            _ => return None,
        }
    };
    if matches!(&source, Source::Cell(cell) if *cell == copy) {
        return None;
    }
    // `copy` held the source before the shift
    let mut back = shift;
    let copied = loop {
        back -= 1;
        let Line::Inst(before) = &asm.lines[back] else { return None };
        if writes_cell(before, copy) {
            let (stored, load) = match before.op {
                Op::Sta => (Registers::A, Op::Lda),
                Op::Stx => (Registers::X, Op::Ldx),
                Op::Sty => (Registers::Y, Op::Ldy),
                _ => return None,
            };
            break match &source {
                Source::Register(bit) => *bit == stored,
                Source::Cell(cell) => {
                    matches!(
                        asm.lines.get(back.checked_sub(1)?)?,
                        Line::Inst(source) if source.op == load && source.operand == **cell
                    )
                }
            };
        }
        if before.op == Op::Jsr || before.op.is_branch() || !unchanged(before) {
            return None;
        }
    };
    (copied && dead_from(&asm.lines, at + 1, tested, &mut HashSet::default(), Exits::default()))
        .then_some((carry, from))
}

/// The memory byte a step of one was added to and stored back, and the step
/// (`true`: up), when `lines[at..at + 4]` (or three, for `inc a`) is one:
/// the length of the match too.
fn step_of(
    lines: &[Line],
    at: usize,
) -> Option<(Operand, bool, usize)> {
    let inst = |n: usize| lines.get(at + n).and_then(Line::inst);
    let memory = |operand: &Operand| {
        matches!(operand, Operand::Direct(_) | Operand::Indexed(_, Index::X)).then(|| operand.clone())
    };
    let first = inst(0)?;
    let imm = |inst: &Inst, value: &str| inst.operand == Operand::Imm(value.to_owned());
    // `lda #1; clc; adc m; sta m`
    if first.op == Op::Lda && (imm(first, "1") || imm(first, "255")) {
        let (carry, add, store) = (inst(1)?, inst(2)?, inst(3)?);
        let up = imm(first, "1");
        let m = memory(&add.operand)?;
        if carry.op == Op::Clc && add.op == Op::Adc && store.op == Op::Sta && store.operand == m {
            return Some((m, up, 4));
        }
        return None;
    }
    let m = memory(&first.operand).filter(|_| first.op == Op::Lda)?;
    let (step, store) = (inst(1)?, inst(2)?);
    // `lda m; inc a; sta m`
    if matches!(step.op, Op::Inc | Op::Dec) && matches!(step.operand, Operand::Acc | Operand::None) {
        if store.op == Op::Sta && store.operand == m {
            return Some((m, step.op == Op::Inc, 3));
        }
        return None;
    }
    // `lda m; clc; adc #1; sta m` and `lda m; sec; sbc #1; sta m`
    let (add, store) = (inst(2)?, inst(3)?);
    if store.op != Op::Sta || store.operand != m || !imm(add, "1") {
        return None;
    }
    match (step.op, add.op) {
        (Op::Clc, Op::Adc) => Some((m, true, 4)),
        (Op::Sec, Op::Sbc) => Some((m, false, 4)),
        _ => None,
    }
}

/// Whether the carry is written before it is read, along every path from
/// `lines[from]`. A call or a return counts as a write: no routine takes the
/// carry in, and none leaves it for a caller to read.
fn carry_dead_from(
    lines: &[Line],
    from: usize,
    seen: &mut HashSet<usize>,
) -> bool {
    let mut at = from;
    while let Some(line) = lines.get(at) {
        if !seen.insert(at) {
            return true;
        }
        at += 1;
        let inst = match line {
            Line::Inst(inst) => inst,
            Line::Label(_) => continue,
            Line::Raw(_) => return false,
        };
        match inst.op {
            Op::Clc | Op::Sec | Op::Cmp | Op::Cpx | Op::Cpy | Op::Jsr | Op::Rts => return true,
            Op::Asl | Op::Lsr if matches!(inst.operand, Operand::Acc | Operand::None) => return true,
            Op::Lda
            | Op::Ldx
            | Op::Ldy
            | Op::Sta
            | Op::Stx
            | Op::Sty
            | Op::Stz
            | Op::Tax
            | Op::Tay
            | Op::Txa
            | Op::Tya
            | Op::Tsx
            | Op::Txs
            | Op::And
            | Op::Ora
            | Op::Eor
            | Op::Inc
            | Op::Dec
            | Op::Inx
            | Op::Iny
            | Op::Dex
            | Op::Dey
            | Op::Bit
            | Op::Pha
            | Op::Pla
            | Op::Phx
            | Op::Phy
            | Op::Plx
            | Op::Ply
            | Op::Nop => {}
            Op::Jmp => {
                return inst
                    .target()
                    .is_some_and(
                        |label| lines
                            .iter()
                            .position(|l| *l == Line::Label(label.to_owned()))
                            .is_some_and(|to| carry_dead_from(lines, to, seen)),
                    );
            }
            Op::Beq | Op::Bne | Op::Bmi | Op::Bpl | Op::Bra => {
                let taken = inst
                    .target()
                    .and_then(|label| lines.iter().position(|l| *l == Line::Label(label.to_owned())))
                    .is_some_and(|to| carry_dead_from(lines, to, seen));
                if inst.op == Op::Bra || !taken {
                    return taken;
                }
            }
            _ => return false,
        }
    }
    false
}

/// 65C02: `lda #0; sta m` (and further `[lda #0;] sta n` after it) with `a`
/// dead after the last store: `stz m`, `stz n`. The zero reaches the cell
/// without `a`: 2 bytes and 2 clocks fewer, and `a` is not clobbered.
pub fn zero_cells_with_stz(asm: &mut Asm) {
    if !asm.cmos {
        return;
    }
    let runs = {
        let exits = Exits::of(asm);
        let lines = &asm.lines;
        let zero = |n: usize| {
            lines.get(n).and_then(Line::inst).is_some_and(|i| i.op == Op::Lda && i.operand == Operand::Imm("0".into()))
        };
        let cell = |n: usize| {
            lines
                .get(n)
                .and_then(Line::inst)
                .is_some_and(|i| i.op == Op::Sta && matches!(i.operand, Operand::Direct(_)))
        };
        let mut runs = Vec::new();
        let mut at = 0;
        while at + 1 < lines.len() {
            if !(zero(at) && cell(at + 1)) {
                at += 1;
                continue;
            }
            // the run: stores, each after a `lda #0` or straight after the last
            // store
            let mut end = at + 2;
            loop {
                if cell(end) {
                    end += 1;
                } else if zero(end) && cell(end + 1) {
                    end += 2;
                } else {
                    break;
                }
            }
            if flags_unread_after(lines, end) && dead_from(lines, end, Registers::A, &mut HashSet::default(), exits) {
                runs.push((at, end));
            }
            at = end;
        }
        runs
    };
    for (at, end) in runs.into_iter().rev() {
        let stores: Vec<Line> = asm.lines[at..end]
            .iter()
            .filter_map(Line::inst)
            .filter(|i| i.op == Op::Sta)
            .map(|i| Line::Inst(Inst { op: Op::Stz, operand: i.operand.clone(), place: i.place }))
            .collect();
        asm.lines.splice(at..end, stores);
    }
}

/// A step of one of a byte in memory as `inc m` or `dec m`.
pub fn step_in_place(asm: &mut Asm) {
    let mut at = 0;
    while at < asm.lines.len() {
        let Some((cell, up, length)) = step_of(&asm.lines, at) else {
            at += 1;
            continue;
        };
        let after = at + length;
        if dead_from(&asm.lines, after, Registers::A, &mut HashSet::default(), Exits::of(asm))
            && carry_dead_from(&asm.lines, after, &mut HashSet::default())
        {
            let place = asm.lines[at].inst().map_or(0, |i| i.place);
            let op = if up { Op::Inc } else { Op::Dec };
            asm.lines.splice(at..after, [Line::Inst(Inst { op, operand: cell, place })]);
        }
        at += 1;
    }
}

/// `lda m; asl|lsr|rol|ror a; sta m` where `a` is dead after the store: the
/// shift done on the byte in memory (`lsr m`). The carry and the sign and zero
/// flags are what the shift of `a` left, and what the store kept.
pub fn shift_in_place(asm: &mut Asm) {
    let mut at = 0;
    while at + 2 < asm.lines.len() {
        let inst = |n: usize| asm.lines.get(at + n).and_then(Line::inst);
        let found = match (inst(0), inst(1), inst(2)) {
            (Some(load), Some(shift), Some(store))
                if load.op == Op::Lda
                    && matches!(
                        load.operand,
                        Operand::Direct(_) | Operand::Indexed(_, Index::X)
                    )
                    && matches!(shift.op, Op::Asl | Op::Lsr | Op::Rol | Op::Ror)
                    && matches!(shift.operand, Operand::Acc | Operand::None)
                    && store.op == Op::Sta
                    && store.operand == load.operand =>
            {
                Some((shift.op, load.operand.clone(), load.place))
            }
            _ => None,
        };
        if let Some((op, operand, place)) = found
            && dead_from(&asm.lines, at + 3, Registers::A, &mut HashSet::default(), Exits::of(asm))
        {
            asm.lines.splice(at..at + 3, [Line::Inst(Inst { op, operand, place })]);
        }
        at += 1;
    }
}
