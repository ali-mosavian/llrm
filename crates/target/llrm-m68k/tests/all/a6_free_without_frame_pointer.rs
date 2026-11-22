//! The pin reserves `%a6` only when the function has a frame pointer
//! (`M68kRegisterInfo::getReservedRegs`, `hasFPImpl`). Here it was always
//! reserved, so a loop with twelve hoisted symbol `lea`s ran out of address
//! registers one earlier than the pin and rebuilt constants every trip
//! (`suba.l %a1,%a1`, `move.w #1,%a1`); early_exit_widths +8 B,
//! known_loads_walks O3 +50 B. A function that needs the frame pointer must
//! still never allocate it.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const LAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn stores_in_a_loop(
    prologue: &str,
    alloca: &str,
) -> String {
    let mut ir = String::from(LAYOUT);
    for i in 0..12 {
        ir += &format!("@g{i} = global i32 0\n");
    }
    ir += &format!("declare void @use(ptr)\ndefine void @f(i32 %n) {{\nentry:\n{prologue}  br label %loop\nloop:\n");
    ir += "  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]\n";
    for i in 0..12 {
        ir += &format!("  store volatile i32 %i, ptr @g{i}\n");
    }
    ir += "  %i1 = add i32 %i, 1\n  %c = icmp eq i32 %i1, %n\n  br i1 %c, label %done, label %loop\ndone:\n";
    ir += alloca;
    ir += "  ret void\n}\n";
    ir
}

#[test]
fn a_function_without_a_frame_pointer_allocates_a6_and_keeps_its_hoisted_leas() {
    let ir = stores_in_a_loop("", "");
    let out = assembly(&ir, cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    let (_, body) = out.split_once("\nf:").expect("f is emitted");
    let rebuilt = body.lines().filter(|l| l.contains("suba.l") || l.contains("move.w\t#1,")).count();
    assert!(body.contains("%a6"), "no frame pointer, yet %a6 is never allocated:\n{out}");
    assert!(body.contains("movem.l") && body.contains("%a6"), "%a6 is callee-saved and must be saved:\n{out}");
    assert_eq!(rebuilt, 0, "{rebuilt} hoisted addresses rebuilt in the loop:\n{out}");
}

#[test]
fn a_function_with_a_dynamic_alloca_keeps_a6_as_its_frame_pointer() {
    let ir = stores_in_a_loop("  %p = alloca i8, i32 %n\n  call void @use(ptr %p)\n", "");
    for level in [Level::O1, Level::O2] {
        let out = assembly(&ir, cpu_of("68000").unwrap(), level).expect("compiles");
        let (_, body) = out.split_once("\nf:").expect("f is emitted");
        assert!(body.contains("link.w\t%a6"), "{level:?}: the dynamic alloca needs a frame pointer:\n{out}");
        // `(d,%a6)` addresses the frame; any other mention is an allocation
        let other = body
            .lines()
            .filter(|l| !l.contains("link.w") && !l.contains("unlk") && l.replace(",%a6)", "").contains("%a6"))
            .count();
        assert_eq!(other, 0, "{level:?}: %a6 is the frame pointer but was allocated:\n{out}");
    }
}

/// Six pointers live across six calls of one routine: the allocator took `%a6`
/// for one of them and the six `jsr (%a6)` (2 B) became `jsr use` (6 B).
/// copy_runs -O2 has 91 such calls: +350 B.
const CALLS: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@t0 = global ptr null
@t1 = global ptr null
@t2 = global ptr null
@t3 = global ptr null
@t4 = global ptr null
@t5 = global ptr null
declare void @use(i32)
define void @f() {
  %p0 = load ptr, ptr @t0
  %p1 = load ptr, ptr @t1
  %p2 = load ptr, ptr @t2
  %p3 = load ptr, ptr @t3
  %p4 = load ptr, ptr @t4
  %p5 = load ptr, ptr @t5
  call void @use(i32 0)
  call void @use(i32 1)
  call void @use(i32 2)
  call void @use(i32 3)
  call void @use(i32 4)
  call void @use(i32 5)
  store volatile i32 0, ptr %p0
  store volatile i32 1, ptr %p1
  store volatile i32 2, ptr %p2
  store volatile i32 3, ptr %p3
  store volatile i32 4, ptr %p4
  store volatile i32 5, ptr %p5
  ret void
}
";

#[test]
fn a6_stays_with_the_call_target_when_a_callee_is_called_often() {
    let out = assembly(CALLS, cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    let direct = out.lines().filter(|l| l.trim() == "jsr\tuse").count();
    assert_eq!(direct, 0, "{direct} calls of `use` left direct:\n{out}");
}

/// DEVIATE M68K-LICM-LIMIT: counting `%a6` in the hoist limit (the pin's 9)
/// held `move.l #65535, %a5` across the loops of early_exit_index -O2 and saved
/// one more register: +4 B (known_loads_walks -O3: +210 B).
#[test]
fn the_hoist_limit_does_not_count_a6_so_a_constant_is_not_held_in_a_saved_register() {
    let out =
        assembly(include_str!("../data/early_exit_index.ll"), cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    assert!(!out.contains("#65535, %a"), "a constant is held in an address register:\n{out}");
}
