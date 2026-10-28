//! A broken hint is recolored after the allocation (`tryHintsRecoloring`).

mod common;

/// matmul at -O2 m32: a range that took a register other than its hint's, after
/// an eviction freed the hint's, kept `mov ecx, edi` and `mov edi, ecx` around
/// the inner loop (7 register copies in `bench_matmul`; 5 once recolored).
#[test]
fn test_a_broken_hint_is_recolored_once_the_register_is_free() {
    let source = format!("{}/bench/matmul/matmul.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "--isel=dag", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let body = asm.split("_bench_matmul proc").nth(1).and_then(|rest| rest.split("endp").next()).unwrap();
    let copies = body
        .lines()
        .filter_map(|line| line.trim().strip_prefix("mov "))
        .filter(|rest| {
            let mut parts = rest.split(", ");
            let register = |part: Option<&str>| part.is_some_and(|p| p.len() == 3 && p.starts_with('e'));
            register(parts.next()) && register(parts.next())
        })
        .count();
    assert!(copies <= 5, "{copies} register copies: {body}");
}
