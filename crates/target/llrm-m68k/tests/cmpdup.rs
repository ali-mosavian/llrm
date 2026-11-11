//! `a[m] == key` then `a[m] < key` (bench/library/bsearch) compared the same
//! two registers twice, `cmp.l; beq; cmp.l; bge`: 2 B and 6 clocks a probe on
//! the 68000 for codes the first compare had left.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const BSEARCH: &str = include_str!("../bench/library/bsearch.ll");

fn compares(options: Options) -> usize {
    let module = llrm_mir::parse::module(BSEARCH).expect("parses");
    let text = text_with(&module, "k", Cpu::M68000, options).expect("compiles");
    text.lines().filter(|line| line.starts_with("\tcmp")).count()
}

#[test]
fn a_compare_the_branch_before_it_made_is_not_repeated() {
    let ours = compares(Options { size: true, ..Options::default() });
    assert_eq!(ours, 2, "bsearch compares {ours} times");
}

#[test]
fn the_pinned_build_keeps_the_repeat() {
    assert!(compares(Options::PINNED) > 2);
}
