//! Constant byte stores joined into one long: byte order and alignment.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

fn four_bytes(first_align: u32) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
define void @f(ptr %a) {{
  store i8 10, ptr %a, align {first_align}
  %p1 = getelementptr i8, ptr %a, i32 1
  store i8 13, ptr %p1, align 1
  %p2 = getelementptr i8, ptr %a, i32 2
  store i8 16, ptr %p2, align 1
  %p3 = getelementptr i8, ptr %a, i32 3
  store i8 19, ptr %p3, align 1
  ret void
}}"
    )
}

/// The pin's `tryStoreMergeOfConstants` packs the first store into the high
/// bits on a big-endian target (`IsLE ? NumStores - 1 - i : i`). The combiner
/// always packed little-endian: bytes 10, 13, 16, 19 became `move.l
/// #319819018` (13 10 0D 0A in memory), and huffman_decode printed 190 for 100
/// at -O2.
#[test]
fn constant_bytes_joined_into_a_long_are_in_big_endian_order() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = assembly(&four_bytes(4), cpu, Level::O2).expect("compiles");
        assert!(text.contains("move.l\t#168628243"), "{cpu:?}: 0x0A0D1013 is the bytes 10 13 16 19 in memory:\n{text}");
    }
}

/// The pin joins stores only where `allowsMemoryAccess` takes the wider type at
/// the first store's alignment; M68k does not take a long at alignment 1, so
/// four byte stores stay four. The combiner joined them into a misaligned
/// `move.l`, which traps on a 68000.
#[test]
fn byte_stores_of_alignment_one_are_not_joined_into_a_long() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = assembly(&four_bytes(1), cpu, Level::O2).expect("compiles");
        assert!(!text.contains("move.l\t#"), "{cpu:?}: no joined long:\n{text}");
        assert_eq!(text.matches("move.b\t#").count(), 4, "{cpu:?}: four byte stores:\n{text}");
    }
}

fn four_global_bytes(
    global_align: u32,
    first_align: u32,
) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
@g = internal global [4 x i8] zeroinitializer, align {global_align}
define i32 @f() {{
  store i8 10, ptr @g, align {first_align}
  %p1 = getelementptr i8, ptr @g, i32 1
  store i8 13, ptr %p1, align 1
  %p2 = getelementptr i8, ptr @g, i32 2
  store i8 16, ptr %p2, align 1
  %p3 = getelementptr i8, ptr @g, i32 3
  store i8 19, ptr %p3, align 1
  %v = load volatile i32, ptr @g, align 1
  ret i32 %v
}}"
    )
}

/// The mid-end's `mergestores` (constants of adjacent cells) wrote the joined
/// value little-endian: huffman_decode at -O2 stored 13 10 0D 0A where 0A 0D 10
/// 13 belong, and printed 190 for 100. Its stores had alignment 1, which the
/// pin's `allowsMemoryAccess` does not join into a long on M68k at all.
#[test]
fn global_byte_stores_of_alignment_one_stay_four_stores_in_the_midend() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = assembly(&four_global_bytes(1, 1), cpu, Level::O2).expect("compiles");
        assert!(!text.contains("move.l\t#"), "{cpu:?}: no joined long:\n{text}");
        assert_eq!(text.matches("move.b\t#").count(), 4, "{cpu:?}: four byte stores:\n{text}");
    }
}

#[test]
fn global_byte_stores_joined_by_the_midend_are_in_big_endian_order() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = assembly(&four_global_bytes(4, 4), cpu, Level::O2).expect("compiles");
        assert!(text.contains("#168628243"), "{cpu:?}: 0x0A0D1013 is 10 13 16 19 in memory:\n{text}");
    }
}
