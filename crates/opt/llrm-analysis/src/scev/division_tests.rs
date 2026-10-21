//! `SCEVDivision` against the pinned LLVM: `tools/scev/division.txt` is what
//! `opt -passes='print<scev-division>'` printed for `tools/scev/division.ll`
//! (`tools/scev/division.py --golden` writes it).

use super::division::print_module_division;

const SOURCE: &str = include_str!("../../../../../tools/scev/division.ll");
const GOLDEN: &str = include_str!("../../../../../tools/scev/division.txt");

/// The quotient and remainder of every signed division of the corpus print as
/// LLVM's: a product splits by its factors, a sum by its terms, an unknown
/// denominator by replacing it with 0 and 1.
#[test]
fn every_division_of_the_corpus_prints_as_the_pinned_llvm_does() {
    let module = llrm_mir::parse::module(SOURCE).expect("the corpus parses");
    let mut got = String::new();
    for line in print_module_division(&module).lines() {
        if let Some(name) = line.strip_prefix("Printing analysis 'Scalar Evolution Division' for function '") {
            got += &format!("@{}\n", name.trim_end_matches("':"));
        } else if line.starts_with("  ") {
            got += &format!("{line}\n");
        }
    }
    assert_eq!(got, GOLDEN);
}
