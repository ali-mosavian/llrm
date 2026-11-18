//! At m32 -Os the object decides between a switch's lookup table and its
//! compares.

mod common;

/// x_switch_ranges at -Os turned `class_of` (66 cases in 114 values, six
/// results) into a 114-byte lookup table: 207 B against gcc's 203 with its
/// compares. The trial compile that builds no table is smaller and stays.
#[test]
fn at_os_a_sparse_switch_is_compares_where_the_table_costs_more() {
    let source = format!(
        "{}/crates/target/llrm-x86-m32/vsgcc/kernels/x_switch_ranges/x_switch_ranges.c",
        env!("CARGO_MANIFEST_DIR")
    );
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-Os", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(!text.contains("switch$table"), "{text}");
}
