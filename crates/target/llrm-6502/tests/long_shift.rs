//! A long shifted by 2, 9 or 10 bits is a carry chain through its two words,
//! one step a bit (llvm-mos's `G_SHLE`/`G_LSHRE`). The type legalizer shifted
//! each word by the whole count and joined them with `ora`: `x << 2` was 34 B
//! and `x << 9` 36 B where the chain is 25 and 24 (llvm-mos 22 and 23).

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

/// The bytes of `k`, a long shifted by `by` bits, and its text.
fn bytes_of(
    op: &str,
    by: u32,
    level: Level,
) -> (u32, String) {
    let name = format!("k_{op}_{by}");
    let ir = format!(
        "target datalayout = \"{}\"
define i32 @{name}(i32 %x) {{
  %y = {op} i32 %x, {by}
  ret i32 %y
}}
",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    // the kernel and the routines the outliner made of it, which follow it
    let body: String =
        text[from..].lines().filter(|line| !line.starts_with(".segment")).collect::<Vec<_>>().join("\n") + "\n";
    (estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes, body)
}

/// What is wrong with the text of `k`, a long shifted by `by` bits: a per-bit
/// loop (a branch, a runtime shift routine), the `ora` that joined the shifted
/// words, a chain that is not one shift step a bit on each byte it moves. The
/// outliner's routines (`__outl_N`, called where the steps were) count once
/// for each call.
fn chain_problems(
    body: &str,
    by: u32,
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut routines = llrm_support::hash::HashMap::<&str, u32>::default();
    let (mut kernel, mut current) = (0u32, None::<&str>);
    let shifts = |line: &str| matches!(
        line.split_whitespace().next(),
        Some("asl" | "lsr" | "ror" | "rol")
    );
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(label) = line.strip_suffix(':') {
            current = label.starts_with("__outl_").then_some(label);
            continue;
        }
        let op = line.split_whitespace().next().unwrap_or("");
        if op.starts_with('b') && op != "brk" && op != "bit" {
            problems.push(format!("a branch: {line}"));
        }
        if op == "ora" {
            problems.push(format!("the words are joined with ora: {line}"));
        }
        if op == "jsr" && !line.contains("__outl_") {
            problems.push(format!("a runtime shift: {line}"));
        }
        if shifts(line) {
            match current {
                Some(routine) => *routines.entry(routine).or_default() += 1,
                None => kernel += 1,
            }
        }
    }
    let calls = |routine: &str| body.matches(&format!("jsr {routine}\n")).count() as u32;
    let steps = kernel + routines.iter().map(|(routine, n)| n * calls(routine)).sum::<u32>();
    // a rotate step for each bit the shift leaves after whole bytes, and no
    // more than one on each of the four bytes for every bit of the count
    let bits = by % 8;
    if steps < bits || steps > 4 * by {
        problems.push(format!("{steps} shift steps for {by} bits; a chain has {bits} to {}", 4 * by));
    }
    problems
}

/// The same at every level: a shift does not depend on it. A byte limit would
/// follow the outliner's choices (`lshr` by 2 is 24 B outlined, 25 inline).
#[test]
fn a_long_shifted_by_a_few_bits_is_a_chain() {
    let kernels = [("shl", 2), ("lshr", 2), ("shl", 9), ("lshr", 9), ("shl", 10), ("lshr", 10)];
    let mut over = Vec::new();
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        for (op, by) in kernels {
            let (_, body) = bytes_of(op, by, level);
            let problems = chain_problems(&body, by);
            if !problems.is_empty() {
                over.push(format!("{op} i32 by {by} at {level:?}: {}\n{body}", problems.join("; ")));
            }
        }
    }
    assert!(over.is_empty(), "{}", over.join("\n"));
}

/// The text the type legalizer wrote (each word shifted by the whole count and
/// joined with `ora`, 34 B for `x << 2`) and a per-bit loop are what the check
/// reports.
#[test]
fn the_check_reports_an_ora_join_and_a_loop() {
    let joined = "k_shl_2:\n  sta rc4\n  stx rc5\n  lda rc2\n  asl a\n  asl a\n  sta rc2\n  lda rc3\n  asl a\n  asl a\n  sta rc3\n  lda rc5\n  lsr a\n  lsr a\n  lsr a\n  lsr a\n  lsr a\n  lsr a\n  ora rc2\n  sta rc2\n  lda rc4\n  asl a\n  asl a\n  ldx rc5\n  rts\n";
    assert!(chain_problems(joined, 2).iter().any(|p| p.contains("ora")), "{:?}", chain_problems(joined, 2));
    let looped = "k_shl_2:\n  ldy #2\n@bit:\n  asl rc4\n  rol rc5\n  dey\n  bne @bit\n  rts\n  jsr __ashlsi3\n";
    let problems = chain_problems(looped, 2);
    assert!(
        problems.iter().any(|p| p.contains("a branch")) && problems.iter().any(|p| p.contains("runtime")),
        "{problems:?}"
    );
}

/// A long shifted by 5 to 7 bits at -O2 and -O3 was a loop of word shifts, the
/// type legalizer's two (`x >> 5` 276 clocks a call); llvm-mos writes it as a
/// byte move and `8 - n` one-bit steps the other way (`shouldOverCorrect`).
/// No loop, no runtime routine, and the clocks of the chain.
#[test]
fn a_long_shifted_by_five_to_seven_bits_is_a_chain_at_speed_levels() {
    let mut over = Vec::new();
    for level in [Level::O2, Level::O3] {
        for by in 5..=7u32 {
            for op in ["shl", "lshr"] {
                let (bytes, body) = bytes_of(op, by, level);
                let estimate = estimate_of(&body);
                // `8 - by` one-bit steps over four bytes (20 clocks each) and
                // the byte moves around them
                let steps = 8 - by;
                let limit = u64::from(steps) * 20 + 90;
                if body.lines().map(str::trim).any(|line| line.starts_with('b') && !line.starts_with("bit")) {
                    over.push(format!("{op} {by} {level:?}: a loop\n{body}"));
                }
                if estimate > limit {
                    over.push(format!("{op} {by} {level:?}: {estimate} clocks over {limit} ({bytes} B)\n{body}"));
                }
            }
        }
    }
    assert!(over.is_empty(), "{}", over.join("\n"));
}

/// -Os keeps what it had: a long shifted by 3 to 7 bits is a call of the
/// runtime, not steps (the 81 programs grew 56 to 213 B when 3..7 bits were
/// unrolled steps there).
#[test]
fn a_long_shifted_by_three_to_seven_bits_is_not_steps_at_os() {
    for by in 3..=7u32 {
        for op in ["shl", "lshr"] {
            let (bytes, body) = bytes_of(op, by, Level::Os);
            assert!(bytes <= 12, "{op} {by} -Os is {bytes} B:\n{body}");
        }
    }
}

/// The estimated clocks of one call of the kernel in `body`.
fn estimate_of(body: &str) -> u64 {
    estimate(Cpu::Mos6502, body).unwrap_or_else(|e| panic!("{e}\n{body}")).cycles
}
