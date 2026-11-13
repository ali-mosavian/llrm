//! Regression: the first call's cleanup (`addq.l #8, %sp`) ran after the second
//! call's arguments were pushed, popped them, and the second call read the
//! first call's arguments (struct_with_i64 -O1 on the 68000: `y.a * 100000`
//! came out as `x.a * 1000`, 2036 for 201036; compare_to_int64 -O2: `4 2` for
//! `39196 4901`). The carry of the 64-bit add is a diamond between the two
//! calls, and the pushes were made above it.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
%struct.S = type { i8, i64 }
define i64 @sum(ptr byval(%struct.S) %0, ptr byval(%struct.S) %1) {
  %3 = getelementptr i8, ptr %0, i32 8
  %4 = load i64, ptr %3
  %5 = getelementptr i8, ptr %1, i32 8
  %6 = load i64, ptr %5
  %7 = add i64 %6, %4
  %8 = load i8, ptr %0
  %9 = sext i8 %8 to i32
  %10 = mul i32 %9, 1000
  %11 = sext i32 %10 to i64
  %12 = add i64 %7, %11
  %13 = load i8, ptr %1
  %14 = sext i8 %13 to i32
  %15 = mul i32 %14, 100000
  %16 = sext i32 %15 to i64
  %17 = add i64 %12, %16
  ret i64 %17
}
";

/// The stack depth at each call, from the pushes and the cleanups in the text
/// (a prologue's `movem` is not counted): the top of the stack at a call must
/// be what was pushed since the call before it, if anything was.
fn calls_read_their_own_arguments(text: &str) -> Result<(), String> {
    let (mut stack, mut group, mut pushed_here) = (Vec::<usize>::new(), 0usize, false);
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("pea\t") || (line.starts_with("move.l\t") && line.ends_with(", -(%sp)")) {
            stack.push(group);
            pushed_here = true;
        } else if let Some(bytes) = popped(line) {
            for _ in 0..bytes / 4 {
                stack.pop();
            }
        } else if line.starts_with("jsr\t") {
            if pushed_here && stack.last() != Some(&group) {
                return Err(format!("the call at `{line}` finds another call's arguments on top"));
            }
            group += 1;
            pushed_here = false;
        }
    }
    Ok(())
}

/// The bytes `line` takes off the stack pointer, if it is a cleanup.
fn popped(line: &str) -> Option<usize> {
    let rest = line.strip_prefix("addq.l\t#").or_else(|| line.strip_prefix("adda.l\t#"))?;
    let (bytes, to) = rest.split_once(", ")?;
    (to == "%sp")
        .then(|| bytes.parse().ok())
        .flatten()
        .or_else(
            || {
                let rest = line.strip_prefix("lea\t(")?;
                let (bytes, _) = rest.split_once(",%sp)")?;
                bytes.parse().ok()
            },
        )
}

#[test]
fn a_call_after_a_diamond_finds_its_own_arguments_on_the_stack() {
    let module = llrm_mir::parse::module(IR).expect("parses");
    let text = text_with(&module, "sum", Cpu::M68000, Options::default()).expect("compiles");
    assert_eq!(calls_read_their_own_arguments(&text), Ok(()), "{text}");
}

/// `sum` with the multiplies the 68000 makes calls of, and a `__mulsi3` of
/// shifts and adds to link them; the low halves carry, which is the diamond
/// between the two calls.
const RUN_IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @__mulsi3(i32 %a, i32 %b) #0 {
entry:
  br label %loop
loop:
  %x = phi i32 [ %a, %entry ], [ %x2, %next ]
  %y = phi i32 [ %b, %entry ], [ %y2, %next ]
  %r = phi i32 [ 0, %entry ], [ %r2, %next ]
  %done = icmp eq i32 %y, 0
  br i1 %done, label %out, label %body
body:
  %bit = and i32 %y, 1
  %set = icmp ne i32 %bit, 0
  %add = add i32 %r, %x
  %r1 = select i1 %set, i32 %add, i32 %r
  br label %next
next:
  %r2 = phi i32 [ %r1, %body ]
  %x2 = shl i32 %x, 1
  %y2 = lshr i32 %y, 1
  br label %loop
out:
  ret i32 %r
}
define internal i64 @sum(ptr %p, ptr %q) #0 {
  %a = getelementptr i8, ptr %p, i32 8
  %al = load i64, ptr %a
  %b = getelementptr i8, ptr %q, i32 8
  %bl = load i64, ptr %b
  %s0 = add i64 %bl, %al
  %pa = load i8, ptr %p
  %pe = sext i8 %pa to i32
  %m1 = mul i32 %pe, 1000
  %m1e = sext i32 %m1 to i64
  %s1 = add i64 %s0, %m1e
  %qa = load i8, ptr %q
  %qe = sext i8 %qa to i32
  %m2 = mul i32 %qe, 100000
  %m2e = sext i32 %m2 to i64
  %s2 = add i64 %s1, %m2e
  ret i64 %s2
}
define i32 @f() {
  %x = alloca [16 x i8], align 8
  %y = alloca [16 x i8], align 8
  store i8 1, ptr %x
  %xb = getelementptr i8, ptr %x, i32 8
  store i64 4294967280, ptr %xb
  store i8 2, ptr %y
  %yb = getelementptr i8, ptr %y, i32 8
  store i64 32, ptr %yb
  %t = call i64 @sum(ptr %x, ptr %y)
  %lo = lshr i64 %t, 4
  %hi = lshr i64 %t, 32
  %l8 = trunc i64 %lo to i8
  %h8 = trunc i64 %hi to i8
  %r8 = add i8 %l8, %h8
  %r = zext i8 %r8 to i32
  ret i32 %r
}
attributes #0 = { noinline }";

/// The sum as the host has it: 1000 + 2 * 100000 on top of 0xfffffff0 + 32,
/// which carries into the high word.
#[test]
fn the_sum_of_two_calls_after_a_carry_is_the_hosts() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(RUN_IR).expect("parses");
    let total = 0xffff_fff0u64 + 32 + 1000 + 2 * 100_000;
    let want = ((total >> 4) as u8).wrapping_add((total >> 32) as u8) as i32;
    let mut all = start("f", &[]);
    for name in ["f", "sum", "__mulsi3"] {
        all.extend(items_with(&module, name, Cpu::M68000, Options::default()).expect("compiles"));
    }
    let path = support::unique_path("stack_cleanup_order");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(want), "a call read another call's arguments");
}
