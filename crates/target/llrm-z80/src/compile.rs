//! A function from IR to sdasz80 text: the shared DAG stages, then the Z80
//! machine function, allocation, expansion and printing.

use llrm_spike_mf::regalloc::{Assignment, eliminate_phis_with, two_address};
use llrm_lir::registers::Regs;
use llrm_spike_mf::tolir::allocate_greedy;

use crate::driver::{Request, selections};
use crate::expand::{self, Line, finish};
use crate::mfunc::{Block, Inst, Kind, MFunction, Operand, Phi, Reg, emit};
use crate::regalloc::Z80Registers;
use crate::REGISTER_INFO;

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
    let (_, selection) = selected.into_iter().find(|(one, _)| one == name).ok_or_else(|| format!("no function @{name}"))?;
    let mut function = emit(name, &selection?)?;
    expand_cmov(&mut function)?;
    if duplicate {
        tail_duplicate_returns(&mut function);
    }
    eliminate_tests(&mut function);
    Ok(function)
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
    two_address(&mut function);
    if std::env::var("LLRM_Z80_DUMP").is_ok_and(|one| one == name) {
        dump(&function);
    }
    let (assignment, slots) = allocate_greedy::<Z80Registers>(&mut function, Regs(&REGISTER_INFO), false)?;
    let reads = expand::reads(&function);
    if std::env::var("LLRM_Z80_DUMP_RA").is_ok_and(|one| one == name) {
        dump(&function);
    }
    remove_identity_copies(&mut function, &assignment);
    use_lanes_in_place(&mut function, &assignment);
    llrm_spike_mf::branch_fold_hoisting::<Z80Registers>(&mut function, &assignment);
    return_conditionally(&mut function);
    let size = |class| if Z80Registers::bytes_of(class) == 1 { 1 } else { 2 };
    Ok((function, assignment, slots.into_iter().map(|(slot, class)| (slot, size(class))).collect(), reads))
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
        block.insts.retain(|inst| match (inst.kind, &inst.operands[..]) {
            (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) => register(*dst) != register(*src),
            _ => true,
        });
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
            let (Kind::Op(op @ ("LO" | "HI")), [Operand::Def(Reg::Virt(byte)), Operand::Use(source)]) = (inst.kind, &inst.operands[..]) else {
                at += 1;
                continue;
            };
            let Ok((lo, hi)) = expand::lanes(register(*source)) else {
                at += 1;
                continue;
            };
            let (byte, half) = (*byte, if op == "LO" { lo } else { hi });
            if defs[byte as usize] != 1 || uses[byte as usize].iter().any(|one| *one != b) {
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
            let clobbered = insts[at + 1..=last].iter().any(|later| {
                later.call
                    || later.operands.iter().any(|o| matches!(o, Operand::Def(r) if <Z80Registers as llrm_spike_mf::Target>::overlaps(register(*r), half)))
            });
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
        }
    }
}

/// `function`, one instruction per line, on stderr (`LLRM_Z80_DUMP=name`).
fn dump(function: &MFunction) {
    for block in &function.blocks {
        eprintln!("bb{}:", block.id);
        for inst in &block.insts {
            eprintln!("  {:?} {:?}{}{}", inst.kind, inst.operands, if inst.sets_flags { " SF" } else { "" }, if inst.reads_flags { " RF" } else { "" });
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
    let mut out = format!("_{name}::\n");
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
/// DEVIATE: the pin sinks one-use IR operands that cost at least `TCC_Expensive`
/// before selection; this works on the selected instructions, on any side that
/// holds a non-copy. Reason: the select is expanded here, not in CodeGenPrepare.
/// Exit: the expansion at selection.
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
        !(inst.call || inst.terminator || inst.barrier || inst.reads_flags || inst.side_effects || inst.may_load || inst.may_store)
    };
    let touches = |inst: &Inst, reg: Reg| inst.operands.iter().any(|o| matches!(o, Operand::Def(r) | Operand::Use(r) if *r == reg));
    let cmov = &function.blocks[block].insts[p];
    let reads = |inst: &Inst, reg: Reg| inst.operands.iter().filter(|o| matches!(o, Operand::Use(r) if *r == reg)).count();
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
        if let Some(&(_, Reg::Phys(_), true)) = pending.iter().map(|&(r, b, m)| (b, r, m)).find(|&(b, r, m)| {
            m && !insts[..b].iter().any(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(x) if *x == r)))
        }).as_ref() {
            return None;
        }
        let Some(&(at, ..)) = resolved.iter().max_by_key(|one| one.0) else { break };
        let must = resolved.iter().any(|&(one, _, must)| one == at && must);
        pending.retain(|&(reg, before, _)| {
            !insts[..before].iter().rposition(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(r) if *r == reg))).is_some_and(|one| one == at)
        });
        if slice.contains(&at) {
            continue;
        }
        let inst = &insts[at];
        let alone = inst.operands.iter().all(|operand| {
            let Operand::Def(reg @ Reg::Virt(_)) = operand else { return true };
            let inside: usize = slice.iter().map(|&one| reads(&insts[one], *reg)).sum();
            let by_cmov = if *reg == value { reads(cmov, *reg) } else { 0 };
            defs.get(reg) == Some(&1) && uses.get(reg).copied().unwrap_or(0) == inside + by_cmov
        });
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
    // after the diamond sees what the slice leaves only if the slice leaves none
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
        let same = |inst: &Inst| {
            is_cmov(inst) && cmov_parts(inst).is_ok_and(|(.., cond)| cond == first || cond == first ^ 1)
        };
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
        let mut jump = Inst::new(Kind::Op(branch), vec![Operand::Block(join), Operand::Use(Reg::Phys(crate::reg::FLAGS))]);
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
            && matches!(block.insts.last().map(|inst| inst.kind), Some(Kind::Op("RET")))
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
                // falls through: no branch at all, or a conditional one that goes elsewhere
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
            "INC8" | "DEC8" | "SHL8" | "SRL8" | "SRA8" | "ADD8r" | "ADD8i" | "SUB8r" | "SUB8i" | "AND8r" | "AND8i" | "OR8r" | "OR8i" | "XOR8r"
                | "XOR8i"
        )
    )
}

/// `optimizeCompareInstr`: the test of a byte against zero (`cp 0`) after the
/// instruction that made the byte and set the flags by it is dropped when only
/// the zero flag is read: `dec c; ld a, c; or a; jp nz` is `dec c; jp nz`. Every
/// `ld` between leaves the flags alone.
fn eliminate_tests(function: &mut MFunction) {
    for block in &mut function.blocks {
        let mut at = 1;
        while at < block.insts.len() {
            let compare = &block.insts[at];
            let is_test = compare.kind == Kind::Op("CP8i")
                && matches!(compare.operands[..], [_, Operand::Use(Reg::Phys(a)), Operand::Imm(0)] if a == crate::reg::A);
            let copy = &block.insts[at - 1];
            let tested = match (&copy.kind, &copy.operands[..]) {
                (Kind::Copy, [Operand::Def(Reg::Phys(a)), Operand::Use(v)]) if *a == crate::reg::A => Some(*v),
                _ => None,
            };
            let (true, Some(value)) = (is_test, tested) else {
                at += 1;
                continue;
            };
            // the instruction that made `value`: it defines it, or copies it out of `a`
            let defines = |inst: &Inst, reg: Reg| inst.operands.contains(&Operand::Def(reg));
            let Some(made) = block.insts[..at - 1].iter().rposition(|inst| defines(inst, value)) else {
                at += 1;
                continue;
            };
            let setter = if block.insts[made].kind == Kind::Copy {
                // `v = COPY a` after an instruction that defines `a` as its result
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
