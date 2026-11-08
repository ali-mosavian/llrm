//! Branches, calls, returns, jump tables, system instructions and debug notes.

use super::integer::CONDS;
use super::*;
use crate::mfunc::Callee;

fn callee_loc(
    cx: &mut To,
    callee: &Callee,
) -> Result<Loc, Unmapped> {
    Ok(match callee {
        Callee::Symbol(name) => Loc::Address(cx.address(&Target::Symbol { name: name.clone(), addend: 0 })?),
        Callee::Register(register) => gpr_loc(*register, Width::X),
    })
}

fn callee_of(
    cx: &Back,
    loc: &Loc,
) -> Callee {
    match loc {
        Loc::Address(address) => match cx.target(address) {
            Target::Symbol { name, .. } => Callee::Symbol(name),
            other => panic!("a symbol callee, found {other:?}"),
        },
        register => Callee::Register(gpr_of(register)),
    }
}

pub(super) fn to_inst(
    cx: &mut To,
    inst: &Inst,
) -> Result<Option<Semantics>, Unmapped> {
    Ok(Some(match inst {
        Inst::Ret { rn } => semantics(Operation::Return, "RET", vec![], vec![gpr_loc(*rn, Width::X)], None),
        Inst::Branch { link: false, target } => {
            semantics(Operation::Jump, "B", vec![], vec![], Some(cx.block(target)?))
        }
        Inst::Branch { link: true, target } => {
            semantics(Operation::Call, "BL", vec![], vec![Loc::Address(cx.address(target)?)], None)
        }
        Inst::BranchCond { cond, target } => {
            semantics(Operation::Branch, "Bcc", vec![], vec![imm(position(&CONDS, *cond))], Some(cx.block(target)?))
        }
        Inst::CompareBranch { nonzero, width, rt, target } => semantics(
            Operation::Branch,
            format!("CB{}Z{}", if *nonzero { "N" } else { "" }, width_letter(*width)),
            vec![],
            vec![gpr_loc(*rt, *width)],
            Some(cx.block(target)?),
        ),
        Inst::TestBranch { nonzero, rt, bit, target } => semantics(
            Operation::Branch,
            if *nonzero { "TBNZ" } else { "TBZ" },
            vec![],
            vec![gpr_loc(*rt, if *bit >= 32 { Width::X } else { Width::W }), imm(i64::from(*bit))],
            Some(cx.block(target)?),
        ),
        Inst::BranchReg { link, rn } => {
            let mut what = semantics(
                if *link { Operation::Call } else { Operation::Jump },
                if *link { "BLR" } else { "BR" },
                vec![],
                vec![gpr_loc(*rn, Width::X)],
                None,
            );
            what.indirect = true;
            what
        }
        Inst::Svc { imm16 } => semantics(Operation::Escape, "SVC", vec![], vec![imm(i64::from(*imm16))], None),
        Inst::Brk { imm16 } => semantics(Operation::Escape, "BRK", vec![], vec![imm(i64::from(*imm16))], None),
        Inst::Nop => semantics(Operation::Nothing, "NOP", vec![], vec![], None),
        _ => return Ok(None),
    }))
}

pub(super) fn to_op(
    cx: &mut To,
    op: &Op,
) -> Result<Option<Semantics>, Unmapped> {
    let registers = |list: &[Reg]| -> Vec<Loc> { list.iter().map(|register| loc(*register, 64)).collect() };
    Ok(Some(match op {
        Op::Call { callee, uses, defs, tail } => {
            let mut sources = vec![callee_loc(cx, callee)?];
            sources.extend(registers(uses));
            let form = if matches!(callee, Callee::Symbol(_)) { "di" } else { "ri" };
            semantics(
                Operation::Call,
                format!("CALL{form}{}", if *tail { ".tail" } else { "" }),
                registers(defs),
                sources,
                None,
            )
        }
        Op::Return { uses, tail: None } => semantics(Operation::Return, "RET_ReallyLR", vec![], registers(uses), None),
        Op::Return { uses, tail: Some(callee) } => {
            let mut sources = vec![callee_loc(cx, callee)?];
            sources.extend(registers(uses));
            let form = if matches!(callee, Callee::Symbol(_)) { "di" } else { "ri" };
            semantics(Operation::Return, format!("TCRETURN{form}"), vec![], sources, None)
        }
        Op::FrameAddress { dst, slot, offset } => super::memory::frame_address(*dst, *slot, *offset),
        Op::JumpTable { dst, scratch, table, index, id, targets } => {
            let mut what = semantics(
                Operation::Jump,
                "JumpTableDest32",
                vec![gpr_loc(*dst, Width::X), gpr_loc(*scratch, Width::X)],
                vec![gpr_loc(*table, Width::X), gpr_loc(*index, Width::X)],
                None,
            );
            what.indirect = true;
            what.table_id = Some(i64::from(*id));
            what.table =
                targets.iter().map(|label| cx.block(&Target::Label(label.clone()))).collect::<Result<_, _>>()?;
            what
        }
        Op::DbgValue { .. } => semantics(Operation::Nothing, "DBG_VALUE", vec![], vec![], None),
        _ => return Ok(None),
    }))
}

pub(super) fn back(
    cx: &Back,
    what: &Semantics,
) -> Option<Op> {
    let name = what.name.as_deref()?;
    let s = &what.sources;
    let block = || what.target.map(|at| cx.block(at));
    let registers = |from: usize| -> Vec<Reg> { s[from..].iter().map(|loc| reg_of(loc, false)).collect() };
    let defs: Vec<Reg> = what.dests.iter().map(|loc| reg_of(loc, false)).collect();
    if let Some(form) = name.strip_prefix("CALL") {
        let (form, tail) = form.strip_suffix(".tail").map_or((form, false), |form| (form, true));
        return (form == "di" || form == "ri").then(|| Op::Call {
            callee: callee_of(cx, &s[0]),
            uses: registers(1),
            defs,
            tail,
        });
    }
    Some(match name {
        "RET_ReallyLR" => Op::Return { uses: registers(0), tail: None },
        "TCRETURNdi" | "TCRETURNri" => Op::Return { uses: registers(1), tail: Some(callee_of(cx, &s[0])) },
        "JumpTableDest32" => Op::JumpTable {
            dst: gpr_of(&what.dests[0]),
            scratch: gpr_of(&what.dests[1]),
            table: gpr_of(&s[0]),
            index: gpr_of(&s[1]),
            id: what.table_id? as u32,
            targets: what.table.iter().map(|&at| cx.template.blocks[at as usize].label.clone()).collect(),
        },
        "RET" => Op::I(Inst::Ret { rn: gpr_of(&s[0]) }),
        "B" => Op::I(Inst::Branch { link: false, target: block()? }),
        "BL" => Op::I(Inst::Branch { link: true, target: cx.target_of(&s[0]) }),
        "Bcc" => Op::I(Inst::BranchCond { cond: member(&CONDS, &s[0]), target: block()? }),
        "BR" => Op::I(Inst::BranchReg { link: false, rn: gpr_of(&s[0]) }),
        "BLR" => Op::I(Inst::BranchReg { link: true, rn: gpr_of(&s[0]) }),
        "TBZ" | "TBNZ" => Op::I(Inst::TestBranch {
            nonzero: name == "TBNZ",
            rt: gpr_of(&s[0]),
            bit: value_of(&s[1]) as u8,
            target: block()?,
        }),
        "SVC" => Op::I(Inst::Svc { imm16: value_of(&s[0]) as u16 }),
        "BRK" => Op::I(Inst::Brk { imm16: value_of(&s[0]) as u16 }),
        "NOP" => Op::I(Inst::Nop),
        _ if name.starts_with("CB") => {
            let (_, letter, _) = split(name);
            Op::I(Inst::CompareBranch {
                nonzero: name.starts_with("CBN"),
                width: width_named(letter),
                rt: gpr_of(&s[0]),
                target: block()?,
            })
        }
        _ => return None,
    })
}
