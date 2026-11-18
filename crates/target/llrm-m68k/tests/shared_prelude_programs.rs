//! `Options::shared_prelude` on whole programs from the bench corpus, through
//! the driver's mid-end at each level. A refusal is a finding: with the flag
//! on, 35 of the corpus' compiles stopped (11-10), the first in `fib` and
//! `strcmp` at -O1 with "an instruction from nowhere". `overflow_loop` is the
//! one the tied write-back address broke: the shared `TwoAddress` tied only the
//! first pair of `ADD16do_WB`, so the post-incremented pointer was allocated
//! apart from the one it advanced and `overflow_builtins` printed wrong
//! results.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, lowered_for};

const PROGRAMS: &[(&str, &str)] = &[
    ("fib_rec", include_str!("fixtures/fib_rec_raw.ll")),
    ("strcmp", include_str!("fixtures/strcmp_raw.ll")),
    ("grep", include_str!("fixtures/grep_raw.ll")),
    // two reads of s[j] before the pointer moves: `add.w (An)+` then `sub.w
    // (An)+`
    ("overflow_loop", include_str!("fixtures/overflow_loop_raw.ll")),
    ("mandel", include_str!("fixtures/mandel_raw.ll")),
    // `move.l sp, -(An)`: the stack pointer is dropped from the sources, and the
    // row's source 1 (the address) was none: the write-back def went untied
    ("nbody_single", include_str!("fixtures/nbody_single_raw.ll")),
    // an i64 result whose low word is computed before a select's split: its copy
    // into the return register stayed in the head, the `RET` that reads it in the join
    ("i64_select_returns", include_str!("fixtures/i64_select_returns.ll")),
    // `CCR_SAVE` reads the flags the `cmp` before it set without naming them
    ("word_compare_branches", include_str!("fixtures/word_compare_branches_raw.ll")),
];

#[test]
fn programs_that_compile_with_the_private_passes_compile_with_the_shared_prelude() {
    let mut refused = Vec::new();
    for (file, text) in PROGRAMS {
        for level in [Level::O1, Level::O2, Level::Os] {
            let module = lowered_for(text, Cpu::M68000, level).unwrap_or_else(|why| panic!("{file}: {why}"));
            let names: Vec<String> = module
                .functions()
                .filter(|(_, _, one)| !one.is_declaration())
                .map(|(id, _, _)| module.global(id).name.as_ref().expect("a name").clone())
                .collect();
            for name in names {
                let options = Options { mid_end: false, ..Options::default() };
                text_with(&module, &name, Cpu::M68000, options)
                    .unwrap_or_else(|why| panic!("{file} @{name}: private: {why}"));
                if let Err(why) = text_with(&module, &name, Cpu::M68000, Options { shared_prelude: true, ..options }) {
                    refused.push(format!("{file} {level:?} @{name}: {why}"));
                }
            }
        }
    }
    assert!(refused.is_empty(), "{refused:#?}");
    assert_eq!(llrm_codegen::backend::twoaddr::skipped_ties(), 0, "a declared tie named an operand its instruction lacks");
}
