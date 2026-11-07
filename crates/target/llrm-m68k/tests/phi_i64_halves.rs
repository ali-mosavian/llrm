//! A phi of an `i64` is two registers, and the block that exports its value
//! gave both halves the one register the phi has: the high half's input was
//! dropped, its register was never defined, and register allocation panicked
//! ("a unique value", `__ashldi3` of softfloat's int64.c at -Oz).

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// `x << n` of a 64-bit constant with libgcc's cases, folded to a byte.
const SHIFT: &str = "
declare i32 @llvm.fshl.i32(i32, i32, i32)
define i32 @f(i32 %n) {
  %big = icmp ugt i32 %n, 63
  br i1 %big, label %out, label %mid
mid:
  %ge = icmp ugt i32 %n, 31
  br i1 %ge, label %wide, label %narrow
wide:
  %m = add i32 %n, -32
  %w = shl i32 2596069104, %m
  br label %join
narrow:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %join, label %shift
shift:
  %sl = shl i32 2596069104, %n
  %sh = call i32 @llvm.fshl.i32(i32 305419896, i32 2596069104, i32 %n)
  br label %join
join:
  %h = phi i32 [ %w, %wide ], [ %sh, %shift ], [ 305419896, %narrow ]
  %l = phi i32 [ 0, %wide ], [ %sl, %shift ], [ 2596069104, %narrow ]
  %lz = zext i32 %l to i64
  %hz = zext i32 %h to i64
  %hs = shl i64 %hz, 32
  %r = or i64 %hs, %lz
  br label %out
out:
  %res = phi i64 [ %r, %join ], [ 0, %0 ]
  %hi = lshr i64 %res, 32
  %hm = xor i64 %hi, 85
  %t = add i64 %hm, %res
  %b = trunc i64 %t to i32
  %c = and i32 %b, 255
  ret i32 %c
}";

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{SHIFT}")).expect("parses")
}

fn host(n: u32) -> i32 {
    let x: u64 = 0x1234_5678_9abc_def0;
    let res = if n > 63 { 0 } else { x << n };
    ((((res >> 32) ^ 85).wrapping_add(res)) & 255) as i32
}

#[test]
fn a_64_bit_value_through_a_phi_keeps_its_high_half() {
    let Some(qemu) = support::qemu() else { return };
    let module = module();
    for n in [0_u32, 1, 8, 31, 32, 33, 40, 63, 64] {
        let mut all = start("f", &[i64::from(n)]);
        all.extend(items_with(&module, "f", Cpu::M68000, Options { mid_end: false, ..Options::default() }).unwrap());
        let path = support::unique_path("phi64");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68000"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(host(n)), "x << {n}");
    }
}
