//! `undef` reaches selection as the machine `IMPLICIT_DEF` (opcode 10), which
//! has no name in the pin's table: `ret i32 undef` was refused with "a machine
//! node with no name: 10", and so was every function that read an
//! uninitialised local or added `undef` (jpeg_fixed at -O2).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn compile(body: &str) -> String {
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\n{body}\n");
    assembly(&ir, cpu_of("68000").unwrap(), Level::O2).unwrap_or_else(|why| panic!("{why}\n{ir}"))
}

#[test]
fn a_function_returning_undef_compiles_at_every_width() {
    for ty in ["i8", "i16", "i32"] {
        let asm = compile(&format!("define {ty} @f() {{\n  ret {ty} undef\n}}"));
        assert!(asm.contains("rts"), "{ty}: {asm}");
    }
}

#[test]
fn a_read_of_an_uninitialised_local_compiles() {
    let asm = compile(
        "define i32 @f(i32 %x) {\n  %p = alloca i32\n  %v = load i32, ptr %p\n  %r = add i32 %x, %v\n  ret i32 %r\n}",
    );
    assert!(asm.contains("rts"), "{asm}");
}

/// value_of_valueless: `int nothing(int) {}` is `ret i32 undef`; the copy of it into `%d0` made `moveq #0,%d0`
/// (and a second move for an i64) where gcc and the pin's `EmitCopyToRegNode` leave the register alone: +6 B a
/// function, 6 cells against gcc.
#[test]
fn a_return_of_undef_is_a_bare_rts() {
    for level in [Level::O1, Level::O2, Level::Os] {
        for ty in ["i8", "i16", "i32", "i64"] {
            let ir = format!("target datalayout = \"{DATALAYOUT}\"\ndefine {ty} @f() {{\n  ret {ty} undef\n}}\n");
            let asm = assembly(&ir, cpu_of("68000").unwrap(), level).expect("compiles");
            let body: Vec<&str> =
                asm.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('.') && !line.ends_with(':')).collect();
            assert_eq!(body, ["rts"], "{ty} {level:?}:\n{asm}");
        }
    }
}
