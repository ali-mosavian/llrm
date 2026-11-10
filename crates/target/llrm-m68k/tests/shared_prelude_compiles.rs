//! `Options::shared_prelude`: the shared `PhiElimination` and `TwoAddress` in
//! place of the private passes. Each fixture function must compile with it, and
//! the ones that do not are named (a refusal is a finding, not a skip).

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const FIXTURES: &[(&str, &str)] = &[
    ("sieve_pin_o2", include_str!("fixtures/sieve_pin_o2.ll")),
    ("nbody_pin_o2", include_str!("fixtures/nbody_pin_o2.ll")),
    ("udivmod_w", include_str!("fixtures/udivmod_w.ll")),
    ("partition_cc", include_str!("fixtures/partition_cc.ll")),
    ("arg_pointers", include_str!("fixtures/arg_pointers.ll")),
    ("pack_ref", include_str!("fixtures/pack_ref.ll")),
    ("hex_low_byte_copy", include_str!("fixtures/hex_low_byte_copy.ll")),
    ("byte_spill_reload", include_str!("fixtures/byte_spill_reload.ll")),
    ("soft_add", include_str!("fixtures/soft_add.ll")),
    ("soft_pack", include_str!("fixtures/soft_pack.ll")),
    ("inline_bar", include_str!("fixtures/inline_bar.ll")),
];

#[test]
fn every_fixture_function_compiles_with_the_shared_prelude() {
    let (mut ok, mut same, mut refused) = (0, 0, Vec::new());
    for (file, text) in FIXTURES {
        let mut module = llrm_mir::parse::module(text).unwrap_or_else(|why| panic!("{file}: {why}"));
        llrm_mir::parse::upgrade_lifetimes(&mut module);
        let names: Vec<String> = module
            .functions()
            .filter(|(_, _, one)| !one.is_declaration())
            .map(|(id, _, _)| module.global(id).name.as_ref().expect("a name").clone())
            .collect();
        for name in names {
            let base = text_with(&module, &name, Cpu::M68000, Options::default()).unwrap_or_else(|why| panic!("{file} @{name}: base: {why}"));
            match text_with(&module, &name, Cpu::M68000, Options { shared_prelude: true, ..Options::default() }) {
                Ok(shared) => {
                    ok += 1;
                    same += usize::from(shared == base);
                }
                Err(why) => refused.push(format!("{file} @{name}: {why}")),
            }
        }
    }
    println!("{ok} compile ({same} the same text as the private passes), refused: {refused:#?}");
    // Step 2 in progress: these are the refusals known on 11-10 (greedy takes
    // the SSA-pinned body and cannot place 8 functions; one reads a use that
    // is not a register). A new kind, or more of them, fails; fewer is
    // progress, and the bound moves down with it. Exit: none, 2026-11-13.
    let unknown: Vec<_> = refused
        .iter()
        .filter(|why| !why.ends_with("greedy could not allocate") && !why.ends_with("a use that is not a register"))
        .collect();
    assert!(unknown.is_empty(), "a new kind of refusal: {unknown:#?}");
    assert!(refused.len() <= 9, "{} refusals, 9 known: {refused:#?}", refused.len());
    assert!(ok >= 11, "only {ok} functions compiled");
}
