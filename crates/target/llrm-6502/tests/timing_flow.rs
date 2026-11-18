//! `estimate_weighted` read a line's mnemonic and branch target again each
//! time a backward branch's walk visited it: nested loops cost the square of
//! their number in `split_whitespace` (7% of the instructions of `idioms`
//! -O2). Each line is read once.

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::timing::{estimate, lines_read};

#[test]
fn nested_loops_read_each_line_once() {
    let depth = 200;
    let mut text = String::new();
    for i in 0..depth {
        text.push_str(&format!("L{i}:\n  nop\n"));
    }
    for i in (0..depth).rev() {
        text.push_str(&format!("  bne L{i}\n"));
    }
    text.push_str("  rts\n");
    let lines = text.lines().count();
    let before = lines_read();
    let priced = estimate(Cpu::Mos6502, &text).unwrap_or_else(|e| panic!("{e}"));
    let read = lines_read() - before;
    assert_eq!(priced.bytes, 3 * depth as u32 + 1, "bytes");
    // the same as before the lines were read once
    assert_eq!(priced.cycles, 990_556, "cycles");
    assert!(read <= 2 * lines, "{read} line reads for {lines} lines");
}
