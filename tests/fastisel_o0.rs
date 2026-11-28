//! -O0 selects with FastISel (`backend::selectiondag::fastisel`) unless
//! `LLRM_FAST_ISEL=0`.

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
    command.env("LLRM_FAST_ISEL", if fast { "1" } else { "0" });
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
    assert!(
        report.lines().any(|line| line.starts_with("main fast") || line.starts_with("_main fast")),
        "x86_64: {report}"
    );
    let target = "x86_32-linux";
    let (dag, _, _) = built(target, "-O0", false);
    let (fast, report, _) = built(target, "-O0", true);
    let main_fast = report.lines().any(|line| line.starts_with("main fast") || line.starts_with("_main fast"));
    assert!(!main_fast, "{target}: main has a variadic call, FastISel must not take it: {report}");
    assert_eq!(dag, fast, "{target}: fallback changed the object");
}

/// `five` is `ret i32 5`: at -O0 with the flag it is selected by FastISel
/// (the report says `fast`); at -O1 the flag does nothing and the DAG path
/// selects it. `LLRM_FAST_ISEL=0` turns it off at -O0.
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
        assert!(!report.contains(" fast"), "{target}: FastISel on with LLRM_FAST_ISEL=0: {report}");
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
    assert!(
        report.lines().any(|line| line == "h fast" || line == "_h fast"),
        "h not taken: {report}{}",
        String::from_utf8_lossy(&out.stderr)
    );
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
    assert!(
        report.lines().any(|line| line == "sum fast" || line == "_sum fast"),
        "sum not taken: {report}{}",
        String::from_utf8_lossy(&out.stderr)
    );
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
    assert!(
        report.lines().any(|line| line == "h fast" || line == "_h fast"),
        "h not taken: {report}{}",
        String::from_utf8_lossy(&out.stderr)
    );
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

/// At m64 a call left its caller to the DAG; `urem_eq.c` and `array.c` end in
/// `printf`. Fixed and variadic callee, integer arguments.
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
    assert!(
        report.lines().any(|line| line == "h fast" || line == "_h fast"),
        "h not taken: {report}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.contains("call") && asm.contains("mov al, 0"), "{asm}");
}

/// At m64 a global array element (`sext` index, `gep`, `lea` of the global)
/// left `h` to the DAG; `array.c` stores and loads exactly this.
#[test]
fn fast_isel_takes_a_global_array_element_at_m64() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), "int a[64];\nint h(int i)\n{\n    a[i] = i;\n    return a[i];\n}\n")
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
    assert!(
        report.lines().any(|line| line == "h fast" || line == "_h fast"),
        "h not taken: {report}{}",
        String::from_utf8_lossy(&out.stderr)
    );
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
    assert!(
        report.lines().any(|line| line == "h fast" || line == "_h fast"),
        "h not taken: {report}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.contains("qword ptr [rbp-") && asm.contains("imul rax"), "{asm}");
}

/// At m64 `zext`/`trunc` between `i32` and `i64` and an `i64` divide left the
/// function to the DAG ("fast declined at Cast"); `urem_eq.c`'s `main` mixes
/// them.
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
    assert!(
        report.lines().any(|line| line == "h fast" || line == "_h fast"),
        "h not taken: {report}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.contains("div rcx") || asm.contains("div r"), "{asm}");
}

/// An `i64` argument or result left a caller to the DAG ("fast declined at
/// Call"): `main` of `urem_eq.c` and `array.c` calls `printf("%ld")` and
/// `report(long)`.
#[test]
fn fast_isel_takes_i64_call_arguments_and_results_at_m64() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "long next(long a);\nvoid report(long v);\nlong h(long a)\n{\n    report(a);\n    return next(a + 1);\n}\n",
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
    assert!(
        report.lines().any(|line| line == "h fast" || line == "_h fast"),
        "h not taken: {report}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.contains("mov rdi, rax") || asm.contains("mov rdi,"), "{asm}");
}

/// A multiply by a constant put the constant in a register (`mov eax, 3;
/// imul eax, ecx`), 5 bytes dearer than the pin's `imul r, r, imm` or `shl`
/// (`llrm_mixed`, `llrm_six` of `wc_mixed.c`).
#[test]
fn fast_isel_multiplies_by_a_constant_with_an_immediate() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), "int h(int a)\n{\n    return a * 3 + a * 8;\n}\n").unwrap();
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
        assert!(
            asm.contains("shl") && asm.lines().any(|line| line.contains("imul") && line.trim_end().ends_with(", 3")),
            "{target}: {asm}"
        );
    }
}

/// A one-use load right before its user was loaded into a register first
/// (`mov rax, [b]; mov rcx, [b]; add rcx, rax`), where the pin folds it
/// (`mov rax, [b]; add rax, [b]`): `llrm_wide` of `wc_mixed.c`, 49 B against
/// 32.
#[test]
fn fast_isel_folds_a_load_into_the_instruction_after_it() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), "int h(int a, int b)\n{\n    return a + b - a * b;\n}\n").unwrap();
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
        let folded = |op: &str| asm.lines().any(|line| line.trim_start().starts_with(op) && line.contains("ptr ["));
        assert!(folded("add") && folded("imul"), "{target}: {asm}");
    }
}

/// A `volatile` load or store left its function to the DAG ("fast declined at
/// Store { volatile: true }"): the pin selects them like any other, only never
/// folding the load. 14 of `div_constants_long.c`'s functions, +458 B over
/// clang.
#[test]
fn fast_isel_takes_volatile_accesses_and_never_folds_a_volatile_load() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), "int h(int a)\n{\n    volatile int v = a;\n    return a + v;\n}\n")
            .unwrap();
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
        assert!(
            !asm.lines().any(|line| line.trim_start().starts_with("add") && line.contains("ptr [")),
            "{target}: a volatile load was folded: {asm}"
        );
    }
}

/// FastISel called `llvm.usub.sat.i64` like a function: the object named a
/// symbol that does not exist and `assemble.rs` panicked ("no entry found for
/// key") on `sat_builtins.c` and `sat_sub_u64_shared_compare.c`. An intrinsic
/// call is the DAG's (F3).
#[test]
fn fast_isel_leaves_an_intrinsic_call_to_the_dag() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(
            scratch.path().join("a.c"),
            "typedef unsigned long long u64;\nu64 h(u64 a, u64 b)\n{\n    return __builtin_elementwise_sub_sat(a, b);\n}\n",
        )
        .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_FAST_ISEL", "1")
            .env("LLRM_VERIFY", "1")
            .current_dir(scratch.path())
            .args(["-O0", "-c", "a.c", "-o", "a.o"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{target}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

/// A shift by a constant left its function to the DAG ("fast declined at
/// Binary(Shl)"): `next` of `div_constants_long.c`, the shifts of
/// `mulconst_i64.c`.
#[test]
fn fast_isel_takes_shifts_by_a_constant() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(
            scratch.path().join("a.c"),
            "int h(int a, unsigned b)\n{\n    return (a << 3) + (a >> 2) + (int)(b >> 5);\n}\n",
        )
        .unwrap();
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
        assert!(asm.contains("shl") && asm.contains("sar") && asm.contains("shr"), "{target}: {asm}");
    }
}

/// A 64-bit constant past `imm32` left its function to the DAG ("fast declined
/// at Binary(Mul)", the store of a wide immediate): `mulconst_i64.c` is 19 such
/// functions, +487 B over clang. The pin makes it with `movabs`.
#[test]
fn fast_isel_materializes_a_wide_constant_with_movabs() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), "long h(long a)\n{\n    return a * 0x123456789abcL + 5;\n}\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
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
    assert!(asm.contains("20015998343868") || asm.to_uppercase().contains("123456789ABC"), "{asm}");
}

/// FastISel selects `llvm.lifetime.start`/`end` to nothing at -O0
/// (`FastISel::selectIntrinsicCall`). Ours declined at the call and sent the
/// whole function to the DAG path: 13 of 14 functions of div_constants_short,
/// +62 M Ir with the flag on.
#[test]
fn fast_isel_takes_a_function_with_lifetime_markers() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.ll"),
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
         target triple = \"x86_64-unknown-linux-gnu\"\n\
         declare void @llvm.lifetime.start.p0(ptr)\n\
         declare void @llvm.lifetime.end.p0(ptr)\n\
         define i32 @marked(i32 %a) {\n  %s = alloca i32\n  call void @llvm.lifetime.start.p0(ptr %s)\n  \
         store i32 %a, ptr %s\n  %v = load i32, ptr %s\n  call void @llvm.lifetime.end.p0(ptr %s)\n  ret i32 %v\n}\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .current_dir(scratch.path())
        .args(["-m64", "-O0", "-S", "a.ll", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap_or_default();
    assert!(report.lines().any(|line| line == "marked fast"), "{report}");
}

/// A `memset` or `memcpy` of an aggregate left its function to the DAG (126 of
/// 494 functions of the tests/run/c corpus declined first at one): the pin
/// makes them library calls.
#[test]
fn fast_isel_takes_memset_and_memcpy_as_library_calls() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(
            scratch.path().join("a.c"),
            "struct big { int a[40]; };\nint h(struct big *p, struct big *q)\n{\n    struct big local;\n    __builtin_memset(&local, 0, sizeof local);\n    *p = local;\n    __builtin_memcpy(q, p, sizeof *q);\n    return 0;\n}\n",
        )
        .unwrap();
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
        assert!(asm.contains("memset") && asm.contains("memcpy"), "{target}: {asm}");
    }
}

/// Every function looked its opcodes up by name again (about 4 M instructions
/// each): on the 225 functions of `memory_expansion` a FastISel compile cost
/// 1709 M instructions, the DAG's 839 M.
#[test]
fn fast_isel_looks_its_opcodes_up_once_per_compile() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "int a(int x) { return x + 1; }\nint b(int x) { return x + 2; }\nint c(int x) { return x + 3; }\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_FAST_ISEL", "1")
        .env("LLRM_DEBUG", "isel")
        .current_dir(scratch.path())
        .args(["-O0", "-c", "a.c", "-o", "a.o"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let log = String::from_utf8_lossy(&out.stderr);
    assert_eq!(log.matches("fast names built").count(), 1, "{log}");
}

/// A struct passed by value was passed as the pointer to it: `variadic.c` ran
/// with -O0 and FastISel printed -1880545068 where the DAG printed 567. The
/// caller is left to the DAG.
#[test]
fn fast_isel_leaves_a_struct_argument_to_the_dag() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(
            scratch.path().join("a.c"),
            "struct big { int i[6]; };\nextern int take(struct big pa, int pb);\nint h(struct big *p)\n{\n    return take(*p, 1);\n}\n",
        )
        .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_FAST_ISEL", "1")
            .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
            .current_dir(scratch.path())
            .args(["-O0", "-S", "a.c", "-o", "a.s"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
        assert!(report.lines().any(|line| line == "h dag" || line == "_h dag"), "{target}: h was taken: {report}");
    }
}

/// One block FastISel cannot take sent the whole function to the DAG (322 of
/// 494 functions of the tests/run/c corpus, 745 of 3041 blocks): now only that
/// block goes, and a phi and a value read in another block cross through
/// registers.
#[test]
fn fast_isel_takes_the_blocks_it_can_of_a_function() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(
            scratch.path().join("a.c"),
            "int h(int a, int b, char *s)\n{\n    int r = a ? b + 1 : b + 2;\n    if (r > 3)\n        r += *s;\n    return r;\n}\n",
        )
        .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_FAST_ISEL", "1")
            .env("LLRM_VERIFY", "1")
            .env("LLRM_DEBUG", "isel")
            .current_dir(scratch.path())
            .args(["-O0", "-S", "a.c", "-o", "a.s"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let log = String::from_utf8_lossy(&out.stderr);
        assert!(log.contains("fast took") && !log.contains("fast took 0"), "{target}: no block taken: {log}");
    }
}

/// A block that starts with a phi was declined ("a value another block reads
/// that is not a register value: Phi"): 241 blocks of the tests/run/c corpus,
/// `h` took 3 of its 4 blocks.
#[test]
fn fast_isel_takes_the_block_a_phi_starts() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), "int h(int a, int b)\n{\n    return a ? b + 1 : b + 2;\n}\n")
            .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_FAST_ISEL", "1")
            .env("LLRM_VERIFY", "1")
            .env("LLRM_DEBUG", "isel")
            .current_dir(scratch.path())
            .args(["-O0", "-S", "a.c", "-o", "a.s"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let log = String::from_utf8_lossy(&out.stderr);
        assert!(log.contains("fast took 4 of 4 blocks"), "{target}: {log}");
    }
}

/// `char` and `short` values (loads, stores, extensions, truncations,
/// parameters) sent their function to the DAG: 145 + 145 loads, 86 truncs and
/// 57 parameters of the tests/run/c corpus.
#[test]
fn fast_isel_takes_char_and_short_values() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(
            scratch.path().join("a.c"),
            "int h(short a, unsigned char c, char *p)\n{\n    short t = a;\n    *p = (char)t;\n    return t + *p + c + (unsigned short)a;\n}\n",
        )
        .unwrap();
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

/// The pin folds a GEP into the access that uses it and a load into the compare
/// after it (`X86SelectAddress`, `tryToFoldLoad`); FastISel emitted `lea` +
/// load and `mov` + `cmp` (`div_constants_short` `.text` 10569 B against the
/// pin's 9576 B).
#[test]
fn fast_isel_folds_an_address_and_a_load_into_the_access() {
    for (target, base) in [("x86_32-linux", "ebp"), ("x86_64-linux", "rbp")] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(
            scratch.path().join("a.c"),
            "int h(int n)\n{\n    int a[8];\n    for (int i = 0; i < 8; i++)\n        a[i] = i * n;\n    return a[3];\n}\n",
        )
        .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_FAST_ISEL", "1")
            .env("LLRM_VERIFY", "1")
            .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
            .current_dir(scratch.path())
            .args(["-O0", "-S", "-masm=intel", "a.c", "-o", "a.s"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
        assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "{target}: h not taken: {report}");
        let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
        assert!(
            asm.contains(&format!("cmp dword ptr [{base}-")) && asm.contains(", 8"),
            "{target}: no cmp on memory: {asm}"
        );
        assert!(asm.contains("*4-") && !asm.contains("lea "), "{target}: the element address was made apart: {asm}");
        // `n` is a local at m64, its own argument slot at m32 (argument copy
        // elision)
        let slot = |sign: &str| asm.contains(&format!("imul eax, dword ptr [{base}{sign}"));
        assert!(slot("-") || slot("+"), "{target}: the load was not folded into the imul: {asm}");
    }
}

/// A `short` read and widened was a `mov ax, [m]` and a `movsx` (6 B); the pin
/// folds the load: `movsx eax, word ptr [m]`.
#[test]
fn fast_isel_folds_a_narrow_load_into_its_extension() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), "int h(short *p, unsigned char *q)\n{\n    return *p + *q;\n}\n")
            .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_FAST_ISEL", "1")
            .env("LLRM_VERIFY", "1")
            .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
            .current_dir(scratch.path())
            .args(["-O0", "-S", "-masm=intel", "a.c", "-o", "a.s"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
        assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "{target}: h not taken: {report}");
        let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
        assert!(asm.contains("movsx eax, word ptr [") || asm.contains("movsx ecx, word ptr ["), "{target}: {asm}");
        assert!(asm.contains("movzx eax, byte ptr [") || asm.contains("movzx ecx, byte ptr ["), "{target}: {asm}");
    }
}

/// `0 - *p` read the operand into a register before the `sub` (the fold wanted
/// a register on the left): the pin folds the load whatever the left operand is
/// (`abs` of `builtins_table`, +11% `.text`).
#[test]
fn fast_isel_folds_a_load_whose_left_operand_is_a_constant() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), "int h(int *p)\n{\n    return 100 - *p;\n}\n").unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_FAST_ISEL", "1")
            .env("LLRM_VERIFY", "1")
            .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
            .current_dir(scratch.path())
            .args(["-O0", "-S", "-masm=intel", "a.c", "-o", "a.s"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
        assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "{target}: h not taken: {report}");
        let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
        assert!(asm.contains("sub eax, dword ptr ["), "{target}: {asm}");
    }
}

/// The pin's -O0 selects with FastISel; ours kept the DAG unless asked (`-O0`
/// compiles cost 3261 M instructions on the 61-program corpus against 3021 M).
/// With no environment variable `five` is selected by FastISel.
#[test]
fn fast_isel_is_the_default_at_o0() {
    for target in ["x86_64-linux", "x86_32-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env_remove("LLRM_FAST_ISEL")
            .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
            .current_dir(scratch.path())
            .args(["-O0", "-c", "a.c", "-o", "a.o"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
        assert!(report.lines().any(|line| line == "five fast" || line == "_five fast"), "{target}: {report}");
    }
}

/// `*p += x` read `*p` into a register and added: only a load on the right
/// folded. The pin commutes a commutative operation to fold a load on the left,
/// the one just before it (`foldMemoryOperand`): `add r, [m]`.
#[test]
fn fast_isel_folds_a_load_that_is_the_first_operand_of_a_commutative_operation() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), "void h(int *p, int x)\n{\n    *p += x;\n    *p &= x;\n}\n")
            .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_FAST_ISEL", "1")
            .env("LLRM_VERIFY", "1")
            .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
            .current_dir(scratch.path())
            .args(["-O0", "-S", "-masm=intel", "a.c", "-o", "a.s"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
        assert!(report.lines().any(|line| line == "h fast" || line == "_h fast"), "{target}: h not taken: {report}");
        let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
        for form in ["add e", "and e"] {
            assert!(
                asm.lines().any(|line| line.trim_start().starts_with(form) && line.contains("dword ptr [")),
                "{target}: no `{form}` on memory: {asm}"
            );
        }
    }
}

/// A zero was `mov eax, 0` at -O0 (5 B): the pin's `ExpandPostRAPseudos` makes
/// `xor eax, eax` (2 B) where the flags are dead, and ours ran no such pass at
/// -O0 (`Peephole` was left out whole): 7 `xor` in the pin's
/// `div_constants_short` against none.
#[test]
fn o0_makes_a_zero_with_xor_on_both_selectors() {
    for target in ["x86_32-linux", "x86_64-linux"] {
        for fast in ["0", "1"] {
            let scratch = tempfile::tempdir().unwrap();
            std::fs::write(scratch.path().join("a.c"), "int h(void)\n{\n    return 0;\n}\n").unwrap();
            let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
                .env("LLRM_CC_TARGET", target)
                .env("LLRM_FAST_ISEL", fast)
                .env("LLRM_VERIFY", "1")
                .current_dir(scratch.path())
                .args(["-O0", "-S", "-masm=intel", "a.c", "-o", "a.s"])
                .output()
                .unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
            assert!(asm.contains("xor eax, eax"), "{target} fast={fast}: {asm}");
            assert!(!asm.contains("mov eax, 0"), "{target} fast={fast}: {asm}");
        }
    }
}

/// The reason a block was declined was formatted for every instruction, the
/// `isel` channel on or off: 4.7 M of a 196 M compile. It is made only when the
/// channel is on, and says where the block was declined.
#[test]
fn the_decline_reason_is_made_for_the_isel_channel_and_names_the_instruction() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), "int h(int x)\n{\n    return (int)(x * 1.5);\n}\n").unwrap();
    let said = |channel: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-cc"));
        command
            .env("LLRM_CC_TARGET", "x86_32-linux")
            .current_dir(scratch.path())
            .args(["-O0", "-c", "a.c", "-o", "a.o"]);
        if let Some(channel) = channel {
            command.env("LLRM_DEBUG", channel);
        }
        let out = command.output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stderr).into_owned()
    };
    assert!(said(Some("isel")).contains("fast declined at Cast(SIToFP) double"));
    assert!(!said(None).contains("declined"));
}

/// FastISel's conditional branch ended in a `jmp` to the false block even when
/// that block comes next, and `ControlFlow` (the pin has no BranchFolder at
/// -O0) ran to remove it: 5.2 M of a 179 M compile. A function FastISel
/// selects whole has no jump to the next block, and no such phase.
#[test]
fn a_function_fastisel_selects_has_no_jump_to_the_next_block_and_no_control_flow_phase() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "int f(int x)\n{\n    int s = 0;\n    while (x > 0) {\n        if (x & 1)\n            s += x;\n        else\n            s -= 1;\n        x--;\n    }\n    return s;\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_DEBUG", "time")
        .env("LLRM_TIME_TOP", "100000")
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(!said.contains("lir jumps"), "{said}");
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = asm.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    for pair in lines.windows(2) {
        if let Some(target) = pair[0].strip_prefix("jmp ") {
            assert_ne!(pair[1], format!("{target}:"), "a jump to the next block:\n{asm}");
        }
    }
}

/// m32's default (watcall) passes parameters in registers. A function with one
/// fell back to the DAG whole (340 of 589 declines, two thirds of the fallback
/// instructions, over the m32 corpus on 2026-11-08).
#[test]
fn fast_isel_takes_register_parameters_at_m32() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "int g(int a, int b) { return a * b + a; }\nint pick(char *p, int i) { return p[i]; }\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .env("LLRM_VERIFY", "1")
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    for name in ["g_", "pick_"] {
        assert!(report.lines().any(|line| line == format!("{name} fast")), "{name} not taken: {report}");
    }
}

/// A load or store of a global at m32 fell back to the DAG: only m64 had an
/// address for one (`rip`). Over the m32 corpus on 2026-11-08 the loads and
/// stores were 2284 of 6040 fallback instructions.
#[test]
fn fast_isel_takes_global_accesses_at_m32() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "int g;\nint table[4];\nint f(int i) { int v = g; g = i; return v; }\nint at(int i) { return table[i] + table[2]; }\nint *where(void) { return &g; }\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_ISEL_REPORT", scratch.path().join("report"))
        .env("LLRM_VERIFY", "1")
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(scratch.path().join("report")).unwrap();
    for name in ["f", "at", "where"] {
        assert!(
            report.lines().any(|line| line == format!("{name} fast") || line == format!("_{name} fast")),
            "{name} not taken: {report}"
        );
    }
}

/// A parameter was stored into a local frame slot and read back (`urem_eq:r5z`,
/// +9 B and a `sub esp, 4` over the pin's llc): the pin's argument copy elision
/// makes the local the parameter's own stack slot. Both selectors, m32.
#[test]
fn a_parameter_stored_into_a_local_is_not_copied_at_m32() {
    for fast in ["0", "1"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(
            scratch.path().join("a.c"),
            "unsigned h(unsigned a)\n{\n    return a % 5 == 0;\n}\nint k(int a)\n{\n    a = a + 1;\n    return a * 2;\n}\n",
        )
        .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", "x86_32-linux")
            .env("LLRM_FAST_ISEL", fast)
            .env("LLRM_VERIFY", "1")
            .current_dir(scratch.path())
            .args(["-O0", "-S", "a.c", "-o", "a.s"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
        assert!(!asm.contains("ebp-"), "fast={fast}: a local slot is left:\n{asm}");
        assert!(!asm.contains("sub esp"), "fast={fast}: a frame is reserved:\n{asm}");
    }
}
