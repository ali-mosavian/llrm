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

/// The cap was 5,000 instructions at every level: QB's `exec_loop` (66 builds,
/// the whole of exec.c's backend compile) and the other functions between
/// 3,000 and 5,000 ran every -Os trial for no byte (3,000 loses none over 113
/// modules). At -O1 the same trials shrink exec.c by a third and stay.
#[test]
fn a_function_of_3100_instructions_is_built_once_at_size_and_tried_at_speed() {
    let builds = |links: usize, size: bool| {
        let module = module_with_chain(links);
        let before = runs().builds;
        let options = Options { size, strict: true, ..Options::default() };
        items_with(&module, "f", Cpu::M68000, options).expect("compiles");
        runs().builds - before
    };
    assert!(builds(2900, true) > 1, "premise: the small function's -Os trials fire");
    assert_eq!(builds(3100, true), 1, "a function over 3,000 instructions ran its -Os trials");
    assert!(builds(3100, false) > 1, "the -O1 trials of a function under 5,000 instructions did not run");
}

/// The class relaxation and the held data constants each built the function
/// again at -Os, allocation and all, and ran the late stage again for their
/// own key: over QB (68020) 49 % of the relaxed builds and 81 % of the
/// held-constant builds found nothing to rewrite and were the kept build
/// again, 6,057 of 27,388 builds, 18 % of the Os compile.
#[test]
fn a_size_trial_whose_rewrite_finds_nothing_costs_neither_a_late_stage_nor_an_allocation() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\ndeclare i32 @ext(i32)\ndefine i32 @f(i32 %a) {\n  %x = call i32 @ext(i32 %a)\n  %y = call i32 @ext(i32 %x)\n  %z = add i32 %x, %y\n  ret i32 %z\n}\n";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let stages = |relax_classes, data_constants| {
        let options = Options { size: true, mid_end: false, relax_classes, data_constants, ..Options::default() };
        let before = runs();
        items_with(&module, "f", Cpu::M68020, options).expect("compiles");
        let after = runs();
        (after.late - before.late, after.allocations - before.allocations)
    };
    let (without, with) = (stages(false, false), stages(true, true));
    assert_eq!(with, without, "(late stages, allocations): {with:?} with the two trials on, {without:?} with them off");
}

/// Choosing the call targets expanded the function to price it as it would
/// stand with the choice, and the build expanded the same function again for
/// its output: one expansion more than the search made, a build in the Os
/// compile (`x expand lines` 4.6 % of the backend's time, QB 68020).
#[test]
fn the_expansion_the_call_target_search_priced_is_the_one_the_build_prints() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\ndeclare i32 @ext(i32)\ndefine i32 @f(i32 %a) {\n  %x = call i32 @ext(i32 %a)\n  %y = call i32 @ext(i32 %x)\n  %z = call i32 @ext(i32 %y)\n  ret i32 %z\n}\n";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let expansions = |call_targets| {
        let options = Options {
            size: true,
            mid_end: false,
            call_targets,
            relax_classes: false,
            data_constants: false,
            ..Options::default()
        };
        let before = runs().expansions;
        items_with(&module, "f", Cpu::M68000, options).expect("compiles");
        runs().expansions - before
    };
    let (off, on) = (expansions(false), expansions(true));
    // the search expands once as it stands and once per set it tries (one
    // callee here); the build adds none
    assert!(on <= off + 1, "{on} expansions with the search, {off} without");
}
