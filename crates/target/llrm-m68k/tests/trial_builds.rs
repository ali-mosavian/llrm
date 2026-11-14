//! M68K-COMPILE-TRIALS: a function whose sign-extended-word add (`suba.w`)
//! fired was built twice, once more without the rewrite, to price it; the pass
//! takes the cpu and the price itself (`sextacc`), so the second build bought
//! nothing. The count is `compile::runs().builds`: the test compared the
//! `LLRM_DEBUG=builds` lines with one sequence, and the shared-CSE and slot
//! trials run more than one (10 builds, 2026-11-14).

use llrm_m68k::compile::{Options, items_with, runs, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const IR: &str = include_str!("sext_acc_ir.ll");

fn module() -> llrm_mir::module::Module {
    let mut module = llrm_mir::parse::module(IR).expect("parses");
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    module
}

fn builds(options: Options) -> usize {
    let module = module();
    let before = runs().builds;
    items_with(&module, "f", Cpu::M68000, options).expect("compiles");
    runs().builds - before
}

#[test]
fn the_sext_add_is_not_a_trial_and_costs_no_build() {
    let on = Options::default();
    let off = Options { sext_adds: false, ..Options::default() };
    let asm = text_with(&module(), "f", Cpu::M68000, on).expect("compiles");
    assert!(asm.contains("suba.w"), "premise: the 68000 takes `suba.w` here:\n{asm}");
    assert_eq!(builds(on), builds(off), "the rewrite cost a build");
}

/// The counter sees a trial: `compare_destinations` fired here is one more build.
#[test]
fn the_build_count_sees_a_trial() {
    let off = Options { compare_destinations: false, ..Options::default() };
    assert!(builds(Options::default()) > builds(off), "a fired trial adds no build: the count is blind");
}
