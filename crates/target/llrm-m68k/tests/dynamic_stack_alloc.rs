//! `int a[n]` and `__builtin_alloca` refused at every level (`Unsupported {
//! DynamicStackAlloc, I32 }`): the target stated no stack pointer for the
//! shared expansion. A runtime-sized alloca is filled and summed back from
//! memory (volatile: nothing folds it away), allocas asking 8, 16 and 64 bytes
//! of alignment (more than the 4 the frames keep) must get it, the pointer read
//! back through a volatile slot so the optimizer cannot assume it, and a fixed
//! local written before them is read after: the frame must address its objects
//! from the frame pointer.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// `f()` counts what is wrong: the sum of the filled words (5 words of `3 * i`:
/// 30), a call made with the stack pointer moved (`g(5, 12)`: -7), the
/// alignment of three further allocations (8, 16 and 64: honored, not clamped
/// to the 4 the frames keep) and the fixed local (42).
const IR: &str = "define i32 @g(i32 %a, i32 %b) noinline {
b0:
  %r = sub i32 %a, %b
  ret i32 %r
}
define i32 @f() {
b0:
  %fixed = alloca i32
  %vn = alloca i32
  %hide = alloca ptr
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
  %al8 = alloca i8, i32 %n, align 8
  store volatile ptr %al8, ptr %hide
  %q8 = load volatile ptr, ptr %hide
  %pi8 = ptrtoint ptr %q8 to i32
  %m8 = and i32 %pi8, 7
  %mbad8 = icmp ne i32 %m8, 0
  %x8 = zext i1 %mbad8 to i32
  %al16 = alloca i8, i32 %n, align 16
  store volatile ptr %al16, ptr %hide
  %q16 = load volatile ptr, ptr %hide
  %pi16 = ptrtoint ptr %q16 to i32
  %m16 = and i32 %pi16, 15
  %mbad16 = icmp ne i32 %m16, 0
  %x16 = zext i1 %mbad16 to i32
  %al64 = alloca i8, i32 %n, align 64
  store volatile ptr %al64, ptr %hide
  %q64 = load volatile ptr, ptr %hide
  %pi64 = ptrtoint ptr %q64 to i32
  %m64 = and i32 %pi64, 63
  %mbad64 = icmp ne i32 %m64, 0
  %x64 = zext i1 %mbad64 to i32
  %gv = call i32 @g(i32 %n, i32 12)
  %gbad = icmp ne i32 %gv, -7
  %w = zext i1 %gbad to i32
  %fv = load i32, ptr %fixed
  %fbad = icmp ne i32 %fv, 42
  %sbad = icmp ne i32 %s1, 30
  %y = zext i1 %fbad to i32
  %z = zext i1 %sbad to i32
  %xy = add i32 %x8, %y
  %xz = add i32 %xy, %z
  %xw = add i32 %xz, %w
  %x1 = add i32 %xw, %x16
  %bad = add i32 %x1, %x64
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

/// The pipeline clamped a dynamic `alloca` to the 4 the frames keep (or refused
/// 16 and 64 by name), so `__builtin_alloca_with_align(n, 512)` handed back
/// 4-aligned memory without a word. It is masked to what it asks.
#[test]
fn an_alloca_asking_more_than_the_frames_keep_is_masked_to_what_it_asks() {
    use llrm_m68k::driver::{Level, assembly, cpu_of};
    for align in [8, 16, 64] {
        let ir = format!(
            "target datalayout = \"{DATALAYOUT}\"\ndeclare void @use(ptr)\ndefine void @f(i32 %n) {{\n  %al = alloca i8, i32 %n, align {align}\n  call void @use(ptr %al)\n  ret void\n}}\n"
        );
        for cpu in ["68000", "68020"] {
            for level in [Level::O2, Level::Os] {
                let text = assembly(&ir, cpu_of(cpu).unwrap(), level)
                    .unwrap_or_else(|why| panic!("{cpu} align {align}: {why}"));
                let (word, long) = (format!("#{}", 65536 - align), format!("#-{align}"));
                assert!(
                    text.contains(&word) || text.contains(&long),
                    "{cpu} align {align}: no mask of the address:\n{text}"
                );
            }
        }
    }
}
