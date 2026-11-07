//! -O0 can select with FastISel (`backend::selectiondag::fastisel`), off unless
//! `LLRM_FAST_ISEL=1`.

use std::process::Command;

const SOURCE: &str = "extern int printf(const char *, ...);\n\
    int five(void) { return 5; }\n\
    int main(void)\n{\n    printf(\"%d\\n\", five());\n    return 0;\n}\n";

/// `SOURCE` compiled for `target` at `level`: (object bytes, ISEL report).
fn built(
    target: &str,
    level: &str,
    fast: bool,
) -> (Vec<u8>, String, std::process::Output) {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-cc"));
    command
        .env("LLRM_CC_TARGET", target)
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args([level, "-c", "a.c", "-o", "a.o"]);
    if fast {
        command.env("LLRM_FAST_ISEL", "1");
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let object = std::fs::read(scratch.path().join("a.o")).unwrap();
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap_or_default();
    (object, report, output)
}

/// FastISel that takes nothing must leave the object the DAG path makes: a
/// side effect of trying (a frame object, a pool cell) would move bytes.
#[test]
fn fast_isel_on_main_alone_leaves_the_dag_object() {
    // m64 takes `main` since the SysV call and global-address selectors
    let (_, report, _) = built("x86_64-linux", "-O0", true);
    assert!(report.lines().any(|line| line.starts_with("main fast") || line.starts_with("_main fast")), "x86_64: {report}");
    let target = "x86_32-linux";
    let (dag, _, _) = built(target, "-O0", false);
    let (fast, report, _) = built(target, "-O0", true);
    let main_fast = report.lines().any(|line| line.starts_with("main fast") || line.starts_with("_main fast"));
    assert!(!main_fast, "{target}: main has a variadic call, FastISel must not take it: {report}");
    assert_eq!(dag, fast, "{target}: fallback changed the object");
}

/// `five` is `ret i32 5`: at -O0 with the flag it is selected by FastISel
/// (the report says `fast`); at -O1 the flag does nothing and the DAG path
/// selects it. Off by default.
#[test]
fn fast_isel_takes_a_constant_return_at_o0_and_not_at_o1() {
    for target in ["x86_64-linux", "x86_32-linux"] {
        let named = |report: &str, how: &str| {
            report.lines().any(|line| line == format!("five {how}") || line == format!("_five {how}"))
        };
        let (_, report, _) = built(target, "-O0", true);
        assert!(named(&report, "fast"), "{target}: -O0 with the flag: {report}");
        let (_, report, _) = built(target, "-O1", true);
        assert!(named(&report, "dag") && !report.contains(" fast"), "{target}: -O1 took FastISel: {report}");
        let (_, report, _) = built(target, "-O0", false);
        assert!(!report.contains(" fast"), "{target}: FastISel on by default: {report}");
    }
}

/// The FastISel `five` computes 5: the program prints it.
#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
#[test]
fn fast_isel_constant_return_computes_the_same_answer() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args(["-O0", "a.c", "-o", "a.out"])
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    assert!(std::fs::read_to_string(scratch.path().join("report")).unwrap().contains("five fast"), "premise: fast");
    let ran = Command::new(scratch.path().join("a.out")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout), "5\n");
}

const ARITH: &str = "extern int printf(const char *, ...);\n\
    int g(int a, int b)\n{\n    int s = a + b;\n    if (s < 10) s = s - 1;\n    return (s & 255) ^ 3;\n}\n\
    int main(void)\n{\n    printf(\"%d %d\\n\", g(2, 3), g(20, 7));\n    return 0;\n}\n";

/// m32 integer core: parameters, locals, add/sub/and/xor, a compare and a
/// branch go through FastISel (`g` is recorded `fast`). The run-corpus check
/// of the answer is `tools/cc_check.sh` with `LLRM_FAST_ISEL=1`.
#[test]
fn fast_isel_takes_the_m32_integer_core() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), ARITH).unwrap();
    let run = |fast: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-cc"));
        command
            .env("LLRM_CC_TARGET", "x86_32-linux")
            .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
            .env("LLRM_VERIFY", "1")
            .current_dir(scratch.path())
            .args(["-O0", "-S", "a.c", "-o", if fast { "fast.s" } else { "dag.s" }]);
        if fast {
            command.env("LLRM_FAST_ISEL", "1");
        }
        let out = command.output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    };
    run(false);
    run(true);
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    assert!(report.lines().any(|line| line == "g fast" || line == "_g fast"), "g not taken: {report}");
    let fast = std::fs::read_to_string(scratch.path().join("fast.s")).unwrap();
    assert!(fast.contains("cmp") && fast.contains("ret"), "{fast}");
}

const CALLS: &str = "extern int printf(const char *, ...);\n\
    static int inc(int x) { return x + 1; }\n\
    int h(int a, int b)\n{\n    return inc(a) / b;\n}\n\
    int main(void)\n{\n    printf(\"%d\\n\", h(9, 2));\n    return 0;\n}\n";

/// Slice 3: a call with an argument and a signed divide go through FastISel.
/// `h` calls `inc` and divides; until calls and `idiv` are selected the
/// function falls back to the DAG and this fails.
#[test]
fn fast_isel_takes_a_call_and_a_divide_at_m32() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), CALLS).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_VERIFY", "1")
        .env("LLRM_DEBUG", "isel")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "h not taken: {report}{}", String::from_utf8_lossy(&out.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.contains("idiv") && asm.contains("call"), "{asm}");
}

/// Slice 4: at m32 a pointer is an `i32`. `get` loads through a pointer
/// parameter and `put` stores through one; both were declined for the type.
#[test]
fn fast_isel_takes_pointers_at_m32() {
    let source = "int get(int *p) { return *p; }\n\
                  void put(int *p, int v) { *p = v + 1; }\n\
                  int main(void) { int x = 1; put(&x, 4); return get(&x) == 5 ? 0 : 1; }\n";
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_VERIFY", "1")
        .env("LLRM_DEBUG", "isel")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    for name in ["get", "put"] {
        assert!(
            report.lines().any(|line| line == format!("{name} fast") || line == format!("_{name} fast")),
            "{name} not taken: {report}{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Slice 5: an array walk at m32: a GEP with a variable index (`lea`), a loop
/// of compares and branches. `sum` was declined at its GetElementPtr.
#[test]
fn fast_isel_takes_an_array_walk_at_m32() {
    let source = "int sum(int *a, int n)\n{\n    int s = 0, i;\n    for (i = 0; i < n; i++) s += a[i];\n    return s;\n}\n\
                  int main(void) { int v[4] = {1, 2, 3, 4}; return sum(v, 4) == 10 ? 0 : 1; }\n";
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_VERIFY", "1")
        .env("LLRM_DEBUG", "isel")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    assert!(report.lines().any(|line| line == "sum fast" || line == "_sum fast"), "sum not taken: {report}{}", String::from_utf8_lossy(&out.stderr));
}

/// An unsigned divide and remainder left `h` to the DAG at m32 (FastISel took
/// only the signed ones), so `x % 17` became a magic multiply where the pin's
/// -O0 emits `div`.
#[test]
fn fast_isel_takes_an_unsigned_divide_and_remainder_at_m32() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "unsigned h(unsigned a, unsigned b)\n{\n    return a / b + a % 17;\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_VERIFY", "1")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "h not taken: {report}");
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.matches("div").count() >= 2 && !asm.contains("idiv"), "{asm}");
}

/// At m64 FastISel declined every function ("a parameter that is not a 4-byte
/// stack cell"): register parameters, frame addressing and `idiv` are taken.
#[test]
fn fast_isel_takes_the_integer_core_at_m64() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "int h(int a, int b)\n{\n    int c = a / b;\n    if (c > a)\n        return c - 1;\n    return c + a;\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_VERIFY", "1")
        .env("LLRM_DEBUG", "isel")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "h not taken: {report}{}", String::from_utf8_lossy(&out.stderr));
}

/// A multiply left every function that had one to the DAG ("fast declined at
/// Binary(Mul)"), so `array.c`'s `i * i % 17` never reached FastISel.
#[test]
fn fast_isel_takes_a_multiply_at_m32_and_m64() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), "int h(int a, int b)\n{\n    return a * b + a;\n}\n").unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_FAST_ISEL", "1")
            .env("LLRM_VERIFY", "1")
            .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
            .current_dir(scratch.path())
            .args(["-O0", "-S", "a.c", "-o", "a.s"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
        assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "{target}: h not taken: {report}");
    }
}

/// At m64 a call left its caller to the DAG; `urem_eq.c` and `array.c` end in `printf`. Fixed and variadic callee, integer arguments.
#[test]
fn fast_isel_takes_a_call_at_m64() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "int inc(int a);\nint many(int a, ...);\nint h(int a)\n{\n    many(a, a);\n    return inc(a + 1);\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_VERIFY", "1")
        .env("LLRM_DEBUG", "isel")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "h not taken: {report}{}", String::from_utf8_lossy(&out.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.contains("call") && asm.contains("mov al, 0"), "{asm}");
}

/// At m64 a global array element (`sext` index, `gep`, `lea` of the global)
/// left `h` to the DAG; `array.c` stores and loads exactly this.
#[test]
fn fast_isel_takes_a_global_array_element_at_m64() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "int a[64];\nint h(int i)\n{\n    a[i] = i;\n    return a[i];\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_VERIFY", "1")
        .env("LLRM_DEBUG", "isel")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "h not taken: {report}{}", String::from_utf8_lossy(&out.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.contains("movsx"), "{asm}");
}

/// A compare whose result is a value (`x == c`, widened) sent every such
/// function to the DAG ("fast declined at ICmp"); `urem_eq.c` is made of them.
#[test]
fn fast_isel_takes_a_compare_as_a_value() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), "int h(unsigned x)\n{\n    return x % 5 == 0;\n}\n").unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_FAST_ISEL", "1")
            .env("LLRM_VERIFY", "1")
            .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
            .current_dir(scratch.path())
            .args(["-O0", "-S", "a.c", "-o", "a.s"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
        assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "{target}: h not taken: {report}");
        let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
        assert!(asm.contains("div") && !asm.contains("imul") && !asm.contains("ror"), "{target}: {asm}");
    }
}

/// At m64 an `i64` parameter, local, add, multiply, compare and return left
/// the function to the DAG ("a parameter that is not a 4-byte stack cell or
/// one register"); `array.c`'s `long s` is a store of one.
#[test]
fn fast_isel_takes_the_i64_core_at_m64() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "long h(long a, long b)\n{\n    long c = a + b;\n    if (c > a)\n        return c - 1;\n    return c * b + 7;\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_VERIFY", "1")
        .env("LLRM_DEBUG", "isel")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "h not taken: {report}{}", String::from_utf8_lossy(&out.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.contains("qword ptr [rbp-") && asm.contains("imul rax"), "{asm}");
}

/// At m64 `zext`/`trunc` between `i32` and `i64` and an `i64` divide left the
/// function to the DAG ("fast declined at Cast"); `urem_eq.c`'s `main` mixes them.
#[test]
fn fast_isel_takes_widening_and_i64_divide_at_m64() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "unsigned long h(unsigned a, unsigned long b)\n{\n    unsigned long w = a;\n    unsigned t = (unsigned)(b / w);\n    return t + b % 10;\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_VERIFY", "1")
        .env("LLRM_DEBUG", "isel")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "h not taken: {report}{}", String::from_utf8_lossy(&out.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.contains("div rcx") || asm.contains("div r"), "{asm}");
}
