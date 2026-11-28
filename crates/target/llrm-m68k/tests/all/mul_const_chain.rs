//! `x * 1000` at -Os on the 68000 was `pea 1000; jsr __mulsi3` and linked the
//! routine: the chain (3 shifts, 2 adds) costs more than the 10 B call site, so
//! the site price alone rejected it, and a program whose only multiply it was
//! paid the routine's bytes (bench/branch +28 B over gcc, control +32, loop
//! +24, memory +20, algebra +14). The function trial now judges the chain with
//! the routine charged unless the module multiplies two variables.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const LAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn asm(body: &str) -> String {
    assembly(&format!("{LAYOUT}{body}"), cpu_of("68000").unwrap(), Level::Os).expect("compiles")
}

#[test]
fn a_constant_multiply_that_is_the_only_one_is_a_chain_not_a_call() {
    let out = asm("define i32 @f(i32 %x) optsize nounwind {\n %r = mul i32 %x, 1000\n ret i32 %r\n}\n\
         define i32 @g(i32 %x) optsize nounwind {\n %r = mul i32 %x, 75\n ret i32 %r\n}\n");
    assert!(!out.contains("__mulsi3"), "the routine is called for x * 1000 or x * 75:\n{out}");
}

#[test]
fn a_constant_multiply_calls_the_routine_the_module_links_anyway() {
    let out = asm("define i32 @f(i32 %x) optsize nounwind {\n %r = mul i32 %x, 1000\n ret i32 %r\n}\n\
         define i32 @g(i32 %x, i32 %y) optsize nounwind {\n %r = mul i32 %x, %y\n ret i32 %r\n}\n");
    let f = out.split("g:").next().unwrap();
    assert!(f.contains("__mulsi3"), "the chain was kept where the routine is linked anyway:\n{out}");
}

/// Magic-number-sized constants, one per function (equal functions would be
/// merged into one).
const MAGIC: [i64; 6] = [0xAAAA_AAAB, 0xCCCC_CCCD, 0x51EB_851F, 0x1062_4DD3, 0x6666_6667, 0x9249_2493];

fn many(count: usize) -> String {
    (0..count)
        .map(|i| {
            format!("define i32 @f{i}(i32 %x) optsize nounwind {{\n %r = mul i32 %x, {}\n ret i32 %r\n}}\n", MAGIC[i])
        })
        .collect()
}

/// Each of six functions that multiply by a constant saw the whole 54 B of
/// `__mulsi3` saved by its own chain and took it: `urem_eq` -Os 68000 went from
/// 446 to 510 B with the routine still saved only once. The routine is charged
/// to the functions that may call it in turn.
#[test]
fn six_functions_that_multiply_by_a_constant_share_the_routine_charge() {
    let alone = asm(&many(1));
    let shared = asm(&many(6));
    assert!(!alone.contains("__mulsi3"), "one function keeps the call where the chain is the smaller module:\n{alone}");
    assert!(shared.contains("__mulsi3"), "six chains cost more than the routine they save once:\n{shared}");
}

/// qlight's `(long) raw * sval` is a `muls.w`, never the routine; the module
/// counted it as a multiply of two variables, so its `* 1000000L` kept the call
/// and linked `__mulsi3` for nothing else (bench/qlight +102 B).
#[test]
fn a_halfword_product_does_not_count_as_linking_the_routine() {
    let out =
        asm("define i32 @h(i16 %a, i16 %b) optsize nounwind {\n %x = sext i16 %a to i32\n %y = sext i16 %b to i32\n \
         %r = mul i32 %x, %y\n ret i32 %r\n}\n\
         define i32 @g(i32 %x) optsize nounwind {\n %r = mul i32 %x, 1000000\n ret i32 %r\n}\n");
    assert!(!out.contains("__mulsi3"), "the chain was refused for a routine nothing links:\n{out}");
}
