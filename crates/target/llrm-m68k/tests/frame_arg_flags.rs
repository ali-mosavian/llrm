//! A `tst` of an argument loaded from the frame was dropped for the load's
//! codes, and the allocator then made the load again at each later reader and
//! dropped the first: the `beq` read the codes of whatever ran before it.
//! spill_store_order -O2 on the 68000: `pack_ref(0x10, ...)` returned at once
//! and left its buffer untouched.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

/// `pack_ref` of spill_store_order.c as the mid-end leaves it
/// (`llrm-m68k-llc -O2 --dump`, last stage).
const PACK_REF: &str = include_str!("fixtures/pack_ref.ll");

/// A stand-in for `gd_read`, and `f(mode)`: pack 19 x 3 pixels into a buffer of
/// 0xEE and fold its 64 bytes into one (`s = s * 31 + byte`, no multiply).
const DRIVER: &str = "
define i32 @gd_read(i32 %x, i32 %y) noinline {
  %r = xor i32 %x, %y
  ret i32 %r
}
define i32 @f(i32 %mode) {
entry:
  %o = alloca [64 x i8], align 4
  br label %fill
fill:
  %i = phi i32 [0, %entry], [%i1, %fill]
  %p = getelementptr i8, ptr %o, i32 %i
  store i8 -18, ptr %p
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, 64
  br i1 %c, label %fill, label %pack
pack:
  call void @pack_ref(i32 %mode, i32 20, i32 7, i32 19, i32 3, ptr %o)
  br label %sum
sum:
  %j = phi i32 [0, %pack], [%j1, %sum]
  %s = phi i32 [0, %pack], [%s1, %sum]
  %q = getelementptr i8, ptr %o, i32 %j
  %b = load i8, ptr %q
  %e = zext i8 %b to i32
  %sh = shl i32 %s, 5
  %t = sub i32 %sh, %s
  %s1 = add i32 %t, %e
  %j1 = add i32 %j, 1
  %d = icmp ult i32 %j1, 64
  br i1 %d, label %sum, label %done
done:
  %r = and i32 %s1, 255
  ret i32 %r
}
";

/// The same fold of what `pack_ref` leaves in the buffer.
fn host(mode: u32) -> i32 {
    let (bx, by, w, h) = (20u32, 7u32, 19u32, 3u32);
    let mut out = [0xEEu8; 64];
    let planes = if mode == 0x10 || mode == 0x0D { 4 } else { 1 };
    let bits = if mode == 0x13 {
        8
    } else if mode == 4 {
        2
    } else {
        1
    };
    let per_plane = (w * bits + 7) / 8;
    let mut base = 0;
    for y in 0..h {
        for i in 0..planes * per_plane {
            out[base + i as usize] = 0;
        }
        for x in 0..w {
            let color = (bx + x) ^ (by + y);
            let at = x * bits;
            if planes > 1 {
                for plane in 0..planes {
                    if color >> plane & 1 != 0 {
                        out[base + (plane * per_plane + (x >> 3)) as usize] |= 0x80 >> (x & 7);
                    }
                }
            } else {
                out[base + (at >> 3) as usize] |= (color << (8 - bits - (at & 7))) as u8;
            }
        }
        base += (planes * per_plane) as usize;
    }
    let mut sum = 0u32;
    for byte in out {
        sum = sum.wrapping_mul(31).wrapping_add(u32::from(byte));
    }
    (sum & 255) as i32
}

#[test]
fn a_frame_argument_tested_for_a_branch_is_tested() {
    let Some(qemu) = support::qemu() else { return };
    let ir: String = PACK_REF
        .lines()
        .filter(|line| !(line.starts_with("declare") && line.contains("@gd_read")))
        .collect::<Vec<_>>()
        .join("\n");
    let module = llrm_mir::parse::module(&(ir + DRIVER)).expect("parses");
    for mode in [0x13u32, 4, 6, 0x10, 0x0D] {
        let mut all = start("f", &[i64::from(mode)]);
        for name in ["f", "gd_read", "pack_ref"] {
            all.extend(items_with(&module, name, Cpu::M68000, Options::default()).expect("compiles"));
        }
        let path = support::unique_path("frame_arg_flags");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68000"], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(host(mode)), "mode {mode:#x}");
    }
}
