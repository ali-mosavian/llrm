//! A procedure only its own module calls is entered by a near call in every
//! frontend: `call` and `ret`, not `call far ptr` and `retf`. Nib's were all
//! far, whatever called them.

use std::path::Path;
use std::process::Command;

pub mod common;

fn compiled(
    tool: &str,
    name: &str,
    source: &str,
    arguments: &[&str],
) -> String {
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let (input, out) = (scratch.path().join(name), scratch.path().join("out.asm"));
    std::fs::write(&input, source).unwrap();
    // llrm-c is gone: its command line is served by llrm-cc over the pinned
    // clang.
    let done = if tool == "llrm-c" {
        common::cc::llrm_c().arg(&input).args(arguments).args(["-O2", "-S", "-o"]).arg(&out).output().unwrap()
    } else {
        Command::new(bin.join(tool)).arg(&input).args(arguments).args(["-O2", "-S", "-o"]).arg(&out).output().unwrap()
    };
    assert!(done.status.success(), "{tool}: {}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

fn procedure<'a>(
    asm: &'a str,
    name: &str,
) -> &'a str {
    let from = asm.find(&format!("{name} proc")).unwrap_or_else(|| panic!("{name} in {asm}"));
    asm[from..].split(&format!("{name} endp")).next().unwrap()
}

/// `down` is recursive, so it stays a call; `up` is what the module exports.
fn near_down(
    asm: &str,
    down: &str,
    up: &str,
) {
    let body = procedure(asm, down);
    assert!(body.starts_with(&format!("{down} proc near")), "{asm}");
    assert!(body.contains(&format!("call {down}\n")) && !body.contains("retf") && body.contains("ret"), "{asm}");
    assert!(procedure(asm, up).starts_with(&format!("{up} proc far")), "{asm}");
}

#[test]
fn test_c_enters_a_static_function_near() {
    let asm = compiled(
        "llrm-c",
        "near.c",
        "static int down(int n) { return n ? n - down(n - 1) : 0; }\nint up(int n) { return down(n); }\n",
        &[],
    );
    near_down(&asm, &common::symbol("down"), &common::symbol("up"));
}

#[test]
fn test_nib_enters_an_unexported_function_near() {
    let source = "fn down(n: i16) -> i16:\n    if n == 0:\n        return 0\n    return n - down(n - 1)\n\n@export(\"cdecl16\")\nfn up(n: i16) -> i16:\n    return down(n)\n\nfn main() -> i16:\n    print(up(3))\n    return 0\n";
    near_down(&compiled("llrm-nib", "near.nib", source, &[]), "_down", "_up");
}

#[test]
fn test_basic_enters_a_module_internal_procedure_near() {
    let source = "DECLARE FUNCTION Down% (n AS INTEGER)\nn% = INT(RND * 5)\nPRINT Down%(n%)\nFUNCTION Down% (n AS INTEGER)\n  IF n = 0 THEN Down% = 0 ELSE Down% = Down%(n - 1) + n\nEND FUNCTION\n";
    let asm = compiled(
        "llrm-qb",
        "near.bas",
        source,
        &["--own-frames", "--whole-program", "--dialect", "pds71", "--runtime", "pds71"],
    );
    let body = procedure(&asm, "DOWN");
    // The recursion is a closed form now (the pin's tail-recursion
    // elimination and IndVars), so the call that enters it near is main's.
    // The argument is not a constant: `Down%(3)` folds to 6 and no procedure
    // is left to enter.
    assert!(body.starts_with("DOWN proc near") && !body.contains("retf"), "{asm}");
    assert!(asm.contains("call DOWN\n"), "{asm}");
}

/// A function only direct calls reach takes the fields it reads in place of the
/// pointer it read them through: a struct's field in C, a slice's data pointer
/// and length in Nib. The callee loads nothing through the pointer, and its
/// callers pass the fields.
#[test]
fn test_c_passes_the_field_a_static_function_reads_through_its_struct_pointer() {
    let source = "struct P { int *data; int len; };\nstatic int sum(const struct P *p, int n) { int len = p->len; return n ? len - sum(p, n - 1) : len; }\nint up(struct P *p) { return sum(p, 3); }\n";
    let asm = compiled("llrm-c", "promote.c", source, &[]);
    let body = procedure(&asm, &common::symbol("sum"));
    // The fields arrive in registers: nothing is read through the pointer or
    // from the stack.
    assert!(!body.contains("ptr [bx+") && !body.contains("ptr [si+") && !body.contains("[bp+"), "{asm}");
    assert!(procedure(&asm, &common::symbol("up")).contains("[bx+2]"), "{asm}");
}

#[test]
fn test_nib_passes_the_slice_fields_an_unexported_function_reads() {
    let source = "fn total(a: &[i16], i: i16) -> i16:\n    if i == 0:\n        return a[0]\n    return total(a, i - 1) + a[i]\n\nfn main() -> i16:\n    let mut a: i16[8] = [1] * 8\n    a[3] = 5\n    print(total(a, 3))\n    return 0\n";
    let asm =
        compiled("llrm-nib", "promote.nib", source, &["-fno-inline-functions", "-fno-inline-functions-called-once"]);
    let body = procedure(&asm, "_total");
    // The descriptor's words were read through the pointer: its length at +0,
    // its data pointer at +4.
    assert!(!body.contains("es:[bx+4]") && !body.contains("[bx+4]"), "{asm}");
}

/// Queens' `safe(q, row, col)` reads `q[r]` for `r < row` under a bounds check
/// on the slice's length, 12, which reaches it as a field of the descriptor.
/// DEVIATE SD443 (the extra IPSCCP after `promote.`, interprocedural.rs; owner w2,
/// fix-by 2026-11-07): the checks
/// are folded. The pin keeps them: its ArgumentPromotion refuses a recursive
/// function's descriptor that holds a pointer (ArgumentPromotion.cpp#L581 at
/// d1106deb71cc), so `opt -O2` keeps the descriptor argument and all three
/// `call @N$EBND` (one in `safe`, two in `place`). Ours promotes the length,
/// the extra IPSCCP folds it (queens -O2: 225467 to 182177 instructions).
/// Exit: by-value slice parameters (#6140) remove the descriptor, then #6159
/// lands and this returns to the pin shape: the three calls.
#[test]
fn test_nib_queens_folds_its_bounds_checks_where_the_pin_keeps_them() {
    let asm = compiled(
        "llrm-nib",
        "queens.nib",
        include_str!("../bench/queens/queens.nib"),
        &["-fno-inline-functions", "-fno-inline-functions-called-once"],
    );
    assert!(!asm.contains("N$EBND"), "{asm}");
}

/// A slice of a module array is a far pointer to DGROUP: pushed as segment and
/// offset and read back with `les`. Every call fills it from one global, so the
/// callee takes the offset and reads through DS.
#[test]
fn test_nib_passes_a_global_array_slice_as_its_offset() {
    let source = "var table: i16[16] = [0] * 16\n\nfn total(a: &[i16], i: i16) -> i16:\n    if i == 0:\n        return a[0]\n    return total(a, i - 1) + a[i]\n\nfn main() -> i16:\n    table[3] = 5\n    print(total(table, 3))\n    return 0\n";
    let asm =
        compiled("llrm-nib", "table.nib", source, &["-fno-inline-functions", "-fno-inline-functions-called-once"]);
    let body = procedure(&asm, "_total");
    assert!(!body.contains("les ") && !body.contains("es:[") && !body.contains("[bp+"), "{asm}");
}

/// Queens' `q` is a local array, so its far pointer is SS:offset: a callee
/// given only that reads it through `ss:`, one word on the stack, where `les`
/// loaded a segment for it at every call.
#[test]
fn test_nib_reads_a_local_array_slice_through_ss() {
    let asm = compiled(
        "llrm-nib",
        "queens.nib",
        include_str!("../bench/queens/queens.nib"),
        &["-fno-inline-functions", "-fno-inline-functions-called-once"],
    );
    let body = procedure(&asm, "_safe");
    assert!(body.contains("ss:[") && !body.contains("les "), "{asm}");
}

/// The exit of the DEVIATE above: the pin's shape, the checks stay. Run it with
/// `--ignored`; #6159 turns it on and the test above goes. (#6159's build has
/// two calls; the three the pin's `opt -O2` keeps were counted in a0ed5eb43,
/// not re-measured.)
#[test]
#[ignore = "the pin shape: on when #6159 lands (exit of the extra IPSCCP DEVIATE)"]
fn test_nib_queens_keeps_its_bounds_checks_as_the_pin_does() {
    let asm = compiled(
        "llrm-nib",
        "queens.nib",
        include_str!("../bench/queens/queens.nib"),
        &["-fno-inline-functions", "-fno-inline-functions-called-once"],
    );
    assert!(asm.contains("call far ptr N$EBND"), "{asm}");
}
