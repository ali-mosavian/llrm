//! A far call through a function pointer in a struct, reached by a register.

mod common;

const KERNEL: &str = "
typedef struct S { void (*a)(const struct S *, unsigned); void (*f)(const struct S *, unsigned, unsigned); } S;
void g(S *p, unsigned x) { p->f(p, x, 1); }
";

/// `farcall` folded the call to read the struct field itself, `call dword ptr
/// [bx+4]`; the allocator left the base in ax and the object was refused
/// ("_g@3: Semantics(op=CALL ..."), so every QB program failed to link
/// (runtime/qb/gline.c `fill->line`). An object is written, not a listing:
/// the listing printed the unencodable `[ax+4]` without complaint.
#[test]
fn test_a_call_through_a_struct_field_writes_an_object() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output =
        common::cc::llrm_cc().current_dir(scratch.path()).args(["-m16", "-Os", "-o", "a.obj", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
