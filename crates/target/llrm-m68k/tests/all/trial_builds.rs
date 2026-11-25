//! M68K-COMPILE-TRIALS: a function whose sign-extended-word add (`suba.w`)
//! fired was built twice, once more without the rewrite, to price it; the pass
//! takes the cpu and the price itself (`sextacc`), so the second build bought
//! nothing. The count is `compile::runs().builds`: the test compared the
//! `LLRM_DEBUG=builds` lines with one sequence, and the shared-CSE and slot
//! trials run more than one (10 builds, 2026-11-14).

use llrm_m68k::compile::{Options, items_with, runs, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const IR: &str = include_str!("../sext_acc_ir.ll");

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

/// The counter sees a trial: `compare_destinations` fired here is one more
/// build.
#[test]
fn the_build_count_sees_a_trial() {
    // a speed build is built once (#9934); the size build runs the trials
    let on = Options { size: true, ..Options::default() };
    let off = Options { compare_destinations: false, ..on };
    assert!(builds(on) > builds(off), "a fired trial adds no build: the count is blind");
}

/// `sext_acc_ir.ll` with a chain of `links` operations before the return.
fn module_with_chain(links: usize) -> llrm_mir::module::Module {
    let mut chain = String::new();
    let mut last = "%s1".to_owned();
    for link in 0..links {
        chain += &format!(
            "  %r{link} = {} i32 {last}, {}\n",
            if link % 2 == 0 { "xor" } else { "add" },
            ["%t", "%n"][link % 2]
        );
        last = format!("%r{link}");
    }
    let text = IR.replace("  ret i32 %s1", &format!("{chain}  ret i32 {last}"));
    let mut module = llrm_mir::parse::module(&text).expect("parses");
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    module
}

fn chain_builds(links: usize) -> usize {
    let module = module_with_chain(links);
    let before = runs().builds;
    let options = Options { size: true, ..Options::default() };
    items_with(&module, "f", Cpu::M68000, options).expect("compiles");
    runs().builds - before
}

/// M68K-TRIAL-CAP: synth.c at -O1 spent 311 s on 21 builds of one
/// 17,000-instruction function; a function over `Tuning::trial_insts_max` is
/// built once.
#[test]
fn a_function_over_the_trial_cap_is_built_once() {
    let cap = llrm_m68k::dagisel::function::tuning().trial_insts_max.expect("the m68k sets a cap");
    assert!(chain_builds(50) > 1, "premise: the small function's trials fire");
    assert_eq!(chain_builds(cap + 100), 1, "a function over the cap ran its trials");
}
