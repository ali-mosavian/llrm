//! The translated greedy allocator and the values an instruction requires in
//! one register.

mod common;

const CENTROID: &str = "typedef struct { short x, y; } CPoint;\n\
void centroid(const CPoint *points, unsigned short count, CPoint *out)\n\
{\n    long x = 0, y = 0;\n    unsigned short i;\n    for (i = 0; i < count; ++i) {\n        x += points[i].x;\n        y += points[i].y;\n    }\n\
    out->x = count ? (short)(x / count) : 0;\n    out->y = count ? (short)(y / count) : 0;\n}\n";

/// examples/interop/main.nib printed `centroid 0,0`: under `LLRM_GREEDY=1` a
/// last-chance recoloring moved `out`, which the Watcom convention delivers in
/// BX, to CX, because only the spill products were kept from being recolored.
/// The checker (`LLRM_GREEDY_CHECK`) reads every requirement of the result.
#[test]
fn a_value_an_instruction_requires_in_one_register_is_not_moved_by_the_greedy_allocator() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), CENTROID).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .env("LLRM_GREEDY_CHECK", "1")
        .args(["-O2", "-march=i486", "-mabi=watcom", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{said}");
    assert!(!said.contains("greedy-check"), "{said}");
}
