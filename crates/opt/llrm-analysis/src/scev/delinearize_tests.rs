//! Delinearization against the pinned LLVM: `tools/scev/delinearize.txt` is
//! what `opt -passes='print<delinearization>'` printed for
//! `tools/scev/delinearize.ll` (`tools/scev/delinearize.py --golden` writes
//! it).

use super::delinearize::print_module_delinearization;

const SOURCE: &str = include_str!("../../../../../tools/scev/delinearize.ll");
const GOLDEN: &str = include_str!("../../../../../tools/scev/delinearize.txt");

/// The array declaration, subscripts and validation of every access of the
/// corpus print as LLVM's: parametric sizes from the strides, fixed sizes from
/// constant steps, none for a stride that divides into no dimension.
#[test]
fn every_access_of_the_corpus_delinearizes_as_the_pinned_llvm_does() {
    let module = llrm_mir::parse::module(SOURCE).expect("the corpus parses");
    let mut got = String::new();
    for line in print_module_delinearization(&module).lines() {
        if let Some(name) = line.strip_prefix("Printing analysis 'Delinearization' for function '") {
            got += &format!("@{}\n", name.trim_end_matches("':"));
        } else if !line.is_empty() && !line.starts_with("Inst:") {
            got += &format!("  {line}\n");
        }
    }
    assert_eq!(got, GOLDEN);
}

/// `getelementptr [0 x [12 x i8]], ptr %a, i64 0, i64 %i, i64 %j` panicked in
/// `getGEPExpr` ("an element type"): the walk asked a zero-length array for its
/// member 0. LLVM indexes an array's element type whatever its length. It
/// stopped `print<da>` on PR148435.
#[test]
fn a_gep_into_a_zero_length_array_has_a_scev() {
    let text = "define void @f(ptr %a, i64 %i, i64 %j) {\nentry:\n  %p = getelementptr inbounds [0 x [12 x i8]], ptr %a, i64 0, i64 %i, i64 %j\n  store i8 0, ptr %p\n  ret void\n}\n";
    let module = llrm_mir::parse::module(text).expect("the function parses");
    let printed = crate::scev::values::print_module_values(&module);
    assert!(printed.contains("((12 * %i)<nsw> + %j + %a)"), "{printed}");
}
