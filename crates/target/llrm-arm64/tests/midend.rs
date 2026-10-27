//! The shared mid-end on a program of a 64-bit, little-endian, Darwin
//! datalayout (clang's own for `arm64-apple-macos`): it reads the module and
//! optimizes it.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LOOP_CALL: &str = include_str!("data/loop_call.ll");

#[test]
fn the_mid_end_removes_the_frame_slots_of_a_loop() {
    let mut program = driver::program(LOOP_CALL, Platform::Darwin).expect("the module reads");
    let before = llrm_mir::print::module(&program.modules[0]);
    // Premise: clang's -O0-shaped IR has the slots the pipeline is to remove.
    assert!(before.contains("alloca i32"), "{before}");
    driver::optimized(&mut program).expect("the pipeline runs");
    let after = llrm_mir::print::module(&program.modules[0]);
    assert!(!after.contains("alloca"), "{after}");
}

/// A constant needing several instructions (`movz` then `movk`s) was built
/// again inside the loop that uses it, because the chain was not one
/// hoistable definition: nbody ran 4 extra instructions per use.
#[test]
fn a_wide_constant_is_built_outside_the_loop_that_uses_it() {
    let text = include_str!("data/wide_constant_loop.ll");
    let asm = driver::assembly(text, Platform::Darwin, true).expect("the function lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let back = lines.iter().rposition(|l| l.starts_with("b.")).expect("a loop branch");
    let target = lines[back].rsplit(' ').next().unwrap();
    let start = lines.iter().position(|l| l.strip_suffix(':') == Some(target)).expect("the loop label");
    // Premise: the constant is built somewhere (not folded away).
    assert!(lines.iter().any(|l| l.starts_with("movk")), "{asm}");
    let inside: Vec<&&str> =
        lines[start..=back].iter().filter(|l| l.starts_with("movk") || l.starts_with("movz")).collect();
    assert!(inside.is_empty(), "constant rebuilt in the loop: {inside:?}\n{asm}");
}

/// An access to a global array at a variable index added the index to the
/// array's address in a register first (`add x9, x8, x10` then `ldrb [x9]`)
/// instead of using the register-offset form: an extra instruction per access.
#[test]
fn a_global_array_is_indexed_in_the_access() {
    let asm = driver::assembly(include_str!("data/global_index.ll"), Platform::Darwin, true).expect("lowers");
    let loads: Vec<&str> = asm.lines().map(str::trim).filter(|l| l.starts_with("ldr")).collect();
    // Premise: both loads are there.
    assert_eq!(loads.len(), 2, "{asm}");
    assert!(loads.iter().all(|l| l.contains(", x") && l.contains(']') && l.matches(',').count() >= 2), "{asm}");
    assert!(!asm.lines().any(|l| l.trim().starts_with("add") && l.contains("lsl #2")), "{asm}");
}

/// QB's store of a SINGLE writes `fpround float %x to float`; the selector
/// refused it ("the cast FPRound") and no QB program with a SINGLE variable
/// compiled for arm64.
#[test]
fn fpround_of_a_float_to_its_own_type_is_the_value() {
    let text = "target triple = \"aarch64-unknown-linux-gnu\"\ntarget datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\ndefine float @f(float %x) {\nb1:\n  %y = fpround float %x to float\n  ret float %y\n}\n";
    let asm = driver::assembly(text, Platform::Linux, false).expect("the function lowers");
    assert!(asm.contains("ret"), "{asm}");
}

/// The page of a global's address (`adrp`) carried no addend while the page
/// offset (`add`) carried it: with the global at 0x1b6e from the start of .bss
/// the two named different pages when the sum crossed one, and QB's NIBBLES
/// passed a pointer 4096 bytes off into its arena (a colour of -1, "Illegal
/// function call").
#[test]
fn the_page_and_the_offset_of_a_global_plus_offset_agree() {
    let text = "target triple = \"aarch64-unknown-linux-gnu\"\ntarget datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\n@g = internal global [24000 x i8] zeroinitializer, align 16\n\ndefine ptr @f() {\nb1:\n  ret ptr getelementptr (i8, ptr @g, i64 6896)\n}\n";
    let asm = driver::assembly(text, Platform::Linux, false).expect("the function lowers");
    let adrp = asm.lines().find(|l| l.trim().starts_with("adrp")).expect("an adrp");
    assert!(adrp.contains("6896"), "{asm}");
}

/// Atomics were refused in the parser, and then no pass may treat one as a plain access. Selected as
/// the pin selects them (`apple-m1`: LSE and RCPC): `ldar`/`ldapr`, `stlr`, one LSE instruction,
/// `casal`, `dmb`.
#[test]
fn atomics_are_kept_and_selected_as_the_pin_does() {
    let asm = driver::assembly(include_str!("data/atomics.ll"), Platform::Darwin, true).expect("lowers");
    let function = |name: &str| -> Vec<String> {
        let start = asm.lines().position(|line| line == format!("{name}:")).expect(name) + 1;
        asm.lines()
            .skip(start)
            .take_while(|line| line.starts_with('\t') || line.starts_with(".L"))
            .filter(|line| line.starts_with('\t'))
            .map(|line| line.trim().to_owned())
            .collect()
    };
    let count = |name: &str, mnemonic: &str| function(name).iter().filter(|line| line.starts_with(mnemonic)).count();
    // Two atomic loads are two instructions; an unused one is not removed.
    assert_eq!(count("_twice_seq_cst", "ldar"), 2, "{asm}");
    assert_eq!(count("_unused_load", "ldar"), 1, "{asm}");
    assert_eq!(count("_publish", "stlr"), 1, "{asm}");
    assert_eq!(count("_acquire", "ldapr"), 1, "{asm}");
    assert_eq!(count("_fetch_add", "ldaddal"), 1, "{asm}");
    assert_eq!(count("_exchange", "swpab"), 1, "{asm}");
    assert!(function("_fetch_sub").iter().any(|line| line.starts_with("ldadd w")), "{asm}");
    assert!(function("_fetch_sub").iter().any(|line| line.starts_with("neg") || line.starts_with("sub")), "{asm}");
    assert!(function("_fetch_and").iter().any(|line| line.starts_with("ldclrl")), "{asm}");
    assert_eq!(count("_compare_exchange", "casal"), 1, "{asm}");
    assert_eq!(function("_fences").iter().filter(|line| line.starts_with("dmb")).count(), 2, "{asm}");
}
