//! u64_shift_variable_kinds -Os (68000/68020, +102/+104 B against gcc):
//! volatile locals passed to a shift libcall are read from `(k,%sp)` between
//! the argument stores. Any instruction naming `%sp` made the window
//! unconvertible, so the fused store (`move.l (20,%sp), (4,%sp)`, 6 B) was
//! never planned and each word cost a load into `%a1` and a store (8 B).

use llrm_m68k::driver::{Level, assembly, cpu_of};

/// `x << n` and `x >> n` of a volatile `u64` for several counts, each result
/// reported in two words: the shifts stay libcalls (`__ashldi3`, `__lshrdi3`)
/// and their arguments are frame words.
fn ir() -> String {
    let mut text = String::from(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
         declare void @report(i32)\n\
         define i32 @main() nounwind optsize {\n  %x = alloca i64\n  %n = alloca i32\n\
         \x20 store volatile i64 -7911603569390985487, ptr %x\n",
    );
    let mut id = 0;
    let mut name = |prefix: &str| {
        id += 1;
        format!("%{prefix}{id}")
    };
    for count in [0, 1, 7, 8] {
        text += &format!("  store volatile i32 {count}, ptr %n\n");
        for op in ["shl", "lshr"] {
            let (v, c, w, r, t, h, u) = (name("v"), name("c"), name("w"), name("r"), name("t"), name("h"), name("u"));
            text += &format!(
                "  {v} = load volatile i64, ptr %x\n  {c} = load volatile i32, ptr %n\n  {w} = zext i32 {c} to i64\n\
                 \x20 {r} = {op} i64 {v}, {w}\n  {t} = trunc i64 {r} to i32\n  call void @report(i32 {t})\n\
                 \x20 {h} = lshr i64 {r}, 32\n  {u} = trunc i64 {h} to i32\n  call void @report(i32 {u})\n"
            );
        }
    }
    text + "  ret i32 0\n}\n"
}

/// A frame word that only becomes a call argument is moved memory to memory,
/// not through `%a1`.
#[test]
fn a_frame_word_passed_as_an_argument_is_not_loaded_into_a_register() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(&ir(), cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(asm.contains("__lshrdi3"), "{cpu}: the shifts are not libcalls:\n{asm}");
        let through = asm
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                line.starts_with("move.l\t%a") && line.ends_with("%sp)") && line.contains(", (")
            })
            .count();
        assert_eq!(through, 0, "{cpu}: argument words go through an address register:\n{asm}");
    }
}
