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
//! A body with quadword values or a 64-bit register is 64-bit code: `gr64` values, `$rdi`..`$r15`, `ADJCALLSTACKDOWN64`,
//! `CALL64pcrel32`, `ri32` immediates, `CMOV64rr`, eight-byte stack slots. Not yet: a 32-bit write into a quadword value
//! (`AND32ri` on `%1.sub_32bit`) and `movzx r64, r32`.
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

use llrm_codegen::model::lir::{Insn, LirBody};
use llrm_codegen::support::hash::IndexMap;
use llrm_lir::registers::RegId;
use llrm_lir::{Held, Imm, Loc, Mem, Space};

thread_local! {
    /// Whether the body being printed is 64-bit code: quadword values, `rsp`,
    /// REX registers, `gr64`.
    static LONG: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The widest each value is named at, for the body being printed.
    static WIDEST: std::cell::RefCell<std::collections::BTreeMap<u32, u32>> = Default::default();
    /// The displacement of the first incoming argument from BP: the saved
    /// frame register and the return address (8 near in m32, 4 near in m16).
    static ARGUMENTS_AT: std::cell::Cell<i64> = const { std::cell::Cell::new(8) };
    /// The local frame objects of the body being printed, by the slot they
    /// were laid out in: first byte and one past the last, in BP-relative
    /// displacements. Their position in this list is the `%stack.N`.
    static JUMP_TABLES: std::cell::RefCell<Vec<Vec<usize>>> = const { std::cell::RefCell::new(Vec::new()) };
    static LOCALS: std::cell::RefCell<Vec<(i64, i64, i64)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A value as an operand: whole, or its low 16 or 8 bits where it is read
/// narrower than it is held.
fn vreg(held: &Held) -> Result<String, String> {
    let widest = WIDEST.with(|w| w.borrow().get(&held.value).copied()).unwrap_or(held.width);
    Ok(match (widest, held.width) {
        (wide, narrow) if wide == narrow => format!("%{}", held.value),
        (8, 4) => format!("%{}.sub_32bit", held.value),
        (8 | 4, 2) => format!("%{}.sub_16bit", held.value),
        (8 | 4 | 2, 1) => format!("%{}.sub_8bit", held.value),
        (wide, narrow) => return Err(format!("value {} read at {narrow} bytes of {wide}", held.value)),
    })
}

/// The name of `register` as `width` bytes of it: `ecx` is `cx` and `cl`.
fn physical(
    register: RegId,
    width: u32,
) -> Result<String, String> {
    let whole = format!("{register:?}").to_lowercase();
    if let Some(family) = quadword_family(&whole) {
        return Ok(family[match width {
            8 => 0,
            4 => 1,
            2 => 2,
            1 => 3,
            _ => return Err(format!("{width} bytes of {whole}")),
        }]
        .to_owned());
    }
    match (width, whole.strip_prefix('e')) {
        (4, _) => Ok(whole),
        // `es` is a segment register, not the low half of an `e` one.
        (2, _) if matches!(whole.as_str(), "es" | "cs" | "ss" | "ds" | "fs" | "gs") => Ok(whole),
        // The m32 table also names the 16- and 8-bit registers.
        (2, None) if matches!(
            whole.as_str(),
            "ax" | "cx" | "dx" | "bx" | "si" | "di" | "bp" | "sp"
        ) => Ok(whole),
        (1, None) if whole.len() == 2 && matches!(&whole[1..], "l" | "h") => Ok(whole),
        (2, Some(rest)) => Ok(rest.to_owned()),
        (1, Some(rest)) if rest.ends_with('x') => Ok(format!("{}l", &rest[..1])),
        _ => Err(format!("{width} bytes of {whole}")),
    }
}

/// The names of a 64-bit register as 8, 4, 2 and 1 bytes: `rdi`, `edi`, `di`,
/// `dil`; None for any other register.
fn quadword_family(name: &str) -> Option<[&'static str; 4]> {
    const FAMILIES: [[&str; 4]; 16] = [
        ["rax", "eax", "ax", "al"],
        ["rcx", "ecx", "cx", "cl"],
        ["rdx", "edx", "dx", "dl"],
        ["rbx", "ebx", "bx", "bl"],
        ["rsp", "esp", "sp", "spl"],
        ["rbp", "ebp", "bp", "bpl"],
        ["rsi", "esi", "si", "sil"],
        ["rdi", "edi", "di", "dil"],
        ["r8", "r8d", "r8w", "r8b"],
        ["r9", "r9d", "r9w", "r9b"],
        ["r10", "r10d", "r10w", "r10b"],
        ["r11", "r11d", "r11w", "r11b"],
        ["r12", "r12d", "r12w", "r12b"],
        ["r13", "r13d", "r13w", "r13b"],
        ["r14", "r14d", "r14w", "r14b"],
        ["r15", "r15d", "r15w", "r15b"],
    ];
    FAMILIES.iter().find(|family| family[0] == name).copied()
}

/// `register` as `width` bytes of it, in the names of the pin's MIR (`$rdi`).
fn named(
    register: RegId,
    width: u32,
) -> String {
    physical(register, width).unwrap_or_else(|_| format!("{register:?}").to_lowercase())
}

/// A physical register as an instruction's operand or implicit use: the name
/// `width` bytes of it have in 64-bit code, the whole register as the 32-bit
/// forms always printed it.
fn copy_register(
    register: RegId,
    width: u32,
) -> String {
    if LONG.with(std::cell::Cell::get) { named(register, width) } else { format!("{register:?}").to_lowercase() }
}

/// The stack pointer, the register `rsp` or `esp` the frame instructions name.
fn stack_pointer() -> &'static str {
    if LONG.with(std::cell::Cell::get) { "rsp" } else { "esp" }
}

/// An immediate form of a quadword instruction takes a sign-extended 32 bits:
/// `ADD64ri32`, `CMP64mi32`, `IMUL64rri32`.
fn imm(
    form: &str,
    width: u32,
) -> &'static str {
    match (form, width == 8) {
        ("ri", true) => "ri32",
        ("ri", false) => "ri",
        ("mi", true) => "mi32",
        ("mi", false) => "mi",
        ("rri", true) => "rri32",
        ("rri", false) => "rri",
        _ => unreachable!("an immediate form"),
    }
}

/// The opcode suffix of an operand of `width` bytes.
fn bits(width: u32) -> Result<&'static str, String> {
    match width {
        1 => Ok("8"),
        2 => Ok("16"),
        4 => Ok("32"),
        8 => Ok("64"),
        other => Err(format!("a {other}-byte operand")),
    }
}

/// The widest each value is named at.
fn widths_of(body: &LirBody) -> Result<std::collections::BTreeMap<u32, u32>, String> {
    let mut out = std::collections::BTreeMap::new();
    for one in body.iter_insns().filter_map(|one| one.what.as_ref()) {
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
        Some(Space::Frame) if mem.base.is_none() => {
            let disp = addr.map_or(0, |a| a.disp) + mem.offset;
            if disp < 0 || addr.and_then(|a| a.slot_home()).is_some() {
                let home = addr.and_then(|a| a.slot_home()).unwrap_or(disp);
                let found =
                    LOCALS.with(|l| l.borrow().iter().position(|(h, ..)| *h == home).map(|k| (k, l.borrow()[k].1)));
                let Some((object, start)) = found else { return Err("a local frame cell".to_owned()) };
                return Ok(format!("%stack.{object}, {}, {}, {}, $noreg", index.0, index.1, disp - start));
            }
            let at = disp - ARGUMENTS_AT.with(|a| a.get());
            let slot = if LONG.with(std::cell::Cell::get) { 8 } else { 4 };
            Ok(format!("%fixed-stack.{}, {}, {}, {}, $noreg", at / slot, index.0, index.1, at % slot))
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

/// An immediate operand as MIR prints it: the number, or a data segment symbol
/// and its displacement (`@seg0 + 4`, a global address operand).
fn immediate(
    imm: &Imm,
    symbols: &mut BTreeSet<i64>,
) -> Result<String, String> {
    if imm.address.is_some() {
        return symbolic(imm, symbols);
    }
    Ok(imm.value.to_string())
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
    at: &[i64],
    targets: &[usize],
    jumps: &mut usize,
    symbols: &mut BTreeSet<i64>,
    callees: &mut BTreeSet<i64>,
    in_register: &BTreeSet<u32>,
    out: &mut String,
) -> Result<(), String> {
    for (held, register) in one.requires.iter().filter(|(held, _)| !in_register.contains(&held.value)) {
        writeln!(out, "    ${} = COPY {}", copy_register(*register, held.width), vreg(held)?).unwrap();
    }
    let Some(what) = &one.what else {
        for (held, register) in &one.delivers {
            writeln!(out, "    {} = COPY ${}", vreg(held)?, copy_register(*register, held.width)).unwrap();
        }
        return Ok(());
    };
    let name = what.name.as_deref().unwrap_or("");
    let eflags = "implicit-def dead $eflags";
    let sp = stack_pointer();
    let dest = what.dests.first();
    match (name, dest, what.sources.as_slice()) {
        // An anchor that names no value is nothing to print.
        ("", None, []) if what.op == llrm_lir::Operation::Nothing => match (one.defines.as_slice(), one.uses.as_slice()) {
            ([], []) => {}
            // An anchor a coalesced copy left: the value it hands on.
            ([to], [from]) => {
                let width = |value: &u32| WIDEST.with(|w| w.borrow().get(value).copied());
                let (Some(to_width), Some(from_width)) = (width(to), width(from)) else {
                    return Err(format!("an anchor of values {from} and {to} of unknown width"));
                };
                writeln!(
                    out,
                    "    {} = COPY {}",
                    vreg(&Held { value: *to, width: to_width, offset: 0 })?,
                    vreg(&Held { value: *from, width: from_width, offset: 0 })?
                )
                .unwrap();
            }
            _ => return Err("an anchor of other than one value".to_owned()),
        },
        ("", None, []) if what.op == llrm_lir::Operation::Return => {
            let used: Vec<String> =
                one.requires.iter().map(|(held, r)| format!("implicit ${}", copy_register(*r, held.width))).collect();
            writeln!(out, "    RET 0{}", used.iter().map(|u| format!(", {u}")).collect::<String>()).unwrap();
        }
        ("call_frame_setup", None, [Loc::Imm(size), Loc::Imm(adjust), Loc::Imm(more)]) => writeln!(
            out,
            "    ADJCALLSTACKDOWN{} {}, {}, {}, implicit-def dead ${sp}, implicit-def dead $eflags, implicit-def dead $ssp, implicit ${sp}, implicit $ssp",
            if sp == "rsp" { 64 } else { 32 },
            immediate(size, symbols)?,
            immediate(adjust, symbols)?,
            immediate(more, symbols)?
        )
        .unwrap(),
        ("call_frame_destroy", None, [Loc::Imm(size), Loc::Imm(adjust)]) => writeln!(
            out,
            "    ADJCALLSTACKUP{} {}, {}, implicit-def dead ${sp}, implicit-def dead $eflags, implicit-def dead $ssp, implicit ${sp}, implicit $ssp",
            if sp == "rsp" { 64 } else { 32 },
            immediate(size, symbols)?,
            immediate(adjust, symbols)?
        )
        .unwrap(),
        ("push", None, [Loc::Held(s)]) if matches!(s.width, 2 | 4 | 8) => {
            writeln!(out, "    PUSH{}r {}, implicit-def ${sp}, implicit ${sp}", bits(s.width)?, vreg(s)?).unwrap()
        }
        ("push", None, [Loc::Imm(i)]) if i.address.is_none() => {
            let form = if sp == "rsp" { "PUSH64i32" } else { "PUSH32i" };
            writeln!(out, "    {form} {}, implicit-def ${sp}, implicit ${sp}", immediate(i, symbols)?).unwrap()
        }
        ("push", None, [Loc::Imm(i)]) => {
            writeln!(out, "    PUSH32i {}, implicit-def $esp, implicit $esp", symbolic(i, symbols)?).unwrap()
        }
        ("call", None, []) if !what.indirect => {
            let callee = what.target.unwrap_or_default();
            callees.insert(callee);
            let used: String =
                one.requires.iter().map(|(held, r)| format!(", implicit ${}", copy_register(*r, held.width))).collect();
            let made: String = one
                .delivers
                .iter()
                .map(|(held, r)| format!(", implicit-def ${}", copy_register(*r, held.width)))
                .collect();
            let (form, saved) = if sp == "rsp" { ("CALL64pcrel32", "csr_64") } else { ("CALLpcrel32", "csr_32") };
            writeln!(
                out,
                "    {form} @callee{callee}, {saved}, implicit ${sp}, implicit $ssp, implicit-def ${sp}, implicit-def $ssp{used}{made}"
            )
            .unwrap();
            for (held, register) in &one.delivers {
                writeln!(out, "    {} = COPY ${}", vreg(held)?, copy_register(*register, held.width)).unwrap();
            }
        }
        // The one-operand divide: the dividend arrives in (dx, ax), the
        // quotient and remainder leave in (ax, dx).
        ("div" | "idiv", Some(Loc::Held(quotient)), [Loc::Held(high), Loc::Held(low), divisor])
            if what.dests.len() == 2 =>
        {
            let (width, form, by) = match divisor {
                Loc::Held(by) => (by.width, "r", vreg(by)?),
                Loc::Mem(m) => (m.width, "m", cell(m, symbols)?),
                _ => return Err("a divide by that source".to_owned()),
            };
            let Some(Loc::Held(remainder)) = what.dests.get(1) else { return Err("a divide without a remainder".to_owned()) };
            let (ax, dx) = match width {
                2 => ("ax", "dx"),
                8 => ("rax", "rdx"),
                _ => ("eax", "edx"),
            };
            if one.requires.is_empty() {
                for (register, held) in [(dx, high), (ax, low)] {
                    if !in_register.contains(&held.value) {
                        writeln!(out, "    ${register} = COPY {}", vreg(held)?).unwrap();
                    }
                }
            }
            let opcode = if name == "idiv" { "IDIV" } else { "DIV" };
            writeln!(
                out,
                "    {opcode}{}{form} {by}, implicit-def ${ax}, implicit-def ${dx}, implicit-def $eflags, implicit ${ax}, implicit ${dx}",
                bits(width)?
            )
            .unwrap();
            writeln!(out, "    {} = COPY ${ax}\n    {} = COPY ${dx}", vreg(quotient)?, vreg(remainder)?).unwrap();
        }
        // A read-modify-write of a cell.
        (op @ ("add" | "sub" | "and" | "or" | "xor"), Some(Loc::Mem(m)), [Loc::Mem(read), rhs]) if read == m => {
            let (form, right) = match rhs {
                Loc::Held(b) => ("mr", vreg(b)?),
                Loc::Imm(i) => (imm("mi", m.width), immediate(i, symbols)?),
                _ => return Err(format!("{op} to a cell from that source")),
            };
            writeln!(out, "    {}{}{form} {}, {right}, {eflags}", op.to_uppercase(), bits(m.width)?, cell(m, symbols)?)
                .unwrap()
        }
        // A call through a register.
        ("call", None, [Loc::Held(target)]) if what.indirect => {
            let used: String =
                one.requires.iter().map(|(held, r)| format!(", implicit ${}", copy_register(*r, held.width))).collect();
            let made: String = one
                .delivers
                .iter()
                .map(|(held, r)| format!(", implicit-def ${}", copy_register(*r, held.width)))
                .collect();
            let (form, saved) = if sp == "rsp" { ("CALL64r", "csr_64") } else { ("CALL32r", "csr_32") };
            writeln!(
                out,
                "    {form} {}, {saved}, implicit ${sp}, implicit $ssp, implicit-def ${sp}, implicit-def $ssp{used}{made}",
                vreg(target)?
            )
            .unwrap();
            for (held, register) in &one.delivers {
                writeln!(out, "    {} = COPY ${}", vreg(held)?, copy_register(*register, held.width)).unwrap();
            }
        }
        // Sign-extends ax into dx.
        ("cwd", Some(Loc::Held(d)), [Loc::Held(s)]) => {
            if one.requires.is_empty() && !in_register.contains(&s.value) {
                writeln!(out, "    $ax = COPY {}", vreg(s)?).unwrap();
            }
            writeln!(out, "    CWD implicit-def $ax, implicit-def $dx, implicit $ax\n    {} = COPY $dx", vreg(d)?).unwrap();
        }
        // `rep stosb`/`rep stosd`: the count, the destination and the value
        // arrive in cx, di and al (ecx, edi and eax), and the count and
        // destination leave.
        ("stosb" | "stosd", Some(Loc::Mem(_)), _) => {
            let (op, value, count, destination) =
                if name == "stosb" { ("STOSB", "al", "cx", "di") } else { ("STOSD", "eax", "ecx", "edi") };
            writeln!(
                out,
                "    REP_{op}_32 implicit-def $ecx, implicit-def $edi, implicit $ecx, implicit ${value}, implicit $edi"
            )
            .unwrap();
            for (held, register) in [(what.dests.get(1), count), (what.dests.get(2), destination)] {
                if let Some(Loc::Held(held)) = held {
                    writeln!(out, "    {} = COPY ${register}", vreg(held)?).unwrap();
                }
            }
        }
        // A segment register read (`mov ax, ss`).
        ("mov", Some(Loc::Held(d)), [Loc::Reg(r)]) if r.width == 2 => {
            writeln!(out, "    {} = COPY ${}", vreg(d)?, format!("{:?}", r.register).to_lowercase()).unwrap()
        }
        // Sign-extends eax into edx.
        ("cdq", Some(Loc::Held(d)), [Loc::Held(s)]) => {
            if one.requires.is_empty() && !in_register.contains(&s.value) {
                writeln!(out, "    $eax = COPY {}", vreg(s)?).unwrap();
            }
            writeln!(out, "    CDQ implicit-def $eax, implicit-def $edx, implicit $eax\n    {} = COPY $edx", vreg(d)?).unwrap();
        }
        // Sign-extends rax into rdx.
        ("cqo", Some(Loc::Held(d)), [Loc::Held(s)]) => {
            if one.requires.is_empty() && !in_register.contains(&s.value) {
                writeln!(out, "    $rax = COPY {}", vreg(s)?).unwrap();
            }
            writeln!(out, "    CQO implicit-def $rax, implicit-def $rdx, implicit $rax\n    {} = COPY $rdx", vreg(d)?).unwrap();
        }
        // A switch: the table's entries are the block's successors.
        ("jmp", None, [Loc::Mem(m)]) if m.base.is_none() && m.addr.is_some_and(|a| a.space == Space::Segment) => {
            let index = m.index.as_ref().ok_or("a jump through a table without an index")?;
            let table = JUMP_TABLES.with(|t| {
                t.borrow_mut().push(targets.to_vec());
                t.borrow().len() - 1
            });
            writeln!(out, "    JMP32m $noreg, {}, {}, %jump-table.{table}, $noreg", m.scale, vreg(index)?).unwrap();
        }
        ("jmp", None, []) => {
            let to = what
                .target
                .and_then(|to| at.iter().position(|a| *a == to))
                .or_else(|| targets.get(*jumps).or(targets.last()).copied())
                .ok_or("a jump without a successor")?;
            writeln!(out, "    JMP_1 %bb.{to}").unwrap();
        }
        (jump, None, []) if condition(jump).is_some() => {
            let taken = what
                .target
                .and_then(|to| at.iter().position(|a| *a == to))
                .or_else(|| targets.first().copied())
                .ok_or("a branch without a successor")?;
            *jumps = 1;
            writeln!(out, "    JCC_1 %bb.{taken}, {}, implicit $eflags", condition(jump).unwrap()).unwrap();
        }
        ("mov", Some(Loc::Held(d)), [Loc::Held(s)]) => writeln!(out, "    {} = COPY {}", vreg(d)?, vreg(s)?).unwrap(),
        ("mov", Some(Loc::Held(d)), [Loc::Imm(i)]) if i.address.is_some() => {
            writeln!(out, "    {} = MOV32ri {}", vreg(d)?, symbolic(i, symbols)?).unwrap()
        }
        // A quadword constant that is not a sign-extended 32 bits is the full
        // 64-bit immediate form.
        ("mov", Some(Loc::Held(d)), [Loc::Imm(i)]) if d.width == 8 => {
            let form = if i32::try_from(i.value).is_ok() { "MOV64ri32" } else { "MOV64ri" };
            writeln!(out, "    {} = {form} {}", vreg(d)?, immediate(i, symbols)?).unwrap()
        }
        ("mov", Some(Loc::Held(d)), [Loc::Imm(i)]) => {
            writeln!(out, "    {} = MOV{}ri {}", vreg(d)?, bits(d.width)?, immediate(i, symbols)?).unwrap()
        }
        ("mov", Some(Loc::Held(d)), [Loc::Mem(m)]) => {
            let from = cell(m, symbols)?;
            // An argument's home is immutable: the pin rematerializes a load
            // from it only when the instruction says what it reads.
            let reads = match from.split(',').next().filter(|home| home.starts_with("%fixed-stack.")) {
                Some(home) => format!(" :: (load (s{}) from {home})", m.width * 8),
                None => String::new(),
            };
            writeln!(out, "    {} = MOV{}rm {}{reads}", vreg(d)?, bits(d.width)?, from).unwrap()
        }
        ("mov", Some(Loc::Mem(m)), [Loc::Held(s)]) => {
            writeln!(out, "    MOV{}mr {}, {}", bits(s.width)?, cell(m, symbols)?, vreg(s)?).unwrap()
        }
        ("mov", Some(Loc::Mem(m)), [Loc::Imm(i)]) => {
            writeln!(out, "    MOV{}{} {}, {}", bits(m.width)?, imm("mi", m.width), cell(m, symbols)?, immediate(i, symbols)?).unwrap()
        }
        ("lea", Some(Loc::Held(d)), [Loc::Mem(m)]) => {
            writeln!(out, "    {} = LEA{}r {}", vreg(d)?, bits(d.width)?, cell(m, symbols)?).unwrap()
        }
        ("lea", Some(Loc::Held(d)), [Loc::Address(a)]) => {
            let mut place = Mem::new(a.addr, 1);
            place.offset = a.offset;
            writeln!(out, "    {} = LEA{}r {}", vreg(d)?, bits(d.width)?, cell(&place, symbols)?).unwrap()
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
        // A select on the flags: the destination is tied to the first source,
        // the value kept where the condition fails.
        (cmov, Some(Loc::Held(d)), [Loc::Held(kept), taken])
            if cmov.starts_with("cmov") && kept.value == d.value && condition(&format!("j{}", &cmov[4..])).is_some() =>
        {
            let code = condition(&format!("j{}", &cmov[4..])).unwrap_or(0);
            let (form, from) = match taken {
                Loc::Held(b) => ("rr", vreg(b)?),
                Loc::Mem(m) => ("rm", cell(m, symbols)?),
                _ => return Err(format!("{cmov} from that source")),
            };
            writeln!(out, "    {} = CMOV{}{form} {}, {from}, {code}, implicit $eflags", vreg(d)?, bits(d.width)?, vreg(kept)?)
                .unwrap()
        }
        (set, Some(Loc::Held(d)), []) if set.starts_with("set") && condition(&format!("j{}", &set[3..])).is_some() => {
            let code = condition(&format!("j{}", &set[3..])).unwrap_or(0);
            writeln!(out, "    {} = SETCCr {code}, implicit $eflags", vreg(d)?).unwrap()
        }
        (set, Some(Loc::Mem(m)), []) if set.starts_with("set") && condition(&format!("j{}", &set[3..])).is_some() => {
            let code = condition(&format!("j{}", &set[3..])).unwrap_or(0);
            writeln!(out, "    SETCCm {}, {code}, implicit $eflags", cell(m, symbols)?).unwrap()
        }
        ("cmp", None, [Loc::Mem(m), Loc::Imm(i)]) => {
            writeln!(out, "    CMP{}{} {}, {}, implicit-def $eflags", bits(m.width)?, imm("mi", m.width), cell(m, symbols)?, immediate(i, symbols)?).unwrap()
        }
        ("cmp", None, [Loc::Mem(m), Loc::Held(a)]) => {
            writeln!(out, "    CMP{}mr {}, {}, implicit-def $eflags", bits(a.width)?, cell(m, symbols)?, vreg(a)?).unwrap()
        }
        ("cmp", None, [Loc::Held(a), Loc::Mem(m)]) => {
            writeln!(out, "    CMP{}rm {}, {}, implicit-def $eflags", bits(a.width)?, vreg(a)?, cell(m, symbols)?).unwrap()
        }
        ("cmp" | "test", None, [Loc::Held(a), Loc::Held(b)]) => {
            let op = if name == "cmp" { "CMP" } else { "TEST" };
            writeln!(out, "    {op}{}rr {}, {}, implicit-def $eflags", bits(a.width)?, vreg(a)?, vreg(b)?).unwrap()
        }
        // The carry comes from the compare before: `bt` then `adc`.
        (op @ ("adc" | "sbb"), Some(Loc::Held(d)), [Loc::Held(a), Loc::Imm(i)]) if a.value == d.value => writeln!(
            out,
            "    {} = {}{}{} {}, {}, implicit-def dead $eflags, implicit $eflags",
            vreg(d)?,
            op.to_uppercase(),
            bits(d.width)?,
            imm("ri", d.width),
            vreg(a)?,
            immediate(i, symbols)?
        )
        .unwrap(),
        (op @ ("adc" | "sbb"), Some(Loc::Held(d)), [Loc::Held(a), Loc::Held(b)]) if a.value == d.value => writeln!(
            out,
            "    {} = {}{}rr {}, {}, implicit-def dead $eflags, implicit $eflags",
            vreg(d)?,
            op.to_uppercase(),
            bits(d.width)?,
            vreg(a)?,
            vreg(b)?
        )
        .unwrap(),
        ("bt", None, [Loc::Held(a), Loc::Imm(i)]) => {
            writeln!(out, "    BT{}ri8 {}, {}, implicit-def $eflags", bits(a.width)?, vreg(a)?, immediate(i, symbols)?).unwrap()
        }
        (op @ ("cmp" | "test"), None, [Loc::Held(a), Loc::Imm(i)]) => {
            writeln!(
                out,
                "    {}{}{} {}, {}, implicit-def $eflags",
                op.to_uppercase(),
                bits(a.width)?,
                imm("ri", a.width),
                vreg(a)?,
                immediate(i, symbols)?
            )
            .unwrap()
        }
        (op @ ("add" | "sub" | "and" | "or" | "xor"), Some(Loc::Held(d)), [Loc::Held(a), rhs]) => {
            let upper = op.to_uppercase();
            let (form, right) = match rhs {
                Loc::Held(b) => ("rr", vreg(b)?),
                Loc::Imm(i) => (imm("ri", d.width), immediate(i, symbols)?),
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
            immediate(i, symbols)?
        )
        .unwrap(),
        // The widening multiply: the low operand arrives in eax (a `requires`
        // copy above), the product leaves in eax and edx (the `delivers`
        // copies).
        (op @ ("mul" | "imul"), Some(Loc::Held(_)), [Loc::Held(_), operand]) if what.dests.len() == 2 => {
            let (form, right, width) = match operand {
                Loc::Held(b) if matches!(b.width, 4 | 8) => ("r", vreg(b)?, b.width),
                Loc::Mem(m) if matches!(m.width, 4 | 8) => ("m", cell(m, symbols)?, m.width),
                _ => return Err(format!("a widening {op} by that operand")),
            };
            let (low, high) = if width == 8 { ("rax", "rdx") } else { ("eax", "edx") };
            writeln!(
                out,
                "    {}{}{form} {right}, implicit-def ${low}, implicit-def ${high}, {eflags}, implicit ${low}",
                op.to_uppercase(),
                bits(width)?
            )
            .unwrap();
            for (held, register) in &one.delivers {
                writeln!(out, "    {} = COPY ${}", vreg(held)?, format!("{register:?}").to_lowercase()).unwrap();
            }
        }
        ("imul", Some(Loc::Held(d)), [Loc::Held(a), Loc::Imm(i)]) if what.dests.len() == 1 => {
            writeln!(out, "    {} = IMUL{}{} {}, {}, {eflags}", vreg(d)?, bits(d.width)?, imm("rri", d.width), vreg(a)?, immediate(i, symbols)?)
                .unwrap()
        }
        ("imul", Some(Loc::Held(d)), [Loc::Held(a), Loc::Mem(m)]) if what.dests.len() == 1 => {
            writeln!(out, "    {} = IMUL{}rm {}, {}, {eflags}", vreg(d)?, bits(d.width)?, vreg(a)?, cell(m, symbols)?)
                .unwrap()
        }
        ("imul", Some(Loc::Held(d)), [Loc::Mem(m), Loc::Imm(i)]) if what.dests.len() == 1 => {
            writeln!(out, "    {} = IMUL{}{} {}, {}, {eflags}", vreg(d)?, bits(d.width)?, if d.width == 8 { "rmi32" } else { "rmi" }, cell(m, symbols)?, immediate(i, symbols)?)
                .unwrap()
        }
        ("imul", Some(Loc::Held(d)), [Loc::Held(a), Loc::Held(b)]) if what.dests.len() == 1 => {
            writeln!(out, "    {} = IMUL{}rr {}, {}, {eflags}", vreg(d)?, bits(d.width)?, vreg(a)?, vreg(b)?).unwrap()
        }
        // A double-register shift by an immediate: the first source is the
        // destination, the second supplies the bits shifted in.
        (op @ ("shld" | "shrd"), Some(Loc::Held(d)), [Loc::Held(a), Loc::Held(b), Loc::Imm(count)]) => writeln!(
            out,
            "    {} = {}{}rri8 {}, {}, {}, {eflags}",
            vreg(d)?,
            op.to_uppercase(),
            bits(d.width)?,
            vreg(a)?,
            vreg(b)?,
            immediate(count, symbols)?
        )
        .unwrap(),
        // The bit scan keeps the destination's old value for a zero source: the
        // pin's `BSR32rr` ties it to the first source.
        (op @ ("bsr" | "bsf"), Some(Loc::Held(d)), [Loc::Held(old), Loc::Held(s)]) if old.value == d.value => {
            writeln!(out, "    {} = {}{}rr {}, {}, {eflags}", vreg(d)?, op.to_uppercase(), bits(d.width)?, vreg(old)?, vreg(s)?)
                .unwrap()
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
        _ => return Err(format!("`{name}` ({:?}) with {} destinations and {} sources: {:?} <- {:?}", what.op, what.dests.len(), what.sources.len(), what.dests, what.sources)),
    }
    Ok(())
}

/// `body` as a MIR module of one function, or the first thing this printer
/// does not know.
/// A register whose first definition writes only part of it has nothing to keep
/// of the rest: the pin's verifier wants that said (`undef %5.sub_8bit = ...`).
fn undef_first_partial_defs(text: &str) -> String {
    // A register read only in part never needs the rest of it: each partial
    // write is `undef`, wherever the paths that write it meet.
    let mut whole = BTreeSet::new();
    let mut partial = BTreeSet::new();
    for line in text.lines() {
        let reads = line.split_once(" = ").map_or(line, |(_, reads)| reads);
        let bytes = reads.as_bytes();
        for (at, _) in reads.match_indices('%') {
            let end = (at + 1..bytes.len()).find(|&k| !bytes[k].is_ascii_digit()).unwrap_or(bytes.len());
            if end == at + 1 {
                continue;
            }
            let value = &reads[at + 1..end];
            if reads[end..].starts_with(".sub_") {
                partial.insert(value)
            } else {
                whole.insert(value)
            };
        }
    }
    let only_in_part: BTreeSet<&str> = partial.difference(&whole).copied().collect();
    let mut defined = BTreeSet::new();
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let first = line.trim_start().split(" = ").next().unwrap_or("");
        let token = first.split(':').next().unwrap_or("");
        let marked = line.trim_start().contains(" = ")
            && token.starts_with('%')
            && token[1..].split('.').next().is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()) && !n.is_empty());
        if marked {
            let value = token[1..].split('.').next().unwrap_or("").to_owned();
            if token.contains(".sub_") && (!defined.contains(&value) || only_in_part.contains(value.as_str())) {
                let indent = &line[..line.len() - line.trim_start().len()];
                out.push_str(&format!("{indent}undef {}\n", line.trim_start()));
                defined.insert(value);
                continue;
            }
            defined.insert(value);
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// A value whose first mention in the text is a read has no definition on the
/// path in (a loop-carried value the first trip reads unset): the pin's
/// verifier wants one, and `IMPLICIT_DEF` is the nothing it stands for.
fn implicit_defs_for_unset_reads(text: &str) -> String {
    let mut defined = BTreeSet::new();
    let mut unset = BTreeSet::new();
    let values = |part: &str| -> Vec<String> {
        let mut found = Vec::new();
        let bytes = part.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at] == b'%' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit) {
                let end = (at + 1..bytes.len()).find(|&k| !bytes[k].is_ascii_digit()).unwrap_or(bytes.len());
                if !part[..at].ends_with("bb.")
                    && !part[..at].ends_with("fixed-stack.")
                    && !part[..at].ends_with("stack.")
                {
                    found.push(part[at + 1..end].to_owned());
                }
                at = end;
            } else {
                at += 1;
            }
        }
        found
    };
    for line in text.lines().filter(|line| line.starts_with("    ")) {
        let (left, right) = match line.split_once(" = ") {
            Some((left, right)) => (left, right),
            None => ("", line),
        };
        for value in values(right) {
            if !defined.contains(&value) {
                unset.insert(value);
            }
        }
        defined.extend(values(left));
    }
    if unset.is_empty() {
        return text.to_owned();
    }
    let mut entry = text.find("  bb.0:\n").map_or(0, |at| at + "  bb.0:\n".len());
    while text[entry..].starts_with("    successors:") || text[entry..].starts_with("    liveins:") {
        entry += text[entry..].find('\n').map_or(0, |end| end + 1);
    }
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..entry]);
    for value in &unset {
        out.push_str(&format!("    %{value} = IMPLICIT_DEF\n"));
    }
    out.push_str(&text[entry..]);
    out
}

pub fn mir(
    body: &LirBody,
    pins: &IndexMap<u32, RegId>,
) -> Result<String, String> {
    let at: Vec<i64> = body.blocks.iter().map(|b| b.at).collect();
    let odds = llrm_codegen::liveness::blockfrequency::MachineBranchProbabilityInfo::of(body);
    let mut symbols = BTreeSet::new();
    let mut callees = BTreeSet::new();
    let mut text = String::new();
    let mut widths = BTreeSet::new();
    let mut register_only = BTreeSet::new();
    let mut indexed = BTreeSet::new();
    let value_widths = widths_of(body)?;
    WIDEST.with(|w| *w.borrow_mut() = value_widths.clone());
    // Quadword values or a 64-bit register anywhere make it 64-bit code: `rsp`,
    // REX registers, `gr64`, eight-byte stack slots.
    let wide_register = |register: &RegId| quadword_family(&format!("{register:?}").to_lowercase()).is_some();
    let long = value_widths.values().any(|width| *width == 8)
        || pins.values().any(wide_register)
        || body
            .insns()
            .iter()
            .any(|one| one.requires.iter().chain(&one.delivers).any(|(_, register)| wide_register(register)));
    LONG.with(|l| l.set(long));
    let first_argument = body
        .insns()
        .iter()
        .filter_map(|one| one.what.as_ref())
        .flat_map(|w| w.dests.iter().chain(&w.sources))
        .filter_map(|loc| match loc {
            Loc::Mem(m) if m.base.is_none() && m.index.is_none() => {
                m.addr.filter(|a| a.space == Space::Frame).map(|a| a.disp + m.offset)
            }
            _ => None,
        })
        .filter(|at| *at >= 0)
        .min()
        .unwrap_or(if long { 16 } else { 8 });
    ARGUMENTS_AT.with(|a| a.set(first_argument));
    let mut locals: Vec<(i64, i64, i64)> = Vec::new();
    for loc in body.iter_insns().filter_map(|one| one.what.as_ref()).flat_map(|w| w.dests.iter().chain(&w.sources)) {
        let named;
        let m = match loc {
            Loc::Mem(m) => m,
            Loc::Address(a) => {
                named = {
                    let mut m = Mem::new(a.addr, 1);
                    m.offset = a.offset;
                    m.width = 1;
                    m
                };
                &named
            }
            _ => continue,
        };
        let Some(a) = m.addr.filter(|a| a.space == Space::Frame && m.base.is_none()) else {
            continue;
        };
        let disp = a.disp + m.offset;
        if disp >= 0 && a.slot_home().is_none() {
            continue;
        }
        let home = a.slot_home().unwrap_or(disp);
        let end = disp + i64::from(m.width);
        match locals.iter_mut().find(|(h, ..)| *h == home) {
            Some(known) => {
                known.1 = known.1.min(disp);
                known.2 = known.2.max(end);
            }
            None => locals.push((home, disp, end)),
        }
    }
    LOCALS.with(|l| *l.borrow_mut() = locals);
    JUMP_TABLES.with(|t| t.borrow_mut().clear());
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
            if one.what.as_ref().and_then(|w| w.name.as_deref()).is_some_and(|name| {
                condition(name).is_some()
                    || name.strip_prefix("set").is_some_and(|rest| condition(&format!("j{rest}")).is_some())
                    || name.strip_prefix("cmov").is_some_and(|rest| condition(&format!("j{rest}")).is_some())
            }) {
                flags_are_read(&mut text);
            }
            instruction(one, &at, &targets, &mut jumps, &mut symbols, &mut callees, &register_only, &mut text)
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
        let taken = block
            .insns
            .last()
            .and_then(|one| one.what.as_ref())
            .and_then(|w| w.target)
            .and_then(|to| at.iter().position(|a| *a == to));
        let fell = match (jumps, targets.as_slice(), ends) {
            (1, [first, second], Some(last)) if condition(last).is_some() => {
                Some(if taken == Some(*second) { *first } else { *second })
            }
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
            // Read as a byte, the class the pin's own selection gives it: the
            // one whose registers have a low byte.
            (Some(8), _, true) => "gr64_with_sub_8bit",
            (Some(8), true, _) => "gr64_nosp",
            (Some(8), ..) => "gr64",
            (Some(2), ..) if long => "gr16",
            (_, ..) if long => "gr32",
            (Some(2), _, true) => "gr16_abcd",
            (Some(2), ..) => "gr16",
            (_, _, true) => "gr32_abcd",
            (_, true, _) => "gr32_nosp",
            _ => "gr32",
        };
        writeln!(out, "  - {{ id: {v}, class: {class} }}").unwrap();
    }
    let text = undef_first_partial_defs(&text);
    let text = implicit_defs_for_unset_reads(&text);
    let objects = text
        .match_indices("%fixed-stack.")
        .filter_map(
            |(at, home)| text[at + home.len()..].split(|c: char| !c.is_ascii_digit()).next()?.parse::<usize>().ok(),
        );
    writeln!(out, "fixedStack:").unwrap();
    for id in 0..=objects.max().unwrap_or(0) {
        let slot = if long { 8 } else { 4 };
        writeln!(out, "  - {{ id: {id}, offset: {}, size: {slot}, alignment: {slot}, isImmutable: true }}", id * slot)
            .unwrap();
    }
    let locals = LOCALS.with(|l| l.borrow().clone());
    if !locals.is_empty() {
        writeln!(out, "stack:").unwrap();
        for (id, (_, start, end)) in locals.iter().enumerate() {
            writeln!(out, "  - {{ id: {id}, offset: {start}, size: {}, alignment: 4 }}", end - start).unwrap();
        }
    }
    let tables = JUMP_TABLES.with(|t| t.borrow().clone());
    if !tables.is_empty() {
        writeln!(out, "jumpTable:\n  kind: block-address\n  entries:").unwrap();
        for (id, blocks) in tables.iter().enumerate() {
            let list: Vec<String> = blocks.iter().map(|to| format!("'%bb.{to}'")).collect();
            writeln!(out, "    - id: {id}\n      blocks: [ {} ]", list.join(", ")).unwrap();
        }
    }
    writeln!(out, "body: |").unwrap();
    out.push_str(&text);
    writeln!(out, "...").unwrap();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_codegen::model::lir::{Block, Insn, LirBlock, LirBody};
    use llrm_codegen::support::hash::IndexMap;
    use llrm_lir::registers::RegId;
    use llrm_lir::{Held, Imm, Loc, Operation, Semantics};

    use super::mir;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4, offset: 0 })
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
    /// the pin refused as a register of no class. It is the one-operand
    /// `IMUL32r`, eax in and eax:edx out, so a soft-float `__mulsf3`
    /// (`mul` of two words) reaches the pin's greedy.
    #[test]
    fn test_a_widening_multiply_prints_as_the_one_operand_form() {
        let mut widening = Insn::new(
            0,
            None,
            Some(Semantics {
                name: Some("imul".into()),
                dests: vec![held(8), held(9)],
                sources: vec![held(6), held(7)],
                ..Semantics::new(Operation::Multiply)
            }),
            vec![8, 9],
            vec![6, 7],
        );
        let word = |value| Held { value, width: 4, offset: 0 };
        widening.requires = vec![(word(6), RegId::EAX)];
        widening.delivers = vec![(word(8), RegId::EAX), (word(9), RegId::EDX)];
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![Arc::new(widening), ret]))];
        let body = LirBody::new("wide", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(
            text.contains(
                "$eax = COPY %6\n    IMUL32r %7, implicit-def $eax, implicit-def $edx, implicit-def dead $eflags, implicit $eax\n    %8 = COPY $eax\n    %9 = COPY $edx"
            ),
            "{text}"
        );
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
            requires: vec![(Held { value: 1, width: 4, offset: 0 }, RegId::ECX)],
            delivers: vec![(Held { value: 2, width: 4, offset: 0 }, RegId::EAX)],
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

    /// A load of an argument's home printed with no memory operand, so the
    /// pin did not see an immutable load and allocated that function as if the
    /// value could not be made again.
    #[test]
    fn test_an_argument_load_names_the_immutable_slot_it_reads() {
        use llrm_lir::{Addr, Mem, Space};
        let cell = Mem::new(Some(Addr::new(Space::Frame, 8)), 4);
        let load = insn("mov", Operation::Move, vec![held(1)], vec![Loc::Mem(cell)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![load, ret]))];
        let body = LirBody::new("arg", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains(":: (load (s32) from %fixed-stack.0)"), "{text}");
    }

    /// m16 passes its stack arguments at BP+4 (not +8) and a function may read
    /// several: the printer refused the first as "a local frame cell" and
    /// declared one fixed-stack object for all.
    #[test]
    fn test_stack_arguments_of_an_m16_frame_are_each_a_fixed_stack_object() {
        use llrm_lir::{Addr, Mem, Space};
        let arg = |disp| Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, disp)), 4));
        let first = insn("mov", Operation::Move, vec![held(1)], vec![arg(4)]);
        let second = insn("mov", Operation::Move, vec![held(2)], vec![arg(8)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![first, second, ret]))];
        let body = LirBody::new("args", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("MOV32rm %fixed-stack.0,"), "{text}");
        assert!(text.contains("MOV32rm %fixed-stack.1,"), "{text}");
        assert!(text.contains("id: 1, offset: 4"), "{text}");
    }

    /// A local array's cells (`[bp-8]`, `[bp-4]`) were refused as "a local
    /// frame cell", so no function with a stack object reached the pin.
    #[test]
    fn test_local_frame_cells_are_stack_objects() {
        use llrm_lir::{Addr, Mem, Space};
        let cell = |disp| Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, disp).in_slot(-8)), 4));
        let store = insn("mov", Operation::Move, vec![cell(-8)], vec![held(1)]);
        let load = insn("mov", Operation::Move, vec![held(2)], vec![cell(-4)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![store, load, ret]))];
        let body = LirBody::new("local", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("stack:\n  - { id: 0, offset: -8, size: 8"), "{text}");
        assert!(text.contains("MOV32mr %stack.0, 1, $noreg, 0, $noreg, %1"), "{text}");
        assert!(text.contains("MOV32rm %stack.0, 1, $noreg, 4, $noreg"), "{text}");
    }

    /// A local array's address (`lea ax, [bp-98]`) and a 16-bit push were
    /// refused, so no m16 call taking an array reached the pin.
    #[test]
    fn test_a_frame_address_and_a_word_push_print() {
        use llrm_lir::{Addr, AddressRef, Mem, Space};
        let place = Addr::new(Space::Frame, -8).in_slot(-8);
        let store = insn("mov", Operation::Move, vec![Loc::Mem(Mem::new(Some(place), 4))], vec![held(1)]);
        let lea = insn("lea", Operation::Move, vec![held(2)], vec![Loc::Address(AddressRef::new(Some(place)))]);
        let push = insn("push", Operation::Push, vec![], vec![held(2)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![store, lea, push, ret]))];
        let body = LirBody::new("addr", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("LEA32r %stack.0, 1, $noreg, 0, $noreg"), "{text}");
    }

    /// `setge` (a flags read into a byte) and `cmp` against a cell were
    /// refused, so a function with either never reached the pin.
    #[test]
    fn test_a_set_on_flags_and_a_compare_with_a_cell_print() {
        let cell = Loc::Mem(llrm_lir::Mem::new(Some(llrm_lir::Addr::new(llrm_lir::Space::Frame, 8)), 4));
        let cmp = insn("cmp", Operation::Compare, vec![], vec![held(1), cell]);
        let set = insn("setge", Operation::Unary, vec![held(2)], vec![]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![cmp, set, ret]))];
        let body = LirBody::new("set", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("CMP32rm %1, %fixed-stack.0"), "{text}");
        assert!(text.contains("%2 = SETCCr 13, implicit $eflags"), "{text}");
    }

    /// `cmp [cell], reg` was refused ("`cmp` ... 2 sources"), so
    /// `_bench_jpeg_fixed` at m32 -Os never reached the pin's greedy and the
    /// TA-10 spill could not be compared with it.
    #[test]
    fn test_a_compare_of_a_cell_with_a_register_prints() {
        let cell = Loc::Mem(llrm_lir::Mem::new(Some(llrm_lir::Addr::new(llrm_lir::Space::Frame, 8)), 4));
        let cmp = insn("cmp", Operation::Compare, vec![], vec![cell, held(1)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![cmp, ret]))];
        let body = LirBody::new("cmpmr", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("CMP32mr %fixed-stack.0, 1, $noreg, 0, $noreg, %1, implicit-def $eflags"), "{text}");
    }

    /// `sete [cell]` (a flag read stored straight to memory) was refused, so
    /// x_life's -Os function never reached the pin.
    #[test]
    fn test_a_set_on_flags_into_a_cell_prints() {
        let cell = Loc::Mem(llrm_lir::Mem::new(Some(llrm_lir::Addr::new(llrm_lir::Space::Frame, 8)), 1));
        let set = insn("sete", Operation::Unary, vec![cell], vec![]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![set, ret]))];
        let body = LirBody::new("setm", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("SETCCm %fixed-stack.0, 1, $noreg, 0, $noreg, 4, implicit $eflags"), "{text}");
    }

    /// A nameless instruction that is not a return (a marker) printed as
    /// `RET 0`, which ended the block early for the pin.
    #[test]
    fn test_a_switch_through_a_table_prints_its_jump_table() {
        use llrm_lir::{Addr, Mem, Space};
        let mut table = Mem::new(Some(Addr::new(Space::Segment, 7)), 4);
        table.index = Some(Held { value: 1, width: 4, offset: 0 });
        table.scale = 4;
        let jump = insn("jmp", Operation::Branch, vec![], vec![Loc::Mem(table)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![
            Block::from(LirBlock { succ: vec![1, 2].into(), ..LirBlock::new(0, vec![jump]) }),
            Block::from(LirBlock::new(1, vec![Arc::clone(&ret)])),
            Block::from(LirBlock::new(2, vec![ret])),
        ];
        let body = LirBody::new("sw", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("a switch through a table prints");
        assert!(text.contains("JMP32m $noreg, 4, %1, %jump-table.0, $noreg"), "{text}");
        assert!(
            text.contains(
                "jumpTable:\n  kind: block-address\n  entries:\n    - id: 0\n      blocks: [ '%bb.1', '%bb.2' ]"
            ),
            "{text}"
        );
    }

    #[test]
    fn test_an_indexed_local_array_cell_is_an_indexed_stack_object() {
        use llrm_lir::{Addr, Mem, Space};
        let mut cell = Mem::new(Some(Addr::new(Space::Frame, -64)), 4);
        cell.index = Some(Held { value: 2, width: 4, offset: 0 });
        cell.scale = 4;
        let load = insn("mov", Operation::Move, vec![held(1)], vec![Loc::Mem(cell)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![load, ret]))];
        let body = LirBody::new("arr", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("an indexed local prints");
        assert!(text.contains("MOV32rm %stack.0, 4, %2, 0, $noreg"), "{text}");
    }

    /// A slot-tagged frame cell at displacement 0 printed as
    /// `%fixed-stack.-2` ("expected a subregister index after '.'") on
    /// matmul -O3 at m32: a cell of a slot is a stack object whatever its
    /// displacement.
    #[test]
    fn test_a_slot_tagged_cell_at_a_small_displacement_is_a_stack_object() {
        use llrm_lir::{Addr, Mem, Space};
        let cell = Mem::new(Some(Addr::new(Space::Frame, 0).in_slot(-256)), 4);
        let store = insn("mov", Operation::Move, vec![Loc::Mem(cell)], vec![held(1)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![store, ret]))];
        let body = LirBody::new("slot", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("a slot cell prints");
        assert!(text.contains("MOV32mr %stack.0, 1, $noreg, 0, $noreg"), "{text}");
        assert!(!text.contains("fixed-stack.-"), "{text}");
    }

    /// A byte written into a register nothing defined before left the pin's
    /// verifier saying "defs don't dominate all uses" and aborting on
    /// grep.c at m32.
    #[test]
    fn test_the_first_partial_write_of_a_register_is_undef() {
        let text = super::undef_first_partial_defs(
            "    %5.sub_8bit = MOV8ri 1\n    %5.sub_8bit = MOV8ri 2\n    %6 = COPY %5\n",
        );
        assert_eq!(text, "    undef %5.sub_8bit = MOV8ri 1\n    %5.sub_8bit = MOV8ri 2\n    %6 = COPY %5\n");
    }

    /// A byte written on three paths and read only as a byte (x_life's
    /// `sete`/`mov bl, 1`/`xor` merge): only the first textual write was
    /// `undef`, and the pin's verifier aborted on the other two.
    #[test]
    fn test_every_partial_write_of_a_register_only_read_in_part_is_undef() {
        let text = super::undef_first_partial_defs(
            "    %5.sub_8bit = MOV8ri 1\n    %5 = MOV32ri 0\n    %5.sub_8bit = MOV8ri 2\n    MOV8mr %7, 1, $noreg, 0, $noreg, %5.sub_8bit\n",
        );
        assert_eq!(
            text,
            "    undef %5.sub_8bit = MOV8ri 1\n    %5 = MOV32ri 0\n    undef %5.sub_8bit = MOV8ri 2\n    MOV8mr %7, 1, $noreg, 0, $noreg, %5.sub_8bit\n"
        );
    }

    #[test]
    fn test_a_divide_a_sign_extend_and_a_multiply_by_a_cell_print() {
        use llrm_lir::{Addr, Mem, Space};
        let cell = Mem::new(Some(Addr::new(Space::Frame, 8)), 4);
        let sign = insn("cdq", Operation::Binary, vec![held(2)], vec![held(1)]);
        let divide = insn("idiv", Operation::Binary, vec![held(3), held(4)], vec![held(2), held(1), held(5)]);
        let times = insn("imul", Operation::Binary, vec![held(3)], vec![held(3), Loc::Mem(cell)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![sign, divide, times, ret]))];
        let body = LirBody::new("div", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("CDQ implicit-def $eax, implicit-def $edx, implicit $eax"), "{text}");
        assert!(text.contains("IDIV32r %5, implicit-def $eax, implicit-def $edx"), "{text}");
        assert!(text.contains("%3 = COPY $eax\n    %4 = COPY $edx"), "{text}");
        assert!(text.contains("%3 = IMUL32rm %3, %fixed-stack.0"), "{text}");
    }

    /// `and` then `sete` printed the `and`'s flags as dead, and the pin's
    /// verifier refused "using an undefined physical register" on
    /// jpeg_single's `SETCCr`.
    #[test]
    fn test_a_set_keeps_the_flags_of_the_instruction_before_it_alive() {
        let imm = |value| Loc::Imm(Imm { value, width: 4, address: None });
        let and = insn("and", Operation::Binary, vec![held(1)], vec![held(1), imm(15)]);
        let set = insn("sete", Operation::Unary, vec![held(2)], vec![]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![and, set, ret]))];
        let body = LirBody::new("set", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("AND32ri %1, 15, implicit-def $eflags"), "{text}");
    }

    /// A loop-carried value read on the first trip before any definition left
    /// the pin's verifier saying "defs don't dominate all uses" and
    /// aborting on a recursive fib at m32.
    #[test]
    fn test_a_value_read_before_any_definition_gets_an_implicit_def() {
        let imm = |value| Loc::Imm(Imm { value, width: 4, address: None });
        let start = insn("mov", Operation::Move, vec![held(1)], vec![imm(0)]);
        let add = insn("add", Operation::Binary, vec![held(1)], vec![held(1), held(2)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![start, add, ret]))];
        let body = LirBody::new("unset", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("  bb.0:\n    %2 = IMPLICIT_DEF\n    %1 = MOV32ri 0"), "{text}");
    }

    #[test]
    fn test_a_cell_update_a_register_call_and_a_word_sign_extend_print() {
        use llrm_lir::{Addr, Mem, Space};
        let cell = Mem::new(Some(Addr::new(Space::Frame, 8)), 2);
        let update = insn("add", Operation::Binary, vec![Loc::Mem(cell.clone())], vec![Loc::Mem(cell), held(2)]);
        let through = Semantics {
            name: Some("call".into()),
            sources: vec![held(3)],
            indirect: true,
            ..Semantics::new(Operation::Call)
        };
        let call = Arc::new(Insn::new(0, None, Some(through), vec![], vec![3]));
        let sign = insn("cwd", Operation::Unary, vec![held(5)], vec![held(4)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![update, call, sign, ret]))];
        let body = LirBody::new("more", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("ADD16mr %fixed-stack.0, 1, $noreg, 0, $noreg, %2, implicit-def dead $eflags"), "{text}");
        assert!(text.contains("CALL32r %3, csr_32"), "{text}");
        assert!(text.contains("CWD implicit-def $ax, implicit-def $dx, implicit $ax"), "{text}");
    }

    #[test]
    fn test_a_segment_read_a_string_store_and_the_es_name_print() {
        use llrm_lir::{Mem, Reg};

        use super::physical;
        let ss = Loc::Reg(Reg { register: RegId::SS, width: 2 });
        let read = insn("mov", Operation::Move, vec![held(1)], vec![ss]);
        let fill = insn(
            "stosb",
            Operation::Move,
            vec![Loc::Mem(Mem::new(None, 0)), held(3), held(4)],
            vec![held(5), held(6), held(7), held(8)],
        );
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![read, fill, ret]))];
        let body = LirBody::new("str", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("%1 = COPY $ss"), "{text}");
        assert!(text.contains("REP_STOSB_32 implicit-def $ecx, implicit-def $edi"), "{text}");
        assert!(text.contains("%3 = COPY $cx\n    %4 = COPY $di"), "{text}");
        assert_eq!(physical(RegId::ES, 2).as_deref(), Ok("es"));
    }

    /// `rep stosd` (a dword fill, the memset of a local array) was refused
    /// ("`stosd` (Fill) with 3 destinations"), so `_bench_jpeg_fixed` at m32
    /// -Os never reached the pin's greedy.
    #[test]
    fn test_a_dword_fill_prints_as_rep_stosd() {
        use llrm_lir::Mem;
        let fill = insn(
            "stosd",
            Operation::Fill,
            vec![Loc::Mem(Mem::new(None, 0)), held(3), held(4)],
            vec![held(5), held(6), held(7)],
        );
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![fill, ret]))];
        let body = LirBody::new("fill", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(
            text.contains("REP_STOSD_32 implicit-def $ecx, implicit-def $edi, implicit $ecx, implicit $eax"),
            "{text}"
        );
        assert!(text.contains("%3 = COPY $ecx\n    %4 = COPY $edi"), "{text}");
    }

    /// `shld` (a double-register shift, the carry of a multi-word shift) was
    /// refused ("`shld` (Funnel) with 1 destinations and 3 sources"), so
    /// `___mulsf3` at m32 never reached the pin's greedy.
    #[test]
    fn test_a_double_shift_by_an_immediate_prints() {
        let count = Loc::Imm(Imm { value: 1, width: 1, address: None });
        let shld = insn("shld", Operation::Funnel, vec![held(2)], vec![held(2), held(1), count]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![shld, ret]))];
        let body = LirBody::new("shld", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("%2 = SHLD32rri8 %2, %1, 1, implicit-def dead $eflags"), "{text}");
    }

    /// `add r, offset sym` was refused ("a symbolic immediate"; only a `mov`
    /// of an address printed), so `_bench_jpeg_single` at m32 -Os never reached
    /// the pin's greedy.
    #[test]
    fn test_an_add_of_a_segment_address_prints_as_a_global_operand() {
        use llrm_lir::{Addr, Space};
        let address =
            Loc::Imm(Imm { value: 4, width: 4, address: Some(Addr { index: 3, ..Addr::new(Space::Segment, 0) }) });
        let add = insn("add", Operation::Binary, vec![held(2)], vec![held(1), address]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![add, ret]))];
        let body = LirBody::new("sym", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("%2 = ADD32ri %1, @seg3 + 4"), "{text}");
        assert!(text.contains("@seg3 = global"), "{text}");
    }

    #[test]
    fn test_a_sixteen_bit_register_is_named_as_itself() {
        use super::physical;
        assert_eq!(physical(RegId::AX, 2).as_deref(), Ok("ax"));
        assert_eq!(physical(RegId::EAX, 2).as_deref(), Ok("ax"));
    }

    /// `imul r, [cell], imm` was refused ("`imul` with 1 destinations and 2
    /// sources"), so frameless.c's `main` could not be allocated by the pin.
    #[test]
    fn test_a_multiply_of_a_cell_by_an_immediate_prints() {
        let imm = Loc::Imm(Imm { value: 3, width: 4, address: None });
        use llrm_lir::{Addr, Mem, Space};
        let cell = Mem::new(Some(Addr::new(Space::Frame, 8)), 4);
        let times = insn("imul", Operation::Binary, vec![held(1)], vec![Loc::Mem(cell), imm]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let body = LirBody::new(
            "m",
            0,
            vec![Block::from(LirBlock::new(0, vec![times, ret]))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("IMUL32rmi %fixed-stack."), "{text}");
    }

    /// An anchor a coalesced copy left (`d[2] u[1]`, no operation) was refused
    /// ("`` with 0 destinations and 0 sources"), so frameless.c's `descend`
    /// could not be allocated by the pin.
    #[test]
    fn test_an_anchor_that_hands_a_value_on_prints_as_a_copy() {
        let anchor = Arc::new(Insn::new(
            0,
            None,
            Some(Semantics { name: Some("".into()), ..Semantics::new(Operation::Nothing) }),
            vec![2],
            vec![1],
        ));
        let start =
            insn("mov", Operation::Move, vec![held(1)], vec![Loc::Imm(Imm { value: 5, width: 4, address: None })]);
        let step = insn(
            "add",
            Operation::Binary,
            vec![held(2)],
            vec![held(2), Loc::Imm(Imm { value: 1, width: 4, address: None })],
        );
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![start, anchor, step, ret]))];
        let body = LirBody::new("anchor", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("%2 = COPY %1"), "{text}");
    }

    #[test]
    fn test_a_nameless_marker_is_not_printed_as_a_return() {
        let marker = insn("", Operation::Nothing, vec![], vec![]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![marker, ret]))];
        let body = LirBody::new("marker", 0, blocks, IndexMap::default(), IndexMap::default());
        let shown = mir(&body, &IndexMap::default());
        let text = shown.unwrap_or_default();
        assert!(text.matches("RET 0").count() <= 1, "{text}");
    }

    /// `test r, imm` was refused ("`test` with 0 destinations and 2
    /// sources"), so x_switch's function could not be allocated by the pin.
    #[test]
    fn test_a_test_against_an_immediate_prints() {
        let imm = Loc::Imm(Imm { value: 4096, width: 4, address: None });
        let test = insn("test", Operation::Compare, vec![], vec![held(1), imm]);
        let jne = insn("jne", Operation::Branch, vec![], vec![]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![
            Block::from(LirBlock { succ: vec![1, 0].into(), ..LirBlock::new(0, vec![test, jne]) }),
            Block::from(LirBlock::new(1, vec![ret])),
        ];
        let body = LirBody::new("t", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("TEST32ri %1, 4096, implicit-def $eflags"), "{text}");
    }

    /// `bsr`, `bt` and `adc` were refused ("`bsr` (Binary) with 1 destinations
    /// and 2 sources"), so the pin's greedy could not allocate x_sf_sub's
    /// `_bench_x_sf_sub` (add_bits inlined) for a same-MIR comparison.
    #[test]
    fn test_a_bit_scan_a_bit_test_and_an_add_with_carry_print() {
        let imm = |value| Loc::Imm(Imm { value, width: 4, address: None });
        let scan = insn("bsr", Operation::Binary, vec![held(1)], vec![held(1), held(2)]);
        let bit = insn("bt", Operation::Compare, vec![], vec![held(2), imm(3)]);
        let carry = insn("adc", Operation::Binary, vec![held(2)], vec![held(2), imm(3)]);
        let ret = insn("", Operation::Return, vec![], vec![]);
        let blocks = vec![Block::from(LirBlock::new(0, vec![scan, bit, carry, ret]))];
        let body = LirBody::new("b", 0, blocks, IndexMap::default(), IndexMap::default());
        let text = mir(&body, &IndexMap::default()).expect("prints");
        assert!(text.contains("%1 = BSR32rr %1, %2, implicit-def dead $eflags"), "{text}");
        assert!(text.contains("BT32ri8 %2, 3, implicit-def $eflags"), "{text}");
        assert!(text.contains("%2 = ADC32ri %2, 3, implicit-def dead $eflags, implicit $eflags"), "{text}");
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
        let k = Loc::Held(Held { value: 1, width: 4, offset: 0 });
        let count = Loc::Held(Held { value: 2, width: 4, offset: 0 });
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

    /// A quadword body (`long recpow(long x, long n)`: arguments in rdi and
    /// rsi, a self call, a quadword multiply, the result in rax)
    /// was refused at its first register ("8 bytes of rdi"), so no m64 function
    /// could be allocated by the pin and every m64 cause card
    /// bisected blind. It prints as the pin's 64-bit forms: `gr64` values,
    /// `CALL64pcrel32`, `IMUL64rr`, `ADJCALLSTACKDOWN64`.
    #[test]
    fn test_a_quadword_recursive_function_prints_its_64_bit_forms() {
        let word = |value| Held { value, width: 8, offset: 0 };
        let quad = |value| Loc::Held(word(value));
        let imm = |value| Loc::Imm(Imm { value, width: 8, address: None });
        let arrive =
            Arc::new(Insn { defines: vec![1, 2], ..Insn::clone(&insn("", Operation::Return, vec![], vec![])) });
        let test = insn("test", Operation::Compare, vec![], vec![quad(2), quad(2)]);
        let jne = insn("jne", Operation::Branch, vec![], vec![]);
        let one = insn("mov", Operation::Move, vec![quad(9)], vec![imm(1)]);
        let half = insn("sar", Operation::Binary, vec![quad(3)], vec![quad(2), imm(1)]);
        let setup = insn("call_frame_setup", Operation::Move, vec![], vec![imm(0), imm(0), imm(0)]);
        let call = Arc::new(Insn {
            requires: vec![(word(1), RegId::RDI), (word(3), RegId::RSI)],
            delivers: vec![(word(4), RegId::RAX)],
            ..Insn::clone(&insn("call", Operation::Call, vec![], vec![]))
        });
        let destroy = insn("call_frame_destroy", Operation::Move, vec![], vec![imm(0), imm(0)]);
        let square = insn("imul", Operation::Multiply, vec![quad(5)], vec![quad(4), quad(4)]);
        let times = insn("imul", Operation::Multiply, vec![quad(6)], vec![quad(5), quad(1)]);
        let big = insn("add", Operation::Binary, vec![quad(7)], vec![quad(6), imm(5)]);
        let ret = Arc::new(Insn {
            requires: vec![(word(7), RegId::RAX)],
            ..Insn::clone(&insn("", Operation::Return, vec![], vec![]))
        });
        let blocks = vec![
            Block::from(LirBlock { succ: vec![0, 1].into(), ..LirBlock::new(0, vec![arrive, test, jne]) }),
            Block::from(LirBlock::new(1, vec![half, setup, call, destroy, square, times, big, ret.clone()])),
            Block::from(LirBlock::new(2, vec![one, ret])),
        ];
        let body = LirBody::new("recpow", 0, blocks, IndexMap::default(), IndexMap::default());
        let pins: IndexMap<u32, RegId> = [(1, RegId::RDI), (2, RegId::RSI)].into_iter().collect();
        let text = mir(&body, &pins).expect("a quadword body prints");
        for form in [
            "liveins: $rdi, $rsi\n",
            "%1 = COPY $rdi",
            "TEST64rr %2, %2, implicit-def $eflags",
            "%3 = SAR64ri %2, 1,",
            "ADJCALLSTACKDOWN64 0, 0, 0, implicit-def dead $rsp,",
            "CALL64pcrel32 @callee0, csr_64, implicit $rsp, implicit $ssp, implicit-def $rsp, implicit-def $ssp, implicit $rdi, implicit $rsi, implicit-def $rax",
            "%4 = COPY $rax",
            "ADJCALLSTACKUP64 0, 0, implicit-def dead $rsp,",
            "%5 = IMUL64rr %4, %4,",
            "%7 = ADD64ri32 %6, 5,",
            "%9 = MOV64ri32 1",
            "RET 0, implicit $rax",
            "class: gr64 }",
        ] {
            assert!(text.contains(form), "`{form}` is missing from\n{text}");
        }
        assert!(!text.contains("gr32") && !text.contains("$esp"), "{text}");
    }
}
