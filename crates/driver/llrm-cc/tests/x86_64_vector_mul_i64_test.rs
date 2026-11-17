//! x86-64: a loop whose induction is multiplied as `i64` is vectorized (the
//! pin's SSE2 `v2i64` multiply row is 7 a register). The row was missing, every
//! width was "not priced", and tile's 64-wide fill stayed scalar: bench tile
//! -O2 ran 14,000 instructions over gcc. The vectorized multiply must also
//! select: `v2i64` has no SSE2 `pmullq`, so it is three `pmuludq`. The scalar
//! loop's price counts no address (the pin's x86 `getAddressComputationCost` is
//! 0): with 1 a load, bench dot -O2 vectorized a sum of `i16` products at VF 8
//! and ran 21,617 instructions against 10,977. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn compile_body(
    name: &str,
    func: &str,
) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vector-mul-i64-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/vector_mul_i64.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let (head, tail) = (format!("{func} proc near"), format!("{func} endp"));
    text.split(head.as_str()).nth(1).and_then(|rest| rest.split(tail.as_str()).next()).expect("a body").to_string()
}

#[test]
fn an_i64_induction_multiply_loop_is_vectorized_at_x86_64() {
    let body = compile_body("fill", "fill_i64");
    let vector_stores = body.matches("xmmword ptr [rdi").count();
    let scalar_stores = body.matches("mov word ptr [rdi").count();
    assert!(
        vector_stores >= 4 && scalar_stores == 0,
        "{vector_stores} vector and {scalar_stores} scalar stores:\n{body}"
    );
}

#[test]
fn a_vector_i64_multiply_selects_pmuludq_at_x86_64() {
    for (func, pieces) in [("mul_v2i64", 3), ("mul_v4i64", 6)] {
        let body = compile_body(func, func);
        let n = body.matches("pmuludq").count();
        assert_eq!(n, pieces, "{func}: {n} pmuludq (three per 128 bits):\n{body}");
    }
}

#[test]
fn a_sum_of_i16_products_stays_scalar_at_x86_64() {
    let body = compile_body("dot_short", "dot_short");
    assert!(!body.contains("pmuludq"), "vectorized (a scalar load priced an address):\n{body}");
}
