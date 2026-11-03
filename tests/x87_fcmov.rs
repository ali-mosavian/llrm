//! A select of two x87 values is `fcmovCC` where the target has the P6
//! conditional moves, and the branch diamond below it.

mod common;

/// `(unsigned long long)long_double`: a select of 2^63 or 0.0 that is
/// subtracted.
const SOURCE: &str = "unsigned long long f(long double d) { return (unsigned long long)d; }\n";

fn assembled(args: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(args)
        .args(["-Os", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// x86-64 has `cmov`: the select of two long double constants was a compare, a
/// jump and a pop on one arm (double_to_uint64 -Os m64: +6 B over main).
#[test]
fn test_a_long_double_select_is_an_fcmov_where_the_target_has_cmov() {
    let text = assembled(&["--target=x86_64-linux"]);
    assert!(text.contains("fcmov"), "{text}");
}

/// The 486 has no `fcmov`: the diamond stays.
#[test]
fn test_a_long_double_select_is_a_branch_where_the_target_has_no_cmov() {
    let text = assembled(&["-m32", "-mabi=sysv", "-march=i486"]);
    assert!(!text.contains("fcmov"), "{text}");
}
