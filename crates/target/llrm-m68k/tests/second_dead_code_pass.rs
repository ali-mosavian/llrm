//! Copies nothing reads reached the coalescer: the pin runs
//! `DeadMachineInstructionElim` a second time, after sinking and the
//! peephole, and the m68k ran it once, before them. Each such copy cost a
//! `shrinkToUses` of its source's whole interval, a few percent of a large
//! function's compile.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_mir::counts::{Event, get};

/// A loop that carries two values nothing outside it reads, through a switch.
const DEAD_CARRIED: &str = "
target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @f(ptr %0, i32 %1) {
  %3 = icmp sgt i32 %1, 0
  br i1 %3, label %4, label %6
4:
  br label %8
5:
  br label %6
6:
  %7 = phi i32 [ 0, %2 ], [ %29, %5 ]
  ret i32 %7
8:
  %9 = phi ptr [ %0, %4 ], [ %15, %28 ]
  %10 = phi i32 [ %1, %4 ], [ %32, %28 ]
  %11 = phi i32 [ %31, %28 ], [ 2, %4 ]
  %12 = phi i32 [ %30, %28 ], [ 1, %4 ]
  %13 = phi i32 [ %29, %28 ], [ 0, %4 ]
  %14 = load i32, ptr %9, align 2
  %15 = getelementptr i8, ptr %9, i32 4
  switch i32 %14, label %28 [
    i32 0, label %16
    i32 1, label %18
    i32 2, label %20
    i32 3, label %22
    i32 4, label %24
    i32 5, label %25
    i32 6, label %26
  ]
16:
  %17 = add nsw i32 %13, 1
  br label %28
18:
  %19 = add nsw i32 %12, 2
  br label %28
20:
  %21 = add nsw i32 %12, %13
  br label %28
22:
  %23 = xor i32 %11, %13
  br label %28
24:
  br label %28
25:
  br label %28
26:
  %27 = add nsw i32 %11, %12
  br label %28
28:
  %29 = phi i32 [ %13, %8 ], [ %17, %16 ], [ %13, %18 ], [ %21, %20 ], [ %13, %22 ], [ %13, %24 ], [ 0, %25 ], [ %13, %26 ]
  %30 = phi i32 [ %12, %8 ], [ %12, %16 ], [ %19, %18 ], [ %12, %20 ], [ %12, %22 ], [ %11, %24 ], [ %12, %25 ], [ %12, %26 ]
  %31 = phi i32 [ %11, %8 ], [ %11, %16 ], [ %11, %18 ], [ %11, %20 ], [ %23, %22 ], [ %11, %24 ], [ %11, %25 ], [ %27, %26 ]
  %32 = add i32 %10, -1
  %exitcond = icmp eq i32 %32, 0
  br i1 %exitcond, label %5, label %8
}
";

#[test]
fn no_copy_that_nothing_reads_reaches_the_coalescer() {
    let before = get(Event::DeadCopiesErased);
    let module = llrm_mir::parse::module(DEAD_CARRIED).expect("parses");
    let options = Options { mid_end: true, short: true, size: true, ..Options::default() };
    text_with(&module, "f", Cpu::M68020, options).expect("compiles");
    let dead = get(Event::DeadCopiesErased) - before;
    assert_eq!(dead, 0, "{dead} unread copies reached the coalescer");
}
