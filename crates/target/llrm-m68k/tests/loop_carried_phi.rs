//! The value a loop carries is joined with the phi result before the element
//! the loop loads (DEVIATE CO1): the maximum stays in one register.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::kernels::source;

/// `arrmax`: `if (p[i] > m) m = p[i]`. The phi of the new element and the old
/// maximum was joined with the element first (the pin's order puts the rare
/// path's block ahead of the loop's own copies), so the maximum lived in two
/// registers and each trip ended `move.l %d1, %d0` on the other path, or
/// `move.l %d0, %d1; bra` on this one: +6 bytes and +78 clocks over the spike
/// allocator's loop.
#[test]
fn test_the_maximum_a_loop_carries_stays_in_one_register() {
    let module = llrm_mir::parse::module(&source("arrmax")).expect("parses");
    let asm = text_with(&module, "k", Cpu::M68000, Options::PINNED).expect("compiles");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let head = lines.iter().position(|line| line.ends_with(':') && line.starts_with(".L")).expect("the loop's label");
    let label = lines[head].trim_end_matches(':');
    let back = lines.iter().rposition(|line| line.ends_with(label) && line.starts_with('b')).expect("branch back");
    let loop_body = lines[head..=back].join("\n");
    assert!(!loop_body.contains("bra"), "the loop jumps over a copy:\n{asm}");
}
