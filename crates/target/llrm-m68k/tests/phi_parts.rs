//! A phi of two registers (an `i64`, a softened `double`) took its second
//! part's incoming value into the first register: the second register had no
//! definition and the live ranges panicked ("a unique value") for an `i64`
//! accumulator around a call, and the sum of a loop of `i64` adds was wrong.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// The accumulator is read by the call and the result feeds the phi.
const CALL_IN_LOOP: &str = "
define i64 @f(i64 %a, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %acc = phi i64 [%a, %entry], [%acc1, %loop]
  %acc1 = call i64 @g(i64 %acc)
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %loop, label %done
done:
  ret i64 %acc1
}
declare i64 @g(i64)";

#[test]
fn an_i64_accumulator_around_a_call_compiles() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module(CALL_IN_LOOP), "f", cpu, Options::default());
        assert!(out.is_ok_and(|text| text.contains("jsr\tg")), "{cpu:?}");
    }
}

/// Five adds of `0x1_0000_0003`: high word 5, low word 15.
const SUM: &str = "
define i32 @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %acc = phi i64 [0, %entry], [%acc1, %loop]
  %acc1 = add i64 %acc, 4294967299
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, 5
  br i1 %c, label %loop, label %done
done:
  %hi = lshr i64 %acc1, 32
  %h = trunc i64 %hi to i32
  %l = trunc i64 %acc1 to i32
  %s = shl i32 %h, 4
  %r = add i32 %s, %l
  ret i32 %r
}";

#[test]
fn both_halves_of_an_i64_phi_carry_around_the_loop() {
    let Some(qemu) = support::qemu() else { return };
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        let mut all = start("f", &[0]);
        all.extend(items_with(&module(SUM), "f", cpu, Options::default()).expect("compiles"));
        let path = support::unique_path("phi_parts");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", flag], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(5 * 16 + 15), "{cpu:?}");
    }
}
