//! A pointer stepped by a constant lived in a data register between two
//! copies (`move.l %a0, %d2; addq.l #6, %d2; move.l %d2, %a0`, 3 instructions,
//! a register and 10 bytes); it is one instruction in the address register,
//! chosen by cost. A counter decremented by `add #-1` is `subq`.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// Sums `n` longs read `stride` bytes apart.
fn walk(stride: i64) -> String {
    format!(
        "define i32 @f(ptr %p, i32 %n) {{
entry:
  br label %l
l:
  %q = phi ptr [ %p, %entry ], [ %q1, %l ]
  %i = phi i32 [ %n, %entry ], [ %i1, %l ]
  %s = phi i32 [ 0, %entry ], [ %s1, %l ]
  %v = load i32, ptr %q
  %s1 = add i32 %s, %v
  %q1 = getelementptr i8, ptr %q, i32 {stride}
  %i1 = sub i32 %i, 1
  %c = icmp eq i32 %i1, 0
  br i1 %c, label %x, label %l
x:
  ret i32 %s1
}}"
    )
}

fn text(
    ir: &str,
    cpu: Cpu,
) -> String {
    text_with(&module(ir), "f", cpu, Options::default()).expect("compiles")
}

/// The step is added in the address register: `addq` for 1 to 8, `lea` up to
/// 16 bits, `adda` beyond; the counter's `add #-1` is `subq`.
#[test]
fn a_pointer_step_is_one_instruction_in_the_address_register() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for (stride, want) in [(6, "addq.l\t#6, %a0"), (12, "lea\t(12,%a0), %a0"), (40000, "adda.l\t#40000, %a0")] {
            let out = text(&walk(stride), cpu);
            assert!(out.contains(want), "{cpu:?} stride {stride}: want {want}:\n{out}");
            assert!(!out.contains("%a0, %d") && !out.contains(", %a0\n\tmove"), "{cpu:?} stride {stride}:\n{out}");
        }
        let out = text(&walk(6), cpu);
        assert!(out.contains("subq.l\t#1, %d1") && !out.contains("add.l\t#-1"), "{cpu:?}:\n{out}");
    }
}

/// A pointer that steps once per outer iteration, past an inner loop, is a phi
/// of either register class; the pass left it as `move.l %a0, %d1; addq.l #1,
/// %d1; move.l %d1, %a0` (a byte checksum loop, 3 instructions per byte).
#[test]
fn a_pointer_stepped_after_an_inner_loop_is_one_addq() {
    let ir = "define i32 @f(ptr %p, i32 %n) {
entry:
  br label %outer
outer:
  %q = phi ptr [ %p, %entry ], [ %q1, %inner.end ]
  %i = phi i32 [ %n, %entry ], [ %i1, %inner.end ]
  %h = phi i32 [ 0, %entry ], [ %h2, %inner.end ]
  %b = load i8, ptr %q
  %z = zext i8 %b to i32
  %h0 = xor i32 %h, %z
  br label %inner
inner:
  %k = phi i32 [ 8, %outer ], [ %k1, %inner ]
  %g = phi i32 [ %h0, %outer ], [ %g1, %inner ]
  %g1 = shl i32 %g, 1
  %k1 = sub i32 %k, 1
  %c = icmp eq i32 %k1, 0
  br i1 %c, label %inner.end, label %inner
inner.end:
  %h2 = add i32 %g1, 1
  %q1 = getelementptr i8, ptr %q, i32 1
  %i1 = sub i32 %i, 1
  %e = icmp eq i32 %i1, 0
  br i1 %e, label %x, label %outer
x:
  ret i32 %h2
}";
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text(ir, cpu);
        let bounced = out.lines().any(|line| line.starts_with("\tmove.l\t%a") && line.contains(", %d"));
        assert!(out.contains("addq.l\t#1, %a") && !bounced, "{cpu:?}:\n{out}");
    }
}

/// Exit status of `f(args)`; `None` without an optional qemu.
fn status(
    ir: &str,
    args: &[i64],
) -> Option<i32> {
    let qemu = support::qemu()?;
    let mut all = start("f", args);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, Options::default()).expect("compiles"));
    let path = support::unique_path("addrarith");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert!(code.is_some(), "f({args:?}) ended on a signal");
    code
}

/// A function that fills a buffer on its stack with `offset / 4 + 1` at each
/// multiple of 4, then sums `n` longs read `stride` bytes apart.
fn filled_walk(stride: i64) -> String {
    format!(
        "define i32 @f(i32 %n) {{
entry:
  %buf = alloca [512 x i8]
  br label %fill
fill:
  %o = phi i32 [ 0, %entry ], [ %o1, %fill ]
  %p = getelementptr i8, ptr %buf, i32 %o
  %w = lshr i32 %o, 2
  %v = add i32 %w, 1
  store i32 %v, ptr %p
  %o1 = add i32 %o, 4
  %done = icmp eq i32 %o1, 512
  br i1 %done, label %go, label %fill
go:
  br label %l
l:
  %q = phi ptr [ %buf, %go ], [ %q1, %l ]
  %i = phi i32 [ %n, %go ], [ %i1, %l ]
  %s = phi i32 [ 0, %go ], [ %s1, %l ]
  %x = load i32, ptr %q
  %s1 = add i32 %s, %x
  %q1 = getelementptr i8, ptr %q, i32 {stride}
  %i1 = sub i32 %i, 1
  %c = icmp eq i32 %i1, 0
  br i1 %c, label %x, label %l
x:
  ret i32 %s1
}}"
    )
}

/// The values: each kind of step (`addq`, `lea`) reads the words it should, and
/// the counter that is `subq` counts the right number of trips.
#[test]
fn walking_by_each_kind_of_step_reads_the_same_words() {
    for stride in [4i64, 8, 12, 100] {
        for n in [1i64, 2, 3, 5] {
            let want: i64 = (0..n).map(|k| k * stride / 4 + 1).sum();
            if let Some(code) = status(&filled_walk(stride), &[n]) {
                assert_eq!(i64::from(code), want & 0xFF, "stride {stride}, n {n}");
            }
        }
    }
}
