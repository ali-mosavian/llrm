//! A 64-bit multiply with no widening multiply reaches the library-call step
//! of the type legalizer (`ExpandIntRes_MUL`); `make_lib_call` named no
//! register for an 8-byte result and panicked ("RetCC_M68k_C names a register"),
//! so `x_fixmul` stopped compiling. The hook now returns the result as two
//! words (`d0`, `d1`), as for a divide or a float conversion; it compiles.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

#[test]
fn a_64_bit_multiply_compiles() {
    let ir = "
define i64 @f(i64 %a, i64 %b) {
  %p = mul i64 %a, %b
  ret i64 %p
}";
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        items_with(&module, "f", cpu, Options::default()).unwrap_or_else(|e| panic!("{cpu:?}: {e}"));
    }
}

/// A fixed-point multiply of two `i64` needs the 128-bit product, whose result
/// is wider than D0:D1: it stays inline (#6858 stopped every 64-bit libcall
/// result to avoid the panic, and with it `__divdi3`, `__fixsfdi` and the
/// other D0:D1 routines).
#[test]
fn a_64_bit_fixed_point_multiply_compiles() {
    let ir = "
declare i64 @llvm.smul.fix.i64(i64, i64, i32)
define i64 @f(i64 %a, i64 %b) {
  %p = call i64 @llvm.smul.fix.i64(i64 %a, i64 %b, i32 16)
  ret i64 %p
}";
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        items_with(&module, "f", cpu, Options::default()).unwrap_or_else(|e| panic!("{cpu:?}: {e}"));
    }
}
