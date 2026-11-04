//! A byte product read in another block is exported.

mod common;

/// `(f & 3) * 0x55` computed before a loop that reads it failed the verifier
/// under the DAG selector (`value 13 is read and not defined on every path`;
/// gfxdev.c `_packed4_glyph`, so no program linking it built): `MUL8r`'s AX
/// result was taken for its flags because the form lists no explicit result,
/// and the copy that exports it was dropped.
#[test]
fn test_a_byte_product_used_in_a_loop_compiles() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "void g(unsigned char *at, unsigned h, unsigned f, unsigned char *b)\n{\n    unsigned ink = (f & 3) * 0x55;\n    for (; h; h--, b++) at[0] = *b & ink;\n}\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["a.c", "-m16", "-march=i386", "-O2", "--isel=dag", "-S", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
