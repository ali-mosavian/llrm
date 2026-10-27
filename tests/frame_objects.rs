//! A local array lies on its element's alignment: after a `short[3]` a
//! `long[4]` was at esp+10, and every dword access to it cost a 486 3 clocks
//! (Intel486 data sheet 240440-002, section 10.1). The frame was aligned only
//! where it keeps a double.

mod common;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-march=i486", "-O1", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

#[test]
fn a_long_array_after_a_short_array_is_on_a_dword() {
    let text = listing(
        "extern void use(short *, long *);\n\
         long f(int n) { short a[3]; long c[4]; use(a, c); return c[n & 3] + a[n & 1]; }\n",
    );
    let dwords: Vec<&str> = text.lines().filter(|line| line.contains("dword ptr [esp+")).collect();
    assert!(!dwords.is_empty(), "{text}");
    for line in dwords {
        let at: i64 = line.rsplit(['+', ']']).nth(1).unwrap().trim().parse().unwrap_or(0);
        assert_eq!(at % 4, 0, "{line}");
    }
}

/// Aligning each `long` after a `short` padded the frame (jpeg_fixed bq32 -Os:
/// +9 B of displacements gone to disp32): the gap a `long` leaves takes the
/// next `short`, so the frame is the size it was when the `long`s were
/// misaligned.
#[test]
fn a_gap_left_by_alignment_takes_the_next_short() {
    let text = listing(
        "extern void use(short *, long *, short *, long *, short *, long *);\n\
         void f(void) { short a; long b; short c; long d; short e; long f; use(&a, &b, &c, &d, &e, &f); }\n",
    );
    assert!(text.contains("sub esp, 24"), "{text}");
}
