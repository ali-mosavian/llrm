//! The code-generator stream is written by cgshim's own formatter (not
//! printf, which cost a third of wccq's instructions on d_faces): values the
//! front end passes as signed, 64-bit, escaped and raw bytes reach the object
//! as the source wrote them.

mod common;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

const SOURCE: &str = r#"long long big = 0x123456789abcdefLL;
long long neg = -5000000000LL;
int low = -2147483647 - 1;
char text[] = "a\"b\\\001\377z";
int pick(int x, int *p)
{
    switch (x) {
    case -7: return 1;
    case 3000000: return p[-3];
    default: return (int)(neg >> 3) + low;
    }
}
"#;

/// A 64-bit value, a negative one and a case label passed through the stream
/// as text: each digit string must read back as written.
/// llrm-cc: the compare and the load name the register clang's IR and the pin's
/// allocator pick (`ecx`, not `edx`), so the operand's digits are matched, not
/// the register.
#[test]
fn test_the_stream_carries_signed_wide_and_escaped_values_exactly() {
    let text = listing(SOURCE);
    for expected in [
        "db 0efh,0cdh,0abh,089h,067h,045h,023h,001h",
        "db 000h,00eh,0fah,0d5h,0feh,0ffh,0ffh,0ffh",
        "db 000h,000h,000h,080h",
        "db 061h,022h,062h,05ch,001h,0ffh,07ah,000h",
    ] {
        assert!(text.lines().any(|line| line.trim() == expected), "missing {expected:?} in\n{text}");
    }
    // The register is the allocator's to choose; the operand's digits are the
    // test.
    for (head, tail) in [("cmp ", ", -7"), ("cmp ", ", 3000000"), ("mov ", "-12]")] {
        assert!(
            text.lines().any(|line| line.trim().starts_with(head) && line.trim().ends_with(tail)),
            "missing {head:?}..{tail:?} in\n{text}"
        );
    }
}
