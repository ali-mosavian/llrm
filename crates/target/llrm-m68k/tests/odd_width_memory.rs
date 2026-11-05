//! A 24-bit field is stored and loaded as the 16 and 8 bits it is made of, the
//! high bits first at a big-endian address: they were low first, so
//! `bitfield_overhang_init` read 0x020103 back from a local initialised with
//! 0x010203.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @ld(ptr %p) {
  %v = load i24, ptr %p
  %z = zext i24 %v to i32
  ret i32 %z
}
define i24 @ld2(ptr %p) {
  %v = load i24, ptr %p
  ret i24 %v
}
define void @st(ptr %p, i32 %x) {
  %t = trunc i32 %x to i24
  store i24 %t, ptr %p
  ret void
}
define void @st2(ptr %p, i24 %x) {
  store i24 %x, ptr %p
  ret void
}
define i32 @main2() {
  %a = alloca [4 x i8]
  store volatile i8 1, ptr %a
  %b = getelementptr i8, ptr %a, i32 1
  store volatile i8 2, ptr %b
  %c = getelementptr i8, ptr %a, i32 2
  store volatile i8 3, ptr %c
  %r = call i32 @ld(ptr %a)
  %r2 = call i24 @ld2(ptr %a)
  %r2z = zext i24 %r2 to i32
  %ok1 = icmp eq i32 %r, 66051
  %ok2 = icmp eq i32 %r2z, 66051
  call void @st(ptr %a, i32 197121)
  %b0 = load volatile i8, ptr %a
  %ok3 = icmp eq i8 %b0, 3
  call void @st2(ptr %a, i24 328707)
  %b1 = load volatile i8, ptr %a
  %ok4 = icmp eq i8 %b1, 5
  %z1 = zext i1 %ok1 to i32
  %z2 = zext i1 %ok2 to i32
  %z3 = zext i1 %ok3 to i32
  %z4 = zext i1 %ok4 to i32
  %s2 = shl i32 %z2, 1
  %s3 = shl i32 %z3, 2
  %s4 = shl i32 %z4, 3
  %t1 = or i32 %z1, %s2
  %t2 = or i32 %s3, %s4
  %t = or i32 %t1, %t2
  ret i32 %t
}
";

/// Bytes 1, 2, 3 load as 0x010203; storing 0x030201 leaves 3 at the address
/// and 0x050403 leaves 5; each of the four checks is a bit of the result.
#[test]
fn a_24_bit_access_keeps_the_high_bits_at_the_address() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(IR).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let mut all = start("main2", &[]);
        for name in ["main2", "ld", "ld2", "st", "st2"] {
            all.extend(items_with(&module, name, cpu, Options::default()).expect("compiles"));
        }
        let path = support::unique_path("odd_width");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &[], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(15), "{cpu:?}");
    }
}
