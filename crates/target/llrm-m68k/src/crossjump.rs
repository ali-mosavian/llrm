//! gcc `cfgcleanup.c` `try_crossjump_bb` (`-fcrossjumping`, on at `-Os`): two
//! blocks that end the same way and run the same last instructions share
//! them, the one jumping into the other. It runs after the epilogue is made,
//! so the closing `movem; rts` the returns repeat is one the others jump to.

use crate::expand::Line;
use crate::mc::{self, Operand};

/// The most blocks compared: `max-crossjump-edges`.
const MAX_BLOCKS: usize = 100;
/// Fewest instructions a shared tail has: `min-crossjump-insns`.
const MIN_INSNS: usize = 2;

fn block_label(
    inst: &mc::Inst,
    prefix: &str,
) -> bool {
    inst.operands.iter().any(|operand| matches!(operand, Operand::Symbol(name, _) if name.starts_with(prefix)))
}

/// A jump that does not fall through, to a block or out of the function.
fn exits(inst: &mc::Inst) -> bool {
    matches!(inst.opcode, "RTS" | "BRA8" | "BRA16")
}

/// An instruction that ends a block without being an exit, or reads where a
/// block begins: a tail never reaches back over it.
fn breaks(
    inst: &mc::Inst,
    prefix: &str,
) -> bool {
    if exits(inst) {
        return false;
    }
    block_label(inst, prefix) || matches!(inst.opcode, "RTE" | "RTR" | "JMP32j" | "JMP32p" | "TRAP")
}

/// The lines of an exit block's instructions, in order: from the last label
/// or break to its jump.
struct Tail {
    insts: Vec<usize>,
}

fn blocks(
    lines: &[Line],
    prefix: &str,
) -> Vec<Tail> {
    let mut out = Vec::new();
    let mut insts: Vec<usize> = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        match line {
            Line::Label(_) => insts.clear(),
            Line::Loc(_) => {}
            Line::Inst(inst) => {
                if exits(inst) {
                    insts.push(at);
                    out.push(Tail { insts: std::mem::take(&mut insts) });
                } else if breaks(inst, prefix) {
                    insts.clear();
                } else {
                    insts.push(at);
                }
            }
        }
    }
    out
}

fn inst_at(
    lines: &[Line],
    at: usize,
) -> &mc::Inst {
    match &lines[at] {
        Line::Inst(inst) => inst,
        _ => unreachable!("an instruction index"),
    }
}

/// `flow_find_cross_jump`: how many instructions the two blocks end in alike.
fn common(
    lines: &[Line],
    one: &Tail,
    other: &Tail,
) -> usize {
    one.insts
        .iter()
        .rev()
        .zip(other.insts.iter().rev())
        .take_while(|(a, b)| inst_at(lines, **a) == inst_at(lines, **b))
        .count()
}

/// `lines` with the common tails of exit blocks shared. `bytes` prices an
/// instruction; `number` and `fresh` name the new labels (`fresh` is the first
/// block index no label uses).
pub fn cross_jump(
    mut lines: Vec<Line>,
    number: usize,
    mut fresh: usize,
    bytes: &dyn Fn(&mc::Inst) -> usize,
) -> Vec<Line> {
    let prefix = format!(".LBB{number}_");
    loop {
        let found = blocks(&lines, &prefix);
        if found.len() < 2 || found.len() > MAX_BLOCKS {
            return lines;
        }
        // the pair sharing the most bytes
        let mut best: Option<(usize, usize, usize, usize)> = None;
        for (x, one) in found.iter().enumerate() {
            for (y, other) in found.iter().enumerate().skip(x + 1) {
                let length = common(&lines, one, other);
                if length < MIN_INSNS {
                    continue;
                }
                let tail: usize =
                    one.insts[one.insts.len() - length..].iter().map(|&at| bytes(inst_at(&lines, at))).sum();
                // a `bra.w` is the dearest jump the merged block gets
                if tail <= 4 {
                    continue;
                }
                if best.is_none_or(|(_, _, _, most)| tail > most) {
                    best = Some((x, y, length, tail));
                }
            }
        }
        let Some((x, y, length, _)) = best else { return lines };
        let (keep, cut) = (&found[x], &found[y]);
        let keep_from = keep.insts[keep.insts.len() - length];
        let cut_from = cut.insts[cut.insts.len() - length];
        let cut_to = *cut.insts.last().expect("an exit");
        let label = fresh;
        fresh += 1;
        // the later lines first, so the earlier indices hold
        let mut edits = vec![
            (
                cut_from,
                cut_to + 1,
                Some(vec![Line::Inst(mc::Inst {
                    opcode: "BRA8",
                    operands: vec![Operand::Symbol(format!("{prefix}{label}"), 0)],
                })]),
            ),
            (keep_from, keep_from, Some(vec![Line::Label(label)])),
        ];
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
        for (from, to, with) in edits {
            lines.splice(from..to, with.unwrap_or_default());
        }
        // a jump to the line after it goes nowhere
        lines = drop_jumps_to_next(lines, &prefix);
    }
}

fn drop_jumps_to_next(
    lines: Vec<Line>,
    prefix: &str,
) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    let mut at = 0;
    while at < lines.len() {
        if let Line::Inst(inst) = &lines[at]
            && inst.opcode == "BRA8"
            && let [Operand::Symbol(name, 0)] = &inst.operands[..]
            && let Some(label) = name.strip_prefix(prefix).and_then(|rest| rest.parse::<usize>().ok())
        {
            let mut next = at + 1;
            while matches!(lines.get(next), Some(Line::Loc(_))) {
                next += 1;
            }
            if matches!(lines.get(next), Some(Line::Label(shown)) if *shown == label) {
                at += 1;
                continue;
            }
        }
        out.push(lines[at].clone());
        at += 1;
    }
    out
}
