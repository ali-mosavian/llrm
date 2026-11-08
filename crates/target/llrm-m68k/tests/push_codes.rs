//! A deferred argument push was made between a `tst` and the `seq` that read
//! its codes: `move.l %d0, -(%sp)` sets them. logical_value -O1 and -Os on the
//! 68000 printed 31704 for 30192 (`pattern * 3`, a libcall at 68000, took its
//! arguments pushed before the `seq` that rewrites the pushed register).

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

/// logical_value.c's `main` as the mid-end leaves it (`-O1`), with its two
/// reports folded into the status, and a `__mulsi3` of shifts and adds.
const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @__mulsi3(i32 %a, i32 %b) noinline {
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
define i32 @f() {
  br label %1
1:
  %2 = phi i32 [ 0, %0 ], [ %29, %31 ]
  %3 = phi i32 [ 0, %0 ], [ %23, %31 ]
  %4 = phi i32 [ 0, %0 ], [ %32, %31 ]
  br label %5
5:
  %6 = phi i32 [ %2, %1 ], [ %29, %5 ]
  %7 = phi i32 [ %3, %1 ], [ %23, %5 ]
  %8 = phi i32 [ 0, %1 ], [ %30, %5 ]
  %9 = icmp ne i32 %4, 0
  %10 = icmp ne i32 %8, 0
  %11 = and i1 %9, %10
  %12 = zext i1 %11 to i32
  %13 = or i32 %8, %4
  %14 = icmp eq i32 %13, 0
  %15 = mul nsw i32 %7, 3
  %16 = add nsw i32 %15, %12
  %17 = select i1 %14, i32 0, i32 2
  %18 = add nsw i32 %16, %17
  %19 = select i1 %11, i32 5, i32 0
  %20 = add nsw i32 %18, %19
  %21 = icmp eq i32 %13, 0
  %22 = select i1 %21, i32 0, i32 7
  %23 = add nsw i32 %20, %22
  %24 = xor i1 %10, true
  %25 = and i1 %9, %24
  %26 = zext i1 %25 to i32
  %27 = select i1 %10, i32 2, i32 0
  %28 = add i32 %6, %27
  %29 = add i32 %28, %26
  %30 = add nuw nsw i32 %8, 1
  %exitcond = icmp eq i32 %30, 3
  br i1 %exitcond, label %31, label %5
31:
  %32 = add nuw nsw i32 %4, 1
  %exitcond1 = icmp eq i32 %32, 3
  br i1 %exitcond1, label %33, label %1
33:
  %s = shl i32 %23, 5
  %t = sub i32 %s, %23
  %u = add i32 %t, %29
  %v = and i32 %u, 255
  ret i32 %v
}";

/// The two numbers the program reports, folded as `f` folds them.
fn host() -> i32 {
    let (mut pattern, mut sum) = (0i32, 0i32);
    for a in 0..3 {
        for b in 0..3 {
            let both = i32::from(a != 0 && b != 0);
            let either = i32::from(a != 0 || b != 0);
            pattern = pattern * 3 + both + either * 2 + both * 5 + either * 7;
            sum += i32::from(a != 0 && b == 0) + i32::from(b != 0) * 2;
        }
    }
    ((pattern.wrapping_mul(31)).wrapping_add(sum) & 255) as i32
}

#[test]
fn a_push_is_not_made_between_a_test_and_the_set_that_reads_it() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(IR).expect("parses");
    for (label, options) in
        [("-O1", Options { strict: true, ..Options::default() }), ("-Os", Options { size: true, ..Options::default() })]
    {
        let mut all = start("f", &[]);
        for name in ["f", "__mulsi3"] {
            all.extend(items_with(&module, name, Cpu::M68000, options).expect("compiles"));
        }
        let path = support::unique_path("push_codes");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68000"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(host()), "{label}");
    }
}
