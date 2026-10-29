//! At m16 -O2 an 8-trip loop nest stays a loop: code bytes outweigh clocks.

mod common;

/// The pin's full-unroll threshold (150, boosted to 600) copied matmul's
/// 8-trip inner product 8 times inside 8 x 8 outer copies: 2045 code bytes at
/// -O2 against 306 rolled (main: 241), 29 `imul`s against 4.
#[test]
fn at_o2_a_nest_of_eight_trip_loops_keeps_its_multiplies_in_a_loop() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    let source = format!("{}/bench/matmul/matmul.c", env!("CARGO_MANIFEST_DIR"));
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-O2", "-m16", "-march=i386", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let multiplies = text.matches("imul").count();
    assert!(multiplies <= 8, "{multiplies} multiplies\n{text}");
}

/// `MaxStoresPerMemset` 16 at -O2 made a 32-byte zeroing eight 6-byte
/// `mov dword ptr [bp-N], 0` (48 bytes) where the fill is about 13: m16 takes
/// its 4-store limit at every level (matmul.nib 2090 -> 405, main 374).
#[test]
fn at_o2_a_32_byte_memset_is_a_fill_not_eight_stores() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("z.c"),
        "extern void use(char *p);\nvoid f(void) { char b[32]; __builtin_memset(b, 0, 32); use(b); }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-O2", "-m16", "-march=i386", "-S", "-o", "z.s", "z.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("z.s")).unwrap();
    assert!(text.contains("rep stos"), "{text}");
    assert!(text.matches("mov dword ptr [bp").count() <= 4, "{text}");
}

/// A six-statement callee with three callers was copied three times by 225-threshold
/// inlining three copies at -O2 (jpeg_single.nib: one 2439-instruction
/// `_main`, 11693 bytes against main's 6016); m16's bar is 50.
#[test]
fn at_o2_a_mid_sized_callee_with_several_callers_stays_a_call() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    let mut body = String::new();
    for i in 0..6 {
        body.push_str(&format!("    a[{i}] = a[{i}] * {} + b[{i}] - ({i} ^ n);\n", i + 3));
    }
    let source = format!(
        "static void g(int *a, int *b, int n) {{\n{body}}}\nint p[16], q[16];\n\
         void f1(int n) {{ g(p, q, n); }}\nvoid f2(int n) {{ g(q, p, n + 1); }}\nvoid f3(int n) {{ g(p, p, n + 2); }}\n"
    );
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-O2", "-m16", "-march=i386", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    assert!(text.matches("call ").count() >= 3, "{text}");
}

/// With no `memset` in the runtime a constant zeroing is dword fills: 1024
/// bytes were `mov cx, 1024; rep stosb` (4 times the clocks of the old
/// selector's `mov cx, 256; rep stosd`; sieve).
#[test]
fn at_o2_a_1024_byte_zeroing_is_rep_stosd() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("z.c"),
        "extern void use(char *p);\nvoid f(void) { char b[1024]; __builtin_memset(b, 0, 1024); use(b); }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-O2", "-m16", "-march=i386", "-S", "-o", "z.s", "z.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("z.s")).unwrap();
    assert!(text.contains("rep stosd") && text.contains("mov cx, 256"), "{text}");
}
