//! A function from IR to sdasz80 text: the shared DAG stages, then the Z80
//! machine function, allocation, expansion and printing.

use llrm_spike_mf::regalloc::{Assignment, eliminate_phis_with, two_address};
use llrm_lir::registers::Regs;
use llrm_spike_mf::tolir::allocate_greedy;

use crate::driver::{Request, selections};
use crate::expand::{Line, finish};
use crate::mfunc::{Block, Inst, Kind, MFunction, Operand, Phi, Reg, emit};
use crate::regalloc::Z80Registers;
use crate::REGISTER_INFO;

/// The machine function after selection and emission, before allocation.
pub fn machine_function(
    ir: &str,
    name: &str,
    request: &Request,
) -> Result<MFunction, String> {
    let selected = selections(ir, request)?;
    let (_, selection) = selected.into_iter().find(|(one, _)| one == name).ok_or_else(|| format!("no function @{name}"))?;
    let mut function = emit(name, &selection?)?;
    expand_cmov(&mut function)?;
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
    let mut function = machine_function(ir, name, request)?;
    eliminate_phis_with(&mut function, false);
    two_address(&mut function);
    let (assignment, slots) = allocate_greedy::<Z80Registers>(&mut function, Regs(&REGISTER_INFO), false)?;
    remove_identity_copies(&mut function, &assignment);
    llrm_spike_mf::branch_fold_hoisting::<Z80Registers>(&mut function, &assignment);
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

/// The function `name` of `ir` as assembly lines.
pub fn lines(
    ir: &str,
    name: &str,
    number: usize,
    request: &Request,
) -> Result<Vec<Line>, String> {
    let (function, assignment, slots) = allocated(ir, name, request)?;
    finish(&function, &assignment, number, &slots)
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
