//! Port of `qbopt/cycles/cycles.py`.
//!
//! Vendored from the runtime pass (`~/work/badlogic/mgl, tools/cycles/cycles.py`,
//! taken 2026-08-29). Published latencies rather than measurements: a ranking.
//! DOSBox charges per instruction and models no latency, which is why this
//! exists alongside it.
//!
//! `CASES`' `mgl: *` entries price mgl's own injected helpers, not what
//! `legacy/calls` emits; the `*: qbopt absorbed, *` entries are this project's
//! `absorb()`/`dividing()`/`consume()` output; the `stock` entries were
//! confirmed 2026-08-30 against the real `B$MUI4`/`B$DVI4`/`B$CPI4`/`B$RMI4`
//! bytes in VBDOS's `VBDCL10E.LIB` (`..\rt\helpi4.asm`, and the `__aFlmul`/
//! `__aFldiv`/`__aFlrem` bodies its thunks jump into).
//!
//! Scores an instruction sequence on a 486, a P5, a P6, K5/K6/K7 and Core,
//! where an idiv is forty cycles and a 16 bit register write followed by a 32
//! bit read is a stall. Costs live in `timings` and are approximate.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::process::Command;
use std::sync::LazyLock;

use crate::cycles::timings::{
    ARCHS, COST, INORDER, ISSUE, LATENCY, LCP_STALL, PARTIAL_STALL, PREFIX,
};
use crate::support::hash::IndexMap;

pub const W16: [&str; 8] = ["ax", "bx", "cx", "dx", "si", "di", "bp", "sp"];
pub const W32: [&str; 8] = ["eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp"];
pub const SEG: [&str; 6] = ["es", "cs", "ss", "ds", "fs", "gs"];
pub const ALU: [&str; 9] = ["add", "adc", "sub", "sbb", "and", "or", "xor", "cmp", "test"];
/// Opcodes carrying a full-width immediate, whose length 66h therefore changes:
/// alu eAX,imm32 and imul/alu/test/mov/push.
pub const IMM_FULL: [&str; 14] = [
    "05", "0d", "15", "1d", "25", "2d", "35", "3d", "69", "81", "a9", "c7", "f7", "68",
];

/// One disassembled instruction: raw hex, mnemonic, operands.
pub type Row = (String, String, String);

/// `bytes.fromhex`: ASCII whitespace between bytes is skipped.
fn _fromhex(hexs: &str) -> Vec<u8> {
    let digits: Vec<u8> = hexs.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    digits
        .chunks(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("ASCII hex");
            u8::from_str_radix(text, 16)
                .unwrap_or_else(|_| panic!("ValueError: non-hexadecimal number found in fromhex() arg"))
        })
        .collect()
}

/// `^[0-9A-F]+\s+([0-9A-F]+)\s+(\S+)\s*(.*)$`, the row regex.
fn _row(line: &str) -> Option<Row> {
    let b = line.as_bytes();
    let hex = |c: u8| c.is_ascii_digit() || (b'A'..=b'F').contains(&c);
    let space = |c: u8| c.is_ascii_whitespace();
    let run = |mut i: usize, keep: &dyn Fn(u8) -> bool| {
        while i < b.len() && keep(b[i]) {
            i += 1;
        }
        i
    };
    let address = run(0, &hex);
    let gap = run(address, &space);
    let raw = run(gap, &hex);
    let second = run(raw, &space);
    let mnemonic = run(second, &|c| !space(c));
    if address == 0 || gap == address || raw == gap || second == raw || mnemonic == second {
        return None;
    }
    Some((
        line[gap..raw].to_owned(),
        line[second..mnemonic].to_lowercase(),
        line[mnemonic..].trim().to_lowercase(),
    ))
}

pub fn disasm(hexs: &str) -> Vec<Row> {
    let b = _fromhex(hexs);
    let mut f = tempfile::Builder::new().suffix(".bin").tempfile().expect("a temporary file");
    f.write_all(&b).expect("the temporary file written");
    let out = Command::new("ndisasm")
        .args(["-b", "16"])
        .arg(f.path())
        .output()
        .expect("FileNotFoundError: ndisasm");
    let out = String::from_utf8_lossy(&out.stdout).into_owned();
    out.lines().filter_map(_row).collect()
}

fn _isdigit(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|c| c.is_ascii_digit())
}

fn _operands(ops: &str) -> Vec<&str> {
    if ops.is_empty() {
        Vec::new()
    } else {
        ops.split(',').map(str::trim).collect()
    }
}

pub fn classify(mnem: &str, ops: &str, raw: &str) -> &'static str {
    let a = _operands(ops);
    let dst = a.first().copied().unwrap_or("");
    let src = a.get(1).copied().unwrap_or("");
    let mem = |o: &str| o.contains('[');
    if mnem == "nop" {
        return "nop";
    }
    if matches!(mnem, "fld" | "fild") {
        return "x87_load";
    }
    if mnem == "fxch" {
        return "x87_exchange";
    }
    if matches!(mnem, "fst" | "fstp") {
        return "x87_store";
    }
    if matches!(mnem, "fist" | "fistp") {
        return "x87_convert_store";
    }
    if matches!(mnem, "fadd" | "faddp" | "fsub" | "fsubr" | "fsubp" | "fsubrp") {
        return if a.iter().any(|one| mem(one)) { "x87_add_m" } else { "x87_add" };
    }
    if matches!(mnem, "fmul" | "fmulp") {
        return if a.iter().any(|one| mem(one)) { "x87_mul_m" } else { "x87_mul" };
    }
    if matches!(mnem, "fdiv" | "fdivr" | "fdivp" | "fdivrp") {
        return if a.iter().any(|one| mem(one)) { "x87_div_m" } else { "x87_div" };
    }
    if mnem == "fldcw" {
        return "x87_control_load";
    }
    if matches!(mnem, "fstcw" | "fnstcw") {
        return "x87_control_store";
    }
    if mnem == "leave" {
        return "leave";
    }
    if mnem.starts_with("rep") {
        return "rep_string";
    }
    if mnem == "jmp" {
        return "jmp_short";
    }
    if mnem.starts_with('j') {
        return "jcc";
    }
    if matches!(mnem, "cwd" | "cdq" | "cbw") {
        return "cdq";
    }
    if matches!(mnem, "les" | "lfs" | "lgs") {
        return "les";
    }
    if mnem == "lea" {
        return "lea";
    }
    if mnem == "push" {
        return if mem(dst) {
            "push_m"
        } else if _isdigit(dst) || dst.starts_with("0x") {
            "push_i"
        } else {
            "push_r"
        };
    }
    if mnem == "pop" {
        return if SEG.contains(&dst) {
            "pop_seg"
        } else if mem(dst) {
            "pop_m"
        } else {
            "pop_r"
        };
    }
    if mnem == "movzx" || mnem == "movsx" {
        return "movzx";
    }
    if mnem == "xchg" {
        return "alu_rr";
    }
    // 16-bit and 32-bit divide are not the same instruction and the second
    // is much the slower -- 27 clocks against 43 on a 486. Deciding by the
    // mnemonic alone prices a widened idiv as though it were free.
    let wide = raw.to_lowercase().starts_with("66");
    if mnem == "mul" {
        return if wide { "mul_r32" } else { "mul_r16" };
    }
    if mnem == "lahf" {
        return "lahf";
    }
    if mnem == "sahf" {
        return "sahf";
    }
    if mnem == "neg" {
        return "alu_rr";
    }
    if mnem == "inc" || mnem == "dec" {
        return "alu_rr";
    }
    if mnem == "imul" {
        if !wide {
            return "mul_r16";
        }
        return if mem(dst) || mem(src) { "imul_m32" } else { "imul_r32" };
    }
    if matches!(mnem, "div" | "idiv") {
        if !wide {
            return "div_r16";
        }
        return if mem(dst) { "idiv_m32" } else { "idiv_r32" };
    }
    if mnem == "call" {
        return "call_far";
    }
    if matches!(mnem, "ret" | "retf") {
        return "ret_far";
    }
    if mnem == "mov" {
        if SEG.contains(&dst) || SEG.contains(&src) {
            return "mov_seg_r";
        }
        if mem(dst) {
            return "mov_mr";
        }
        if mem(src) {
            return "mov_rm";
        }
        if !src.is_empty() && (src.starts_with("0x") || _isdigit(src)) {
            return "mov_ri";
        }
        return "mov_rr";
    }
    if matches!(mnem, "shl" | "shr" | "sar" | "rol" | "ror" | "rcl" | "rcr" | "sal") && !mem(dst) && src == "1" {
        return "shift_r1";
    }
    if matches!(mnem, "shl" | "shr" | "sar" | "rol" | "ror" | "rcl" | "rcr" | "sal" | "shld" | "shrd") {
        return "shift_ri";
    }
    if ALU.contains(&mnem) {
        // CMP/TEST only read their memory operand; charging them `alu_mr`
        // gives them ADD's read/modify/write cost. The allocator's folding
        // model already treats this as the read-only ALU-memory form.
        if matches!(mnem, "cmp" | "test") && (mem(dst) || mem(src)) {
            return "alu_rm";
        }
        if mem(dst) {
            return "alu_mr";
        }
        if mem(src) {
            return "alu_rm";
        }
        return "alu_rr";
    }
    "unknown"
}

/// Not everything can be issued one per cycle and forgotten. For these the
/// cost column is already an occupancy/throughput ranking, so it is used as
/// one; for everything else the machine's width is the limit.
pub const NOTPIPE: [&str; 12] = [
    "idiv_r32", "idiv_m32", "div_r16", "call_far", "ret_far", "push_m", "pop_m", "pop_seg",
    "mov_seg_r", "les", "lahf", "sahf",
];

/// The register names an operand list touches, 16 and 32 bit alike.
pub fn regs_of(ops: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for tok in ops.split(|c: char| !c.is_ascii_lowercase()).filter(|tok| !tok.is_empty()) {
        if W16.contains(&tok) || W32.contains(&tok) || SEG.contains(&tok) {
            out.insert(if W32.contains(&tok) { tok[1..].to_owned() } else { tok.to_owned() });
        }
    }
    out
}

/// `text[lo:hi]`, clamped as Python slices are.
fn _slice(text: &str, lo: usize, hi: usize) -> &str {
    let hi = hi.min(text.len());
    &text[lo.min(hi)..hi]
}

/// `round(x, 1)`: both round the exact binary value half to even.
fn _round1(x: f64) -> f64 {
    format!("{x:.1}").parse().expect("a float")
}

pub type Detail = (String, &'static str, Vec<i64>, Vec<&'static str>);
/// (standing alone, back to back), per arch.
pub type Totals = (Vec<i64>, Vec<f64>);

/// Cycles per arch, as the larger of how long the sequence occupies the
/// machine and how long its dependency chain takes. In order those are the
/// same, the sum; out of order an expensive instruction nobody waits for is
/// nearly free.
pub fn score(rows: &[Row]) -> (Totals, Vec<Detail>) {
    let n = ARCHS.len();
    let mut occupancy = vec![0.0f64; n];
    let mut ready: Vec<BTreeMap<String, i64>> = vec![BTreeMap::new(); n];
    let mut chain = vec![0i64; n];
    let mut written16: BTreeSet<String> = BTreeSet::new();
    let mut detail = Vec::new();

    for (raw, mnem, ops) in rows {
        let kind = classify(mnem, ops, raw);
        let mut c = COST.get(kind).unwrap_or(&COST["unknown"]).to_vec();
        let mut lat = LATENCY.get(kind).unwrap_or(&LATENCY["unknown"]).to_vec();
        let mut notes = Vec::new();
        let a = _operands(ops);
        let mut srcs = regs_of(ops);
        let mut dsts = regs_of(a.first().copied().unwrap_or(""));

        // The x87 stack is implicit in the printed operands. One token keeps
        // the accumulator chain without allocating all eight positions.
        if kind == "x87_load" {
            dsts.insert("x87-stack".to_owned());
        } else if matches!(
            kind,
            "x87_store"
                | "x87_convert_store"
                | "x87_add"
                | "x87_add_m"
                | "x87_mul"
                | "x87_mul_m"
                | "x87_div"
                | "x87_div_m"
        ) {
            srcs.insert("x87-stack".to_owned());
            dsts.insert("x87-stack".to_owned());
        }

        for o in &a {
            let r = o.trim_matches(|c| matches!(c, '[' | ']' | '+' | ' '));
            if W32.contains(&r) && written16.contains(&r[1..]) {
                for k in 0..n {
                    c[k] += PARTIAL_STALL[k];
                    lat[k] += PARTIAL_STALL[k];
                }
                notes.push("partial-reg");
                written16.remove(&r[1..]);
            }
        }
        // each prefix costs a decode clock on the in-order parts
        let mut npfx = 0;
        let lower = raw.to_lowercase();
        let mut h = lower.as_str();
        while matches!(
            _slice(h, 0, 2),
            "66" | "67" | "26" | "2e" | "36" | "3e" | "64" | "65" | "f0" | "f2" | "f3"
        ) {
            npfx += 1;
            h = &h[2..];
        }
        let mut stall = vec![0i64; n];
        for k in 0..n {
            c[k] += PREFIX[k] * npfx;
            lat[k] += PREFIX[k] * npfx;
        }
        if lower.starts_with("67") || _slice(&lower, 2, 4) == "67" {
            for k in 0..n {
                stall[k] += LCP_STALL[k];
            }
            notes.push("lcp-addr");
        }
        // 66h only stalls when it changes the immediate's length. The imm8
        // forms -- 6B imul, 83 alu, C1 shift -- are the same length either way.
        let op = if lower.starts_with("66") { _slice(&lower, 2, 4) } else { "" };
        let ob = op.as_bytes();
        if IMM_FULL.contains(&op) || (!op.is_empty() && ob[0] == b'b' && b"89abcdef".contains(&ob[1])) {
            for k in 0..n {
                stall[k] += LCP_STALL[k];
            }
            notes.push("lcp");
        }

        for k in 0..n {
            c[k] += stall[k];
            lat[k] += stall[k];
            let start = srcs.iter().map(|r| ready[k].get(r).copied().unwrap_or(0)).max().unwrap_or(0).max(0);
            let done = start + lat[k];
            chain[k] = chain[k].max(done);
            for r in &dsts {
                ready[k].insert(r.clone(), done);
            }
            if INORDER[k] != 0 || NOTPIPE.contains(&kind) {
                occupancy[k] += c[k] as f64;
            } else {
                occupancy[k] += 1.0 / ISSUE[k] as f64 + stall[k] as f64;
            }
        }

        if let Some(first) = a.first() {
            if W16.contains(first) && !matches!(kind, "alu_mr" | "mov_mr") {
                written16.insert((*first).to_owned());
            }
            if W32.contains(first) {
                written16.remove(&first[1..]);
            }
        }
        detail.push((format!("{mnem} {ops}"), kind, c, notes));
    }

    // Standing alone a sequence waits for its own dependencies and costs the
    // chain; surrounded by other work what is left is the issue slots.
    let lone = (0..n).map(|k| occupancy[k].max(chain[k] as f64).round_ties_even() as i64).collect();
    let bulk = (0..n).map(|k| _round1(occupancy[k])).collect();
    ((lone, bulk), detail)
}

/// The two long pushes and the far call, which every crackable operation pays
/// before the callee is entered. Common to the stock routine and to ours.
pub const CALL4: &str = "66FF365C0066FF365A009AAAAAAAAA";

pub static CASES: LazyLock<IndexMap<&'static str, String>> = LazyLock::new(|| {
    let call4 = |tail: &str| format!("{CALL4}{tail}");
    IndexMap::from_iter([
        // ---- c = a AND b
        ("and: BC halves", "A15A008B165C0023065600231658 00A35E0089166000".replace(' ', "")),
        ("and: widened", "66A15A00662306560066A35E00668BD066C1EA10".to_owned()),
        ("and: widened, no dx", "66A15A00662306560066A35E00".to_owned()),
        // ---- c = ((a AND b) + a) XOR b
        (
            "chain: BC halves",
            "A15A008B165C00230656002316580003065600131658003306560033165800A35E0089166000".to_owned(),
        ),
        ("chain: widened", "66A15A0066230656006603065600663306560066A35E00".to_owned()),
        // ---- a long multiply: B$MUI4 when both high words are zero, and when not
        ("mul: stock fast", call4("558BEC8B46088B4E0C0BC88B4E0A75098B4606F7E15DCA0800")),
        (
            "mul: stock full",
            call4("558BEC8B46088B4E0C0BC88B4E0A750953F7E18BD88B4606F7660C03D88B4606F7E103D35B5DCA0800"),
        ),
        // mgl's do_mui4, a helper mgl injected; not what calls emits
        ("mul: mgl call", call4("558BEC668B460666F76E0A668BD066C1EA105DCA0800")),
        ("mul: mgl pow2", "66A15A0066C1E008EB03909090".to_owned()),
        // absorb() against a memory operand; consume() off the stack
        ("mul: qbopt absorbed, memory", "66A10000660FAF0600006650585A".to_owned()),
        ("mul: qbopt absorbed, register", "66586659660FAFC16650585A".to_owned()),
        // ---- a long compare: B$CPI4 rebuilds the flags by hand
        ("cmp: stock", call4("558BEC508B460C3B460875118B460A3B46069F250041D1E8D0E40AE09E585DCA0800")),
        ("cmp: mgl call", call4("558BEC6650668B460A663B460666585DCA0800")),
        ("cmp: mgl inlined", "66A15A00663B065C00EB03909090".to_owned()),
        // absorb() leaves no value to restore: B$CPI4 answers in flags
        ("cmp: qbopt absorbed, memory", "665066A10000663B0600006658".to_owned()),
        // compare_consume()'s bp-relative form, both operands on the stack
        ("cmp: qbopt absorbed, stack", "5566528BEC668B560A663B56068B560489560C668B56008D660C5D".to_owned()),
        // ---- dividing by 256: B$DVI4's short path
        (
            "div256: stock",
            call4(concat!(
                "558BEC57565333FF8B46080BC07D11",
                "8B460C0BC07D11",
                "0BC075158B4E0A8B460833D2F7F18BD8",
                "8B4606F7F18BD3EB38",
                "4F7507",
                "5B5E5F5DCA0800"
            )),
        ),
        ("div256: mgl call", call4("558BEC6651668B4606668B4E0A669966F7F9668BD066C1EA1066595DCA0800")),
        ("div256: mgl shift", call4("558BEC668B4606669966C1EA186603C266C1F808668BD066C1EA105DCA0800")),
        ("div: qbopt absorbed, memory", "66A10000668B0E0000669966F7F96650585A".to_owned()),
        ("div: qbopt absorbed, register", "66586659669966F7F96650585A".to_owned()),
        // ---- B$RMI4, MOD by 256: answers from dx, subtract-back sign fixup
        (
            "rmi4: stock",
            call4(concat!(
                "558BEC535733FF8B46080BC07D11",
                "8B460C0BC07D10",
                "0BC075188B4E0A8B460833D2F7F1",
                "8B4606F7F18BC233D2",
                "4F7943EB48",
                "5F5B5DCA0800"
            )),
        ),
        ("rmi4: qbopt absorbed, memory", "66A10000668B0E0000669966F7F9668BC26650585A".to_owned()),
        ("rmi4: qbopt absorbed, register", "66586659669966F7F9668BC26650585A".to_owned()),
    ])
});

/// B$DVI4's other half: a divisor over 16 bits is shifted right one bit per
/// pass, which a straight line walk cannot price. Scored separately, per pass.
pub const DVI4_LOOP: &str = "D1EBD1D9D1EAD1D80BDB75F4";

pub fn report(name: &str, hexs: &str) -> (String, usize, Totals, Vec<Detail>) {
    let rows = disasm(hexs);
    let (total, detail) = score(&rows);
    (name.to_owned(), rows.len(), total, detail)
}

/// `format(x, "g")`.
fn _g(x: f64) -> String {
    if x == 0.0 {
        return if x.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    let sci = format!("{x:.5e}");
    let (mantissa, exponent) = sci.split_once('e').expect("exponent form");
    let exponent: i32 = exponent.parse().expect("an integer exponent");
    let strip = |text: String| {
        if text.contains('.') {
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            text
        }
    };
    if (-4..6).contains(&exponent) {
        strip(format!("{x:.*}", (5 - exponent) as usize))
    } else {
        let sign = if exponent < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", strip(mantissa.to_owned()), exponent.abs())
    }
}

fn _header() -> String {
    format!("{:24}{:>4}  ", "case", "ins") + &ARCHS.iter().map(|a| format!("{a:>7}")).collect::<String>()
}

pub fn table(title: &str, which: usize, cases: &IndexMap<&'static str, String>) {
    println!();
    println!("{title}");
    println!("{}", _header());
    println!("{}", "-".repeat(28 + 7 * ARCHS.len()));
    for (name, hexs) in cases {
        let (_, cnt, tot, _) = report(name, hexs);
        let figures: Vec<f64> = if which == 0 { tot.0.iter().map(|&t| t as f64).collect() } else { tot.1 };
        println!("{name:24}{cnt:>4}  {}", figures.iter().map(|&t| format!("{:>7}", _g(t))).collect::<String>());
    }
}

/// `python -m qbopt.cycles.cycles [hex]`.
pub fn main(argv: &[String]) -> i32 {
    if let Some(hexs) = argv.first() {
        let (_, cnt, tot, det) = report("argv", hexs);
        println!("{cnt} instructions");
        for (txt, kind, c, notes) in &det {
            let costs = c.iter().map(i64::to_string).collect::<Vec<_>>().join(", ");
            println!("   {txt:32} {kind:12} [{costs}] {}", notes.join(" "));
        }
        let alone = ARCHS.iter().zip(&tot.0).map(|(a, t)| format!("{a}={t}")).collect::<Vec<_>>();
        let bulk = ARCHS.iter().zip(&tot.1).map(|(a, t)| format!("{a}={}", _g(*t))).collect::<Vec<_>>();
        println!("   alone {}", alone.join(" "));
        println!("   bulk  {}", bulk.join(" "));
        return 0;
    }

    table("standing alone -- the sequence waits for its own dependencies", 0, &CASES);
    table("back to back -- the machine has other work to overlap the waiting", 1, &CASES);
    let (_, n, t, _) = report("loop", DVI4_LOOP);
    println!();
    println!("B$DVI4's normalising loop, one pass of {n} instructions:");
    println!("   {}", ARCHS.iter().zip(&t.0).map(|(a, c)| format!("{a}={c}")).collect::<Vec<_>>().join("  "));
    println!("   it runs once per significant bit of the divisor above 16, so a");
    println!("   divisor near 2^31 costs fifteen of these on top of the figures");
    println!("   above -- which are its short path, not its worst one.");
    0
}

#[cfg(test)]
mod tests {
    use iced_x86::Register;

    use super::*;
    use crate::backend::{cpu, schedule};
    use crate::model::{ir, lir};

    #[test]
    fn test_memory_pop_is_not_priced_as_a_register_pop() {
        assert_eq!(classify("pop", "dword [bp-4]", "668f46fc"), "pop_m");
        assert_eq!(cpu::profile("386").unwrap().cost("pop_m").unwrap(), 5);
        assert!(cpu::names().iter().all(|name| cpu::profile(*name).unwrap().prices("pop_m")));
    }

    #[test]
    fn test_memory_compare_is_priced_as_a_read_not_a_read_modify_write() {
        assert_eq!(classify("cmp", "word [bp-4],0", "837efc00"), "alu_rm");
        assert_eq!(classify("test", "word [bp-4],1", "f746fc0100"), "alu_rm");
        assert_eq!(classify("add", "word [bp-4],1", "8346fc01"), "alu_mr");
    }

    #[test]
    fn test_scalar_double_shifts_use_the_integer_shift_price() {
        for mnemonic in ["shld", "shrd"] {
            assert_eq!(classify(mnemonic, "edx,eax,16", ""), "shift_ri");
            let reg = |register| ir::Loc::Reg(ir::Reg { register, width: 4 });
            let what = ir::Semantics {
                name: Some(mnemonic.to_owned()),
                dests: vec![reg(Register::EDX)],
                sources: vec![
                    reg(Register::EDX),
                    reg(Register::EAX),
                    ir::Loc::Imm(ir::Imm { value: 16, width: 1, address: None }),
                ],
                ..ir::Semantics::new(ir::Operation::Funnel)
            };
            let one = lir::Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]);
            assert_eq!(schedule::_form(&one), "shift_ri");
            assert_eq!(schedule::_pair_class(&one), "np");
        }
    }

    /// The D1 shift by one was priced as the two-clock imm8 form; it takes
    /// three on the 486, and `add r,r` one.
    #[test]
    fn test_a_shift_by_one_is_priced_as_the_three_clock_form() {
        assert_eq!(classify("shl", "ax,1", "d1e0"), "shift_r1");
        assert_eq!(classify("shl", "ax,2", "c1e002"), "shift_ri");
        assert_eq!(classify("shl", "word [bp-4],1", "d166fc"), "shift_ri");
        let reg = || ir::Loc::Reg(ir::Reg { register: Register::AX, width: 2 });
        let what = ir::Semantics {
            name: Some("shl".to_owned()),
            dests: vec![reg()],
            sources: vec![reg(), ir::Loc::Imm(ir::Imm { value: 1, width: 1, address: None })],
            ..ir::Semantics::new(ir::Operation::Binary)
        };
        let one = lir::Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]);
        assert_eq!(schedule::_form(&one), "shift_r1");
        assert_eq!(COST["shift_r1"][0], 3);
    }

    #[test]
    fn test_complete_far_pointer_loads_share_one_priced_form() {
        for mnemonic in ["les", "lfs", "lgs"] {
            assert_eq!(classify(mnemonic, "bx,[bp+6]", ""), "les");
            assert!(cpu::names().iter().all(|name| cpu::profile(*name).unwrap().prices("les")));
        }
    }

    #[test]
    fn test_x87_exchange_is_explicitly_priced_for_every_cpu() {
        let expected = [("386", 18), ("486", 4), ("P5", 1), ("P6", 0), ("K5", 2), ("K6", 2), ("K7", 2), ("Core", 0)];
        assert_eq!(classify("fxch", "st1", "d9c9"), "x87_exchange");
        let got: Vec<(&str, i64)> =
            cpu::names().into_iter().map(|name| (name, cpu::profile(name).unwrap().cost("x87_exchange").unwrap())).collect();
        assert_eq!(got, expected);
    }

    /// Python's `report()` over every case: count, alone, bulk and notes.
    const PYTHON: &str = "\
and: BC halves| 6 [8, 8, 10, 7, 7, 9, 11] [8.0, 8.0, 2.0, 1.5, 2.0, 2.0, 1.5] []
and: widened| 5 [12, 11, 10, 7, 7, 9, 11] [12.0, 11.0, 1.7, 1.2, 1.7, 1.7, 1.2] []
and: widened, no dx| 3 [7, 7, 10, 7, 7, 9, 11] [7.0, 7.0, 1.0, 0.8, 1.0, 1.0, 0.8] []
chain: BC halves| 10 [16, 16, 18, 13, 13, 15, 19] [16.0, 16.0, 3.3, 2.5, 3.3, 3.3, 2.5] []
chain: widened| 5 [13, 13, 18, 13, 13, 15, 19] [13.0, 13.0, 1.7, 1.2, 1.7, 1.7, 1.2] []
mul: stock fast| 14 [68, 34, 45, 14, 15, 17, 46] [68.0, 34.0, 45.3, 14.5, 15.3, 17.3, 46.5] []
mul: stock full| 22 [103, 62, 48, 16, 18, 20, 48] [103.0, 62.0, 48.0, 16.5, 18.0, 20.0, 48.5] []
mul: mgl call| 11 [82, 35, 50, 14, 14, 16, 49] [82.0, 35.0, 50.3, 13.8, 14.3, 16.3, 48.8] ['lcp']
mul: mgl pow2| 6 [11, 8, 4, 3, 3, 4, 5] [11.0, 8.0, 2.0, 1.5, 2.0, 2.0, 1.5] []
mul: qbopt absorbed, memory| 5 [40, 18, 14, 11, 10, 13, 14] [40.0, 18.0, 1.7, 1.2, 1.7, 1.7, 1.2] []
mul: qbopt absorbed, register| 6 [47, 19, 11, 9, 8, 10, 10] [47.0, 19.0, 2.0, 1.5, 2.0, 2.0, 1.5] []
cmp: stock| 20 [74, 34, 51, 18, 20, 22, 52] [74.0, 34.0, 50.7, 18.5, 19.7, 21.7, 51.5] []
cmp: mgl call| 11 [59, 26, 44, 14, 14, 16, 46] [59.0, 26.0, 44.3, 13.8, 14.3, 16.3, 45.8] []
cmp: mgl inlined| 6 [11, 9, 7, 5, 5, 6, 8] [11.0, 9.0, 2.0, 1.5, 2.0, 2.0, 1.5] []
cmp: qbopt absorbed, memory| 4 [12, 9, 11, 8, 8, 9, 12] [12.0, 9.0, 1.3, 1.0, 1.3, 1.3, 1.0] []
cmp: qbopt absorbed, stack| 10 [19, 15, 25, 13, 14, 18, 23] [19.0, 15.0, 3.3, 2.5, 3.3, 3.3, 2.5] ['partial-reg']
div256: stock| 33 [138, 91, 97, 67, 69, 71, 95] [138.0, 91.0, 97.0, 66.8, 69.0, 71.0, 94.8] []
div256: mgl call| 15 [111, 79, 90, 56, 56, 57, 76] [111.0, 79.0, 90.3, 56.5, 56.3, 57.3, 75.5] ['lcp']
div256: mgl shift| 14 [66, 32, 45, 14, 15, 17, 46] [66.0, 32.0, 45.3, 14.5, 15.3, 17.3, 46.5] []
div: qbopt absorbed, memory| 7 [62, 58, 48, 44, 43, 43, 33] [62.0, 58.0, 47.0, 43.5, 43.0, 42.0, 30.5] ['lcp']
div: qbopt absorbed, register| 7 [68, 58, 48, 44, 43, 42, 32] [68.0, 58.0, 47.0, 43.5, 43.0, 42.0, 30.5] ['lcp']
rmi4: stock| 31 [133, 89, 96, 66, 68, 70, 94] [133.0, 89.0, 96.3, 66.2, 68.3, 70.3, 94.2] []
rmi4: qbopt absorbed, memory| 8 [64, 60, 48, 44, 43, 43, 33] [64.0, 60.0, 47.3, 43.8, 43.3, 42.3, 30.8] ['lcp']
rmi4: qbopt absorbed, register| 8 [70, 60, 48, 44, 43, 42, 32] [70.0, 60.0, 47.3, 43.8, 43.3, 42.3, 30.8] ['lcp']
loop| 6 [16, 6, 2, 2, 2, 2, 2] [16.0, 6.0, 2.0, 1.5, 2.0, 2.0, 1.5] []
";

    #[test]
    fn test_scores_match_python() {
        let mut cases: Vec<(&str, String)> = CASES.iter().map(|(n, h)| (*n, h.clone())).collect();
        cases.push(("loop", DVI4_LOOP.to_owned()));
        let mut got = String::new();
        for (name, hexs) in &cases {
            let (_, cnt, (lone, bulk), det) = report(name, hexs);
            let notes: BTreeSet<&str> = det.iter().flat_map(|d| d.3.iter().copied()).collect();
            let lone = lone.iter().map(i64::to_string).collect::<Vec<_>>().join(", ");
            let bulk = bulk.iter().map(|&x| crate::support::pyrepr::float(x)).collect::<Vec<_>>().join(", ");
            let notes = notes.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(", ");
            got += &format!("{name}| {cnt} [{lone}] [{bulk}] [{notes}]\n");
        }
        assert_eq!(got, PYTHON);
    }

    #[test]
    fn test_g_matches_python_format() {
        assert_eq!(_g(12.0), "12");
        assert_eq!(_g(45.3), "45.3");
        assert_eq!(_g(0.8), "0.8");
        assert_eq!(_g(1234567.0), "1.23457e+06");
    }
}
