use llrm_mir::interpret::{Val, run};

const TEXT: &str = "declare void @llvm.experimental.memset.pattern.p0.i16.i32(ptr, i16, i32, i1)\n\
    define i32 @f(i32 %n) {\n  %buf = alloca [8 x i16]\n\
    call void @llvm.experimental.memset.pattern.p0.i16.i32(ptr %buf, i16 7, i32 %n, i1 false)\n\
    %last = getelementptr inbounds i16, ptr %buf, i32 3\n  %v = load i16, ptr %last\n  %w = zext i16 %v to i32\n  ret i32 %w\n}\n";

fn answer(
    module: &llrm_mir::module::Module,
    n: u128,
) -> Val {
    run(module, "f", vec![Val::Int { bits: n, width: 32 }], 10_000).unwrap_or_else(|trap| panic!("{trap:?}"))
}

/// The loop writes the elements the intrinsic does: element 3 is the pattern
/// once the length reaches it, and the loop never runs for length zero.
#[test]
fn the_loop_stores_what_the_intrinsic_stores() {
    let before = llrm_mir::parse::module(TEXT).expect("parses");
    let mut after = before.clone();
    super::memsetpattern::expanded(&mut after).expect("expands");
    assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
    assert!(!llrm_mir::print::module(&after).contains("call void @llvm.experimental"), "the call is gone");
    for n in [0, 3, 4, 8] {
        assert_eq!(answer(&after, n), answer(&before, n), "length {n}");
    }
}

/// The shadow run tries the DAG path on every function, so it needs the same
/// expanded input as `--isel=dag`: it was handed the intrinsic and 9 functions
/// of the m32 corpus (pattern_fill, fill_nest, idioms, loop_fusion, huge/fillw)
/// were refused `intrinsic call llvm.experimental.memset.pattern`.
#[test]
fn the_shadow_run_expands_patterns_as_the_dag_run_does() {
    use crate::backend::memsetpattern::expands_for;
    use crate::backend::shadow::Selector;
    assert!(expands_for(Selector::Dag) && expands_for(Selector::Shadow));
    assert!(!expands_for(Selector::Old), "the old selector keeps the intrinsic");
}
