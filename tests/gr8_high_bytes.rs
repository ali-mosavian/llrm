//! The byte values of a function take the high byte registers too (the GR8
//! class of X86RegisterInfo.td: AL CL DL AH CH DH BL BH).

mod common;

/// vsgcc's x_reverse at -O1: the second byte of the swap went to a register of
/// its own and the loop's counter was evicted and spilled, so the byte was
/// widened with `movzx` (3 clocks on a 486) and `n` was read from the stack.
/// The pin loads it with `mov ch, byte [reg]` and spills nothing; which of
/// esi and edi holds the pointer is the allocation's choice.
#[test]
fn two_bytes_of_a_swap_share_a_register_and_nothing_spills() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/target/llrm-x86-m32/vsgcc/kernels/x_reverse/x_reverse.c");
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O1", "--isel=dag", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let body = asm.split("_bench_x_reverse proc").nth(1).and_then(|rest| rest.split("endp").next()).unwrap();
    assert!(body.lines().any(|l| l.trim_start().starts_with("mov ch, byte ptr [")), "{body}");
    assert!(!body.contains("[esp+") || body.matches("[esp+").count() == 1, "{body}");
}
