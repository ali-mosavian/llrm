//! `a[i]` of 32-bit elements at 68020: `lsl.l #2, %d1; move.l (0,%a0,%d1), %d0`
//! where the 68020's scaled index is `move.l (%a0,%d1.l*4), %d0`: 2 bytes and a
//! register move more per access (bsearch, arrsum, copy, partition against gcc
//! -m68020). The 68000 has no scale.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const LOAD: &str = "
define i32 @f(ptr %a, i32 %i) {
  %p = getelementptr i32, ptr %a, i32 %i
  %v = load i32, ptr %p
  ret i32 %v
}";

fn text(cpu: Cpu) -> String {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{LOAD}")).expect("parses");
    text_with(&module, "f", cpu, Options::default()).expect("compiles")
}

#[test]
fn an_i32_index_is_scaled_by_the_address_at_68020() {
    for cpu in [Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060] {
        let out = text(cpu);
        assert!(out.contains("*4"), "{cpu:?}: no scaled index:\n{out}");
        assert!(!out.contains("lsl.l\t#2"), "{cpu:?}: the shift is left:\n{out}");
    }
}

#[test]
fn the_68000_and_68010_have_no_scale() {
    for cpu in [Cpu::M68000, Cpu::M68010] {
        let out = text(cpu);
        assert!(!out.contains('*'), "{cpu:?}: a scale on a CPU that has none:\n{out}");
    }
}

/// `table[i]` of 2-, 4- and 8-byte elements: the scale must be the element's,
/// and the right element must come back (a wrong scale field would still
/// assemble).
const TABLES: &str = "
define i32 @f(i32 %i, i32 %kind) {
  %t8 = alloca [8 x i64], align 4
  %t4 = alloca [8 x i32], align 4
  %t2 = alloca [8 x i16], align 4
  br label %fill
fill:
  %n = phi i32 [ 0, %0 ], [ %n1, %fill ]
  %p8 = getelementptr i64, ptr %t8, i32 %n
  %p4 = getelementptr i32, ptr %t4, i32 %n
  %p2 = getelementptr i16, ptr %t2, i32 %n
  %v = add i32 %n, 11
  store volatile i64 0, ptr %p8, align 4
  %v8 = zext i32 %v to i64
  store volatile i64 %v8, ptr %p8, align 4
  store volatile i32 %v, ptr %p4, align 4
  %v2 = trunc i32 %v to i16
  store volatile i16 %v2, ptr %p2, align 2
  %n1 = add i32 %n, 1
  %done = icmp eq i32 %n1, 8
  br i1 %done, label %pick, label %fill
pick:
  %q8 = getelementptr i64, ptr %t8, i32 %i
  %q4 = getelementptr i32, ptr %t4, i32 %i
  %q2 = getelementptr i16, ptr %t2, i32 %i
  %a8 = load volatile i64, ptr %q8, align 4
  %a4 = load volatile i32, ptr %q4, align 4
  %a2 = load volatile i16, ptr %q2, align 2
  %b8 = trunc i64 %a8 to i32
  %b2 = zext i16 %a2 to i32
  %is8 = icmp eq i32 %kind, 8
  %is4 = icmp eq i32 %kind, 4
  %r1 = select i1 %is4, i32 %a4, i32 %b2
  %r = select i1 %is8, i32 %b8, i32 %r1
  ret i32 %r
}";

#[test]
fn the_scaled_element_is_the_right_one_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{TABLES}")).expect("parses");
    let text = text_with(&module, "f", Cpu::M68020, Options::default()).expect("compiles");
    for scale in ["*2", "*4", "*8"] {
        assert!(text.contains(scale), "no {scale} operand:\n{text}");
    }
    for (i, kind) in [(0_i64, 2_i64), (3, 2), (7, 2), (1, 4), (5, 4), (2, 8), (6, 8)] {
        let mut all = start("f", &[i, kind]);
        all.extend(items_with(&module, "f", Cpu::M68020, Options::default()).unwrap());
        let path = support::unique_path("scaled");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68020"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some((i + 11) as i32), "element {i} of the {kind}-byte table");
    }
}

/// `a[i]` of `unsigned short` or `short` widened to `int`: the load is an
/// extending pseudo (`MOVZXd32f16`, `MOVSXd32f16`) with no scaled sibling, so
/// the shift stayed (`lsl.l #1; move.w (0,%a0,%d0)`: 2 B and a cycle over
/// gcc's `move.w (%a0,%d0.l*2)`) on every widened `short` array read.
const WIDENED: &str = "
define i32 @z(ptr %a, i32 %i) {
  %p = getelementptr i16, ptr %a, i32 %i
  %v = load i16, ptr %p
  %w = zext i16 %v to i32
  ret i32 %w
}
define i32 @s(ptr %a, i32 %i) {
  %p = getelementptr i16, ptr %a, i32 %i
  %v = load i16, ptr %p
  %w = sext i16 %v to i32
  ret i32 %w
}";

#[test]
fn a_widened_short_element_is_scaled_by_the_address() {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{WIDENED}")).expect("parses");
    for name in ["z", "s"] {
        let out = text_with(&module, name, Cpu::M68020, Options::default()).expect("compiles");
        assert!(out.contains("*2"), "{name}: no scaled index:\n{out}");
        assert!(!out.contains("lsl.l\t#1"), "{name}: the shift is left:\n{out}");
    }
}

/// Elements {-2, 300} of a `short` table, widened and shifted down, as a byte.
const WIDENED_RUN: &str = "
define i32 @z(i32 %i, i32 %sh) {
  %t = alloca [2 x i16], align 2
  store volatile i16 -2, ptr %t, align 2
  %p1 = getelementptr i16, ptr %t, i32 1
  store volatile i16 300, ptr %p1, align 2
  %q = getelementptr i16, ptr %t, i32 %i
  %v = load volatile i16, ptr %q
  %w = zext i16 %v to i32
  %r = lshr i32 %w, %sh
  %m = and i32 %r, 255
  ret i32 %m
}
define i32 @s(i32 %i, i32 %sh) {
  %t = alloca [2 x i16], align 2
  store volatile i16 -2, ptr %t, align 2
  %p1 = getelementptr i16, ptr %t, i32 1
  store volatile i16 300, ptr %p1, align 2
  %q = getelementptr i16, ptr %t, i32 %i
  %v = load volatile i16, ptr %q
  %w = sext i16 %v to i32
  %r = lshr i32 %w, %sh
  %m = and i32 %r, 255
  ret i32 %m
}";

#[test]
fn a_widened_short_element_is_the_right_one_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    let module =
        llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{WIDENED_RUN}")).expect("parses");
    // (function, element, shift, byte): zero extension leaves the high half 0,
    // sign extension fills it
    for (name, i, sh, want) in
        [("z", 0_i64, 16_i64, 0), ("s", 0, 16, 255), ("z", 0, 0, 254), ("z", 1, 0, 44), ("s", 1, 8, 1)]
    {
        let mut all = start(name, &[i, sh]);
        all.extend(items_with(&module, name, Cpu::M68020, Options::default()).unwrap());
        let path = support::unique_path("scaledext");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68020"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(want), "{name} element {i} >> {sh}");
    }
}

/// A stack array of `short`s read through an index the selector put in the
/// base slot of a zero-extending byte load (`seed 44` of the difftest corpus,
/// reduced): the fold bailed out because the `lea` was copied to a data-only
/// register, and left `lsl.l #1`, `move.l %d0, %a1`, `move.l %a0, %d0`.
#[test]
fn a_shifted_base_of_an_extending_load_is_scaled_too() {
    let module = llrm_mir::parse::module(include_str!("data/scaled-local-index.ll")).expect("parses");
    let out = text_with(&module, "test", Cpu::M68020, Options::default()).expect("compiles");
    assert!(out.contains("*2"), "no scaled index:\n{out}");
    assert!(!out.contains("lsl.l\t#1"), "the shift is left:\n{out}");
}
