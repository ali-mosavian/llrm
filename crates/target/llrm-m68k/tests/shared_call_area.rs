//! A call area shared by two calls on different paths: the arguments of both
//! were stored to the stack pointer, but the frame reserved no area. recpow
//! `-O1` on the 68000 stored `power`'s arguments over its own return address
//! (`move.l %d2, (4,%sp)` after `move.l %d2, -(%sp)`) and crashed.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define internal fastcc range(i32 -30010, 30011) i32 @power(i32 noundef range(i32 -30010, 30011) %0, i32 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) nofree nosync {
  br label %tailrecurse

tailrecurse:
  %3 = phi i32 [ %0, %2 ], [ %16, %14 ]
  %4 = phi i32 [ %1, %2 ], [ %17, %14 ]
  %5 = icmp eq i32 %4, 0
  br i1 %5, label %common.ret, label %6

6:
  %7 = and i32 %4, 1
  %8 = icmp eq i32 %7, 0
  br i1 %8, label %14, label %9

common.ret:
  %common.op = phi i32 [ %13, %9 ], [ 1, %tailrecurse ]
  ret i32 %common.op

9:
  %10 = add nsw i32 %4, -1
  %11 = tail call fastcc i32 @power(i32 noundef %3, i32 noundef %10) nobuiltin "no-builtins"
  %12 = mul nsw i32 %11, %3
  %13 = srem i32 %12, 30011
  br label %common.ret

14:
  %15 = mul nsw i32 %3, %3
  %16 = urem i32 %15, 30011
  %17 = ashr exact i32 %4, 1
  br label %tailrecurse
}

define i32 @__mulsi3(i32 %a, i32 %b) noinline {
entry:
  br label %loop
loop:
  %x = phi i32 [ %a, %entry ], [ %x2, %body ]
  %y = phi i32 [ %b, %entry ], [ %y2, %body ]
  %r = phi i32 [ 0, %entry ], [ %r1, %body ]
  %done = icmp eq i32 %y, 0
  br i1 %done, label %out, label %body
body:
  %bit = and i32 %y, 1
  %set = icmp ne i32 %bit, 0
  %add = add i32 %r, %x
  %r1 = select i1 %set, i32 %add, i32 %r
  %x2 = shl i32 %x, 1
  %y2 = lshr i32 %y, 1
  br label %loop
out:
  ret i32 %r
}
define i32 @__umodsi3(i32 %a, i32 %b) noinline {
entry:
  br label %loop
loop:
  %x = phi i32 [ %a, %entry ], [ %x2, %body ]
  %more = icmp uge i32 %x, %b
  br i1 %more, label %body, label %out
body:
  %x2 = sub i32 %x, %b
  br label %loop
out:
  ret i32 %x
}
define i32 @__modsi3(i32 %a, i32 %b) noinline {
entry:
  br label %loop
loop:
  %x = phi i32 [ %a, %entry ], [ %x2, %body ]
  %more = icmp sge i32 %x, %b
  br i1 %more, label %body, label %out
body:
  %x2 = sub i32 %x, %b
  br label %loop
out:
  ret i32 %x
}
define i32 @f() {
  %r = call i32 @power(i32 7, i32 13)
  %m = and i32 %r, 255
  ret i32 %m
}"#;

/// `power(7, 13)` as `power` computes it, folded to the status byte.
fn host() -> i32 {
    fn power(
        base: i64,
        exp: i64,
    ) -> i64 {
        if exp == 0 {
            1
        } else if exp & 1 == 0 {
            power(base * base % 30011, exp >> 1)
        } else {
            power(base, exp - 1) * base % 30011
        }
    }
    (power(7, 13) & 255) as i32
}

fn assert_frame_for_stores(options: Options) {
    let module = llrm_mir::parse::module(IR).expect("parses");
    let text = text_with(&module, "power", Cpu::M68000, options).unwrap();
    let stores = text.contains(", (4,%sp)") || text.contains(", (%sp)");
    let frame = text.contains("lea\t(-8,%sp)") || text.contains("suba.l\t#8") || text.contains("subq.l\t#8");
    assert!(!stores || frame, "stores into a call area the frame does not have:\n{text}");
}

#[test]
fn stores_into_the_call_area_find_a_frame_that_reserves_one() {
    assert_frame_for_stores(Options::default());
}

/// Without the copies the loop is rotated: the `mulsi3` call is laid out before
/// the `ADJCALLSTACKDOWN` whose area it uses. Its `power` sequence was pushed
/// away and no frame was left for the stores (-Os takes this layout too).
#[test]
fn a_call_laid_out_before_the_area_it_shares_still_finds_a_frame() {
    assert_frame_for_stores(Options { tail_duplicate: false, ..Options::default() });
}

#[test]
fn recursive_power_returns_the_hosts_value() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(IR).expect("parses");
    let options = Options { strict: true, ..Options::default() };
    let mut all = start("f", &[]);
    for name in ["f", "power", "__mulsi3", "__umodsi3", "__modsi3"] {
        all.extend(items_with(&module, name, Cpu::M68000, options).expect("compiles"));
    }
    let path = support::unique_path("shared_call_area");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(host()));
}
