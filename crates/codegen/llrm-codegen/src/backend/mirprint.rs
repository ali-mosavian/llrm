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

use llrm_lir::registers::RegId;
use llrm_lir::{Held, Imm, Loc, Mem, Space};

use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

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

/// The name of `register` as `width` bytes of it: `ecx` is `cx` and `cl`.
fn physical(
    register: RegId,
    width: u32,
) -> Result<String, String> {
    let whole = format!("{register:?}").to_lowercase();
    match (width, whole.strip_prefix('e')) {
        (4, _) => Ok(whole),
        (2, Some(rest)) => Ok(rest.to_owned()),
        (1, Some(rest)) if rest.ends_with('x') => Ok(format!("{}l", &rest[..1])),
        _ => Err(format!("{width} bytes of {whole}")),
    }
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

/// The last `$eflags` definition of the block being printed is read by the
/// branch about to follow: it is not `dead`, which the pin's verifier refuses.
fn flags_are_read(text: &mut String) {
    let mut end = text.len();
    while end > 0 {
        let start = text[..end].rfind('\n').map_or(0, |at| at + 1);
        let line = &text[start..end];
        if line.starts_with("  bb.") {
            return;
        }
        if line.contains("implicit-def") && line.contains("$eflags") {
            let live = line.replace("implicit-def dead $eflags", "implicit-def $eflags");
            text.replace_range(start..end, &live);
            return;
        }
        end = start.saturating_sub(1);
    }
}

/// One instruction as MIR lines.
fn instruction(
    one: &Insn,
    targets: &[usize],
    jumps: &mut usize,
    symbols: &mut BTreeSet<i64>,
    callees: &mut BTreeSet<i64>,
    in_register: &BTreeSet<u32>,
    out: &mut String,
) -> Result<(), String> {
    for (held, register) in one.requires.iter().filter(|(held, _)| !in_register.contains(&held.value)) {
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
        ("call_frame_setup", None, [Loc::Imm(size), Loc::Imm(adjust), Loc::Imm(more)]) => writeln!(
            out,
            "    ADJCALLSTACKDOWN32 {}, {}, {}, implicit-def dead $esp, implicit-def dead $eflags, implicit-def dead $ssp, implicit $esp, implicit $ssp",
            immediate(size)?,
            immediate(adjust)?,
            immediate(more)?
        )
        .unwrap(),
        ("call_frame_destroy", None, [Loc::Imm(size), Loc::Imm(adjust)]) => writeln!(
            out,
            "    ADJCALLSTACKUP32 {}, {}, implicit-def dead $esp, implicit-def dead $eflags, implicit-def dead $ssp, implicit $esp, implicit $ssp",
            immediate(size)?,
            immediate(adjust)?
        )
        .unwrap(),
        ("push", None, [Loc::Held(s)]) if s.width == 4 => {
            writeln!(out, "    PUSH32r {}, implicit-def $esp, implicit $esp", vreg(s)?).unwrap()
        }
        ("push", None, [Loc::Imm(i)]) if i.address.is_none() => {
            writeln!(out, "    PUSH32i {}, implicit-def $esp, implicit $esp", immediate(i)?).unwrap()
        }
        ("call", None, []) if !what.indirect => {
            let callee = what.target.unwrap_or_default();
            callees.insert(callee);
            let used: String =
                one.requires.iter().map(|(_, r)| format!(", implicit ${}", format!("{r:?}").to_lowercase())).collect();
            let made: String = one
                .delivers
                .iter()
                .map(|(_, r)| format!(", implicit-def ${}", format!("{r:?}").to_lowercase()))
                .collect();
            writeln!(
                out,
                "    CALLpcrel32 @callee{callee}, csr_32, implicit $esp, implicit $ssp, implicit-def $esp, implicit-def $ssp{used}{made}"
            )
            .unwrap();
            for (held, register) in &one.delivers {
                writeln!(out, "    {} = COPY ${}", vreg(held)?, format!("{register:?}").to_lowercase()).unwrap();
            }
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
        (op @ ("neg" | "inc" | "dec" | "not"), Some(Loc::Held(d)), [Loc::Held(s)]) => {
            let flags = if op == "not" { "" } else { ", implicit-def dead $eflags" };
            writeln!(out, "    {} = {}{}r {}{flags}", vreg(d)?, op.to_uppercase(), bits(d.width)?, vreg(s)?).unwrap()
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
        ("imul", Some(Loc::Held(d)), [Loc::Held(a), Loc::Imm(i)]) if what.dests.len() == 1 => {
            writeln!(out, "    {} = IMUL{}rri {}, {}, {eflags}", vreg(d)?, bits(d.width)?, vreg(a)?, immediate(i)?)
                .unwrap()
        }
        ("imul", Some(Loc::Held(d)), [Loc::Held(a), Loc::Held(b)]) if what.dests.len() == 1 => {
            writeln!(out, "    {} = IMUL{}rr {}, {}, {eflags}", vreg(d)?, bits(d.width)?, vreg(a)?, vreg(b)?).unwrap()
        }
        (op @ ("sar" | "shl" | "shr"), Some(Loc::Held(d)), [Loc::Held(a), Loc::Held(_)]) => writeln!(
            out,
            "    {} = {}{}rCL {}, {eflags}, implicit $cl",
            vreg(d)?,
            op.to_uppercase(),
            bits(d.width)?,
            vreg(a)?
        )
        .unwrap(),
        _ => return Err(format!("`{name}` with {} destinations and {} sources", what.dests.len(), what.sources.len())),
    }
    Ok(())
}

/// `body` as a MIR module of one function, or the first thing this printer
/// does not know.
pub fn mir(
    body: &LirBody,
    pins: &IndexMap<u32, RegId>,
) -> Result<String, String> {
    let at: Vec<i64> = body.blocks.iter().map(|b| b.at).collect();
    let odds = crate::liveness::blockfrequency::MachineBranchProbabilityInfo::of(body);
    let mut symbols = BTreeSet::new();
    let mut callees = BTreeSet::new();
    let mut text = String::new();
    let mut widths = BTreeSet::new();
    let mut register_only = BTreeSet::new();
    let mut indexed = BTreeSet::new();
    let value_widths = widths_of(body)?;
    WIDEST.with(|w| *w.borrow_mut() = value_widths.clone());
    // A value read as its low byte needs a register that has one.
    let byte_read: BTreeSet<u32> = body
        .insns()
        .iter()
        .filter_map(|one| one.what.as_ref())
        .flat_map(|one| one.dests.iter().chain(&one.sources).flat_map(llrm_lir::values))
        .filter(|held| held.width == 1 && value_widths.get(&held.value).is_some_and(|wide| *wide > 1))
        .map(|held| held.value)
        .collect();
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
        // The entry's own nameless instruction that defines only values pinned
        // to registers, with others after it, is the arguments arriving in
        // them: live-ins copied out, not a return.
        let entry_arguments = (n == 0 && block.insns.len() > 1)
            .then(|| block.insns.first())
            .flatten()
            .filter(
                |one| one.what.as_ref().is_some_and(|w| {
                    w.name.as_deref().unwrap_or("").is_empty() && w.dests.is_empty() && w.sources.is_empty()
                }) && !one.defines.is_empty()
                    && one.defines.iter().all(|value| pins.contains_key(value)),
            );
        if let Some(arrive) = entry_arguments {
            let list: Vec<String> = arrive
                .defines
                .iter()
                .map(|value| {
                    physical(pins[value], value_widths.get(value).copied().unwrap_or(4)).map(|r| format!("${r}"))
                })
                .collect::<Result<_, _>>()?;
            writeln!(text, "    liveins: {}", list.join(", ")).unwrap();
        }
        let mut jumps = 0;
        for one in block.insns.iter() {
            widths.extend(one.defines.iter().chain(&one.uses).copied());
            if entry_arguments.is_some_and(|arrive| std::sync::Arc::ptr_eq(arrive, one)) {
                for value in &one.defines {
                    let register = physical(pins[value], value_widths.get(value).copied().unwrap_or(4))?;
                    writeln!(text, "    %{value} = COPY ${register}").unwrap();
                }
                continue;
            }
            // A copy into a pinned value is the pin's own `$cl = COPY
            // %k.sub_8bit`: the register is the destination, no
            // virtual register stands between.
            if let Some((dest, source)) =
                one.what.as_ref().and_then(|w| match (w.name.as_deref(), &w.dests[..], &w.sources[..]) {
                    (Some("mov"), [Loc::Held(d)], [Loc::Held(s)]) if pins.contains_key(&d.value) => Some((d, s)),
                    _ => None,
                })
            {
                let register = physical(pins[&dest.value], dest.width).map_err(|why| format!("block {n}: {why}"))?;
                writeln!(text, "    ${register} = COPY {}", vreg(source).map_err(|why| format!("block {n}: {why}"))?)
                    .unwrap();
                register_only.insert(dest.value);
                continue;
            }
            if one.what.as_ref().and_then(|w| w.name.as_deref()).is_some_and(|name| condition(name).is_some()) {
                flags_are_read(&mut text);
            }
            instruction(one, &targets, &mut jumps, &mut symbols, &mut callees, &register_only, &mut text)
                .map_err(|why| format!("block {n}: {why}"))?;
            // A value pinned to a register is that register from its
            // definition on: the `$cl = COPY` an instruction's implicit use
            // reads.
            for value in one.defines.iter().filter(|value| pins.contains_key(*value)) {
                let width = value_widths.get(value).copied().unwrap_or(4);
                let register = physical(pins[value], width).map_err(|why| format!("block {n}: {why}"))?;
                writeln!(text, "    ${register} = COPY %{value}").unwrap();
            }
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
    for callee in &callees {
        writeln!(out, "  declare void @callee{callee}()").unwrap();
    }
    writeln!(out, "  define i32 @{}(i32 %a) {{\n    ret i32 0\n  }}\n...", body.name).unwrap();
    let adjusts = if callees.is_empty() { "" } else { "frameInfo:\n  adjustsStack: true\n" };
    writeln!(out, "---\nname: {}\ntracksRegLiveness: true\n{adjusts}registers:", body.name).unwrap();
    for v in widths.iter().filter(|v| !register_only.contains(*v)) {
        let class = match (value_widths.get(v), indexed.contains(v), byte_read.contains(v)) {
            (Some(1), ..) => "gr8",
            (Some(2), ..) => "gr16",
            (_, _, true) => "gr32_abcd",
            (_, true, _) => "gr32_nosp",
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

    use llrm_lir::registers::RegId;
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
    /// A loop counter's `dec` sets the flags its `jne` reads. The printer wrote
    /// `implicit-def dead $eflags`, and the pin's `llc` refused the MIR ("Using
    /// an undefined physical register"), so no loop with a `dec; jne` could be
    /// allocated by the pin.
    #[test]
    fn test_a_flag_setting_op_the_branch_reads_is_not_dead() {
        let dec = insn("dec", Operation::Binary, vec![held(1)], vec![held(1)]);
        let jne = insn("jne", Operation::Branch, vec![], vec![]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![
            Block::from(LirBlock { succ: vec![0, 1].into(), ..LirBlock::new(0, vec![dec, jne]) }),
            Block::from(LirBlock::new(1, vec![ret])),
        ];
        let body = LirBody::new("count", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("DEC32r %1, implicit-def $eflags\n    JCC_1"), "{text}");
    }

    /// The widening `v8, v9 := imul v6, v7` has two destinations; the printer
    /// took the first and wrote `IMUL32rr` with two undefined registers, which
    /// the pin refused as a register of no class.
    #[test]
    fn test_a_widening_multiply_is_refused_not_misprinted() {
        let widening = insn("imul", Operation::Binary, vec![held(8), held(9)], vec![held(6), held(7)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![widening, ret]))];
        let body = LirBody::new("wide", 0, blocks, IndexMap::default(), IndexMap::default());
        let refused = mir(&body, &IndexMap::default()).expect_err("refused");
        assert!(refused.contains("imul"), "{refused}");
    }

    /// `place` of the queens row calls itself: the pin's `llc` refused the MIR
    /// of any function with a call or a register argument, so none of them
    /// could be allocated by the pin on llrm's input.
    #[test]
    fn test_a_register_argument_and_a_call_print_as_the_pin_reads_them() {
        let imm = |value| Loc::Imm(Imm { value, width: 4, address: None });
        let arrive = Arc::new(Insn { defines: vec![1], ..Insn::clone(&insn("", Operation::Return, vec![], vec![])) });
        let setup = insn("call_frame_setup", Operation::Move, vec![], vec![imm(4), imm(0), imm(0)]);
        let push = insn("push", Operation::Push, vec![], vec![held(1)]);
        let call = Arc::new(Insn {
            requires: vec![(Held { value: 1, width: 4 }, RegId::ECX)],
            delivers: vec![(Held { value: 2, width: 4 }, RegId::EAX)],
            ..Insn::clone(&insn("call", Operation::Call, vec![], vec![]))
        });
        let destroy = insn("call_frame_destroy", Operation::Move, vec![], vec![imm(4), imm(0)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![arrive, setup, push, call, destroy, ret]))];
        let body = LirBody::new("calls", 0, blocks, IndexMap::default(), IndexMap::default());
        let pins: IndexMap<u32, RegId> = [(1, RegId::ECX)].into_iter().collect();
        let text = mir(&body, &pins).expect("prints");
        assert!(text.contains("liveins: $ecx\n"), "{text}");
        assert!(text.contains("%1 = COPY $ecx"), "{text}");
        assert!(text.contains("ADJCALLSTACKDOWN32 4, 0, 0,"), "{text}");
        assert!(text.contains("PUSH32r %1,"), "{text}");
        assert!(text.contains("CALLpcrel32 @callee0, csr_32"), "{text}");
        assert!(text.contains("%2 = COPY $eax"), "{text}");
        assert!(text.contains("adjustsStack: true"), "{text}");
    }

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
        let text = mir(&body, &IndexMap::default()).expect("prints");
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
        assert!(
            mir(&LirBody::new("none", 0, vec![], IndexMap::default(), IndexMap::default()), &IndexMap::default())
                .is_ok()
        );
    }

    /// A copy into a pinned value was printed with a virtual register between
    /// the value and `$cl`; the pin's allocator then had no copy hint for the
    /// source and put an induction variable in `eax` with a `mov ecx, eax` per
    /// trip (x_cordic), where its own MIR is `$cl = COPY %k.sub_8bit`.
    #[test]
    fn a_copy_into_a_pinned_value_is_a_copy_into_its_register() {
        let k = Loc::Held(Held { value: 1, width: 4 });
        let count = Loc::Held(Held { value: 2, width: 4 });
        let start =
            insn("mov", Operation::Move, vec![held(1)], vec![Loc::Imm(Imm { value: 0, width: 4, address: None })]);
        let copy = insn("mov", Operation::Move, vec![count.clone()], vec![k]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let body = LirBody::new(
            "pinned",
            0,
            vec![Block::from(LirBlock::new(0, vec![start, copy, ret]))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let pins: IndexMap<u32, llrm_lir::registers::RegId> =
            [(2, llrm_lir::registers::RegId::ECX)].into_iter().collect();
        let text = mir(&body, &pins).expect("prints");
        assert!(text.contains("$ecx = COPY %1"), "{text}");
        assert!(!text.contains("%2"), "the pinned value stands between: {text}");
    }
}
