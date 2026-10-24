//! Pre-allocation LIR as LLVM MIR, so the pin's `llc -run-pass=greedy`
//! allocates the same function (a like-for-like check of quality and cost).
//!
//! # How it works
//! `mir` prints a body whose instructions are the 32-bit integer forms of
//! `mov`, `lea`, `add`, `sub`, `and`, `or`, `xor`, `neg`, `sar`, `shl`, `shr`,
//! `cmp`, `test` and the jumps, with registers, immediates, frame arguments and
//! indexed cells of a data segment. A value is a `gr32` virtual register; a
//! `requires` or `delivers` entry is a `COPY` from or to its physical register;
//! the data segment is a global array `@seg<n>`; the first incoming stack
//! argument is `%fixed-stack.0`. Anything else is refused with its name.
//!
//! # Example
//! `v1 := mov [bp+8]; cmp v1, 0; jg` becomes
//! `%1 = MOV32rm %fixed-stack.0, 1, $noreg, 0, $noreg`,
//! `CMP32ri %1, 0, implicit-def $eflags` and `JCC_1 %bb.2, 15, implicit
//! $eflags`.
//!
//! # Used by
//! `LLRM_MIR_PRINT=<function>` in the greedy shadow run;
//! `tools/mirprint/check.py`.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use llrm_lir::{Held, Imm, Loc, Mem, Space};

use crate::model::lir::{Insn, LirBody};

thread_local! {
    /// The widest each value is named at, for the body being printed.
    static WIDEST: std::cell::RefCell<std::collections::BTreeMap<u32, u32>> = Default::default();
}

/// A value as an operand: whole, or its low 16 or 8 bits where it is read
/// narrower than it is held.
fn vreg(held: &Held) -> Result<String, String> {
    let widest = WIDEST.with(|w| w.borrow().get(&held.value).copied()).unwrap_or(held.width);
    Ok(match (widest, held.width) {
        (wide, narrow) if wide == narrow => format!("%{}", held.value),
        (4, 2) => format!("%{}.sub_16bit", held.value),
        (4 | 2, 1) => format!("%{}.sub_8bit", held.value),
        (wide, narrow) => return Err(format!("value {} read at {narrow} bytes of {wide}", held.value)),
    })
}

/// The opcode suffix of an operand of `width` bytes.
fn bits(width: u32) -> Result<&'static str, String> {
    match width {
        1 => Ok("8"),
        2 => Ok("16"),
        4 => Ok("32"),
        other => Err(format!("a {other}-byte operand")),
    }
}

/// The widest each value is named at.
fn widths_of(body: &LirBody) -> Result<std::collections::BTreeMap<u32, u32>, String> {
    let mut out = std::collections::BTreeMap::new();
    for one in body.insns().iter().filter_map(|one| one.what.as_ref()) {
        for held in one.dests.iter().chain(&one.sources).flat_map(llrm_lir::values) {
            let wide = out.entry(held.value).or_insert(held.width);
            *wide = (*wide).max(held.width);
        }
    }
    Ok(out)
}

/// The five address operands of an x86 memory reference.
fn cell(
    mem: &Mem,
    symbols: &mut BTreeSet<i64>,
) -> Result<String, String> {
    let index = match &mem.index {
        Some(held) => (mem.scale, vreg(held)?),
        None => (1, "$noreg".to_owned()),
    };
    let base = match &mem.base {
        Some(held) => vreg(held)?,
        None => "$noreg".to_owned(),
    };
    let addr = mem.addr.as_ref();
    match addr.map(|a| a.space) {
        Some(Space::Frame) if mem.base.is_none() && mem.index.is_none() => {
            let at = addr.map_or(0, |a| a.disp) + mem.offset - 8;
            if at < 0 {
                return Err("a local frame cell".to_owned());
            }
            Ok(format!("%fixed-stack.{}, 1, $noreg, {}, $noreg", at / 4, at % 4))
        }
        Some(Space::Segment) => {
            let a = addr.expect("an address");
            symbols.insert(a.index);
            Ok(format!("{base}, {}, {}, @seg{} + {}, $noreg", index.0, index.1, a.index, a.disp + mem.offset))
        }
        None => Ok(format!("{base}, {}, {}, {}, $noreg", index.0, index.1, mem.offset)),
        Some(Space::Literal) => {
            let a = addr.expect("an address");
            Ok(format!("{base}, {}, {}, {}, $noreg", index.0, index.1, a.disp + mem.offset))
        }
        Some(other) => Err(format!("an address in {other:?}")),
    }
}

fn immediate(imm: &Imm) -> Result<i64, String> {
    if imm.address.is_some() {
        return Err("a symbolic immediate".to_owned());
    }
    Ok(imm.value)
}

/// The address a `mov` loads: a data segment symbol and its displacement.
fn symbolic(
    imm: &Imm,
    symbols: &mut BTreeSet<i64>,
) -> Result<String, String> {
    let Some(addr) = imm.address.as_ref().filter(|a| a.space == Space::Segment) else {
        return Err("an immediate address outside the data segment".to_owned());
    };
    symbols.insert(addr.index);
    Ok(format!("@seg{} + {}", addr.index, addr.disp + imm.value))
}

fn condition(name: &str) -> Option<u8> {
    Some(match name {
        "jo" => 0,
        "jno" => 1,
        "jb" => 2,
        "jae" => 3,
        "je" => 4,
        "jne" => 5,
        "jbe" => 6,
        "ja" => 7,
        "js" => 8,
        "jns" => 9,
        "jl" => 12,
        "jge" => 13,
        "jle" => 14,
        "jg" => 15,
        _ => return None,
    })
}

/// One instruction as MIR lines.
fn instruction(
    one: &Insn,
    targets: &[usize],
    jumps: &mut usize,
    symbols: &mut BTreeSet<i64>,
    out: &mut String,
) -> Result<(), String> {
    for (held, register) in &one.requires {
        writeln!(out, "    ${} = COPY {}", format!("{register:?}").to_lowercase(), vreg(held)?).unwrap();
    }
    let Some(what) = &one.what else {
        for (held, register) in &one.delivers {
            writeln!(out, "    {} = COPY ${}", vreg(held)?, format!("{register:?}").to_lowercase()).unwrap();
        }
        return Ok(());
    };
    let name = what.name.as_deref().unwrap_or("");
    let eflags = "implicit-def dead $eflags";
    let dest = what.dests.first();
    match (name, dest, what.sources.as_slice()) {
        ("", None, []) => {
            let used: Vec<String> =
                one.requires.iter().map(|(_, r)| format!("implicit ${}", format!("{r:?}").to_lowercase())).collect();
            writeln!(out, "    RET 0{}", used.iter().map(|u| format!(", {u}")).collect::<String>()).unwrap();
        }
        ("jmp", None, []) => {
            let to = targets.get(*jumps).or(targets.last()).ok_or("a jump without a successor")?;
            writeln!(out, "    JMP_1 %bb.{to}").unwrap();
        }
        (jump, None, []) if condition(jump).is_some() => {
            let taken = targets.first().ok_or("a branch without a successor")?;
            *jumps = 1;
            writeln!(out, "    JCC_1 %bb.{taken}, {}, implicit $eflags", condition(jump).unwrap()).unwrap();
        }
        ("mov", Some(Loc::Held(d)), [Loc::Held(s)]) => writeln!(out, "    {} = COPY {}", vreg(d)?, vreg(s)?).unwrap(),
        ("mov", Some(Loc::Held(d)), [Loc::Imm(i)]) if i.address.is_some() => {
            writeln!(out, "    {} = MOV32ri {}", vreg(d)?, symbolic(i, symbols)?).unwrap()
        }
        ("mov", Some(Loc::Held(d)), [Loc::Imm(i)]) => {
            writeln!(out, "    {} = MOV{}ri {}", vreg(d)?, bits(d.width)?, immediate(i)?).unwrap()
        }
        ("mov", Some(Loc::Held(d)), [Loc::Mem(m)]) => {
            writeln!(out, "    {} = MOV{}rm {}", vreg(d)?, bits(d.width)?, cell(m, symbols)?).unwrap()
        }
        ("mov", Some(Loc::Mem(m)), [Loc::Held(s)]) => {
            writeln!(out, "    MOV{}mr {}, {}", bits(s.width)?, cell(m, symbols)?, vreg(s)?).unwrap()
        }
        ("mov", Some(Loc::Mem(m)), [Loc::Imm(i)]) => {
            writeln!(out, "    MOV{}mi {}, {}", bits(m.width)?, cell(m, symbols)?, immediate(i)?).unwrap()
        }
        ("lea", Some(Loc::Held(d)), [Loc::Mem(m)]) => {
            writeln!(out, "    {} = LEA{}r {}", vreg(d)?, bits(d.width)?, cell(m, symbols)?).unwrap()
        }
        ("neg", Some(Loc::Held(d)), [Loc::Held(s)]) => {
            writeln!(out, "    {} = NEG{}r {}, {eflags}", vreg(d)?, bits(d.width)?, vreg(s)?).unwrap()
        }
        (op @ ("movsx" | "movzx"), Some(Loc::Held(d)), [source]) => {
            let (from, operand) = match source {
                Loc::Held(s) => (bits(s.width)?, ("rr", vreg(s)?)),
                Loc::Mem(m) => (bits(m.width)?, ("rm", cell(m, symbols)?)),
                _ => return Err(format!("{op} of that source")),
            };
            writeln!(out, "    {} = {}{}{}{from} {}", vreg(d)?, op.to_uppercase(), bits(d.width)?, operand.0, operand.1)
                .unwrap()
        }
        ("cmp" | "test", None, [Loc::Held(a), Loc::Held(b)]) => {
            let op = if name == "cmp" { "CMP" } else { "TEST" };
            writeln!(out, "    {op}{}rr {}, {}, implicit-def $eflags", bits(a.width)?, vreg(a)?, vreg(b)?).unwrap()
        }
        ("cmp", None, [Loc::Held(a), Loc::Imm(i)]) => {
            writeln!(out, "    CMP{}ri {}, {}, implicit-def $eflags", bits(a.width)?, vreg(a)?, immediate(i)?).unwrap()
        }
        (op @ ("add" | "sub" | "and" | "or" | "xor"), Some(Loc::Held(d)), [Loc::Held(a), rhs]) => {
            let upper = op.to_uppercase();
            let (form, right) = match rhs {
                Loc::Held(b) => ("rr", vreg(b)?),
                Loc::Imm(i) => ("ri", immediate(i)?.to_string()),
                Loc::Mem(m) => ("rm", cell(m, symbols)?),
                _ => return Err(format!("{op} with that source")),
            };
            writeln!(out, "    {} = {upper}{}{form} {}, {right}, {eflags}", vreg(d)?, bits(d.width)?, vreg(a)?).unwrap()
        }
        (op @ ("sar" | "shl" | "shr"), Some(Loc::Held(d)), [Loc::Held(a), Loc::Imm(i)]) => writeln!(
            out,
            "    {} = {}{}ri {}, {}, {eflags}",
            vreg(d)?,
            op.to_uppercase(),
            bits(d.width)?,
            vreg(a)?,
            immediate(i)?
        )
        .unwrap(),
        _ => return Err(format!("`{name}` with {} destinations and {} sources", what.dests.len(), what.sources.len())),
    }
    Ok(())
}

/// `body` as a MIR module of one function, or the first thing this printer
/// does not know.
pub fn mir(body: &LirBody) -> Result<String, String> {
    let at: Vec<i64> = body.blocks.iter().map(|b| b.at).collect();
    let odds = crate::liveness::blockfrequency::MachineBranchProbabilityInfo::of(body);
    let mut symbols = BTreeSet::new();
    let mut text = String::new();
    let mut widths = BTreeSet::new();
    let mut indexed = BTreeSet::new();
    let value_widths = widths_of(body)?;
    WIDEST.with(|w| *w.borrow_mut() = value_widths.clone());
    for (n, block) in body.blocks.iter().enumerate() {
        let targets: Vec<usize> = block
            .succ
            .iter()
            .map(|to| at.iter().position(|a| a == to).ok_or_else(|| format!("successor {to} of block {n}")))
            .collect::<Result<_, _>>()?;
        writeln!(text, "  bb.{n}:").unwrap();
        if !targets.is_empty() {
            // The odds llrm's own frequencies use, so the pin weighs the same
            // blocks the same way.
            let mut distinct: Vec<usize> = Vec::new();
            for to in &targets {
                if !distinct.contains(to) {
                    distinct.push(to.to_owned());
                }
            }
            let list: Vec<String> = distinct
                .iter()
                .enumerate()
                .map(|(k, t)| format!("%bb.{t}(0x{:08x})", odds.get_edge_probability(n, k).get_numerator()))
                .collect();
            writeln!(text, "    successors: {}", list.join(", ")).unwrap();
        }
        let mut jumps = 0;
        for one in block.insns.iter() {
            widths.extend(one.defines.iter().chain(&one.uses).copied());
            instruction(one, &targets, &mut jumps, &mut symbols, &mut text)
                .map_err(|why| format!("block {n}: {why}"))?;
            for mem in one.what.iter().flat_map(|w| w.dests.iter().chain(&w.sources)).filter_map(|loc| match loc {
                Loc::Mem(mem) => Some(mem),
                _ => None,
            }) {
                indexed.extend(mem.base.iter().chain(&mem.index).map(|held| held.value));
            }
        }
        // A block that falls off its end reaches its successor by the next
        // block or by a jump MIR must spell.
        let ends = block.insns.last().and_then(|one| one.what.as_ref()).map(|w| w.name.as_deref().unwrap_or(""));
        let fell = match (jumps, targets.as_slice(), ends) {
            (1, [_, other], Some(last)) if condition(last).is_some() => Some(*other),
            (0, [only], Some(last)) if last != "jmp" && !last.is_empty() => Some(*only),
            _ => None,
        };
        if let Some(to) = fell.filter(|to| *to != n + 1) {
            writeln!(text, "    JMP_1 %bb.{to}").unwrap();
        }
    }
    let mut out = String::new();
    writeln!(out, "--- |").unwrap();
    for s in &symbols {
        writeln!(out, "  @seg{s} = global [65536 x i8] zeroinitializer").unwrap();
    }
    writeln!(out, "  define i32 @{}(i32 %a) {{\n    ret i32 0\n  }}\n...", body.name).unwrap();
    writeln!(out, "---\nname: {}\ntracksRegLiveness: true\nregisters:", body.name).unwrap();
    for v in &widths {
        let class = match (value_widths.get(v), indexed.contains(v)) {
            (Some(1), _) => "gr8",
            (Some(2), _) => "gr16",
            (_, true) => "gr32_nosp",
            _ => "gr32",
        };
        writeln!(out, "  - {{ id: {v}, class: {class} }}").unwrap();
    }
    writeln!(out, "fixedStack:\n  - {{ id: 0, offset: 0, size: 4, alignment: 4, isImmutable: true }}\nbody: |")
        .unwrap();
    out.push_str(&text);
    writeln!(out, "...").unwrap();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::{Held, Imm, Loc, Operation, Semantics};

    use super::mir;
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    fn insn(
        name: &str,
        op: Operation,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Arc<Insn> {
        let what =
            Semantics { name: Some(name.into()), dests: dests.clone(), sources: sources.clone(), ..Semantics::new(op) };
        let defines = dests.iter().filter_map(|l| if let Loc::Held(h) = l { Some(h.value) } else { None }).collect();
        let uses = sources.iter().filter_map(|l| if let Loc::Held(h) = l { Some(h.value) } else { None }).collect();
        Arc::new(Insn::new(0, None, Some(what), defines, uses))
    }

    /// A loop-shaped body prints as MIR the pin's parser takes: two-address
    /// `add`, an immediate compare, and a conditional jump whose fallthrough
    /// block is not the next one gets the `JMP_1` MIR needs (its absence was
    /// "CFG successors don't match" on every `jne` that skips a block).
    #[test]
    fn test_a_branch_to_a_far_block_gets_its_jump_and_forms_are_32_bit() {
        let imm = |value| Loc::Imm(Imm { value, width: 4, address: None });
        let start = insn("mov", Operation::Move, vec![held(1)], vec![imm(0)]);
        let step = insn("add", Operation::Binary, vec![held(1)], vec![held(1), imm(16)]);
        let cmp = insn("cmp", Operation::Compare, vec![], vec![held(1), imm(128)]);
        let jne = insn("jne", Operation::Branch, vec![], vec![]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![
            Block::from(LirBlock { succ: vec![1].into(), ..LirBlock::new(0, vec![start]) }),
            Block::from(LirBlock { succ: vec![1, 3].into(), ..LirBlock::new(1, vec![step, cmp, jne]) }),
            Block::from(LirBlock::new(2, vec![Arc::clone(&ret)])),
            Block::from(LirBlock::new(3, vec![ret])),
        ];
        let body = LirBody::new("loop", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body).expect("prints");
        assert!(text.contains("%1 = MOV32ri 0"), "{text}");
        assert!(text.contains("%1 = ADD32ri %1, 16, implicit-def dead $eflags"), "{text}");
        assert!(text.contains("CMP32ri %1, 128, implicit-def $eflags"), "{text}");
        assert!(
            text.contains("JCC_1 %bb.1, 5, implicit $eflags\n    JMP_1 %bb.3"),
            "the taken edge, then the far fallthrough: {text}"
        );
        assert!(
            text.contains("successors: %bb.1(0x"),
            "the edges carry llrm's odds; without them the pin weighs both 50/50: {text}"
        );
        assert!(mir(&LirBody::new("none", 0, vec![], IndexMap::default(), IndexMap::default())).is_ok());
    }
}
