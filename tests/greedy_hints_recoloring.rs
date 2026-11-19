//! A broken hint is recolored after the allocation (`tryHintsRecoloring`).

mod common;

/// bintree at -O2 m32: the range live across `call _walk` took a register other
/// than its hint's, after an eviction freed the hint's. Recolored, the copy
/// into it sits before the loop; not, `mov esi, ecx` and `mov ecx, esi` both
/// ran around the call every trip (3 register copies in the loop, 2 recolored).
/// matmul used to show this (7 copies, 5 recolored) until its full unroll was
/// capped (bf74ea331c): recoloring no longer changes it.
#[test]
fn test_a_broken_hint_is_recolored_once_the_register_is_free() {
    let source = format!("{}/bench/bintree/bintree.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "--isel=dag", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = asm.lines().collect();
    let is_copy = |line: &str| {
        line.trim()
            .strip_prefix("mov ")
            .is_some_and(
                |rest| {
                    let mut parts = rest.split(", ");
                    let register = |part: Option<&str>| part.is_some_and(|p| p.len() == 3 && p.starts_with('e'));
                    register(parts.next()) && register(parts.next())
                },
            )
    };
    // The loop around the call: from its label to the jump back to it.
    let (head, back) = (0..lines.len())
        .find_map(|back| {
            let target = lines[back].trim().strip_prefix('j').and_then(|jump| jump.split_whitespace().nth(1))?;
            let head = lines.iter().position(|line| line.trim() == format!("{target}:"))?;
            (head < back && lines[head..back].iter().any(|line| line.contains("call _walk"))).then_some((head, back))
        })
        .expect("a loop that calls _walk");
    let copies = lines[head..=back].iter().filter(|line| is_copy(line)).count();
    assert!(copies <= 2, "{copies} register copies in the loop: {}", lines[head..=back].join("\n"));
}
