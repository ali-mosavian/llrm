//! `h = h * 31 + g(x + i)` over many independent calls: the decomposed multiply
//! is free of the call chain the library call sat in, the scheduler runs the
//! calls first, and their results spill: select_of_constants 68000 -Os grew
//! 2756 B (15 B a call) over keeping the multiply. A function that used the
//! chain is also built without and the cheaper kept (DEVIATE M68K-MUL-LATE).

use llrm_m68k::compile::{Options, items_bytes, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::dagisel::mulprice::Mode;

fn hash_of(calls: usize) -> String {
    let mut ir = String::from(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\ndeclare i8 @g(i32, i32)\ndefine i32 @f(i32 %x, i32 %y) {\n  %h0 = add i32 %x, 0\n",
    );
    for i in 1..=calls {
        ir += &format!(
            "  %b{i} = call i8 @g(i32 %x, i32 %y)\n  %c{i} = zext i8 %b{i} to i32\n  %m{i} = mul i32 %h{p}, 31\n  %h{i} = add i32 %m{i}, %c{i}\n",
            p = i - 1
        );
    }
    ir + &format!("  ret i32 %h{calls}\n}}\n")
}

fn bytes(
    ir: &str,
    mode: Mode,
) -> usize {
    let module = llrm_mir::parse::module(ir).expect("parses");
    let options = Options { mul: mode, size: true, ..Options::default() };
    items_bytes(&items_with(&module, "f", Cpu::M68000, options).expect("compiles"))
}

#[test]
fn a_function_the_chain_makes_larger_keeps_the_multiply() {
    let ir = hash_of(24);
    let (chain, kept, trial) = (bytes(&ir, Mode::Pin), bytes(&ir, Mode::Kept), bytes(&ir, Mode::Trial));
    assert!(chain > kept, "premise: the chain spills here ({chain} B against {kept} B)");
    assert!(trial <= kept, "the trial kept the chain: {trial} B against {kept} B kept");
}

#[test]
fn a_function_the_chain_makes_smaller_keeps_the_chain() {
    let ir = hash_of(2);
    let (chain, kept, trial) = (bytes(&ir, Mode::Pin), bytes(&ir, Mode::Kept), bytes(&ir, Mode::Trial));
    assert!(chain < kept, "premise: the chain is smaller here ({chain} B against {kept} B)");
    assert_eq!(trial, chain);
}

#[test]
fn a_tie_keeps_the_build_without() {
    use llrm_m68k::timing::Price;
    let module = llrm_mir::parse::module(&hash_of(2)).expect("parses");
    let build = items_with(&module, "f", Cpu::M68000, Options::default()).expect("compiles");
    for price in [Price::Size, Price::Strict, Price::Rate] {
        assert!(!llrm_m68k::trial::keeps(&build, &build, Cpu::M68000, price), "{price:?}: equal builds kept the first");
    }
}

/// `a*3 + b*5 + c*11 + d*7` at -O2 on the 68000: the build that keeps every
/// multiply hoists `lea __mulsi3, %a2` and calls through it, which the trial
/// priced as free (the call through a register named no routine), so it kept
/// four library calls (162 clocks or more each) for one: i64_param_home -O2
/// grew 10 B and slowed.
#[test]
fn a_call_through_the_hoisted_target_register_is_priced_as_the_routine() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\ndefine i32 @f(i32 %a, i32 %b, i32 %c, i32 %d) {\n  %x = mul i32 %a, 3\n  %y = mul i32 %b, 5\n  %z = mul i32 %c, 11\n  %w = mul i32 %d, 7\n  %s = add i32 %x, %y\n  %t = add i32 %z, %w\n  %r = add i32 %s, %t\n  ret i32 %r\n}\n";
    let out = llrm_m68k::driver::assembly(ir, llrm_m68k::driver::cpu_of("68000").unwrap(), llrm_m68k::driver::Level::O2).expect("compiles");
    let calls = out.matches("jsr").count();
    assert!(calls <= 1, "{calls} library calls kept:\n{out}");
}
