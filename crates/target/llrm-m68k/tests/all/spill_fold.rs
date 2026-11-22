//! A spilled byte an instruction reads is read from its frame slot. Without a
//! fold hook the spiller reloaded the whole long and then compared its low
//! byte: `move.l (8,%sp), %d0; cmpi.b #3, %d0` (8 B) for `cmpi.b #3, (11,%sp)`
//! (6 B); known_loads_walks had 36 such reloads (2118 B, 2072 B with the fold).

use llrm_m68k::driver::{Level, assembly, cpu_of, object};

/// Ten bytes live across ten calls: four of them spill, and each is compared
/// with what a call returns.
fn ir() -> String {
    let mut text = String::from(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
         declare i8 @h(i32)\n\
         define i32 @f(ptr %p) nounwind {\n",
    );
    for i in 0..10 {
        text += &format!("  %q{i} = getelementptr i8, ptr %p, i32 {i}\n  %v{i} = load volatile i8, ptr %q{i}\n");
    }
    let mut sum = "0".to_owned();
    for i in 0..10 {
        let (a, b) =
            if i % 2 == 0 { (format!("%v{i}"), format!("%t{i}")) } else { (format!("%t{i}"), format!("%v{i}")) };
        text += &format!(
            "  %t{i} = call i8 @h(i32 {i})\n  %c{i} = icmp ult i8 {a}, {b}\n  %z{i} = zext i1 %c{i} to i32\n  %s{i} = add i32 {sum}, %z{i}\n"
        );
        sum = format!("%s{i}");
    }
    text += &format!("  ret i32 {sum}\n}}\n");
    text
}

#[test]
fn a_spilled_byte_is_compared_in_its_slot() {
    let asm = assembly(&ir(), cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    let folded = asm.lines().filter(|line| line.contains("cmp.b") && line.contains(",%sp)")).count();
    assert!(folded > 0, "no compare reads a spilled byte from its slot:\n{asm}");
}

/// A slot read in place cannot live in an address register, which `move.l %d0,
/// %a3` makes 2 B instead of 4. With the fold on every function,
/// long_compare_chain's object grew 4 B (1892 B to 1896 B, -Os 68000); the
/// trial builds the function without the fold too and keeps the smaller.
#[test]
fn the_fold_does_not_take_a_slot_out_of_an_address_register_for_more_bytes() {
    let ir = include_str!("../fixtures/long_compare_chain.ll");
    let size = object(ir, cpu_of("68000").unwrap(), Level::Os, "t.ll").expect("compiles").len();
    assert!(size <= 1892, "the object is {size} B (1892 B with the trial)");
}
