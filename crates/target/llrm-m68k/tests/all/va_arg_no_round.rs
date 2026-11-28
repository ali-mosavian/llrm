//! `va_arg(ap, int)` rounded the list pointer to the type's alignment, 2:
//! `addq.l #1,%d; and.w #65534,%d`, 6 B per `va_arg` (variadic -Os 68000 +26 B
//! over gcc in `walk`, +34 in `sum`). Every stack argument sits in a
//! 4-byte slot (`getMinStackArgumentAlignment`), so no rounding is owed.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const SUM: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @f(i32 %n, ...) {
  %ap = alloca ptr, align 2
  call void @llvm.va_start.p0(ptr %ap)
  %v = va_arg ptr %ap, i32
  call void @llvm.va_end.p0(ptr %ap)
  ret i32 %v
}
declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)
"#;

#[test]
fn a_va_arg_int_reads_the_next_slot_without_rounding_the_list_pointer() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let out = assembly(SUM, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(!out.contains("65534") && !out.contains("and."), "{cpu}: the list pointer is rounded:\n{out}");
        }
    }
}

const LOOP: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @sum(i32 %n, ...) {
entry:
  %ap = alloca ptr, align 2
  call void @llvm.va_start.p0(ptr %ap)
  %c = icmp sgt i32 %n, 0
  br i1 %c, label %loop, label %done
loop:
  %i = phi i32 [ %n, %entry ], [ %j, %loop ]
  %t = phi i32 [ 0, %entry ], [ %u, %loop ]
  %v = va_arg ptr %ap, i32
  %u = add i32 %t, %v
  %j = sub i32 %i, 1
  %d = icmp sgt i32 %j, 0
  br i1 %d, label %loop, label %done
done:
  %r = phi i32 [ 0, %entry ], [ %u, %loop ]
  call void @llvm.va_end.p0(ptr %ap)
  ret i32 %r
}
declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)
"#;

/// The list is `char *` and `va_start` stores one address through it. It stayed
/// a stack slot (`lea (-4,%sp),%sp`, a store and a load per `va_arg`: `sum` -Os
/// 52 B against gcc's 22); it is a register now.
#[test]
fn the_va_list_of_a_summing_loop_stays_out_of_the_stack_frame() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let out = assembly(LOOP, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(
                !out.contains("(0, %sp)") && !out.contains("(0,%sp)"),
                "{cpu} {level:?}: the list is in memory:\n{out}"
            );
            assert!(
                !out.contains("-4, %sp") && !out.contains("subq.l\t#4, %sp"),
                "{cpu} {level:?}: a frame slot:\n{out}"
            );
        }
    }
}

/// The `store` that writes the stepped list pointer back was created with the
/// `va_arg`'s value type, not `void`, so it defined a dead `i32`
/// (x86-m32-bytes' review of the first F2 head). No stage of the mid-end may
/// hold a store with a result.
#[test]
fn the_pointer_store_a_va_arg_becomes_has_no_result() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("va-stages-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    llrm_m68k::driver::assembly_dumping(LOOP, cpu_of("68020").unwrap(), Level::O2, Some(&dir)).expect("compiles");
    let mut seen = 0;
    let mut stack = vec![dir.clone()];
    while let Some(path) = stack.pop() {
        for entry in std::fs::read_dir(&path).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                seen += 1;
                for line in text.lines().filter(|line| line.contains("= store")) {
                    panic!("{}: a store with a result: {line}", path.display());
                }
            }
        }
    }
    std::fs::remove_dir_all(&dir).ok();
    assert!(seen > 10, "only {seen} stage files dumped");
}
