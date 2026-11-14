//! The lz4 kernel's exit, its counter kept in `y`, and its long branches.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

fn lz4() -> String {
    let ir = format!("target datalayout = \"{}\"\n{}", datalayout(), include_str!("bench/lz4.ll"));
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("lz4:").unwrap_or_else(|| panic!("no lz4 in\n{text}"));
    text[from..].split(".segment").next().unwrap().to_owned()
}

/// The exit left `lda rc20; pha; pla` (a save around a dead store), and the
/// `pla` hid `y` from `load_straight_into_a`, so `ldy rc22; tya` stayed: 4 B
/// and 7 clocks on the way out, and a counter stepped through `y` cost 3 B
/// more than `dec rc22` in the loop.
#[test]
fn lz4_has_no_empty_save_and_no_counter_through_y() {
    let body = lz4();
    let lines: Vec<&str> = body.lines().map(str::trim).collect();
    let saves = lines.iter().filter(|line| **line == "pha").count();
    let through_y = lines.windows(2).filter(|w| w[0].starts_with("ldy rc") && w[1] == "tya").count();
    assert!(saves == 0 && through_y == 0, "{saves} pha, {through_y} ldy;tya\n{body}");
}

/// Two branches out of reach of the exit each made `bcc skip; jmp exit`: the
/// second can reach the first's `jmp` with a 2 B branch, 3 B less.
#[test]
fn lz4_long_branches_share_a_jump() {
    let body = lz4();
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    let shared = body.lines().filter(|line| line.starts_with("lz4_far") && line.ends_with("t:")).count();
    assert!(shared >= 1 && bytes <= 312, "{shared} shared jumps, {bytes} B\n{body}");
}
