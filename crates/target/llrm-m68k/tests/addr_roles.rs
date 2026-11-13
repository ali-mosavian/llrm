//! A loop over a stack array with the counter as the index: `SelectARII` kept
//! the DAG's order, so the counter was the base (`%a1`) and the frame's `lea`
//! the index (`%a2`); the increment and compare went through `move.l %a1, %d0`
//! and `move.l %d0, %a1`, three registers for one counter (seed 210 of the
//! difftest corpus, +62 B once LSR keeps the 0-based counter).

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// Stores `i + z` at `t[i]` for 0..8, then reads `t[k]`.
const LOOP: &str = "
define i8 @f(i32 %k, i8 %z) {
  %t = alloca [8 x i8], align 1
  br label %l
l:
  %i = phi i32 [ 0, %0 ], [ %n, %l ]
  %m = trunc i32 %i to i8
  %v = add i8 %m, %z
  %p = getelementptr inbounds i8, ptr %t, i32 %i
  store i8 %v, ptr %p
  %n = add nuw nsw i32 %i, 1
  %c = icmp eq i32 %n, 8
  br i1 %c, label %x, label %l
x:
  %q = getelementptr inbounds i8, ptr %t, i32 %k
  %r = load volatile i8, ptr %q
  ret i8 %r
}";

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{LOOP}")).expect("parses")
}

fn options() -> Options {
    Options { mid_end: false, ..Options::default() }
}

#[test]
fn the_pointer_is_the_base_and_the_counter_the_index() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module(), "f", cpu, options()).expect("compiles");
        // the array is addressed from the stack pointer (`frameindexed`) or a register holding its `lea`
        let store = out
            .lines()
            .find(|line| line.starts_with("\tmove.b\t%d") && (line.contains("(0,%a") || line.contains(",%sp,%d")))
            .expect("a store");
        let inner = &store[store.find('(').unwrap() + 1..store.rfind(')').unwrap()];
        let parts: Vec<&str> = inner.split(',').collect();
        let base_is_a_pointer = parts[1].starts_with("%a") || parts[1] == "%sp";
        assert!(base_is_a_pointer && parts[2].starts_with("%d"), "{cpu:?}: the counter is the base:\n{out}");
        assert!(!out.contains("move.l\t%a"), "{cpu:?}: the counter goes through a copy:\n{out}");
    }
}

#[test]
fn the_loop_still_fills_the_array_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    for k in [0_i64, 3, 7] {
        let mut all = start("f", &[k, 0]);
        all.extend(items_with(&module(), "f", Cpu::M68020, options()).unwrap());
        let path = support::unique_path("roles");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68020"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(k as i32), "element {k}");
    }
}

/// Two sign-extended halfword loads at `t[i]` and `t[i] + 2`: selection copied
/// the frame `lea` to the second load's index slot, so the counter stayed the
/// base: the `lea` was made in the loop, and `move.l %a1, %d1; addq.l #4, %d1;
/// move.l %d1, %a1` stepped the counter (parity at -Os, +24 B).
const PAIR_LOOP: &str = "
define i32 @g(i16 %z) {
  %t = alloca [16 x i16], align 2
  br label %l
l:
  %i = phi i32 [ 0, %0 ], [ %n, %l ]
  %s = phi i32 [ 0, %0 ], [ %a, %l ]
  %p = getelementptr inbounds i8, ptr %t, i32 %i
  %q = getelementptr inbounds i8, ptr %p, i32 2
  store volatile i16 %z, ptr %p
  %x = load i16, ptr %p
  %y = load i16, ptr %q
  %xe = sext i16 %x to i32
  %ye = sext i16 %y to i32
  %m = mul i32 %xe, %ye
  %a = add i32 %s, %m
  %n = add nuw nsw i32 %i, 4
  %c = icmp eq i32 %n, 32
  br i1 %c, label %x2, label %l
x2:
  ret i32 %a
}";

#[test]
fn a_copied_frame_address_is_the_base_of_the_second_load() {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{PAIR_LOOP}")).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module, "g", cpu, options()).expect("compiles");
        assert!(!out.contains("move.l\t%a"), "{cpu:?}: the counter goes through a copy:\n{out}");
        assert!(!out.contains("\tlea\t(0,%sp)"), "{cpu:?}: the frame address is made in the loop:\n{out}");
    }
}
