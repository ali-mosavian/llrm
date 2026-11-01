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
    allocated_with(ir, name, request, true)
}

fn allocated_with(
    ir: &str,
    name: &str,
    request: &Request,
    duplicate: bool,
) -> Result<(MFunction, Assignment, Vec<(i32, usize)>), String> {
    let mut function = machine_function_with(ir, name, request, duplicate)?;
    eliminate_phis_with(&mut function, false);
    two_address(&mut function);
    if std::env::var("LLRM_Z80_DUMP").is_ok_and(|one| one == name) {
        dump(&function);
    }
    let (assignment, slots) = allocate_greedy::<Z80Registers>(&mut function, Regs(&REGISTER_INFO), false)?;
    remove_identity_copies(&mut function, &assignment);
    llrm_spike_mf::branch_fold_hoisting::<Z80Registers>(&mut function, &assignment);
    return_conditionally(&mut function);
    let size = |class| if Z80Registers::bytes_of(class) == 1 { 1 } else { 2 };
    Ok((function, assignment, slots.into_iter().map(|(slot, class)| (slot, size(class))).collect()))
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
        let (function, assignment, slots) = allocated_with(ir, name, request, duplicate)?;
        finish(&function, &assignment, number, &slots)
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

/// `EmitLoweredSelect`: a run of `CMOV` pseudos on one condition (or its
/// opposite) becomes a diamond. The block jumps to the join on the condition;
/// the empty block it falls through to is the other path; the join holds a phi
/// per `CMOV` and what followed the run. A later `CMOV` that reads an earlier
/// one's result is renamed to the value that result has on each path.
///
/// The arms are empty, and `ld` leaves the flags alone, so a copy the phis
/// become does not disturb a later reader of the compare.
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
        let mut tail = function.blocks[at].insts.split_off(p);
        let (_, _, _, first) = cmov_parts(&tail[0])?;
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
        function.blocks.insert(at + 1, Block { id: otherwise, insts: Vec::new(), phis: Vec::new() });
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
