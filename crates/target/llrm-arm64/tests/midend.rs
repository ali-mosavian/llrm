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
    // The loop closes on its last branch to an earlier label: `b.cond`, `cbnz`
    // or a plain `b` (a count-down loop is `cbnz` into a block that jumps
    // back).
    let label = |l: &str| l.strip_suffix(':').map(str::to_owned);
    let back = (0..lines.len())
        .rev()
        .find(|&at| {
            let mnemonic = lines[at].split_whitespace().next().unwrap_or("");
            let branch =
                mnemonic == "b" || mnemonic.starts_with("b.") || matches!(mnemonic, "cbz" | "cbnz" | "tbz" | "tbnz");
            let target = lines[at].rsplit(' ').next().unwrap();
            branch && lines[..at].iter().any(|l| label(l).as_deref() == Some(target))
        })
        .unwrap_or_else(|| panic!("a loop branch: {asm}"));
    let target = lines[back].rsplit(' ').next().unwrap();
    let start = lines.iter().position(|l| label(l).as_deref() == Some(target)).expect("the loop label");
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

/// `@llvm.global_ctors` was refused (`appending`), and then would have been
/// a data symbol nobody calls: each constructor is a pointer in the loader's
/// init section, lowest priority first, and an entry without a function is
/// dropped.
#[test]
fn global_ctors_become_init_function_pointers_in_priority_order() {
    let object = driver::object(include_str!("data/global_ctors.ll"), Platform::Darwin, "ctors.o", true)
        .expect("the module lowers");
    let init = object
        .sections
        .iter()
        .find(|section| section.role == llrm_object::Role::InitFunctions)
        .expect("an init-function section");
    assert_eq!(init.image.len(), 16, "two pointers");
    let names: Vec<&str> = init
        .relocs
        .iter()
        .map(|reloc| match reloc.target {
            llrm_object::Target::Symbol(index) => object.symbols[index].name.as_str(),
            ref other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(names, ["_early", "_late"]);
    // The pipeline keeps what only the constructor list names.
    for name in names {
        let symbol = object.symbols.iter().find(|symbol| symbol.name == name).expect("a symbol");
        assert!(matches!(symbol.definition, llrm_object::Definition::Defined { .. }), "{name}");
    }
    assert!(object.symbols.iter().all(|symbol| !symbol.name.contains("llvm.global_ctors")));
    // dyld runs a `__DATA,__mod_init_func` section (type
    // S_MOD_INIT_FUNC_POINTERS, 9).
    let file = driver::object_file(include_str!("data/global_ctors.ll"), Platform::Darwin, "ctors.o", true).unwrap();
    let at = file.windows(16).position(|w| w.starts_with(b"__mod_init_func\0")).expect("the section name");
    let flags = u32::from_le_bytes(file[at + 64..at + 68].try_into().unwrap());
    assert_eq!(flags, 9, "S_MOD_INIT_FUNC_POINTERS");
}

/// A name clang writes `"\01_pthread_join"` is the symbol as given; the Mach-O underscore was added
/// again, and `ld` looked for `__pthread_join`.
#[test]
fn a_verbatim_name_gets_no_underscore() {
    let text = "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
target triple = \"arm64-apple-macosx27.2.0\"\n\
declare i32 @\"\\01_pthread_join\"(ptr, ptr)\n\
define i32 @f(ptr %t) {\n  %r = call i32 @\"\\01_pthread_join\"(ptr %t, ptr null)\n  ret i32 %r\n}\n";
    let object = driver::object(text, Platform::Darwin, "verbatim.o", true).expect("lowers");
    let names: Vec<&str> = object.symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert!(names.contains(&"_pthread_join"), "{names:?}");
    assert!(!names.contains(&"__pthread_join"), "{names:?}");
}
