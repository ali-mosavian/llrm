//! A static function called from one place goes with that call, whatever its size.

use std::process::Command;

const KERNEL: &str = "
static int sum(const int *a, int n)
{
    int s = 0, i;
    for (i = 0; i < n; ++i) {
        if (a[i] > 3) s += a[i] * a[i]; else s -= a[i];
        s ^= s >> 3;
        s += (a[i] & 7) * (s | 1);
    }
    return s;
}
static int twice(const int *a, int n)
{
    int s = 0, i;
    for (i = 0; i < n; ++i) {
        if (a[i] > 5) s += a[i] * 3; else s -= a[i];
        s ^= s >> 2;
        s += (a[i] & 3) * (s | 1);
    }
    return s;
}
int f(const int *a, int n) { return sum(a, n) + 1; }
int g(const int *a, int n) { return twice(a, n) + twice(a + 1, n - 1); }
";

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(directory).args(["-m32", "-march=i486"]).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// Only a body under 24 operations was inlined into its one call: queens' `safe` stayed a call (and `place`
/// held its values in the frame) where gcc inlines it and ran 30% fewer clocks.
#[test]
fn test_a_static_function_called_once_is_inlined_at_any_size() {
    for flags in [&["-O2"][..], &["-Os"]] {
        let text = listing(flags);
        assert!(!text.contains("sum_ proc"), "{flags:?}: sum survived: {text}");
        assert!(text.contains("twice_ proc"), "{flags:?}: twice, called twice, is no last call: {text}");
    }
    // Where inlining is off, it is off.
    assert!(listing(&["-O2", "-fno-inline-functions"]).contains("sum_ proc"));
}

/// What "nothing else reaches it" excludes: each of these is called from one place and stays defined, because
/// something besides that call can still enter it.
fn survives(source: &str, name: &str) {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(directory).args(["-m32", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    assert!(text.contains(&format!("{name} proc")), "{name} was inlined away: {text}");
}

const BODY: &str = "{ int s = 0, i; for (i = 0; i < n; ++i) { if (a[i] > 3) s += a[i] * a[i]; else s -= a[i]; s ^= s >> 3; s += (a[i] & 7) * (s | 1); } return s; }";

#[test]
fn test_an_exported_function_called_once_stays_defined() {
    survives(&format!("int body(const int *a, int n) {BODY}\nint f(const int *a, int n) {{ return body(a, n) + 1; }}"), "body_");
}

#[test]
fn test_a_function_whose_address_is_taken_stays_defined() {
    survives(&format!("static int body(const int *a, int n) {BODY}\nint (*volatile hook)(const int *, int);\nint f(const int *a, int n) {{ hook = body; return body(a, n) + 1; }}"), "body_");
}

#[test]
fn test_a_function_a_table_names_stays_defined() {
    survives(&format!("static int body(const int *a, int n) {BODY}\nint (*const table[1])(const int *, int) = {{ body }};\nint f(const int *a, int n) {{ return body(a, n) + 1; }}\nint g(const int *a, int n) {{ return table[0](a, n); }}"), "body_");
}
