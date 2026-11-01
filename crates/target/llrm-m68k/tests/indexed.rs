//! `(An)+` and `-(An)`: the accesses the indexed combine forms, in the text and
//! in the value the program computes under `qemu-m68k` (the exit status, 8
//! bits). The pin sets no indexed action, so each loop here was a load or store
//! plus a separate pointer update, 3 instructions through a data register.

use std::path::PathBuf;

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn qemu() -> Option<PathBuf> {
    ["/usr/bin/qemu-m68k", "/usr/local/bin/qemu-m68k"].iter().map(PathBuf::from).find(|path| path.exists())
}

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// Exit status of `f(args)`; `None` without qemu.
fn status(
    ir: &str,
    args: &[i64],
    options: Options,
) -> Option<i32> {
    let qemu = qemu().or_else(|| {
        eprintln!("qemu-m68k is not installed: not run");
        None
    })?;
    let mut all = start("f", args);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, options).expect("compiles"));
    let path = support::unique_path("indexed");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    code
}

fn asm(
    ir: &str,
    options: Options,
) -> String {
    text_with(&module(ir), "f", Cpu::M68000, options).expect("compiles")
}

const RAW: Options = Options { mid_end: false, ..Options::PINNED };
const INDEXED: Options = Options { indexed: true, ..RAW };

/// Eight elements of `bits` bits: `buf[i] = i + 1` through a stepped pointer,
/// then their sum through another, `bits` wide; the sum is the answer, 36.
/// Upward the pointer moves after the access, downward (`down`) before it,
/// from the end.
fn fill_and_sum(
    bits: u32,
    down: bool,
) -> String {
    let step = bits / 8;
    let value = if bits == 32 {
        "  %v = add i32 %i, 1\n".to_owned()
    } else {
        format!("  %t = trunc i32 %i to i{bits}\n  %v = add i{bits} %t, 1\n")
    };
    let (enter, store, load, store_move, load_move) = if down {
        (
            "%end",
            "%pn",
            "%qn",
            format!("  %pn = getelementptr i8, ptr %p, i32 -{step}\n"),
            format!("  %qn = getelementptr i8, ptr %q, i32 -{step}\n"),
        )
    } else {
        (
            "%buf",
            "%p",
            "%q",
            format!("  %pn = getelementptr i8, ptr %p, i32 {step}\n"),
            format!("  %qn = getelementptr i8, ptr %q, i32 {step}\n"),
        )
    };
    let (store_before, store_after) = if down { (store_move.as_str(), "") } else { ("", store_move.as_str()) };
    let (load_before, load_after) = if down { (load_move.as_str(), "") } else { ("", load_move.as_str()) };
    format!(
        "define i32 @f() {{
entry:
  %buf = alloca [32 x i8], align 4
  %end = getelementptr i8, ptr %buf, i32 {total}
  br label %fill
fill:
  %i = phi i32 [ 0, %entry ], [ %i1, %fill ]
  %p = phi ptr [ {enter}, %entry ], [ %pn, %fill ]
{store_before}{value}  store i{bits} %v, ptr {store}, align 1
{store_after}  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, 8
  br i1 %c, label %fill, label %mid
mid:
  br label %sum
sum:
  %j = phi i32 [ 0, %mid ], [ %j1, %sum ]
  %s = phi i{bits} [ 0, %mid ], [ %s1, %sum ]
  %q = phi ptr [ {enter}, %mid ], [ %qn, %sum ]
{load_before}  %x = load i{bits}, ptr {load}, align 1
  %s1 = add i{bits} %s, %x
{load_after}  %j1 = add i32 %j, 1
  %d = icmp slt i32 %j1, 8
  br i1 %d, label %sum, label %done
done:
  %r = zext i{bits} %s1 to i32
  ret i32 %r
}}",
        total = 8 * step,
    )
}

fn forms(
    text: &str,
    marker: &str,
) -> usize {
    text.lines().filter(|line| line.contains(marker)).count()
}

/// A loop that walks a pointer up read and wrote it with a load or store and a
/// separate update (`move.l %a0,%d2; add.l #4,%d2; move.l %d2,%a0`). It must
/// walk it through `(An)+`, at byte, word and long, and the program must
/// still compute the same sum.
#[test]
fn a_pointer_walked_up_is_post_incremented_and_the_sum_is_unchanged() {
    for bits in [8, 16, 32] {
        let ir = fill_and_sum(bits, false);
        let text = asm(&ir, INDEXED);
        assert!(forms(&text, ")+") >= 2, "i{bits}: no post-increment:\n{text}");
        assert_eq!(forms(&asm(&ir, RAW), ")+"), 0, "i{bits}: the pin form has one");
        if let Some(code) = status(&ir, &[], INDEXED) {
            assert_eq!(code, 36, "i{bits}:\n{text}");
        }
    }
}

/// The same walking down, before the access: `-(An)`.
#[test]
fn a_pointer_walked_down_is_pre_decremented_and_the_sum_is_unchanged() {
    for bits in [8, 16, 32] {
        let ir = fill_and_sum(bits, true);
        let text = asm(&ir, INDEXED);
        assert!(forms(&text, "-(%a") >= 2, "i{bits}: no pre-decrement:\n{text}");
        assert_eq!(forms(&asm(&ir, RAW), "-(%a"), 0, "i{bits}: the pin form has one");
        if let Some(code) = status(&ir, &[], INDEXED) {
            assert_eq!(code, 36, "i{bits}:\n{text}");
        }
    }
}

/// The base is read again after the access that updates it: the tied pair must
/// leave the old value in a register of its own, or the second read sees the
/// updated one. `v + (q - p)` is 7 + 4.
#[test]
fn a_base_that_is_live_after_the_access_keeps_its_old_value() {
    // `%k` (0) keeps the base from being a frame index, which is never indexed
    let ir = "define i32 @f(i32 %k) {
entry:
  %buf = alloca [8 x i32], align 4
  %p = getelementptr i8, ptr %buf, i32 %k
  store volatile i32 7, ptr %p, align 4
  %v = load volatile i32, ptr %p, align 4
  %q = getelementptr i8, ptr %p, i32 4
  %a = ptrtoint ptr %p to i32
  %b = ptrtoint ptr %q to i32
  %d = sub i32 %b, %a
  %r = add i32 %v, %d
  ret i32 %r
}";
    let text = asm(ir, INDEXED);
    assert!(forms(&text, ")+") >= 1, "no post-increment:\n{text}");
    if let Some(code) = status(ir, &[0], INDEXED) {
        assert_eq!(code, 11, "{text}");
    }
}
