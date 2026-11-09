//! A select is a branch over the copy of its true arm, before allocation.
//!
//! `CMOV d, f, t, cc` becomes `d = f; b!cc skip; (a block) d = t; skip:` (`d = f` is
//! left out when `d` is `f`, and `d = t` when `d` is `t`, the branch inverted).
//! The arm values are then copies in a block of their own: a value that is
//! already in a cell stays there, where the pseudo made the allocator stage
//! it in `x`/`y`/a cell ahead of the compare (`k_strrchr`: `ldy rc2; lda rc3;
//! sta rc6`, with `pha`/`pla` around them, every trip, for a `last = s` that
//! is taken on a match only).
//!
//! The pin has no counterpart: x86 keeps `CMOV` as an instruction. llvm-mos
//! lowers `G_SELECT` to the same diamond before allocation
//! ([MOSLowerSelect.cpp](https://github.com/llvm-mos/llvm-mos/blob/a40207a371b60ff787ece46817d9c0f62eada719/llvm/lib/Target/MOS/MOSLowerSelect.cpp#L134))
//! and sinks the arm's defining instructions into it.
//!
//! DEVIATE 6502-SELECT-DIAMOND (owner 6502, exit 2026-11-30): not translated
//! are `SinkValue` (an arm computed only for the select stays ahead of the
//! branch) and the merge of selects on one test. Selects whose result the
//! preceding compare reads stay `CMOV`.

use crate::dagisel::generated::ids::opcode as op;
use crate::emit::{MInst, Operand};
use crate::select::Item;

const BRANCHES: [u16; 6] = [op::BEQ, op::BNE, op::BCC, op::BCS, op::BMI, op::BPL];

/// `items` with every select a diamond.
pub fn lower(items: &[Item]) -> Vec<Item> {
    // the A of an A/B, as `LLRM_6502_NO_ROUND2` is
    if std::env::var_os("LLRM_6502_NO_SELECT_DIAMOND").is_some() {
        return items.to_vec();
    }
    let mut out = Vec::with_capacity(items.len());
    let mut labels = 0;
    for (at, item) in items.iter().enumerate() {
        let Item::Inst(inst) = item else {
            out.push(item.clone());
            continue;
        };
        let word = match inst.opcode {
            op::CMOV8 => false,
            op::CMOV16 => true,
            _ => {
                out.push(item.clone());
                continue;
            }
        };
        let Some(diamond) = diamond(items, at, inst, word, &mut labels) else {
            out.push(item.clone());
            continue;
        };
        out.extend(diamond);
    }
    out
}

fn reg_name(operand: &Operand) -> Option<&str> {
    match operand {
        Operand::Reg(name) => Some(name),
        _ => None,
    }
}

fn diamond(
    items: &[Item],
    at: usize,
    inst: &MInst,
    word: bool,
    labels: &mut usize,
) -> Option<Vec<Item>> {
    let o = &inst.operands;
    let (d, f, t) = (reg_name(o.first()?)?, reg_name(o.get(1)?)?, reg_name(o.get(2)?)?);
    let Operand::Imm(code) = o.get(3)? else { return None };
    let code = usize::try_from(*code).ok().filter(|code| *code < BRANCHES.len())?;
    if reads_compare(items, at, d) {
        return None;
    }
    let copy = if word { op::COPY16 } else { op::COPY8 };
    let make = |opcode: u16, operands: Vec<Operand>| {
        let mut made = MInst::new(opcode, operands);
        made.origin = inst.origin;
        Item::Inst(made)
    };
    let to = |source: &str| make(copy, vec![Operand::Reg(d.to_owned()), Operand::Reg(source.to_owned())]);
    *labels += 1;
    let skip = format!("_sd{labels}");
    let mut out = Vec::new();
    match (d == f, d == t) {
        (true, true) => return Some(out),
        // taken: d already holds the true arm
        (false, true) => {
            out.push(make(BRANCHES[code], vec![Operand::Label(skip.clone())]));
            out.push(Item::Label(format!("{skip}a")));
            out.push(to(f));
        }
        _ => {
            if d != f {
                out.push(to(f));
            }
            out.push(make(BRANCHES[code ^ 1], vec![Operand::Label(skip.clone())]));
            out.push(Item::Label(format!("{skip}a")));
            out.push(to(t));
        }
    }
    out.push(Item::Label(skip));
    Some(out)
}

/// Whether the compare whose flags the select reads names `d`: writing `d`
/// before the branch would change what the compare is redone from.
fn reads_compare(
    items: &[Item],
    at: usize,
    d: &str,
) -> bool {
    for item in items[..at].iter().rev() {
        let Item::Inst(inst) = item else { return true };
        if matches!(inst.opcode, op::CMP8 | op::CMP16 | op::CMPZ16 | op::CMP32 | op::CMP8M | op::CMP16M) {
            return inst.operands.iter().any(|operand| reg_name(operand) == Some(d));
        }
    }
    true
}
