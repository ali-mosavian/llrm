//! An indirect call through a table reads the pointer in the call: gcc and
//! clang emit `call [table+edx*4]`, one instruction where we made `mov ecx,
//! [table+ecx*4]` and `call ecx`.

use std::process::Command;

/// bench/x_funcptr: four functions called through a table; at -Os it was 130
/// bytes and 25,519 instructions against clang's 123 and 18,018.
const FUNCPTR: &str = "static int f0(int x) { return x + 3; } static int f1(int x) { return x * 5; }\n\
static int f2(int x) { return x ^ 0x55; } static int f3(int x) { return x >> 1; }\n\
int (*table[4])(int) = { f0, f1, f2, f3 };\n\
long f(int n)\n{\n    int i, x = 1; long sum = 0;\n\
    for (i = 0; i < n; ++i) { x = table[(i * 7 + (x & 3)) & 3](x) & 0xFFFF; sum += x; }\n    return sum;\n}\n";

fn listing(level: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), FUNCPTR).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", "-mabi=sysv", "-march=i486", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

#[test]
fn a_call_through_a_table_reads_the_pointer_in_the_call() {
    for level in ["-O1", "-O2", "-Os"] {
        let text = listing(level);
        assert!(text.contains("call dword ptr _table["), "{level}: the pointer was loaded apart:\n{text}");
    }
}
