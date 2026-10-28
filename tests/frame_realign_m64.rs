//! A local over-aligned past the stack's 16 bytes is aligned at run time: the
//! frame ands the stack pointer (`and rsp, -64`) and addresses its locals
//! through it, its arguments through the frame register.

use std::process::Command;

const OBJECT: &str = "extern int use(int *);\n\
    int aligned(int n) { _Alignas(64) int v[4] = {n, 1, 2, 3}; return use(v); }\n\
    int aligned7(int a, int b, int c, int d, int e, int f, int g) {\n\
        _Alignas(64) int v[4] = {a, b, c, d}; return use(v) + (e + f + g) * 64; }\n";

const DRIVER: &str = "#include <stdint.h>\n\
    #include <stdio.h>\n\
    int use(int *p) { return (int)((uintptr_t)p & 63); }\n\
    int aligned(int);\n\
    int aligned7(int, int, int, int, int, int, int);\n\
    __attribute__((noinline)) int at(int k) {\n\
        volatile char *pad = __builtin_alloca(k * 16 + 16);\n\
        pad[0] = 0;\n\
        return aligned(k) + 1000 * aligned7(1, 2, 3, 4, 5, 6, 7 + k) - 64000 * (18 + k);\n\
    }\n\
    int main(void) {\n\
        for (int k = 0; k < 4; k++) printf(\"%d\\n\", at(k));\n\
        return 0;\n\
    }\n";

/// llrm-cc kept the stack alignment of 16 for a `_Alignas(64)` local: `sub rsp,
/// 24; lea rdi, [rsp]` addressed it wherever the stack happened to be, 3 calls
/// in 4 of different depth. Each call prints its misalignment (0) and the
/// seven arguments' arithmetic, read after the realignment (0).
#[test]
fn test_an_over_aligned_local_is_aligned_at_run_time() {
    let clang = std::env::var_os("LLRM_PIN_CLANG").expect("LLRM_PIN_CLANG names the pinned clang");
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    std::fs::write(dir.join("a.c"), OBJECT).unwrap();
    std::fs::write(dir.join("main.c"), DRIVER).unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(dir)
        .args(["-m64", "-O1", "-fno-builtin", "-c", "-o", "a.o", "a.c"])
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let linked = Command::new(&clang).current_dir(dir).args(["-O1", "-o", "run", "main.c", "a.o"]).output().unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    let ran = Command::new(dir.join("run")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout), "0\n0\n0\n0\n");
}
