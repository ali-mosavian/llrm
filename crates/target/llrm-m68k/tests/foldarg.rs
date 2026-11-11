//! A loop bounded by a stack argument kept it in a register for the whole of
//! the loop (`move.l 4(%sp), %a1` and `cmp.l %a1, %d2`, a saved `%d2` as
//! well): 30 B against gcc's 22 B (bench/kernels/fib). The compare reads the
//! slot.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const LOOP: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %a = phi i32 [0, %entry], [%b, %loop]
  %b = phi i32 [1, %entry], [%s, %loop]
  %s = add i32 %a, %b
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %s
}"#;

fn compile(
    cpu: Cpu,
    size: bool,
) -> String {
    let module = llrm_mir::parse::module(LOOP).expect("parses");
    text_with(&module, "k", cpu, Options { size, ..Options::default() }).expect("compiles")
}

#[test]
fn a_loop_bound_in_a_stack_argument_is_compared_in_place_at_a_smaller_size() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = compile(cpu, true);
        assert!(out.contains("cmp.l\t(8,%sp)"), "{cpu:?}: the argument was loaded first:\n{out}");
        assert!(!out.contains("(8,%sp), %a"), "{cpu:?}: a register holds the argument:\n{out}");
    }
}

#[test]
fn the_same_loop_keeps_the_argument_in_a_register_at_speed() {
    let out = compile(Cpu::M68000, false);
    assert!(!out.contains("cmp.l\t(8,%sp)"), "a memory compare in the loop at speed:\n{out}");
}

/// The load that a `ble` reads the codes of is not folded away: the guard
/// `n < 1` tests the loaded argument, and the later compare read the slot.
/// `idiv_loop_os` at -Os returned 0 for `total(300)` (3454): nothing set the
/// codes `ble` read.
#[test]
fn a_load_whose_codes_a_branch_reads_stays() {
    let ir = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @f(i32 %n) {
entry:
  %c = icmp slt i32 %n, 1
  br i1 %c, label %exit, label %loop
loop:
  %i = phi i32 [ 1, %entry ], [ %i1, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %m = urem i32 %i, 7
  %s1 = add i32 %s, %m
  %i1 = add i32 %i, 1
  %d = icmp slt i32 %i, %n
  br i1 %d, label %loop, label %exit
exit:
  %r = phi i32 [ 0, %entry ], [ %s1, %loop ]
  ret i32 %r
}
"#;
    let module = llrm_mir::parse::module(ir).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module, "f", cpu, Options { size: true, ..Options::default() }).expect("compiles");
        let lines: Vec<&str> = out.lines().map(str::trim).collect();
        let branch = lines.iter().position(|l| l.starts_with('b') && !l.starts_with("bra")).expect("a branch");
        let before = lines[..branch].iter().rev().find(|l| !l.ends_with(':')).expect("an instruction");
        assert!(
            ["tst", "cmp", "move.l\t("].iter().any(|op| before.starts_with(op)),
            "{cpu:?}: the codes the branch reads are set by `{before}`:\n{out}"
        );
    }
}
