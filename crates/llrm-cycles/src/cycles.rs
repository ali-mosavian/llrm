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
use std::sync::LazyLock;

use iced_x86::{Decoder, DecoderOptions, Formatter, NasmFormatter};

use crate::timings::{
    ARCHS, COST, INORDER, ISSUE, LATENCY, LCP_STALL, PARTIAL_STALL, PREFIX,
};
use llrm_support::hash::IndexMap;

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

/// ndisasm's text for 16-bit code, decoded in process: the installed ndisasm
/// is not an instrument, 3.01 prints `shr bx,1` as `shr bx,0x0`.
pub fn disasm(hexs: &str) -> Vec<Row> {
    let b = _fromhex(hexs);
    let mut decoder = Decoder::new(16, &b, DecoderOptions::NONE);
    let mut formatter = NasmFormatter::new();
    let options = formatter.options_mut();
    options.set_hex_prefix("0x");
    options.set_hex_suffix("");
    options.set_uppercase_hex(false);
    options.set_small_hex_numbers_in_decimal(true);
    options.set_add_leading_zero_to_hex_numbers(false);
    options.set_branch_leading_zeros(false);
    options.set_show_branch_size(false);
    options.set_space_after_operand_separator(false);
    let mut rows = Vec::new();
    for insn in &mut decoder {
        let at = insn.ip() as usize;
        let raw: String = b[at..at + insn.len()].iter().map(|byte| format!("{byte:02X}")).collect();
        let mut mnemonic = String::new();
        formatter.format_mnemonic(&insn, &mut mnemonic);
        let mut ops = String::new();
        formatter.format_all_operands(&insn, &mut ops);
        rows.push((raw, mnemonic.to_lowercase(), ops.trim().to_lowercase()));
    }
    rows
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
pub fn _g(x: f64) -> String {
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

