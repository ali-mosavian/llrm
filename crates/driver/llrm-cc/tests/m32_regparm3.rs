//! `-mregparm=3` and `regparm(N)` mark the first integer arguments `inreg`; the
//! i386 System V convention takes them in EAX, EDX, ECX and keeps the rest on
//! the stack (`CC_X86_32_C`). Ignored, a function read them from the stack
//! where a regparm caller (the QuickrBasic Makefile builds with
//! `-mregparm=3`) put them in registers. Executed against gcc's
//! `-mregparm=3`: i64, regparm(2) and a variadic call agree.

use std::path::Path;
use std::process::Command;

fn listing(
    name: &str,
    source: &str,
    flags: &[&str],
) -> String {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("regparm3-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), source).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .args(flags)
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("c.s")).unwrap()
}

/// The body of `name` in a MASM listing.
fn body<'a>(
    listing: &'a str,
    name: &str,
) -> &'a str {
    let start = listing.find(&format!("{name} proc near")).unwrap();
    let end = listing[start..].find(&format!("{name} endp")).unwrap();
    &listing[start..start + end]
}

const SOURCE: &str = "__attribute__((noinline)) int f(int a, int b, int c, int d) { return a * 3 + b * 5 + c * 7 + d; }\n\
                      int g(int, int, int, int);\nint h(int x) { return g(x, 2, 3, 4) + 1; }\n";

#[test]
fn test_m32_regparm3_the_callee_reads_three_arguments_from_registers() {
    let text = listing("callee", SOURCE, &["-mregparm=3"]);
    let f = body(&text, "f");
    // a, b, c are in EAX, EDX, ECX: only d (the fourth) is on the stack. On the
    // stack all four were `[esp+4]` .. `[esp+16]`.
    assert!(f.contains("[esp+4]"), "d is not read from the stack:\n{f}");
    for slot in ["[esp+8]", "[esp+12]", "[esp+16]"] {
        assert!(!f.contains(slot), "an argument of regparm(3) is read from {slot}:\n{f}");
    }
}

#[test]
fn test_m32_regparm3_the_caller_passes_three_arguments_in_registers() {
    let text = listing("caller", SOURCE, &["-mregparm=3"]);
    let h = body(&text, "h");
    assert!(h.contains("mov edx, 2") && h.contains("mov ecx, 3"), "the 2nd and 3rd are not in EDX and ECX:\n{h}");
    // The fourth is the one stack argument. The row keeps the stack 16-aligned
    // (`-mstack-alignment=16`), so the frame is the word and 8 bytes
    // of padding: clang's own `h` (-m32 -mregparm=3, fork of the pin) reserves
    // 12 and restores with `add esp, 12`.
    assert!(
        h.contains("pushd 4") || h.contains("[esp], 4"),
        "the fourth is not stored as the one stack argument:\n{h}"
    );
    assert!(h.contains("add esp, 12"), "the caller restores the word and its padding:\n{h}");
}

#[test]
fn test_m32_regparm_attribute_of_two_leaves_the_third_on_the_stack() {
    let source =
        "__attribute__((regparm(2), noinline)) int two(int a, int b, int c) { return a * 3 + b * 5 + c * 7; }\n";
    let text = listing("two", source, &[]);
    let two = body(&text, "two");
    assert!(two.contains("[esp+4]") && !two.contains("[esp+8]"), "c is the only stack argument:\n{two}");
}

#[test]
fn test_m32_regparm3_an_i64_takes_two_registers() {
    let source =
        "__attribute__((noinline)) int u(int a, long long b, int c) { return a + (int)(b >> 3) * 5 + c * 7; }\n";
    let text = listing("wide", source, &["-mregparm=3"]);
    let u = body(&text, "u");
    // a in EAX, b in EDX:ECX (`shrd edx, ecx, 3`), c on the stack.
    assert!(u.contains("shrd edx, ecx, 3"), "b is not in EDX:ECX:\n{u}");
    assert!(u.contains("[esp+4]") && !u.contains("[esp+8]"), "c is the only stack argument:\n{u}");
}

#[test]
fn test_m32_regparm3_a_variadic_call_takes_every_argument_on_the_stack() {
    let source = "int sum(int, ...);\nint callsum(int a) { return sum(2, a, 7); }\n";
    let text = listing("variadic", source, &["-mregparm=3"]);
    let call = body(&text, "callsum");
    assert_eq!(call.matches("push").count(), 3, "a variadic call pushes all three:\n{call}");
}

#[test]
fn test_m32_regparm3_a_struct_result_pointer_is_in_eax_and_not_popped() {
    let source = "struct S { int a, b, c; };\n\
                  __attribute__((noinline)) struct S s3(int x, int y) { struct S s = {x, y, x + y}; return s; }\n\
                  int use(int x) { struct S s = s3(x, 5); return s.a + s.b + s.c; }\n";
    let text = listing("sret", source, &["-mregparm=3"]);
    let s3 = body(&text, "s3");
    // The result pointer is the first `inreg` argument (EAX), x and y follow in
    // EDX and ECX: on the stack the callee popped it with `ret 4`.
    assert!(s3.contains("mov dword ptr [eax], edx"), "the result is not written through EAX:\n{s3}");
    assert!(!s3.contains("ret 4"), "the callee pops a pointer it was not given on the stack:\n{s3}");
    let user = body(&text, "use");
    assert!(user.contains("mov edx, eax") && user.contains("mov ecx, 5"), "x and 5 are not in EDX and ECX:\n{user}");
}
