//! At -Os a loop of several stores stays one where a fill for each store is
//! more bytes than the loop.

mod common;

fn fill_listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(directory)
        .args(flags)
        .args(["-m16", "-march=i386", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

const TWO_OBJECTS: &str = "short x[64], y[64];\nvoid init(void)\n{\n    int i;\n    for (i = 0; i < 64; ++i) {\n        x[i] = 0;\n        y[i] = -1;\n    }\n}\n";

const ONE_OBJECT: &str = "struct { short a[64], b[64]; } s;\nvoid init(void)\n{\n    int i;\n    for (i = 0; i < 64; ++i) {\n        s.a[i] = -1;\n        s.b[i] = -1;\n    }\n}\n";

/// lru.c m16 -Os: a 3-trip loop of four stores became five `rep stosb`, 32
/// bytes over the loop.
#[test]
fn at_os_two_stores_a_trip_are_not_two_rep_stos() {
    let text = fill_listing(TWO_OBJECTS, &["-Os"]);
    assert!(!text.contains("rep stos"), "{text}");
}

#[test]
fn at_o2_the_same_loop_is_fills() {
    let text = fill_listing(TWO_OBJECTS, &["-O2"]);
    assert!(text.contains("rep stos"), "{text}");
}

/// bdb4328bed: a size-only fill pays its clocks in the bytes it saves (10
/// clocks a byte at m16 -Os). This 64-trip loop is 19 B against the fill's 16,
/// and the rule prices the fill's extra clocks above 3 B x 10, so
/// the loop stays. It was one fill before the rule; QCport
/// -Os modules are 41 B smaller in all with it (sc.c 11499 to 11484).
#[test]
fn at_os_stores_into_one_object_stay_a_loop_when_the_fill_saves_too_little() {
    let text = fill_listing(ONE_OBJECT, &["-Os"]);
    assert!(!text.contains("rep stos"), "{text}");
    assert!(text.contains("jne"), "{text}");
}
