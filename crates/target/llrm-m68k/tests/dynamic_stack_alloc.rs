//! `int a[n]` and `__builtin_alloca` refused at every level (`Unsupported {
//! DynamicStackAlloc, I32 }`): the target stated no stack pointer for the
//! shared expansion. A runtime-sized alloca is filled and summed back from
//! memory (volatile: nothing folds it away), a 4-aligned one (what the frames
//! keep: `kept_stack_align`; more is clamped to it) is checked, and a fixed
//! local written before them is read after: the frame must address its objects
//! from the frame pointer.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// `f()` counts what is wrong: the sum of the filled words (5 words of `3 * i`:
/// 30), a call made with the stack pointer moved (`g(5, 12)`: -7), the 4-byte
/// alignment of the second allocation (an over-aligned one is clamped to it, or
/// refused where it needs the realignment the frames do not have) and the fixed
/// local (42).
const IR: &str = "define i32 @g(i32 %a, i32 %b) noinline {
b0:
  %r = sub i32 %a, %b
  ret i32 %r
}
define i32 @f() {
b0:
  %fixed = alloca i32
  %vn = alloca i32
  store i32 42, ptr %fixed
  store volatile i32 5, ptr %vn
  %n = load volatile i32, ptr %vn
  %a = alloca i32, i32 %n, align 4
  br label %loop
loop:
  %i = phi i32 [ 0, %b0 ], [ %i1, %loop ]
  %s = phi i32 [ 0, %b0 ], [ %s1, %loop ]
  %slot = getelementptr i32, ptr %a, i32 %i
  %v = mul i32 %i, 3
  store volatile i32 %v, ptr %slot
  %r = load volatile i32, ptr %slot
  %s1 = add i32 %s, %r
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, %n
  br i1 %c, label %loop, label %done
done:
  %al = alloca i8, i32 %n, align 4
  %pi = ptrtoint ptr %al to i32
  %m = and i32 %pi, 3
  %mbad = icmp ne i32 %m, 0
  %gv = call i32 @g(i32 %n, i32 12)
  %gbad = icmp ne i32 %gv, -7
  %w = zext i1 %gbad to i32
  %fv = load i32, ptr %fixed
  %fbad = icmp ne i32 %fv, 42
  %sbad = icmp ne i32 %s1, 30
  %x = zext i1 %mbad to i32
  %y = zext i1 %fbad to i32
  %z = zext i1 %sbad to i32
  %xy = add i32 %x, %y
  %xyz = add i32 %xy, %z
  %bad = add i32 %xyz, %w
  ret i32 %bad
}";

#[test]
fn a_runtime_sized_alloca_compiles_and_runs() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{IR}")).expect("parses");
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        for options in [Options::default(), Options { size: true, ..Options::default() }] {
            let mut all = start("f", &[]);
            for name in ["f", "g"] {
                all.extend(
                    items_with(&module, name, cpu, options)
                        .unwrap_or_else(|why| panic!("{cpu:?} size={} @{name}: {why}", options.size)),
                );
            }
            let path = support::unique_path("dynamic_stack_alloc");
            write_executable(&path, &executable(&all).unwrap()).unwrap();
            let code = support::run(&qemu, &["-cpu", flag], &path);
            std::fs::remove_file(&path).ok();
            assert_eq!(
                code,
                Some(0),
                "{cpu:?} size={}: the alloca, its alignment or the fixed local is wrong",
                options.size
            );
        }
    }
}
