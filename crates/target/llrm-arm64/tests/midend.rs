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

/// `llrm-arm64-llc` had no `-Os`: clang's `-Os` IR (`optsize` functions) went
/// through the -O2 pipeline, so `tests/llc/list.c` came out 452 bytes where the
/// pin's `clang -Os` makes 120.
#[test]
fn the_size_level_makes_smaller_code_than_the_speed_level() {
    let text = include_str!("data/list_optsize.ll");
    let lines = |level| driver::assembly(text, Platform::Darwin, level).expect("lowers").lines().count();
    assert!(lines(driver::Level::Os) * 2 < lines(driver::Level::O2), "-Os is no smaller than -O2");
    assert!(driver::Level::parse("s") == Ok(driver::Level::Os) && driver::Level::parse("q").is_err());
}

/// Every i32 argument was moved with a literal `orr w, wzr, w` the register
/// allocator could not see through, so `f(int a, int b)` spent two instructions
/// copying its arguments (`tests/llc/call.c`: 32 bytes where the pin's `clang
/// -O2` makes 20).
#[test]
fn an_i32_argument_is_used_in_the_register_it_arrives_in() {
    let text = "define i32 @f(i32 %a, i32 %b) {\n  %x = mul i32 %a, %a\n  %y = mul i32 %b, %b\n  %r = add i32 %x, %y\n  ret i32 %r\n}\n";
    let asm = driver::assembly(
        &format!("target datalayout = \"{}\"\n{text}", Platform::Darwin.datalayout()),
        Platform::Darwin,
        true,
    )
    .expect("lowers");
    assert!(!asm.contains("orr"), "an argument was copied:\n{asm}");
}

/// `a*b + c` and `c - a*b` were a `mul` and an `add`/`sub`, one instruction
/// more than the `madd`/`msub` the pin's `clang -O2` makes; of two added
/// products, one is fused and the other stays a `mul`.
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
        let two =
            asm(&format!("  %m = mul i{bits} %a, %b\n  %n = mul i{bits} %c, %c\n  %r = add i{bits} %m, %n\n"), bits);
        assert_eq!(mnemonics(&two), ["madd", "madd", "ret"], "{two}");
    }
}

/// A GEP stored through was priced for an 8-byte access, so a `float` store at
/// a scale of 4 cost an instruction of address arithmetic LLVM does not count,
/// the fill loop of `tests/llc/fsum.c` came to 162 against a limit of 150 and
/// stayed a loop: 452 bytes where the pin folds `main` to two constants.
#[test]
fn a_float_array_fill_loop_is_unrolled_and_folded() {
    let asm = driver::assembly(include_str!("data/fill_two_arrays.ll"), Platform::Darwin, true).expect("lowers");
    assert!(!asm.contains("fmul") && !asm.contains("b."), "the fill loop was left:\n{asm}");
}

/// `extractvalue (insertvalue (insertvalue poison, c0, 0), c1, 1), 0` was never
/// simplified, so a struct returned in two registers and read at once kept its
/// `insertvalue`/`extractvalue` chain and its packing shifts:
/// `tests/llc/structs.c` came to twice the pin's size.
#[test]
fn a_member_read_from_an_aggregate_built_of_constants_is_the_constant() {
    let mut program =
        driver::program(include_str!("data/struct_by_value.ll"), Platform::Darwin).expect("the module reads");
    driver::optimized(&mut program).expect("the pipeline runs");
    let after = llrm_mir::print::module(&program.modules[0]);
    assert!(!after.contains("insertvalue") && !after.contains("extractvalue"), "{after}");
}

/// A function passed to an inlined helper became the copy's callee without the
/// walk looking again, so `apply(plus, 6, 7)` left `bl plus` in `main` where
/// the pin folds it to a constant (`tests/darwin/manyargs.ll`: 296 bytes
/// against 156).
#[test]
fn a_call_the_inlining_of_a_helper_makes_direct_is_inlined_too() {
    let asm = driver::assembly(include_str!("data/apply_function_pointer.ll"), Platform::Darwin, true).expect("lowers");
    let calls: Vec<&str> = asm.lines().filter(|l| l.trim_start().starts_with("bl ")).collect();
    assert_eq!(calls.len(), 1, "only printf is called: {calls:?}");
}

/// `llvm.fmuladd` of constants were never folded, so `tests/darwin/floats.ll`
/// kept a chain of `fmadd` on `fmov` constants where the pin prints the
/// results.
#[test]
fn a_fused_multiply_add_of_constants_is_a_constant() {
    let text = format!(
        "target datalayout = \"{}\"\ndeclare double @llvm.fmuladd.f64(double, double, double)\ndeclare float @llvm.fmuladd.f32(float, float, float)\n\
         define double @d() {{\n  %r = call double @llvm.fmuladd.f64(double 2.0, double 3.0, double 1.0)\n  ret double %r\n}}\n\
         define float @s() {{\n  %r = call float @llvm.fmuladd.f32(float 2.0, float 4.0, float 1.0)\n  ret float %r\n}}\n",
        Platform::Darwin.datalayout()
    );
    let asm = driver::assembly(&text, Platform::Darwin, true).expect("lowers");
    assert!(!asm.contains("fmadd") && asm.contains("#7.0") && asm.contains("#9.0"), "{asm}");
}

/// A loop of constant-array reads and divisions feeding one hash was left as a
/// loop: the exit value of its latch value was never computed by running the
/// loop on constants (`getSCEVAtScope`'s fold of an instruction whose operands
/// are constant at scope), so `tests/darwin/division.ll` came to 10640 bytes
/// where the pin's is 284 (its hash loops stayed, unrolled).
#[test]
fn a_hash_loop_over_constants_is_computed_at_compile_time() {
    let asm = driver::assembly(include_str!("darwin/division.ll"), Platform::Darwin, true).expect("lowers");
    let lines = asm.lines().count();
    assert!(lines < 300, "{lines} lines where the pin's is under 100:\n{asm}");
}

/// A function with a variable-length array was never inlined (`dynamic alloca`
/// refused, the copy's stack could not be given back), so a VLA helper called
/// with constants stayed a call: `tests/darwin/vla.ll` came to 488 bytes where
/// the pin's is 204.
#[test]
fn a_function_with_a_variable_length_array_is_inlined_with_the_stack_given_back() {
    let mut program =
        driver::program(include_str!("data/vla_variable.ll"), Platform::Darwin).expect("the module reads");
    driver::optimized(&mut program).expect("the pipeline runs");
    let after = llrm_mir::print::module(&program.modules[0]);
    assert!(!after.contains("call void @g"), "g was left a call:\n{after}");
    assert!(after.contains("stacksave") && after.contains("stackrestore"), "the stack is bracketed:\n{after}");
}

/// The same with constants: the array's count is the argument, so the copy's
/// alloca is static and no loop or call is left.
#[test]
fn a_variable_length_array_of_a_constant_size_is_a_frame_slot() {
    let asm = driver::assembly(include_str!("darwin/vla.ll"), Platform::Darwin, true).expect("lowers");
    assert!(!asm.contains("_vla"), "vla was left a function:\n{asm}");
}

/// The frame was made at entry and the return block shared, so a path that
/// never calls paid for the frame too: `if (x == 0) return 1; return g(x) + 1;`
/// pushed `x29`/`x30` before testing `x` where the pin tests first (shrink
/// wrapping).
#[test]
fn a_path_that_needs_no_frame_returns_before_the_frame_is_made() {
    let asm = driver::assembly(include_str!("data/early_return.ll"), Platform::Darwin, true).expect("lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).filter(|l| !l.ends_with(':')).collect();
    let first_branch = lines.iter().position(|l| l.starts_with("cbz") || l.starts_with("cbnz")).expect("a test of x");
    assert!(lines[..first_branch].iter().all(|l| !l.starts_with("sub sp") && !l.starts_with("stp")), "{asm}");
}

/// `printf("hi\n")`, `printf("%c", x)` and `printf("\n")` with the result
/// unused stayed calls of `printf` where the pin calls `puts` and `putchar`
/// (`LibCallSimplifier`), each a variadic call with its stack argument setup.
#[test]
fn an_unused_printf_of_a_constant_format_is_puts_or_putchar() {
    let asm = driver::assembly(include_str!("data/printf_puts.ll"), Platform::Darwin, true).expect("lowers");
    assert!(!asm.contains("_printf"), "{asm}");
    assert_eq!(asm.matches("_puts").count(), 1, "{asm}");
    assert_eq!(asm.matches("_putchar").count(), 2, "{asm}");
}

/// A `printf` whose result is read is not `puts` (the count differs), and an
/// empty format is no call.
#[test]
fn a_printf_whose_result_is_read_stays_and_an_empty_format_goes() {
    let text = include_str!("data/printf_puts.ll").replace(
        "define void @f(i32 %x) {",
        "@.e = private unnamed_addr constant [1 x i8] zeroinitializer\ndefine i32 @f(i32 %x) {\n  call i32 (ptr, ...) @printf(ptr @.e)",
    );
    let text = text.replace("ret void", "%r = call i32 (ptr, ...) @printf(ptr @.hi)\n  ret i32 %r");
    let asm = driver::assembly(&text, Platform::Darwin, true).expect("lowers");
    assert_eq!(asm.matches("_printf").count(), 1, "{asm}");
}

/// A frame of only saved registers took `sub sp` and a separate `stp`, and `add
/// sp` after the last `ldp`, where the pin pre-indexes the first store and
/// post-indexes the last load: two instructions more in every such function
/// (`fib`: 116 against 109).
#[test]
fn a_frame_of_saved_registers_moves_the_stack_pointer_in_the_pair_access() {
    let asm = driver::assembly(include_str!("darwin/fib.ll"), Platform::Darwin, true).expect("lowers");
    let fib = asm.split("_main:").next().expect("fib first");
    assert!(fib.contains("[sp, #-32]!") && fib.contains("[sp], #32"), "{fib}");
    assert!(!fib.contains("sub sp") && !fib.contains("add sp"), "{fib}");
}

/// `return g(x + 1)` marked `tail` was `bl _g` between a frame made and taken
/// down, where the pin jumps (`b _g`) and needs no frame (`recchop`: 85
/// instructions against 73). A call marked `notail`, or whose result is changed
/// before the return, stays a call.
#[test]
fn a_tail_call_whose_result_is_returned_is_a_jump() {
    let asm = driver::assembly(include_str!("data/tail_call.ll"), Platform::Darwin, true).expect("lowers");
    let of = |name: &str| {
        let from = asm.find(&format!("{name}:")).expect(name);
        let rest = &asm[from + name.len()..];
        rest.split("\n_").next().expect("body").to_owned()
    };
    let jump = of("_jump");
    assert!(jump.contains("b _g") && !jump.contains("bl _g") && !jump.contains("stp"), "{jump}");
    assert!(of("_plain").contains("bl _g"), "{asm}");
    assert!(of("_changed").contains("bl _g"), "{asm}");
}

/// `paths(n, n)` copied `n` to `x1` and then `x1` back to `x0` (`x0 <- v` came
/// first and ended `n`'s stay in `x0`), where the pin has the one `mov x1, x0`
/// (`rectwo`).
#[test]
fn an_argument_passed_twice_stays_in_the_register_it_came_in() {
    let asm = driver::assembly(include_str!("data/same_argument_twice.ll"), Platform::Darwin, true).expect("lowers");
    assert_eq!(asm.lines().filter(|l| l.trim().starts_with("orr")).count(), 1, "{asm}");
}

/// `fprintf(F, "hi")`, `fprintf(F, "%c", x)`, `fprintf(F, "%s", s)`,
/// `fputs("hi", F)` and `fwrite(s, 1, 1, F)` with the result unused stayed
/// `fprintf`, `fputs` and `fwrite` calls the pin forms as `fwrite`, `fputc` and
/// `fputs`; `fprintf(F, "%d", x)` is not one of them.
#[test]
fn an_unused_fprintf_fputs_or_fwrite_is_the_cheaper_routine() {
    let asm = driver::assembly(include_str!("data/fprintf_fwrite.ll"), Platform::Darwin, true).expect("lowers");
    assert_eq!(asm.matches("_fprintf").count(), 1, "{asm}");
    assert_eq!(asm.matches("_fwrite").count(), 2, "{asm}");
    assert_eq!(asm.matches("_fputc").count(), 2, "{asm}");
    assert_eq!(asm.matches("_fputs").count(), 1, "{asm}");
}

/// `return fn(x + 1)` through a pointer was `blr` between a frame made and
/// taken down; the pin jumps (`br`), its target held where the epilogue leaves
/// it.
#[test]
fn a_tail_call_through_a_pointer_is_a_register_jump() {
    let asm = driver::assembly(include_str!("data/tail_indirect.ll"), Platform::Darwin, true).expect("lowers");
    assert!(asm.contains("br x") && !asm.contains("blr") && !asm.contains("stp"), "{asm}");
}

/// `x / y` and `x % y` in different blocks each divided: two `sdiv` where the
/// pin hoists the division and takes the remainder as `x - (x / y) * y`
/// (`div_constants`: 2228 instructions against 1424, -147 with the pass). Waits
/// for DivRemPairs (#5085).
#[test]
#[ignore = "needs DivRemPairs (#5085)"]
fn a_division_and_a_remainder_of_the_same_operands_share_one_divide() {
    let asm = driver::assembly(include_str!("data/div_rem_pair.ll"), Platform::Darwin, true).expect("lowers");
    assert_eq!(asm.matches("sdiv").count(), 1, "{asm}");
}

/// `llvm.experimental.memset.pattern` (the pin's idiom for an `i16` fill
/// loop) reached selection unexpanded and failed the build: scroll,
/// fill_nest and loop_fusion produced no object.
#[test]
fn a_memset_pattern_becomes_a_store_loop() {
    let asm =
        driver::assembly(include_str!("data/memset_pattern.ll"), Platform::Darwin, true).expect("the intrinsic lowers");
    assert!(asm.contains("strh"), "{asm}");
}

/// A constant several calls use was made once and kept in a callee-saved
/// register (`movz x19, #0` and a save and restore of `x19`, a `mov` at each
/// use); with every callee-saved register taken it went through a stack slot
/// instead. The pin makes it again at each use: loop_fusion's `main` was 1.84x.
#[test]
fn a_constant_used_by_several_calls_is_made_at_each_use() {
    let asm = driver::assembly(include_str!("data/constants_across_calls.ll"), Platform::Darwin, true)
        .expect("the function lowers");
    assert!(!asm.contains("x19"), "{asm}");
    assert_eq!(asm.matches("movz x0, #0").count(), 2, "{asm}");
}

/// An add whose only reader the mid-end removed (a store to a write-only
/// global) stayed in the machine code: loop_fusion's loops each computed
/// `i + 200` for nothing.
#[test]
fn an_unread_result_is_not_computed() {
    let asm = driver::assembly(include_str!("data/dead_after_writeonly.ll"), Platform::Darwin, true)
        .expect("the function lowers");
    assert!(!asm.contains("#200"), "{asm}");
}

/// The loads that fed a store to a write-only global stayed after the store
/// was removed: byvalue_struct_address kept five `ldr` the pin does not have.
#[test]
fn the_operands_of_a_removed_store_go_with_it() {
    let asm = driver::assembly(include_str!("data/loads_of_written_only_global.ll"), Platform::Darwin, true)
        .expect("the function lowers");
    assert!(!asm.contains("ldr"), "{asm}");
}

/// Two internal globals one function stores before it loads (`acc = 0.0`
/// ahead of `acc += step`) stayed in memory: float_in_memory_loop kept the
/// loads and stores of both, 1.59x the pin, which makes them stack slots and
/// promotes them away.
#[test]
#[ignore = "LICM promotes acc before globalsra runs: a load of it precedes the dominating store (float_in_memory_loop, 108 B against the pin's 68)"]
fn a_global_dead_on_entry_to_its_only_function_leaves_memory() {
    let asm = driver::assembly(include_str!("data/global_dead_on_entry.ll"), Platform::Darwin, true)
        .expect("the function lowers");
    assert!(!asm.contains("adrp"), "{asm}");
}

/// The pin's clang writes `llvm.lifetime.start(ptr)`, without the size; the
/// passes read `(size, ptr)`, so every marker looked like a call that keeps
/// its object, and a 64 KB array nothing needed stayed on the stack:
/// wide_frame was 824 bytes against the pin's 36.
#[test]
fn a_lifetime_marker_without_a_size_does_not_keep_its_object() {
    let asm = driver::assembly(include_str!("data/lifetime_one_argument.ll"), Platform::Darwin, true)
        .expect("the function lowers");
    assert!(asm.lines().count() < 40, "{asm}");
}

/// Atomics were refused in the parser, and then no pass may treat one as a
/// plain access. Selected as the pin selects them (`apple-m1`: LSE and RCPC):
/// `ldar`/`ldapr`, `stlr`, one LSE instruction, `casal`, `dmb`.
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

/// A `{ T, i1 }` carried round a back edge in a phi (a cmpxchg retry loop has
/// one) was refused with "a phi with no register": an aggregate phi is copied
/// member by member.
#[test]
fn an_aggregate_phi_lowers() {
    let asm = driver::assembly(include_str!("data/agg_phi.ll"), Platform::Darwin, true).expect("lowers");
    assert!(asm.matches("bl _step").count() >= 2, "{asm}");
}

/// A name clang writes `"\01_pthread_join"` is the symbol as given; the Mach-O
/// underscore was added again, and `ld` looked for `__pthread_join`.
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

/// macOS's `_FORTIFY_SOURCE` headers leave `llvm.objectsize` in the IR, and it
/// was refused as an intrinsic with no lowering (`os.c`, `block.c` and
/// `video.c` of the QB runtime). An object of unknown size is all ones, or zero
/// where the minimum is asked.
#[test]
fn objectsize_of_an_unknown_object_is_minus_one_or_zero() {
    let asm = driver::assembly(include_str!("data/objectsize.ll"), Platform::Darwin, true).expect("lowers");
    let body = |name: &str| -> String {
        let start = asm.lines().position(|line| line == format!("{name}:")).expect(name) + 1;
        asm.lines()
            .skip(start)
            .take_while(|line| line.starts_with('\t') || line.starts_with(".L"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(body("_maximum").contains("#-1") || body("_maximum").contains("movn"), "{asm}");
    assert!(body("_minimum").contains("#0") || body("_minimum").contains("xzr"), "{asm}");
}

/// A `weak` or `linkonce_odr` definition was exported as a strong one: the QB
/// runtime's weak no-op hooks (`__attribute__((weak))`) were "duplicate
/// symbols" against the real ones at link time.
#[test]
fn a_weak_definition_is_exported_weak() {
    let text = "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
target triple = \"arm64-apple-macosx27.2.0\"\n\
define weak void @hook() {\n  ret void\n}\n\
define void @strong() {\n  ret void\n}\n";
    let object = driver::object(text, Platform::Darwin, "weak.o", true).expect("lowers");
    let binding = |name: &str| object.symbols.iter().find(|symbol| symbol.name == name).unwrap().binding;
    assert_eq!(binding("_hook"), llrm_object::Binding::Weak);
    assert_eq!(binding("_strong"), llrm_object::Binding::Public);
}

/// A global's address (`adrp` and `add`) used after a call was kept in a
/// callee-saved register, and when those ran out stored to the frame and loaded
/// back: `text_write` of the QB runtime had 81 stack accesses where the pin's
/// clang has none. Two operations make it, so it is made again at each use like
/// any one-instruction constant.
#[test]
fn a_global_address_is_made_again_not_spilled() {
    let asm = driver::assembly(include_str!("data/global_addresses_across_calls.ll"), Platform::Darwin, true)
        .expect("lowers");
    let spills = asm.lines().filter(|l| l.contains("[sp") && (l.contains("str x") || l.contains("ldr x"))).count();
    assert_eq!(spills, 0, "{asm}");
}

/// Twenty global addresses a loop with a call stores through were all hoisted
/// to its preheader, more than the callee-saved registers hold, and spilled for
/// the whole loop: `text_write` of the QB runtime came to 403 instructions
/// where the pin's clang makes 202. A loop hoists as many as fit.
#[test]
fn a_loop_does_not_hoist_more_addresses_than_it_has_registers_for() {
    let asm =
        driver::assembly(include_str!("data/global_addresses_in_a_loop.ll"), Platform::Darwin, true).expect("lowers");
    let spills = asm.lines().filter(|l| l.contains("[sp") && (l.contains("str x") || l.contains("ldr x"))).count();
    assert_eq!(spills, 0, "{asm}");
}

/// Stores at several offsets from `base + index` each added the offset to the
/// base and used the register-register form (`add x9, x0, #8; strb w10, [x9,
/// x1]`, three times), where clang adds base and index once and uses immediate
/// offsets: `lh_alloc` of the QB runtime came to 362 instructions against 90.
#[test]
fn accesses_at_several_offsets_from_base_plus_index_share_the_sum() {
    let asm = driver::assembly(include_str!("data/base_index_offsets.ll"), Platform::Darwin, true).expect("lowers");
    let adds = asm.lines().filter(|l| l.trim_start().starts_with("add ")).count();
    assert_eq!(adds, 1, "{asm}");
}

/// A compare read by two selects and a branch was made a value (`cset`) and
/// each reader tested it again (`ands w9, #1`): `lh_alloc` of the QB runtime.
/// The pin compares once and the selects and the branch read the flags.
#[test]
fn a_compare_read_by_selects_and_a_branch_is_made_once_and_not_materialized() {
    let asm = driver::assembly(include_str!("data/select_pair_and_branch.ll"), Platform::Darwin, true).expect("lowers");
    assert!(!asm.contains("csinc w") && !asm.contains("ands wzr"), "{asm}");
    assert_eq!(asm.matches("subs xzr, x20, #32").count(), 1, "{asm}");
}

/// Inline-cost decisions were only visible through two undocumented environment
/// variables; the pin's clang has no `-debug-only=inline-cost`, so a cost
/// mismatch (`lh_alloc`, 311 vs 90 instructions) could not be read off. The
/// option prints the decision and each callee instruction's cost.
#[test]
fn debug_only_inline_cost_prints_the_decision_and_the_instruction_costs() {
    let llc = std::path::Path::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/inline_small_callee.ll");
    let run = |extra: &[&str]| {
        let out = std::env::temp_dir().join(format!("inline_cost_{}.s", extra.len()));
        let said = std::process::Command::new(llc)
            .args(extra)
            .args(["-S", "-o"])
            .arg(&out)
            .arg(ir)
            .output()
            .expect("llc runs");
        assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
        String::from_utf8_lossy(&said.stderr).into_owned()
    };
    let traced = run(&["-debug-only=inline-cost"]);
    assert!(traced.contains("'twice' inlined into 'f'") && traced.contains("cost="), "{traced}");
    assert!(traced.contains("[inlinetrace]"), "{traced}");
    assert!(traced.contains("shl i64 %x, 1"), "the callee's body the costs were taken on: {traced}");
    assert!(run(&[]).is_empty());
}

/// The arm64 backend printed nothing between the IR and the asm, so where a
/// value was spilled could only be read off the final code (`B_PNTC`, 83 extra
/// stack accesses). `ISEL_DUMP=1` prints the operations after each machine
/// phase, as the x86 backend does.
#[test]
fn isel_dump_prints_the_operations_after_each_machine_phase() {
    let llc = std::path::Path::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/inline_small_callee.ll");
    let run = |dump: bool| {
        let out = std::env::temp_dir().join(format!("isel_dump_{dump}.s"));
        let mut command = std::process::Command::new(llc);
        command.args(["-S", "-o"]).arg(&out).arg(ir).env_remove("ISEL_DUMP");
        if dump {
            command.env("ISEL_DUMP", "1");
        }
        let said = command.output().expect("llc runs");
        assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
        String::from_utf8_lossy(&said.stderr).into_owned()
    };
    let dumped = run(true);
    let phases: Vec<&str> = dumped.lines().filter_map(|line| line.strip_prefix("=== ")).collect();
    for phase in ["Select @f", "Prepass @f", "RegAlloc @f"] {
        assert!(phases.contains(&phase), "{phase} missing from {phases:?}");
    }
    assert!(run(false).is_empty());
}

/// `c == '%' || c == '&' || c == '!' || c == '#'` stayed four compares joined
/// by `or` and a `select` of `p + 1` (4 instructions over clang): the early
/// SimplifyCFG did not merge the compare blocks, as it priced the `sext` of
/// each byte load as an instruction, and InstCombine never regrouped the
/// equalities, so no switch was made.
#[test]
fn an_or_chain_of_byte_equalities_is_one_switch_not_joined_compares() {
    let mut program =
        driver::program(include_str!("data/or_chain_of_equalities.ll"), Platform::Darwin).expect("the module reads");
    driver::optimized(&mut program).expect("the pipeline runs");
    let after = llrm_mir::print::module(&program.modules[0]);
    assert!(!after.contains(" or i1 ") && !after.contains(" select "), "{after}");
}

/// A loop whose body holds a `memset.pattern` was sized as 5 (the intrinsic
/// priced as one call), so 25 rows were unrolled under the threshold of 150:
/// `gd_set_mode` carried 25 calls where clang keeps the loop. The pin prices
/// the intrinsic 4, a body of 8.
#[test]
fn a_loop_of_memset_patterns_is_not_fully_unrolled() {
    let asm = driver::assembly(include_str!("data/memset_pattern_rows.ll"), Platform::Darwin, true).expect("lowers");
    let stores = asm.lines().filter(|line| line.trim_start().starts_with("strh")).count();
    assert_eq!(stores, 1, "{asm}");
}

/// `-g` on arm64 wrote no debug information at all (`Object::debug` was always
/// none): a debugger could not name a function or print a variable. At -O0 each
/// variable lives in its frame object, `a` at `sp+0` and the frame 32 bytes, so
/// 32 bytes below the canonical frame address.
#[test]
fn a_variable_in_the_frame_has_its_place_below_the_frame_address() {
    use llrm_object::debug::Location;
    let object =
        driver::object_with(include_str!("data/debug_o0.ll"), Platform::Linux, "debug_o0", driver::Level::O0, true)
            .expect("lowers");
    let info = object.debug.as_ref().expect("debug information");
    let function = info.functions.iter().find(|one| one.name == "f").expect("the function");
    let places: Vec<(&str, &Location)> =
        function.variables.iter().map(|one| (one.name.as_str(), &one.location)).collect();
    assert_eq!(
        places,
        [
            ("a", &Location::Frame { disp: -32 }),
            ("b", &Location::Frame { disp: -28 }),
            ("t", &Location::Frame { disp: -24 })
        ]
    );
    use llrm_object::ObjectTarget;
    let elf = llrm_arm64::mc::Arm64Objects.write_object("elf", &object).expect("the DWARF writer takes it");
    assert!(elf.windows(8).any(|window| window == b".debug_i"), "a .debug_info section");
}

/// The line table of an arm64 object: a row where the source place changes,
/// and one at the function's start for the line it is declared at (clang's
/// rows for the same function, bar its `prologue_end` flags).
#[test]
fn an_arm64_object_has_a_line_row_where_the_source_place_changes() {
    let object =
        driver::object_with(include_str!("data/debug_o0.ll"), Platform::Linux, "debug_o0", driver::Level::O0, true)
            .expect("lowers");
    let rows: Vec<(usize, u32, u32)> = object
        .debug
        .as_ref()
        .expect("debug information")
        .lines
        .iter()
        .map(|one| (one.offset, one.line, one.column))
        .collect();
    assert_eq!(
        rows,
        [
            (0, 2, 0),
            (16, 3, 13),
            (20, 3, 11),
            (24, 3, 18),
            (28, 3, 16),
            (32, 3, 7),
            (36, 4, 10),
            (40, 4, 12),
            (44, 4, 3)
        ]
    );
}

/// `-g` must not change the code: the source place each operation carries
/// through the passes decides nothing. The fixtures (clang's -O0 and -O2
/// output) lower to the same bytes with and without it.
#[test]
fn debug_information_leaves_the_code_as_it_was() {
    for (text, level) in
        [(include_str!("data/debug_o0.ll"), driver::Level::O0), (include_str!("data/debug_o2.ll"), driver::Level::O2)]
    {
        let text_of = |debug| {
            let object = driver::object_with(text, Platform::Linux, "same", level, debug).expect("lowers");
            object.sections[0].image.clone()
        };
        assert!(!text_of(false).is_empty(), "premise: code");
        assert_eq!(text_of(false), text_of(true));
    }
}
