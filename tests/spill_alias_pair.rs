//! Spill-side changes keep what a value was when it was loaded.

mod common;

use std::path::Path;
use std::process::Command;

/// Nine doubles loaded through a pointer stay live across something that
/// changes the memory they were loaded from, then all are used. A value
/// remade from its address after the change reads the new contents. `P(name)`
/// is the kernel's name in the compiled copy and `ref_name` in gcc's.
const KERNELS: &str = r#"/* Nine or more doubles loaded through a pointer stay live across something
   that changes the memory they were loaded from. A value remade from the
   address after the change reads the new contents: wrong. */
#ifndef P
#define P
#endif
extern void P(touch)(double *p);

#define SUM(a) ((a)[0] * (a)[1] + (a)[1] * (a)[2] + (a)[2] * (a)[3] + (a)[3] * (a)[4] + (a)[4] * (a)[5] + (a)[5] * (a)[6] + (a)[6] * (a)[7] + (a)[7] * (a)[8] + (a)[8] * (a)[0])

/* x0..x8 loaded, then a[3] changes, then every x is used. */
#define BODY(CHANGE) \
    double x0 = a[0], x1 = a[1], x2 = a[2], x3 = a[3], x4 = a[4]; \
    double x5 = a[5], x6 = a[6], x7 = a[7], x8 = a[8]; \
    CHANGE; \
    return x0 * x1 + x1 * x2 + x2 * x3 + x3 * x4 + x4 * x5 + x5 * x6 + x6 * x7 + x7 * x8 + x8 * x0 \
         + x0 * x4 + x1 * x5 + x2 * x6 + x3 * x7 + x4 * x8;

/* a store through the pointer itself */
double P(k_same)(double *a) { BODY(a[3] = 5.0) }
/* a store through another pointer argument, which the caller makes alias */
double P(k_other)(double *a, double *b) { BODY(b[3] = 5.0) }
/* a call that may write what a points to */
double P(k_call)(double *a) { BODY(P(touch)(a)) }
/* no change at all: the loads may be remade */
double P(k_none)(double *a) { BODY((void)0) }
"#;

/// The driver runs each kernel from both compilers on the same data and
/// prints `GREEN n` when n kernels agree to 1e-9 (x87 precision differs from
/// gcc -O0's stores to memory in the last bits).
const DRIVER: &str = r#"#include <stdio.h>
#include <math.h>
static int differ(double a, double b) { return fabs(a - b) > 1e-9 * fabs(b); }
double k_same(double *), k_other(double *, double *), k_call(double *), k_none(double *);
double ref_k_same(double *), ref_k_other(double *, double *), ref_k_call(double *), ref_k_none(double *);
void touch(double *p) { p[3] = 5.0; }
void ref_touch(double *p) { p[3] = 5.0; }
int main(void)
{
    double a[9], r[9];
    int bad = 0, i, compared = 0;
    for (i = 0; i < 9; i++) a[i] = r[i] = 1.0 + i * 0.37;
    { compared++; double x = k_same(a), y = ref_k_same(r); if (differ(x, y)) { printf("%.17g vs %.17g\n", x, y); bad = 1; } }
    for (i = 0; i < 9; i++) a[i] = r[i] = 1.0 + i * 0.37;
    { compared++; double x = k_other(a, a), y = ref_k_other(r, r); if (differ(x, y)) { printf("%.17g vs %.17g\n", x, y); bad = 1; } }
    for (i = 0; i < 9; i++) a[i] = r[i] = 1.0 + i * 0.37;
    { compared++; double x = k_call(a), y = ref_k_call(r); if (differ(x, y)) { printf("%.17g vs %.17g\n", x, y); bad = 1; } }
    for (i = 0; i < 9; i++) a[i] = r[i] = 1.0 + i * 0.37;
    { compared++; double x = k_none(a), y = ref_k_none(r); if (differ(x, y)) { printf("%.17g vs %.17g\n", x, y); bad = 1; } }
    printf("%s %d\n", bad ? "RED" : "GREEN", compared);
    return bad;
}
"#;

fn run(
    directory: &Path,
    program: &str,
    arguments: &[&str],
) -> std::process::Output {
    Command::new(program).current_dir(directory).args(arguments).output().unwrap()
}

/// A store through the pointer itself, through another pointer argument the
/// caller makes alias it, a call that writes it, and no change at all: the
/// value is what the load read, at every level. Needs `gcc -m32` and a 32-bit
/// libc; a host without one skips, loudly. The pair was written for the
/// rematerialisation of loads (x87 and integer) and is the correctness test of
/// any change to what a spill reloads or remakes.
#[test]
fn test_a_value_loaded_before_a_store_is_not_read_again_after_it() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("probe.c"), "int main(void){return 0;}").unwrap();
    let probe = run(directory, "gcc", &["-m32", "-no-pie", "probe.c", "-o", "probe"]);
    if !probe.status.success() || !run(directory, "./probe", &[]).status.success() {
        eprintln!("skipped: no gcc -m32 that links and runs here");
        return;
    }
    std::fs::write(directory.join("own.c"), regex_free(KERNELS, "")).unwrap();
    std::fs::write(directory.join("refk.c"), regex_free(KERNELS, "ref_")).unwrap();
    std::fs::write(directory.join("main.c"), DRIVER).unwrap();
    let reference = run(directory, "gcc", &["-m32", "-O0", "-c", "refk.c", "-o", "ref.o"]);
    assert!(reference.status.success(), "{}", String::from_utf8_lossy(&reference.stderr));
    for level in ["-O1", "-O2", "-O3", "-Os"] {
        let made = common::cc::llrm_cc()
            .current_dir(directory)
            .args(["-m32", "-mabi=sysv", "-march=i486", level, "-fobject-format=elf", "-o", "own.o", "own.c"])
            .output()
            .unwrap();
        assert!(made.status.success(), "{level}: {}", String::from_utf8_lossy(&made.stderr));
        let linked = run(directory, "gcc", &["-m32", "-no-pie", "main.c", "own.o", "ref.o", "-lm", "-o", "pair"]);
        assert!(linked.status.success(), "{level}: {}", String::from_utf8_lossy(&linked.stderr));
        let ran = run(directory, "./pair", &[]);
        assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "GREEN 4", "{level}");
    }
}

/// `kernels` with every `P(name)` written as `prefix` + `name`.
fn regex_free(
    kernels: &str,
    prefix: &str,
) -> String {
    let mut out = String::new();
    let mut rest = kernels;
    while let Some(at) = rest.find("P(") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        let close = after.find(')').unwrap();
        out.push_str(prefix);
        out.push_str(&after[..close]);
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}
