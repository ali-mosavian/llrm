//! `llrm-arm64-llc --version` printed "unrecognized argument": a dist's backend could not be told from another build's
//! (tools/stamp.py refused every dist that carried it, or took one copied in from anywhere).

#[test]
fn the_llc_answers_version_with_the_commit_it_was_built_from() {
    llrm_codegen::support::version::assert_answers(env!("CARGO_BIN_EXE_llrm-arm64-llc"), "llrm-arm64-llc", env!("LLRM_BUILD_SHA"));
}
