//! A function from IR to sdasz80 text: the shared DAG stages, then the Z80
//! machine function, allocation, expansion and printing.

use llrm_lir::registers::{RegId, Regs};
use llrm_spike_mf::cfg::Cfg;
use llrm_spike_mf::regalloc::{Assignment, Target, eliminate_phis_with, two_address};
use llrm_spike_mf::tolir::allocate_greedy;

use crate::driver::{Request, selections};
use crate::expand::{self, Line, finish};
use crate::mfunc::{Block, Inst, Kind, MFunction, Operand, Phi, Reg, emit};
use crate::regalloc::Z80Registers;
use crate::{REGISTER_INFO, reg};

/// The machine function after selection and emission, before allocation.
pub fn machine_function(
    ir: &str,
    name: &str,
    request: &Request,
) -> Result<MFunction, String> {
    machine_function_with(ir, name, request, true)
}

/// [`machine_function`], with or without the return blocks copied into their
/// predecessors.
fn machine_function_with(
    ir: &str,
    name: &str,
    request: &Request,
    duplicate: bool,
) -> Result<MFunction, String> {
    let selected = selections(ir, request)?;
    let (_, selection) =
        selected.into_iter().find(|(one, _)| one == name).ok_or_else(|| format!("no function @{name}"))?;
    let mut function = emit(name, &selection?)?;
    expand_cmov(&mut function)?;
    if duplicate {
        tail_duplicate_returns(&mut function);
    }
    eliminate_tests(&mut function);
    eliminate_dead(&mut function);
    Ok(function)
}

/// Drops the instructions whose results nothing reads (`ZEXT8` of a byte whose
/// widening an `EXTRACT_SUBREG` bypassed). Only an instruction that defines
/// virtual registers alone, touches no memory and has no other effect goes.
fn eliminate_dead(function: &mut MFunction) {
    loop {
        let mut read = std::collections::HashSet::new();
        for inst in function.blocks.iter().flat_map(|block| &block.insts) {
            for operand in &inst.operands {
                if let Operand::Use(Reg::Virt(v)) = operand {
                    read.insert(*v);
                }
            }
        }
        for block in &function.blocks {
            for phi in &block.phis {
                for (_, reg) in &phi.incoming {
                    if let Reg::Virt(v) = reg {
                        read.insert(*v);
                    }
                }
            }
        }
        let mut removed = false;
        for block in &mut function.blocks {
            let before = block.insts.len();
            block.insts.retain(|inst| {
                let defs: Vec<&Operand> = inst.operands.iter().filter(|o| matches!(o, Operand::Def(_))).collect();
                let dead = !defs.is_empty()
                    && defs.iter().all(|o| matches!(o, Operand::Def(Reg::Virt(v)) if !read.contains(v)))
                    && !(inst.call
                        || inst.terminator
                        || inst.barrier
                        || inst.side_effects
                        || inst.may_store
                        || inst.may_load);
                !dead
            });
            removed |= block.insts.len() != before;
        }
        if !removed {
            return;
        }
    }
}

/// The function after phi elimination, two-address rewriting and allocation,
/// with the register each virtual register got and the slots spilled to, each
/// with its size in bytes.
pub fn allocated(
    ir: &str,
    name: &str,
    request: &Request,
) -> Result<(MFunction, Assignment, Vec<(i32, usize)>), String> {
    allocated_with(ir, name, request, true).map(|(function, assignment, slots, _)| (function, assignment, slots))
}

type Allocated = (MFunction, Assignment, Vec<(i32, usize)>, std::collections::HashMap<u32, usize>);

fn allocated_with(
    ir: &str,
    name: &str,
    request: &Request,
    duplicate: bool,
) -> Result<Allocated, String> {
    let mut function = machine_function_with(ir, name, request, duplicate)?;
    eliminate_phis_with(&mut function, false);
    let mut hoisted = function.clone();
    if !hoist_constants(&mut hoisted) {
        return allocate(function, name);
    }
    // the allocator does not see that a byte of a pair is read in place, so a
    // constant kept across a loop can cost more than making it there: the
    // hoisted loop stays only if it is no bigger and no slower
    let (plain, with) = (allocate(function, name), allocate(hoisted, name));
    let price = |one: &Allocated| {
        let (function, assignment, slots, reads) = one;
        finish(function, assignment, 0, slots, reads.clone()).ok().map(|lines| expand::price(&lines))
    };
    match (plain, with) {
        (Ok(plain), Ok(with)) => match (price(&plain), price(&with)) {
            (Some((pb, pt)), Some((wb, wt))) if wb > pb || wt > pt => Ok(plain),
            _ => Ok(with),
        },
        (_, Ok(with)) => Ok(with),
        (plain, Err(_)) => plain,
    }
}

/// Allocates `function`; where the first allocation fails, once more with the
/// copies from the argument registers left unjoined. A joined copy pins the
/// argument's pair for the whole of the value's range: a `long long` held in
/// four argument registers has no room left to split (`greedy could not
/// allocate`). The pin does not join them either (`canJoinPhys`).
fn allocate(
    function: MFunction,
    name: &str,
) -> Result<Allocated, String> {
    match allocate_once(function.clone(), name) {
        Ok(done) => Ok(done),
        Err(first) => {
            crate::regalloc::UNJOINED.with(|one| one.set(true));
            let again = allocate_once(function, name);
            crate::regalloc::UNJOINED.with(|one| one.set(false));
            again.map_err(|_| first)
        }
    }
}

fn allocate_once(
    mut function: MFunction,
    name: &str,
) -> Result<Allocated, String> {
    two_address(&mut function);
    read_bytes_through_their_pairs(&mut function);
    use_the_shift_carry(&mut function);
    if std::env::var("LLRM_Z80_DUMP").is_ok_and(|one| one == name) {
        dump(&function);
    }
    let (assignment, slots) = allocate_greedy::<Z80Registers>(&mut function, Regs(&REGISTER_INFO), false)?;
    let reads = expand::reads(&function);
    if std::env::var("LLRM_Z80_DUMP_RA").is_ok_and(|one| one == name) {
        dump(&function);
    }
    remove_identity_copies(&mut function, &assignment);
    tidy_copies(&mut function, &assignment);
    use_lanes_in_place(&mut function, &assignment);
    llrm_spike_mf::branch_fold_hoisting::<Z80Registers>(&mut function, &assignment);
    place_copies_where_needed(&mut function, &assignment);
    read_copies_at_their_source(&mut function, &assignment);
    return_conditionally(&mut function);
    let size = |class| if Z80Registers::bytes_of(class) == 1 { 1 } else { 2 };
    let mut slots: Vec<(i32, usize)> = slots.into_iter().map(|(slot, class)| (slot, size(class))).collect();
    if slots_are_a_stack(&mut function, &assignment) {
        slots.clear();
    } else {
        spills_first(&mut function, &assignment);
    }
    Ok((function, assignment, slots, reads))
}

/// The sign of a word tested next to its shift left by one (`c & 0x8000 ? .. :
/// ..` beside `c << 1`) is the carry the shift left behind: `jr c` after `sla
/// e; rl d` (or `add hl, hl`). The test copied the high byte before the shift
/// and read its bit after (`ld h, d` ... `bit 7, h`), 5 B and 12 T on each of
/// crc16's 8 trips.
///
/// Only a test whose value is the one the shift read (the copy the shift is
/// tied to, unchanged since), read by branches in the block, with no flags set
/// between the shift and the test.
pub fn use_the_shift_carry(function: &mut MFunction) {
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            let found = (|| {
                let shift = &block.insts[at];
                let (
                    Kind::Op("SHL16"),
                    [Operand::Def(Reg::Virt(shifted)), Operand::Use(Reg::Virt(tied)), Operand::Imm(1)],
                ) = (shift.kind, &shift.operands[..])
                else {
                    return None;
                };
                if shifted != tied {
                    return None;
                }
                // the copy the shift's operand was made by, and the value it
                // copies
                let copy = block
                    .insts[..at]
                    .iter()
                    .rposition(
                        |inst| matches!(
                            &inst.operands[..],
                            [Operand::Def(Reg::Virt(d)), ..] if d == shifted
                        ),
                    )?;
                let [Operand::Def(_), Operand::Use(Reg::Virt(old))] = block.insts[copy].operands[..] else {
                    return None;
                };
                if block.insts[copy].kind != Kind::Copy {
                    return None;
                }
                let writes = |inst: &Inst, v: u32| {
                    inst.operands.iter().any(|o| matches!(o, Operand::Def(Reg::Virt(w)) if *w == v))
                };
                if block.insts[copy + 1..at].iter().any(|inst| writes(inst, old) || inst.call) {
                    return None;
                }
                // the test of the old value, after the shift with no flag set
                // between
                let mut test = None;
                for (k, inst) in block.insts.iter().enumerate().skip(at + 1) {
                    if inst.kind == Kind::Op("BIT7_16")
                        && matches!(
                            &inst.operands[..],
                            [_, Operand::Use(Reg::Virt(v))] if *v == old
                        )
                    {
                        test = Some(k);
                        break;
                    }
                    if inst.sets_flags || inst.call || writes(inst, old) {
                        return None;
                    }
                }
                let test = test?;
                // every reader is a branch of this block
                let readers: Vec<usize> =
                    (test + 1..block.insts.len()).filter(|&k| block.insts[k].reads_flags).collect();
                let flags_set_later = block.insts[test + 1..].iter().any(|inst| inst.sets_flags);
                let ends_here = block.insts.last().is_some_and(|inst| inst.terminator);
                let branches = readers.iter().all(|&k| matches!(block.insts[k].kind, Kind::Op("JPZ" | "JPNZ")));
                (!readers.is_empty() && branches && !flags_set_later && ends_here).then_some((test, readers))
            })();
            if let Some((test, readers)) = found {
                for k in readers {
                    block.insts[k].kind = match block.insts[k].kind {
                        Kind::Op("JPNZ") => Kind::Op("JPC"),
                        _ => Kind::Op("JPNC"),
                    };
                }
                block.insts.remove(test);
            }
            at += 1;
        }
    }
}

/// A byte loaded through `hl` that is wanted in `a` is loaded through
/// the pointer's own pair (`ld a, (bc)`), so the copy into `hl` is gone and
/// `hl` stays free for a pointer that has no other form (`cp (hl)`): scmp
/// copied both its pointers into `hl` on every trip. The use in `a` is
/// followed through copies, around loops.
fn read_bytes_through_their_pairs(function: &mut MFunction) {
    let copy_of = |inst: &Inst| match (inst.kind, &inst.operands[..]) {
        (Kind::Copy, [Operand::Def(to), Operand::Use(from)]) => Some((*to, *from)),
        _ => None,
    };
    // the values wanted in `a`: some use is a copy into `a` or into such a
    // value
    let mut in_a = std::collections::HashSet::<u32>::new();
    loop {
        let before = in_a.len();
        for inst in function.blocks.iter().flat_map(|block| &block.insts) {
            if let Some((to, Reg::Virt(from))) = copy_of(inst) {
                if to == Reg::Phys(reg::A) || matches!(to, Reg::Virt(w) if in_a.contains(&w)) {
                    in_a.insert(from);
                }
            }
        }
        if in_a.len() == before {
            break;
        }
    }
    for block in &mut function.blocks {
        for at in 0..block.insts.len().saturating_sub(1) {
            let (copy, load) = (&block.insts[at], &block.insts[at + 1]);
            let (
                Some((Reg::Phys(hl), _)),
                Kind::Op("LD8rm"),
                [Operand::Def(Reg::Virt(byte)), Operand::Use(Reg::Phys(pointer))],
            ) = (copy_of(copy), load.kind, &load.operands[..])
            else {
                continue;
            };
            if hl != reg::HL || *pointer != reg::HL || !in_a.contains(byte) {
                continue;
            }
            let (Operand::Use(from), Operand::Def(to)) = (copy.operands[1].clone(), load.operands[0].clone()) else {
                continue;
            };
            let mut read = load.clone();
            read.kind = Kind::Op("LD8Arp");
            read.operands = vec![Operand::Def(Reg::Phys(reg::A)), Operand::Use(from)];
            let mut out = copy.clone();
            out.operands = vec![Operand::Def(to), Operand::Use(Reg::Phys(reg::A))];
            block.insts[at] = read;
            block.insts[at + 1] = out;
        }
    }
}

/// The pair `register` is, or is half of, if `push` and `pop` can move it: not
/// `a`, whose pair holds the flags.
fn stack_pair(register: llrm_lir::registers::RegId) -> Option<llrm_lir::registers::RegId> {
    [reg::BC, reg::DE, reg::HL].into_iter().find(|pair| REGISTER_INFO.root(register) == *pair)
}

/// Spill slots that nothing else needs a frame for become `push` and `pop`, and
/// the frame goes: `digits` kept a counter across a call with `ld -1(ix), b` /
/// `ld b, -1(ix)` and the 13 bytes of frame setup and teardown SDCC does
/// without.
///
/// Every path must treat the slots as a stack: a store with the slot empty, a
/// load of the slot on top, an empty stack at each return, one stack where
/// paths meet. A byte slot saves its pair, so the other half must not be live
/// after the `pop` (`PhysLiveness`). `a` is not saved this way: its pair is the
/// flags.
pub fn slots_are_a_stack(
    function: &mut MFunction,
    assignment: &Assignment,
) -> bool {
    use llrm_spike_mf::livephys::PhysLiveness;
    if !function.locals.is_empty() || !function.fixed.is_empty() {
        return false;
    }
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let spill = |inst: &Inst| match (inst.kind, &inst.operands[..]) {
        (Kind::Op("SPILL8" | "SPILL16"), [Operand::Frame(slot), Operand::Use(reg)]) => {
            Some((true, *slot, register(*reg)))
        }
        (Kind::Op("RELOAD8" | "RELOAD16"), [Operand::Def(reg), Operand::Frame(slot)]) => {
            Some((false, *slot, register(*reg)))
        }
        _ => None,
    };
    let mut any = false;
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        if inst.operands.iter().any(|one| matches!(one, Operand::Frame(_))) {
            match spill(inst) {
                Some((_, _, register)) if stack_pair(register).is_some() => any = true,
                _ => return false,
            }
        }
    }
    if !any {
        return false;
    }
    let succs = llrm_spike_mf::regalloc::successors(function);
    let mut entry: Vec<Option<Vec<i32>>> = vec![None; function.blocks.len()];
    entry[0] = Some(Vec::new());
    let mut work = vec![0usize];
    while let Some(b) = work.pop() {
        let mut stack = entry[b].clone().expect("a block on the list has a stack");
        for inst in &function.blocks[b].insts {
            match spill(inst) {
                Some((true, slot, _)) if !stack.contains(&slot) => stack.push(slot),
                Some((false, slot, _)) if stack.last() == Some(&slot) => {
                    stack.pop();
                }
                Some(_) => return false,
                None => {}
            }
        }
        if succs[b].is_empty() && !stack.is_empty() {
            return false;
        }
        for &next in &succs[b] {
            match &entry[next] {
                Some(seen) if *seen != stack => return false,
                Some(_) => {}
                None => {
                    entry[next] = Some(stack.clone());
                    work.push(next);
                }
            }
        }
    }
    // the other half of a pair a byte slot saves is dead once the pair is
    // restored
    let live = PhysLiveness::of::<Z80Registers>(function, assignment, &REGISTER_INFO);
    for (b, block) in function.blocks.iter().enumerate() {
        for (at, inst) in block.insts.iter().enumerate() {
            let Some((false, _, register)) = spill(inst) else { continue };
            let pair = stack_pair(register).expect("checked above");
            if register != pair {
                let after = live.live_before::<Z80Registers>(function, assignment, &REGISTER_INFO, b, at + 1);
                let (lo, hi) = expand::lanes(pair).expect("a pair has two halves");
                let mate = if register == lo { hi } else { lo };
                if after.is_live(&REGISTER_INFO, mate) {
                    return false;
                }
            }
        }
    }
    for inst in function.blocks.iter_mut().flat_map(|block| &mut block.insts) {
        match inst.kind {
            Kind::Op("SPILL8" | "SPILL16") => {
                inst.kind = Kind::Op("PUSHP");
                inst.operands.remove(0);
                inst.may_store = false;
            }
            Kind::Op("RELOAD8" | "RELOAD16") => {
                inst.kind = Kind::Op("POPP");
                inst.operands.remove(1);
                inst.may_load = false;
            }
            _ => {}
        }
    }
    true
}

/// Moves the entry block's spills of argument registers up to its start while
/// nothing before them writes the register or touches the slot, so the
/// prologue can `push` them: crc16 stored `n` and `p` with `ld -1(ix), a` /
/// `ld -3(ix), l` / `ld -2(ix), h` after a pair of `push af`, 9 B more than
/// `push af` / `push hl`.
pub fn spills_first(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    if llrm_spike_mf::regalloc::successors(function).iter().any(|next| next.contains(&0)) {
        return;
    }
    let insts = &mut function.blocks[0].insts;
    let mut first = 0;
    for at in 0..insts.len() {
        let (Kind::Op("SPILL8" | "SPILL16"), [Operand::Frame(slot), Operand::Use(source)]) =
            (insts[at].kind, &insts[at].operands[..])
        else {
            continue;
        };
        let (slot, source) = (*slot, register(*source));
        let blocked = insts[first..at]
            .iter()
            .any(
                |before| before.call
                    || before.barrier
                    || before.terminator
                    || before.operands.iter().any(|operand| match operand {
                        Operand::Def(r) => Z80Registers::overlaps(register(*r), source),
                        Operand::Frame(other) => *other == slot,
                        _ => false,
                    }),
            );
        if !blocked {
            let moved = insts.remove(at);
            insts.insert(first, moved);
            first += 1;
        }
    }
}

/// A constant made in a loop is made once before it (`MachineLICM`'s hoisting
/// of a trivially rematerializable instruction): `find` made `ld hl, #buf` on
/// every trip. Where the registers run out the allocator makes the constant
/// again at its uses, as it does for any rematerializable value. Whether
/// anything moved.
fn hoist_constants(function: &mut MFunction) -> bool {
    let mut moved = false;
    let cfg = Cfg::new(function);
    let mut defs = std::collections::HashMap::<u32, usize>::new();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        for operand in &inst.operands {
            if let Operand::Def(Reg::Virt(v)) = operand {
                *defs.entry(*v).or_default() += 1;
            }
        }
    }
    let constant = |inst: &Inst| {
        inst.rematerializable
            && matches!(
                &inst.operands[..],
                [Operand::Def(Reg::Virt(v)), Operand::Imm(_) | Operand::Symbol(..)] if defs[v] == 1
            )
    };
    // the innermost loop first, so a constant moves out as many loops as it can
    for (header, body) in cfg.loops().iter().rev() {
        let outside: Vec<usize> = cfg.preds[*header].iter().copied().filter(|p| !body[*p]).collect();
        let [pre] = outside[..] else { continue };
        if cfg.succs[pre] != [*header] {
            continue;
        }
        let mut hoisted = Vec::new();
        for (_, block) in function.blocks.iter_mut().enumerate().filter(|(b, _)| body[*b]) {
            let (out, keep): (Vec<Inst>, Vec<Inst>) = std::mem::take(&mut block.insts).into_iter().partition(&constant);
            block.insts = keep;
            hoisted.extend(out);
        }
        let made: Vec<u32> = hoisted
            .iter()
            .filter_map(|inst| match inst.operands[0] {
                Operand::Def(Reg::Virt(v)) => Some(v),
                _ => None,
            })
            .collect();
        // a commutable instruction that adds the constant to a fixed register
        // takes the other operand there
        for (_, block) in function.blocks.iter_mut().enumerate().filter(|(b, _)| body[*b]) {
            for at in 0..block.insts.len() {
                let inst = &block.insts[at];
                let uses: Vec<usize> = (0..inst.operands.len())
                    .filter(|&n| matches!(inst.operands[n], Operand::Use(Reg::Virt(_))))
                    .collect();
                let (true, [other]) = (inst.commutable, &uses[..]) else { continue };
                let Operand::Use(Reg::Virt(c)) = inst.operands[*other] else { continue };
                let Some(copy) = (0..at).rev().find(|&n| {
                    matches!(
                        &block.insts[n].operands[..],
                        [Operand::Def(Reg::Phys(_)), Operand::Use(Reg::Virt(_))]
                    )
                }) else {
                    continue;
                };
                let Operand::Use(Reg::Virt(x)) = block.insts[copy].operands[1] else { continue };
                if block.insts[copy].kind == Kind::Copy
                    && made.contains(&x)
                    && !made.contains(&c)
                    && function.classes[x as usize] == function.classes[c as usize]
                {
                    block.insts[copy].operands[1] = Operand::Use(Reg::Virt(c));
                    block.insts[at].operands[*other] = Operand::Use(Reg::Virt(x));
                }
            }
        }
        let insts = &mut function.blocks[pre].insts;
        let at = insts.iter().position(|inst| inst.terminator).unwrap_or(insts.len());
        moved |= !hoisted.is_empty();
        insts.splice(at..at, hoisted);
    }
    moved
}

/// Drops the copies the allocation gave one register.
///
/// DEVIATE from `llrm_spike_mf::remove_identity_copies`, which compares the
/// registers' roots: `b` and `c` are both `bc`, and `ld b, c` was dropped as an
/// identity (a select returned the wrong byte). Reason: that helper was written
/// for targets whose views of a root are one register. Exit: the shared helper
/// compares lanes.
pub fn remove_identity_copies(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    for block in &mut function.blocks {
        block
            .insts
            .retain(
                |inst| match (inst.kind, &inst.operands[..]) {
                    (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) => register(*dst) != register(*src),
                    _ => true,
                },
            );
    }
}

/// The copies the allocation left, cleaned: the copies back first, since one
/// that reads a register stops the copy before it from moving down.
pub fn tidy_copies(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    remove_copies_back(function, assignment);
    sink_copies_to_the_branch(function, assignment);
}

/// Moves a copy down to the block's branches when nothing after it reads or
/// writes either register, so the compare before it stays last among the
/// instructions a branch hoist must move over: sum_arr's `ld b, a` sat ahead
/// of `or a, a`, and the `ld a, #0` both successors begin with could not be
/// hoisted past a compare that reads `a`.
pub fn sink_copies_to_the_branch(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    for block in &mut function.blocks {
        let end = block.insts.iter().rposition(|inst| !inst.terminator).map_or(0, |last| last + 1);
        let mut at = 0;
        while at + 1 < end {
            let inst = &block.insts[at];
            let (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) = (inst.kind, &inst.operands[..]) else {
                at += 1;
                continue;
            };
            let (dst, src) = (register(*dst), register(*src));
            let blocked = |later: &Inst| {
                later.call
                    || later.barrier
                    || later.may_store
                    || later.side_effects
                    || later.operands.iter().any(|operand| match operand {
                        Operand::Def(r) | Operand::Use(r) => {
                            let r = register(*r);
                            Z80Registers::overlaps(r, dst)
                                || (matches!(operand, Operand::Def(_)) && Z80Registers::overlaps(r, src))
                        }
                        Operand::Frame(_) => false,
                        _ => false,
                    })
            };
            // only all the way: a copy stopped short sits between a reload and
            // the compare it folds into
            if (at + 1..end).any(|later| blocked(&block.insts[later])) {
                at += 1;
                continue;
            }
            let moved = block.insts.remove(at);
            block.insts.insert(end - 1, moved);
        }
    }
}

/// Drops a copy of what the register already holds: a copy back into the
/// register another copy read from (`ld b, a` ... `ld a, b`), or the same copy
/// again, while neither register is written in between. A block only one
/// earlier block enters starts from the copies that block ended with.
///
/// DEVIATE from `llrm_spike_mf::remove_redundant_copies`, which compares
/// roots (`ld a, c` after `ld b, a` is no copy back), stays in one block and
/// misses the repeat. Reason: `b` and `c` are lanes of one root. Exit: the
/// shared helper compares lanes.
pub fn remove_copies_back(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    copies_in_force(function, assignment, true);
}

/// The copies in force at the end of each block, as `(destination, source)`
/// registers; `drop` also removes the ones a copy repeats.
fn copies_in_force(
    function: &mut MFunction,
    assignment: &Assignment,
    drop: bool,
) -> Vec<Vec<(RegId, RegId)>> {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let successors = llrm_spike_mf::regalloc::successors(function);
    let mut entered = vec![0usize; function.blocks.len()];
    for to in successors.iter().flatten() {
        entered[*to] += 1;
    }
    // `(destination, source)` of the copies in force at the end of each block
    let mut ended: Vec<Vec<(RegId, RegId)>> = vec![Vec::new(); function.blocks.len()];
    for at in 0..function.blocks.len() {
        // the block's one entrance, if it came earlier in the layout
        let mut held = match successors.iter().position(|next| next.contains(&at)) {
            Some(from) if entered[at] == 1 && from < at => ended[from].clone(),
            _ => Vec::new(),
        };
        let mut keep = Vec::with_capacity(function.blocks[at].insts.len());
        for inst in std::mem::take(&mut function.blocks[at].insts) {
            let copy = match (inst.kind, &inst.operands[..]) {
                (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) => Some((register(*dst), register(*src))),
                _ => None,
            };
            let repeats = copy.is_some_and(|(dst, src)| held.contains(&(src, dst)) || held.contains(&(dst, src)));
            if drop && repeats {
                continue;
            }
            if inst.call {
                held.clear();
            }
            let written: Vec<RegId> = inst
                .operands
                .iter()
                .filter_map(|o| if let Operand::Def(r) = o { Some(register(*r)) } else { None })
                .collect();
            held.retain(|&(to, from)| {
                !written.iter().any(|&w| Z80Registers::overlaps(w, to) || Z80Registers::overlaps(w, from))
            });
            held.extend(copy);
            keep.push(inst);
        }
        function.blocks[at].insts = keep;
        ended[at] = held;
    }
    ended
}

/// Moves the copy a loop head starts with into the back edge, when every other
/// entrance has made it already: `ld a, b` at a loop head, where the entry
/// has just made it, runs every trip and once more at the exit; on the back
/// edge alone it runs once a trip. Elsewhere (a diamond's join) the move made
/// popcount and bsearch8 larger: the copy in a block of its own kept a jump.
/// The flow into the join is the sum of its entrances, so the count of copies
/// run never grows, and the code does not grow either: the copy moves, it is
/// not copied.
///
/// The pin has no such step: its allocator joins the copy out of existence
/// (`canJoinPhys`) or leaves it where selection put it. Here the join of an
/// argument's entry copy into its register is refused (`joins_pinned_copies`),
/// so the copy stays where selection put it.
pub fn place_copies_where_needed(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let successors = llrm_spike_mf::regalloc::successors(function);
    let mut changed = true;
    while changed {
        changed = false;
        let ended = copies_in_force(function, assignment, false);
        for head in 1..function.blocks.len() {
            let entrances: Vec<usize> =
                (0..function.blocks.len()).filter(|&from| successors[from].contains(&head)).collect();
            // the head's first copy, behind nothing that touches either
            // register (digits' call-stack marker came first)
            let Some((at, dst, src)) = function.blocks[head].insts.iter().enumerate().find_map(|(at, inst)| {
                let (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) = (inst.kind, &inst.operands[..]) else {
                    return None;
                };
                let (dst, src) = (register(*dst), register(*src));
                let clear = function.blocks[head].insts[..at].iter().all(|before| {
                    !(before.call
                        || before.barrier
                        || before.side_effects
                        || before.may_store
                        || before.may_load
                        || before.operands.iter().any(|operand| match operand {
                            Operand::Def(r) | Operand::Use(r) => {
                                Z80Registers::overlaps(register(*r), dst) || Z80Registers::overlaps(register(*r), src)
                            }
                            _ => false,
                        }))
                });
                clear.then_some((at, dst, src))
            }) else {
                continue;
            };
            if entrances.len() < 2 || dst == src {
                continue;
            }
            let made = |from: usize| ended[from].contains(&(dst, src)) || ended[from].contains(&(src, dst));
            let lacking: Vec<usize> = entrances.iter().copied().filter(|&from| !made(from)).collect();
            // one entrance lacks it, by a block that only leads here
            let [from] = lacking[..] else { continue };
            if successors[from] != [head] {
                continue;
            }
            let block = &function.blocks[from];
            let end = block.insts.iter().rposition(|inst| !inst.terminator).map_or(0, |last| last + 1);
            let touched = block
                .insts[end..]
                .iter()
                .any(
                    |inst| inst.operands.iter().any(|operand| match operand {
                        Operand::Def(r) | Operand::Use(r) => {
                            Z80Registers::overlaps(register(*r), dst) || Z80Registers::overlaps(register(*r), src)
                        }
                        _ => false,
                    }),
                );
            // a block of nothing but its jump is an edge a branch folds away;
            // the copy would keep it, and the jump with it
            if touched || end == 0 {
                continue;
            }
            let copy = function.blocks[head].insts.remove(at);
            function.blocks[from].insts.insert(end, copy);
            changed = true;
            break;
        }
    }
}

/// A word compared right after a copy of it reads the copy's source: digits'
/// entry made `ld e, l` / `ld d, h` for the compare of the dividend, and the
/// loop head wanted it back in `hl`. Only a `CP16i`, which takes any pair,
/// is rewritten, when the copy's destination is dead after the last compare
/// and the source is not written before it. The copy is then dropped.
pub fn read_copies_at_their_source(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    use llrm_spike_mf::livephys::PhysLiveness;
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let pair = |r: RegId| [reg::BC, reg::DE, reg::HL].contains(&r);
    'again: loop {
        let live = PhysLiveness::of::<Z80Registers>(function, assignment, &REGISTER_INFO);
        for b in 0..function.blocks.len() {
            for at in 0..function.blocks[b].insts.len() {
                let inst = &function.blocks[b].insts[at];
                let (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) = (inst.kind, &inst.operands[..]) else {
                    continue;
                };
                let (dst, src) = (register(*dst), register(*src));
                if dst == src || !pair(dst) || !pair(src) {
                    continue;
                }
                let mut compares = Vec::new();
                for (later, inst) in function.blocks[b].insts.iter().enumerate().skip(at + 1) {
                    let reads = inst
                        .operands
                        .iter()
                        .any(
                            |o| matches!(
                                o,
                                Operand::Use(r) if Z80Registers::overlaps(register(*r), dst)
                            ),
                        );
                    if reads {
                        let exact = inst.kind == Kind::Op("CP16i")
                            && inst.operands.iter().all(|o| match o {
                                Operand::Use(r) if Z80Registers::overlaps(register(*r), dst) => register(*r) == dst,
                                _ => true,
                            });
                        if !exact {
                            compares.clear();
                            break;
                        }
                        compares.push(later);
                    }
                    let writes = |to: RegId| {
                        inst.operands
                            .iter()
                            .any(|o| matches!(o, Operand::Def(r) if Z80Registers::overlaps(register(*r), to)))
                    };
                    if inst.call || inst.barrier || writes(src) || writes(dst) {
                        break;
                    }
                }
                let Some(&last) = compares.last() else { continue };
                // the source stays as it is to the last compare (a write in the
                // compare's own instruction comes after its read)
                let after = live.live_before::<Z80Registers>(function, assignment, &REGISTER_INFO, b, last + 1);
                if after.is_live(&REGISTER_INFO, dst) {
                    continue;
                }
                let touched = function
                    .blocks[b]
                    .insts[at + 1..=last]
                    .iter()
                    .any(
                        |inst| inst.call
                            || inst
                                .operands
                                .iter()
                                .any(|o| matches!(
                                    o,
                                    Operand::Def(r) if Z80Registers::overlaps(register(*r), src) || Z80Registers::overlaps(register(*r), dst)
                                ) ),
                    );
                if touched {
                    continue;
                }
                for &later in &compares {
                    for operand in &mut function.blocks[b].insts[later].operands {
                        if matches!(operand, Operand::Use(r) if register(*r) == dst) {
                            *operand = Operand::Use(Reg::Phys(src));
                        }
                    }
                }
                function.blocks[b].insts.remove(at);
                continue 'again;
            }
        }
        break;
    }
}

/// Reads the byte a `LO`/`HI` extracts straight from its half of the pair when
/// the pair keeps its value to the last use, instead of copying it to a byte
/// register first (`ld d, c` / `cp a, d` becomes `cp a, c`).
///
/// Only a byte defined once and used later in its own block, with no write to
/// that half and no call in between, is rewritten.
pub fn use_lanes_in_place(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    // a byte read in place frees the half it was copied into, so another byte
    // kept from that half may now be read in place too
    while use_lanes_once(function, assignment) {}
}

fn use_lanes_once(
    function: &mut MFunction,
    assignment: &Assignment,
) -> bool {
    let mut removed = false;
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let mut defs = vec![0usize; function.classes.len()];
    let mut uses = vec![Vec::new(); function.classes.len()];
    for (b, block) in function.blocks.iter().enumerate() {
        for inst in &block.insts {
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(v)) => defs[*v as usize] += 1,
                    Operand::Use(Reg::Virt(v)) => uses[*v as usize].push(b),
                    _ => {}
                }
            }
        }
    }
    for b in 0..function.blocks.len() {
        let mut at = 0;
        while at < function.blocks[b].insts.len() {
            let inst = &function.blocks[b].insts[at];
            let (Kind::Op(op @ ("LO" | "HI")), [Operand::Def(Reg::Virt(byte)), Operand::Use(source)]) =
                (inst.kind, &inst.operands[..])
            else {
                at += 1;
                continue;
            };
            let Ok((lo, hi)) = expand::lanes(register(*source)) else {
                at += 1;
                continue;
            };
            let (byte, half) = (*byte, if op == "LO" { lo } else { hi });
            // a byte nothing virtual reads is read through its register by an
            // instruction that names it (a copy into `a` that allocation made
            // the identity is gone): it is still made
            if defs[byte as usize] != 1
                || uses[byte as usize].is_empty()
                || uses[byte as usize].iter().any(|one| *one != b)
            {
                at += 1;
                continue;
            }
            let insts = &function.blocks[b].insts;
            let mut last = at;
            for (k, later) in insts.iter().enumerate().skip(at + 1) {
                if later.operands.iter().any(|o| matches!(o, Operand::Use(Reg::Virt(v)) if *v == byte)) {
                    last = k;
                }
            }
            // a PACK that reads the byte writes the pair after reading both
            // halves (`pack`), so its own write to the half does
            // not clobber it: the low half of a pair kept while the
            // high half is replaced stays where it is
            let checked = if insts[last].kind == Kind::Op("PACK") { last } else { last + 1 };
            let clobbered = insts[at + 1..checked]
                .iter()
                .any(
                    |later| later.call
                        || later.operands.iter().any(|o| {
                            matches!(
                                o,
                                Operand::Def(r) if <Z80Registers as llrm_spike_mf::Target>::overlaps(register(*r), half)
                            )
                        }),
                );
            if clobbered {
                at += 1;
                continue;
            }
            for later in &mut function.blocks[b].insts[at + 1..=last] {
                for operand in &mut later.operands {
                    if matches!(operand, Operand::Use(Reg::Virt(v)) if *v == byte) {
                        *operand = Operand::Use(Reg::Phys(half));
                    }
                }
            }
            function.blocks[b].insts.remove(at);
            removed = true;
        }
    }
    removed
}

/// `function`, one instruction per line, on stderr (`LLRM_Z80_DUMP=name`).
fn dump(function: &MFunction) {
    for block in &function.blocks {
        eprintln!("bb{}:", block.id);
        for inst in &block.insts {
            eprintln!(
                "  {:?} {:?}{}{}",
                inst.kind,
                inst.operands,
                if inst.sets_flags { " SF" } else { "" },
                if inst.reads_flags { " RF" } else { "" }
            );
        }
    }
}

/// The function `name` of `ir` as assembly lines.
pub fn lines(
    ir: &str,
    name: &str,
    number: usize,
    request: &Request,
) -> Result<Vec<Line>, String> {
    let compile = |duplicate: bool| -> Result<Vec<Line>, String> {
        let (function, assignment, slots, reads) = allocated_with(ir, name, request, duplicate)?;
        finish(&function, &assignment, number, &slots, reads)
    };
    // Copying a return block into its predecessors saves a `jp` and often a
    // move, or costs a copy of the block: the smaller result, then the faster,
    // is kept.
    let copied = compile(true);
    let kept = compile(false);
    match (copied, kept) {
        (Ok(one), Ok(two)) => Ok(if expand::price(&one) <= expand::price(&two) { one } else { two }),
        (Ok(one), Err(_)) | (Err(_), Ok(one)) => Ok(one),
        (Err(why), Err(_)) => Err(why),
    }
    .map(expand::relax)
}

/// The function as text: its label, then one line per instruction.
pub fn text(
    ir: &str,
    name: &str,
    number: usize,
    request: &Request,
) -> Result<String, String> {
    let mut out = format!("{}::\n", crate::data::symbol(name));
    for line in lines(ir, name, number, request)? {
        match line {
            Line::Label(label) => out.push_str(&format!("{label}:\n")),
            Line::Inst(text) => out.push_str(&format!("\t{text}\n")),
        }
    }
    Ok(out)
}

/// The conditional jump for a `Z80::CondCode` (`NZ` 0, `Z` 1, `NC` 2, `C` 3).
fn branch_for(cond: i64) -> Option<&'static str> {
    Some(match cond {
        0 => "JPNZ",
        1 => "JPZ",
        2 => "JPNC",
        3 => "JPC",
        _ => return None,
    })
}

fn is_cmov(inst: &Inst) -> bool {
    matches!(inst.kind, Kind::Op("CMOV8" | "CMOV16"))
}

/// A `CMOV`'s result, false value, true value and condition.
fn cmov_parts(inst: &Inst) -> Result<(Reg, Reg, Reg, i64), String> {
    let [Operand::Def(result), Operand::Use(on_false), Operand::Use(on_true), Operand::Imm(cond), Operand::Use(_)] =
        inst.operands[..]
    else {
        return Err("a CMOV with unexpected operands".to_owned());
    };
    Ok((result, on_false, on_true, cond))
}

/// `CodeGenPrepare::sinkSelectOperand`: the instructions that only make one
/// side of the `CMOV` at `p` move out of the block into the arm that side
/// takes, so the other path does not run them. They come back in order, taken
/// out of `insts`; none when nothing but copies would move or a hazard shows.
///
/// DEVIATE: the pin sinks one-use IR operands that cost at least
/// `TCC_Expensive` before selection; this works on the selected instructions,
/// on any side that holds a non-copy. Reason: the select is expanded here, not
/// in CodeGenPrepare. Exit: the expansion at selection.
fn sunk_operand(
    function: &MFunction,
    block: usize,
    p: usize,
    value: Reg,
) -> Option<Vec<usize>> {
    let insts = &function.blocks[block].insts[..p];
    let mut uses: std::collections::HashMap<Reg, usize> = std::collections::HashMap::new();
    let mut defs: std::collections::HashMap<Reg, usize> = std::collections::HashMap::new();
    for one in &function.blocks {
        for inst in &one.insts {
            for operand in &inst.operands {
                match operand {
                    Operand::Use(reg @ Reg::Virt(_)) => *uses.entry(*reg).or_default() += 1,
                    Operand::Def(reg @ Reg::Virt(_)) => *defs.entry(*reg).or_default() += 1,
                    _ => {}
                }
            }
        }
    }
    let movable = |inst: &Inst| {
        !(inst.call
            || inst.terminator
            || inst.barrier
            || inst.reads_flags
            || inst.side_effects
            || inst.may_load
            || inst.may_store)
    };
    let touches = |inst: &Inst, reg: Reg| {
        inst.operands.iter().any(|o| matches!(o, Operand::Def(r) | Operand::Use(r) if *r == reg))
    };
    let cmov = &function.blocks[block].insts[p];
    let reads =
        |inst: &Inst, reg: Reg| inst.operands.iter().filter(|o| matches!(o, Operand::Use(r) if *r == reg)).count();
    // From the last instruction back, so every reader of a result is in the
    // slice before its maker is looked at. A value made here that anything
    // else reads is an input, not part of the slice; a machine register a
    // member reads must come from a member.
    let mut slice = std::collections::BTreeSet::new();
    let mut pending = vec![(value, insts.len(), false)];
    loop {
        let resolved: Vec<(usize, Reg, bool)> = pending
            .iter()
            .filter_map(|&(reg, before, must)| {
                insts[..before]
                    .iter()
                    .rposition(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(r) if *r == reg)))
                    .map(|at| (at, reg, must))
                    .or(None)
            })
            .collect();
        if let Some(&(_, Reg::Phys(_), true)) = pending
            .iter()
            .map(|&(r, b, m)| (b, r, m))
            .find(|&(b, r, m)| {
                m && !insts[..b]
                    .iter()
                    .any(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(x) if *x == r)))
            })
            .as_ref()
        {
            return None;
        }
        let Some(&(at, ..)) = resolved.iter().max_by_key(|one| one.0) else { break };
        let must = resolved.iter().any(|&(one, _, must)| one == at && must);
        pending.retain(|&(reg, before, _)| {
            !insts[..before]
                .iter()
                .rposition(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(r) if *r == reg)))
                .is_some_and(|one| one == at)
        });
        if slice.contains(&at) {
            continue;
        }
        let inst = &insts[at];
        let alone = inst
            .operands
            .iter()
            .all(
                |operand| {
                    let Operand::Def(reg @ Reg::Virt(_)) = operand else { return true };
                    let inside: usize = slice.iter().map(|&one| reads(&insts[one], *reg)).sum();
                    let by_cmov = if *reg == value { reads(cmov, *reg) } else { 0 };
                    defs.get(reg) == Some(&1) && uses.get(reg).copied().unwrap_or(0) == inside + by_cmov
                },
            );
        if !movable(inst) || !alone {
            if must {
                return None;
            }
            continue;
        }
        slice.insert(at);
        for operand in &inst.operands {
            match operand {
                Operand::Use(reg @ Reg::Virt(_)) => pending.push((*reg, at, false)),
                Operand::Use(reg) if *reg != Reg::Phys(crate::reg::FLAGS) => pending.push((*reg, at, true)),
                _ => {}
            }
        }
    }
    if !slice.iter().any(|&at| matches!(insts[at].kind, Kind::Op(_))) {
        return None;
    }
    // a register of the machine the slice touches is touched by nothing left
    // between its first instruction and the `CMOV`
    let first = *slice.iter().next()?;
    for &at in &slice {
        for operand in &insts[at].operands {
            let (Operand::Def(reg @ Reg::Phys(_)) | Operand::Use(reg @ Reg::Phys(_))) = operand else { continue };
            if (first..insts.len()).any(|other| !slice.contains(&other) && touches(&insts[other], *reg)) {
                return None;
            }
        }
    }
    // the flags the branch reads are not made by the slice, and a flags reader
    // after the diamond sees what the slice leaves only if the slice leaves
    // none
    let flags = Reg::Phys(crate::reg::FLAGS);
    let makes_flags = slice.iter().any(|&at| touches(&insts[at], flags));
    if makes_flags {
        if insts.iter().rposition(|inst| touches(inst, flags)).is_some_and(|last| slice.contains(&last)) {
            return None;
        }
        let tail = &function.blocks[block].insts[p + 1..];
        let wanted = tail.iter().find(|inst| inst.reads_flags || touches(inst, flags));
        if wanted.is_some_and(|inst| inst.reads_flags) {
            return None;
        }
    }
    Some(slice.into_iter().collect())
}

/// `EmitLoweredSelect`: a run of `CMOV` pseudos on one condition (or its
/// opposite) becomes a diamond. The block jumps to the join on the condition;
/// the empty block it falls through to is the other path; the join holds a phi
/// per `CMOV` and what followed the run. A later `CMOV` that reads an earlier
/// one's result is renamed to the value that result has on each path.
///
/// The arms are empty, and `ld` leaves the flags alone, so a copy the phis
/// become does not disturb a later reader of the compare. A lone `CMOV` whose
/// side is made by instructions nothing else reads has them moved into the
/// arm that side takes (`sunk_operand`).
///
/// DEVIATE: only `CMOV`s that follow one another are one diamond, and the
/// cascaded case is not translated. Reason: nothing selected it yet. Exit: when
/// a kernel needs it.
fn expand_cmov(function: &mut MFunction) -> Result<(), String> {
    let mut next_id = function.fresh_block_id();
    let mut at = 0;
    while at < function.blocks.len() {
        let Some(p) = function.blocks[at].insts.iter().position(is_cmov) else {
            at += 1;
            continue;
        };
        let alone = !function.blocks[at].insts.get(p + 1).is_some_and(is_cmov);
        let (_, on_false, on_true, first) = cmov_parts(&function.blocks[at].insts[p])?;
        // the side made in the block alone moves; the true side takes the
        // inverted jump to reach its arm
        let sunk = if alone {
            sunk_operand(function, at, p, on_false)
                .map(|slice| (false, slice))
                .or_else(|| sunk_operand(function, at, p, on_true).map(|slice| (true, slice)))
        } else {
            None
        };
        let (flip, moved) = match sunk {
            Some((flip, slice)) => {
                let mut taken = Vec::new();
                for &index in slice.iter().rev() {
                    taken.push(function.blocks[at].insts.remove(index));
                }
                taken.reverse();
                (flip, taken)
            }
            None => (false, Vec::new()),
        };
        let p = function.blocks[at].insts.iter().position(is_cmov).ok_or("the CMOV vanished")?;
        let mut tail = function.blocks[at].insts.split_off(p);
        let first = if flip { first ^ 1 } else { first };
        let same =
            |inst: &Inst| is_cmov(inst) && cmov_parts(inst).is_ok_and(|(.., cond)| cond == first || cond == first ^ 1);
        let taken = tail.iter().take_while(|inst| same(inst)).count();
        let cmovs: Vec<Inst> = tail.drain(..taken).collect();
        let branch = branch_for(first).ok_or_else(|| format!("a CMOV on condition {first}"))?;
        let mut head_value = std::collections::HashMap::new();
        let mut other_value = std::collections::HashMap::new();
        let mut phis = Vec::new();
        for cmov in &cmovs {
            let (result, on_false, on_true, cond) = cmov_parts(cmov)?;
            let (when_true, when_false) = if cond == first { (on_true, on_false) } else { (on_false, on_true) };
            let on_head = head_value.get(&when_true).copied().unwrap_or(when_true);
            let on_other = other_value.get(&when_false).copied().unwrap_or(when_false);
            head_value.insert(result, on_head);
            other_value.insert(result, on_other);
            phis.push(Phi { result, incoming: vec![(at, on_head), (at + 1, on_other)] });
        }
        let (otherwise, join) = (next_id, next_id + 1);
        next_id += 2;
        let mut jump =
            Inst::new(Kind::Op(branch), vec![Operand::Block(join), Operand::Use(Reg::Phys(crate::reg::FLAGS))]);
        jump.terminator = true;
        jump.reads_flags = true;
        function.blocks[at].insts.push(jump);
        function.blocks.insert(at + 1, Block { id: otherwise, insts: moved, phis: Vec::new() });
        function.blocks.insert(at + 2, Block { id: join, insts: tail, phis });
        // the join is where control leaves the original block, and every block
        // after it moved down by two
        for (index, block) in function.blocks.iter_mut().enumerate() {
            if index == at + 2 {
                continue;
            }
            for phi in &mut block.phis {
                for (pred, _) in &mut phi.incoming {
                    if *pred == at {
                        *pred = at + 2;
                    } else if *pred > at {
                        *pred += 2;
                    }
                }
            }
        }
        at += 1;
    }
    Ok(())
}

/// The most instructions, `ret` included, of a block that is copied into its
/// predecessors.
const TAIL_DUPLICATION_SIZE: usize = 4;

/// `TailDuplicator`: a small block that ends in `ret` is copied into each
/// predecessor that reaches it without a conditional branch, so `jp` (three
/// bytes) and the phi copy ahead of it become a `ret`. A predecessor that
/// branches to it conditionally keeps the original, with that edge's phi
/// inputs.
fn tail_duplicate_returns(function: &mut MFunction) {
    let mut j = 1;
    while j < function.blocks.len() {
        let block = &function.blocks[j];
        let small = block.insts.len() <= TAIL_DUPLICATION_SIZE
            && matches!(
                block.insts.last().map(|inst| inst.kind),
                Some(Kind::Op("RET"))
            )
            && block.insts.iter().filter(|inst| inst.terminator).count() == 1;
        if !small {
            j += 1;
            continue;
        }
        let id = block.id;
        let preds: Vec<usize> = (0..function.blocks.len())
            .filter(|&p| llrm_spike_mf::regalloc::successors(function)[p].contains(&j))
            .collect();
        let reaches_unconditionally = |function: &MFunction, p: usize| {
            let last = function.blocks[p].insts.last();
            match last {
                Some(inst) if inst.barrier => {
                    matches!(inst.kind, Kind::Op("JP")) && inst.operands == [Operand::Block(id)]
                }
                // falls through: no branch at all, or a conditional one that
                // goes elsewhere
                last => {
                    p + 1 == j
                        && last.is_none_or(|inst| !inst.terminator || !inst.operands.contains(&Operand::Block(id)))
                }
            }
        };
        let copy_into: Vec<usize> = preds.iter().copied().filter(|&p| reaches_unconditionally(function, p)).collect();
        if copy_into.is_empty() {
            j += 1;
            continue;
        }
        let tail = function.blocks[j].insts.clone();
        for &p in &copy_into {
            // the phi inputs for this edge, as copies
            let mut moves = Vec::new();
            for phi in &mut function.blocks[j].phis {
                if let Some(at) = phi.incoming.iter().position(|(pred, _)| *pred == p) {
                    let (_, input) = phi.incoming.remove(at);
                    moves.push(Inst::copy(phi.result, input));
                }
            }
            let insts = &mut function.blocks[p].insts;
            if insts.last().is_some_and(|inst| inst.barrier) {
                insts.pop();
            }
            insts.extend(moves);
            insts.extend(tail.iter().cloned());
        }
        // the original goes when nothing branches to it now
        let others = preds.len() - copy_into.len();
        if others == 0 {
            function.blocks.remove(j);
            for block in &mut function.blocks {
                for phi in &mut block.phis {
                    for (pred, _) in &mut phi.incoming {
                        if *pred > j {
                            *pred -= 1;
                        }
                    }
                }
            }
        } else {
            j += 1;
        }
    }
}

/// `jp cc, T` over a bare `ret` that falls into `T` is `ret !cc`: the branch
/// returns on the opposite condition and the code after it is `T`.
fn return_conditionally(function: &mut MFunction) {
    use llrm_spike_mf::Jump;
    let mut i = 0;
    while i + 2 < function.blocks.len() {
        let (here, bare, after) = (&function.blocks[i], &function.blocks[i + 1], &function.blocks[i + 2]);
        let branch = here.insts.last().filter(|inst| inst.terminator && !inst.barrier);
        let to_after = branch.is_some_and(|inst| inst.operands.first() == Some(&Operand::Block(after.id)));
        let bare_ret = matches!(&bare.insts[..], [one] if one.kind == Kind::Op("RET")) && bare.phis.is_empty();
        let referenced = function
            .blocks
            .iter()
            .flat_map(|block| block.insts.iter())
            .any(|inst| inst.terminator && inst.operands.contains(&Operand::Block(bare.id)));
        if to_after && bare_ret && !referenced {
            let reversed = Z80Registers::reverse(branch.expect("a branch"));
            if let Some(Inst { kind: Kind::Op(name), .. }) = reversed {
                let returning = match name {
                    "JPNZ" => "RETNZ",
                    "JPZ" => "RETZ",
                    "JPNC" => "RETNC",
                    _ => "RETC",
                };
                let last = function.blocks[i].insts.len() - 1;
                let mut inst = Inst::new(Kind::Op(returning), vec![]);
                inst.reads_flags = true;
                function.blocks[i].insts[last] = inst;
                function.blocks.remove(i + 1);
                for block in &mut function.blocks {
                    for phi in &mut block.phis {
                        for (pred, _) in &mut phi.incoming {
                            if *pred > i + 1 {
                                *pred -= 1;
                            }
                        }
                    }
                }
                continue;
            }
        }
        i += 1;
    }
}

/// Whether `inst` leaves the Z flag set by its 8-bit result.
fn sets_zero_by_result(inst: &Inst) -> bool {
    matches!(
        inst.kind,
        Kind::Op(
            "INC8"
                | "DEC8"
                | "SHL8"
                | "SRL8"
                | "SRA8"
                | "ADD8r"
                | "ADD8i"
                | "SUB8r"
                | "SUB8i"
                | "AND8r"
                | "AND8i"
                | "OR8r"
                | "OR8i"
                | "XOR8r"
                | "XOR8i"
        )
    )
}

/// `optimizeCompareInstr`: the test of a byte against zero (`cp 0`) after the
/// instruction that made the byte and set the flags by it is dropped when only
/// the zero flag is read: `dec c; ld a, c; or a; jp nz` is `dec c; jp nz`.
/// Every `ld` between leaves the flags alone.
fn eliminate_tests(function: &mut MFunction) {
    for block in &mut function.blocks {
        let mut at = 1;
        while at < block.insts.len() {
            let compare = &block.insts[at];
            let is_test = compare.kind == Kind::Op("CP8i")
                && matches!(
                    compare.operands[..],
                    [_, Operand::Use(Reg::Phys(a)), Operand::Imm(0)] if a == crate::reg::A
                );
            let copy = &block.insts[at - 1];
            let tested = match (&copy.kind, &copy.operands[..]) {
                (Kind::Copy, [Operand::Def(Reg::Phys(a)), Operand::Use(v)]) if *a == crate::reg::A => Some(*v),
                _ => None,
            };
            let (true, Some(value)) = (is_test, tested) else {
                at += 1;
                continue;
            };
            // the instruction that made `value`: it defines it, or copies it
            // out of `a`
            let defines = |inst: &Inst, reg: Reg| inst.operands.contains(&Operand::Def(reg));
            let Some(made) = block.insts[..at - 1].iter().rposition(|inst| defines(inst, value)) else {
                at += 1;
                continue;
            };
            let setter = if block.insts[made].kind == Kind::Copy {
                // `v = COPY a` after an instruction that defines `a` as its
                // result
                made.checked_sub(1).filter(|&before| defines(&block.insts[before], Reg::Phys(crate::reg::A)))
            } else {
                Some(made)
            };
            let Some(setter) = setter.filter(|&one| sets_zero_by_result(&block.insts[one])) else {
                at += 1;
                continue;
            };
            let flags_kept = block.insts[setter + 1..at - 1].iter().all(|inst| !inst.sets_flags && !inst.call);
            let readers = block.insts[at + 1..].iter().take_while(|inst| !inst.sets_flags);
            let only_zero = readers
                .filter(|inst| inst.reads_flags)
                .all(|inst| matches!(inst.kind, Kind::Op("JPZ" | "JPNZ" | "RETZ" | "RETNZ")));
            if flags_kept && only_zero {
                block.insts.drain(at - 1..=at);
                at -= 1;
            } else {
                at += 1;
            }
        }
    }
}
