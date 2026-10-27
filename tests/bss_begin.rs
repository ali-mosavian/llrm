//! `-mbss-begin`: each object with near uninitialised data opens it with an
//! empty segment `BSSB` of the same class, so a start-up can zero from the
//! first byte of all of it whatever order the linker met the objects in (the QB
//! runtime's start.asm, #1488).

mod common;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("t.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m16", "-Os"])
        .args(flags)
        .args(["-S", "-o", "t.s", "t.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("t.s")).unwrap()
}

const WITH_STATIC: &str = "static int counter;\nint bump(void) { return ++counter; }\n";

#[test]
fn test_an_object_with_a_zero_static_opens_its_bss_with_bssb() {
    let text = listing(WITH_STATIC, &["-mbss-begin"]);
    let begin = text.find("BSSB segment word public 'BSS'").unwrap_or_else(|| panic!("no BSSB: {text}"));
    let bss = text.find(".data?").expect("the static is in _BSS");
    assert!(begin < bss, "BSSB follows _BSS: {text}");
    assert!(text.contains("BSSB ends") && text.contains("DGROUP group BSSB"), "{text}");
}

#[test]
fn test_without_the_flag_or_without_a_static_there_is_no_bssb() {
    assert!(!listing(WITH_STATIC, &[]).contains("BSSB"));
    assert!(!listing("int one(void) { return 1; }\n", &["-mbss-begin"]).contains("BSSB"));
}
