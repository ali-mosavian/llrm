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

/// `llrm-arm64-llc` had no `-Os`: clang's `-Os` IR (`optsize` functions) went through the -O2
/// pipeline, so `tests/llc/list.c` came out 452 bytes where the pin's `clang -Os` makes 120.
#[test]
fn the_size_level_makes_smaller_code_than_the_speed_level() {
    let text = include_str!("data/list_optsize.ll");
    let lines = |level| driver::assembly(text, Platform::Darwin, level).expect("lowers").lines().count();
    assert!(lines(driver::Level::Os) * 2 < lines(driver::Level::O2), "-Os is no smaller than -O2");
    assert!(driver::Level::parse("s") == Ok(driver::Level::Os) && driver::Level::parse("q").is_err());
}

/// Every i32 argument was moved with a literal `orr w, wzr, w` the register allocator could not see
/// through, so `f(int a, int b)` spent two instructions copying its arguments (`tests/llc/call.c`:
/// 32 bytes where the pin's `clang -O2` makes 20).
#[test]
fn an_i32_argument_is_used_in_the_register_it_arrives_in() {
    let text = "define i32 @f(i32 %a, i32 %b) {\n  %x = mul i32 %a, %a\n  %y = mul i32 %b, %b\n  %r = add i32 %x, %y\n  ret i32 %r\n}\n";
    let asm = driver::assembly(&format!("target datalayout = \"{}\"\n{text}", Platform::Darwin.datalayout()), Platform::Darwin, true)
        .expect("lowers");
    assert!(!asm.contains("orr"), "an argument was copied:\n{asm}");
}

/// `a*b + c` and `c - a*b` were a `mul` and an `add`/`sub`, one instruction more than the `madd`/`msub`
/// the pin's `clang -O2` makes; of two added products, one is fused and the other stays a `mul`.
#[test]
fn a_product_added_or_subtracted_is_one_multiply_accumulate() {
    let asm = |body: &str, bits: u32| {
        let text = format!(
            "target datalayout = \"{}\"\ndefine i{bits} @f(i{bits} %a, i{bits} %b, i{bits} %c) {{\n{body}  ret i{bits} %r\n}}\n",
            Platform::Darwin.datalayout()
        );
        driver::assembly(&text, Platform::Darwin, true).expect("lowers")
    };
    let mnemonics = |asm: &str| -> Vec<String> {
        asm.lines().filter(|l| l.starts_with('\t')).map(|l| l.split_whitespace().next().unwrap().to_owned()).collect()
    };
    for bits in [32, 64] {
        let add = asm(&format!("  %m = mul i{bits} %a, %b\n  %r = add i{bits} %m, %c\n"), bits);
        assert_eq!(mnemonics(&add), ["madd", "ret"], "{add}");
        let sub = asm(&format!("  %m = mul i{bits} %a, %b\n  %r = sub i{bits} %c, %m\n"), bits);
        assert_eq!(mnemonics(&sub), ["msub", "ret"], "{sub}");
        // The product on the left of a subtract is not an `msub`.
        let neg = asm(&format!("  %m = mul i{bits} %a, %b\n  %r = sub i{bits} %m, %c\n"), bits);
        assert_eq!(mnemonics(&neg), ["madd", "sub", "ret"], "{neg}");
        let two = asm(
            &format!("  %m = mul i{bits} %a, %b\n  %n = mul i{bits} %c, %c\n  %r = add i{bits} %m, %n\n"),
            bits,
        );
        assert_eq!(mnemonics(&two), ["madd", "madd", "ret"], "{two}");
    }
}

/// A GEP stored through was priced for an 8-byte access, so a `float` store at a scale of 4 cost an
/// instruction of address arithmetic LLVM does not count, the fill loop of `tests/llc/fsum.c` came to
/// 162 against a limit of 150 and stayed a loop: 452 bytes where the pin folds `main` to two constants.
#[test]
fn a_float_array_fill_loop_is_unrolled_and_folded() {
    let asm = driver::assembly(include_str!("data/fill_two_arrays.ll"), Platform::Darwin, true).expect("lowers");
    assert!(!asm.contains("fmul") && !asm.contains("b."), "the fill loop was left:\n{asm}");
}

/// `extractvalue (insertvalue (insertvalue poison, c0, 0), c1, 1), 0` was never simplified, so a
/// struct returned in two registers and read at once kept its `insertvalue`/`extractvalue` chain
/// and its packing shifts: `tests/llc/structs.c` came to twice the pin's size.
#[test]
fn a_member_read_from_an_aggregate_built_of_constants_is_the_constant() {
    let mut program =
        driver::program(include_str!("data/struct_by_value.ll"), Platform::Darwin).expect("the module reads");
    driver::optimized(&mut program).expect("the pipeline runs");
    let after = llrm_mir::print::module(&program.modules[0]);
    assert!(!after.contains("insertvalue") && !after.contains("extractvalue"), "{after}");
}

/// A function passed to an inlined helper became the copy's callee without the walk looking again,
/// so `apply(plus, 6, 7)` left `bl plus` in `main` where the pin folds it to a constant
/// (`tests/darwin/manyargs.ll`: 296 bytes against 156).
#[test]
fn a_call_the_inlining_of_a_helper_makes_direct_is_inlined_too() {
    let asm = driver::assembly(include_str!("data/apply_function_pointer.ll"), Platform::Darwin, true).expect("lowers");
    let calls: Vec<&str> = asm.lines().filter(|l| l.trim_start().starts_with("bl ")).collect();
    assert_eq!(calls.len(), 1, "only printf is called: {calls:?}");
}
