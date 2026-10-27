//! BC objects are lifted to regular IR: no target intrinsic, no inline assembly
//! and no target attribute in what the frontend emits, and every intrinsic it
//! calls is on a short list. Address spaces (near, far and segment pointers;
//! space 2 is the DGROUP selector) are the one target-flavoured element
//! allowed, so they are not checked here.

use std::collections::BTreeSet;

/// Intrinsics the lifter may call, by name up to the type suffix: standard LLVM
/// ones, and the QB runtime routines it declares (`@llrm.qb.*`).
const ALLOWED: [&str; 9] = [
    "@llvm.lrint.",
    "@llvm.memset.",
    "@llvm.memcpy.",
    "@llvm.memmove.",
    "@llvm.sqrt.",
    "@llvm.fabs.",
    "@llvm.uadd.with.overflow.",
    "@llvm.usub.with.overflow.",
    "@llrm.qb.",
];

/// What in `text` a pure frontend must not emit.
fn impure(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for word in text.split(|c: char| !(c.is_alphanumeric() || "._@$-".contains(c))) {
        let named = word.starts_with("@llvm.") || word.starts_with("@llrm.");
        if named && !ALLOWED.iter().any(|prefix| word.starts_with(prefix)) {
            found.insert(format!("intrinsic {word}"));
        }
        if word.contains("llvm.x86") || word == "asm" || word.starts_with("target-") {
            found.insert(format!("target content {word}"));
        }
    }
    found
}

#[test]
fn a_planted_target_intrinsic_inline_asm_or_target_attribute_is_found() {
    let text = "call @llvm.x86.sse.rcp.ss(x)\ncall asm \"nop\"\nattributes { \"target-cpu\"=\"i486\" }\ncall @llrm.ia16.asm.90.0.0.0.n()\ncall @llvm.sqrt.f64(x)";
    let found = impure(text);
    assert_eq!(found.len(), 5, "{found:?}");
    assert!(impure("call @llvm.sqrt.f64(x)\ncall @llrm.qb.B$PRNT()").is_empty());
}

#[test]
fn every_sample_bc_object_lifts_to_ir_with_no_target_content() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/omf");
    let mut files: Vec<_> =
        std::fs::read_dir(&dir).expect("the samples").map(|one| one.expect("entry").path()).collect();
    files.sort();
    let mut lifted = 0;
    for path in files {
        let Ok(Some(found)) = llrm_omf::module::load(&path) else { continue };
        let Ok(raised) = llrm_bcobj::raise(&found, &llrm_x86_m16::machine::BUILT_IN) else { continue };
        lifted += 1;
        let found = impure(&llrm_mir::print::module(&raised.module));
        assert!(found.is_empty(), "{}: {found:?}", path.display());
    }
    assert!(lifted >= 100, "only {lifted} samples lifted: the check saw too little");
}
