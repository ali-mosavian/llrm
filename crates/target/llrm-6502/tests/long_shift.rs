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

/// The same at every level: a shift does not depend on it.
#[test]
fn a_long_shifted_by_a_few_bits_is_a_chain() {
    // (op, bits, limit): the chain's bytes
    let kernels = [("shl", 2, 25), ("lshr", 2, 24), ("shl", 9, 24), ("lshr", 9, 28), ("shl", 10, 32), ("lshr", 10, 36)];
    let mut over = Vec::new();
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        for (op, by, limit) in kernels {
            let (bytes, body) = bytes_of(op, by, level);
            println!("ROW {op} {by} {level:?}: {bytes} (limit {limit})");
            if bytes > limit {
                over.push(format!("{op} i32 by {by} at {level:?}: {bytes} B > {limit}\n{body}"));
            }
        }
    }
    assert!(over.is_empty(), "{}", over.join("\n"));
}
