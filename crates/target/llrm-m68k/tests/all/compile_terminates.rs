//! A combine that rewrote an extension to an any-extension and back looped
//! forever: `__ashldi3` (the 32-bit halves of a 64-bit shift) never finished
//! compiling on the 68000 and 68020 (59 min at 100% CPU). Compiling is bounded.

use std::sync::mpsc;
use std::time::Duration;

use llrm_m68k::driver::{Level, assembly, cpu_of};

const SHIFT64: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i64 @f(i64 %x, i32 %n) {
  %lo = trunc i64 %x to i32
  %xs = lshr i64 %x, 32
  %hi = trunc i64 %xs to i32
  %big = icmp uge i32 %n, 32
  br i1 %big, label %a, label %b
a:
  %m = sub i32 %n, 32
  %h1 = shl i32 %lo, %m
  br label %j
b:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %c, label %d
c:
  br label %j
d:
  %lo2 = shl i32 %lo, %n
  %t = shl i32 %hi, %n
  %s = sub i32 32, %n
  %u = lshr i32 %lo, %s
  %hi2 = or i32 %t, %u
  br label %j
j:
  %rlo = phi i32 [ 0, %a ], [ %lo, %c ], [ %lo2, %d ]
  %rhi = phi i32 [ %h1, %a ], [ %hi, %c ], [ %hi2, %d ]
  %zl = zext i32 %rlo to i64
  %zh = zext i32 %rhi to i64
  %sh = shl i64 %zh, 32
  %r = or i64 %sh, %zl
  ret i64 %r
}";

#[test]
fn a_64_bit_shift_from_32_bit_halves_compiles() {
    for cpu in ["68000", "68020"] {
        let (done, wait) = mpsc::channel();
        std::thread::spawn(move || {
            let out = assembly(SHIFT64, cpu_of(cpu).unwrap(), Level::O2);
            let _ = done.send(out.is_ok());
        });
        assert_eq!(wait.recv_timeout(Duration::from_secs(120)), Ok(true), "{cpu} did not finish compiling");
    }
}
