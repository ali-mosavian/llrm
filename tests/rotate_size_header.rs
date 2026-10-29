//! At -Os loop rotation copies a header only when it is four instructions at
//! most (`looprotate::SIZE_MAX_HEADER`).

mod common;

fn rotate_size_listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_c().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

const WALK: &str =
    "extern void use(int);\nvoid walk(const char *p) { while (((*p ^ 5) + 3) * 7 != 0) { use(*p); p++; } }\n";
const COUNT: &str = "extern void use(int);\nvoid count(int n) { int i; for (i = 0; i < n; i++) use(i); }\n";

/// The pin copies the header above the loop at -Os too: clang -Os -m32
/// -march=i386 on this loop loads the byte before it and again at its end
/// (`movb (%esi), %al; cmpb $-8; je` ... `movb (%esi), %al; cmpb $-8; jne`),
/// so the count is the pin's, two. The earlier bound of one kept a header with
/// a load out of the copy, which LoopRotate does not.
#[test]
fn at_os_a_header_with_a_load_is_copied_as_the_pin_does() {
    let text = rotate_size_listing(WALK, &["-m16", "-march=i386", "-Os"]);
    assert_eq!(text.matches("byte ptr [").count(), 2, "{text}");
}

/// llrm-cc: counts the loads of the byte, as above.
#[test]
fn at_o2_the_same_header_is_copied() {
    let text = rotate_size_listing(WALK, &["-m16", "-march=i386", "-O2"]);
    assert_eq!(text.matches("byte ptr [").count(), 2, "{text}");
}

#[test]
fn at_os_a_compare_and_branch_header_is_still_rotated() {
    let text = rotate_size_listing(COUNT, &["-m16", "-march=i386", "-Os"]);
    assert!(!text.contains("jmp"), "{text}");
}
