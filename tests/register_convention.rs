//! -m32's default convention is Open Watcom's register one (calling.toml's `watcall32`): arguments in
//! EAX, EDX, EBX, ECX, then the stack, popped by the callee. An explicit `cdecl32` keeps the stack.
//! Each expectation was read from `wcc386 -3r`'s own output for the same signature.

use std::process::Command;

/// The listing of `source` for -m32.
fn listing(source: &str) -> String {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("t.nib");
    std::fs::write(&path, format!("{source}\nfn main() -> i32:\n    return 0\n")).unwrap();
    let out = directory.path().join("t.asm");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib")).args(["-m32", "-O2", "-S"]).arg(&path).arg("-o").arg(&out).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// The lines of procedure `name`, trimmed.
fn procedure(listing: &str, name: &str) -> Vec<String> {
    let from = listing.find(&format!("{name} proc")).unwrap_or_else(|| panic!("{name} in:\n{listing}"));
    listing[from..].lines().skip(1).take_while(|line| !line.ends_with("endp")).map(|line| line.trim().to_owned()).collect()
}

/// A call with a nonzero cleanup after it, as caller-pops convention has it.
fn pops_after(lines: &[String]) -> bool {
    lines.windows(2).any(|pair| pair[0].starts_with("call") && pair[1].starts_with("add esp"))
}

/// Before the default was registers, the third argument was read from [ebp+16]; it arrives in EBX.
#[test]
fn the_first_four_arguments_arrive_in_eax_edx_ebx_ecx() {
    let text = listing("@export(\"watcall32\")\nfn third(a: i32, b: u32, c: i32) -> i32:\n    return c\n@export(\"watcall32\")\nfn fourth(a: i32, b: i32, c: i32, d: i32) -> i32:\n    return d\n");
    assert_eq!(procedure(&text, "third_"), ["L0_0:", "mov eax, ebx", "ret"]);
    assert_eq!(procedure(&text, "fourth_"), ["L1_0:", "mov eax, ecx", "ret"]);
}

/// `wcc386` took the fifth and sixth from the stack, lowest first, and popped them with `ret 8`: a callee
/// that left them for the caller to pop unbalanced every call.
#[test]
fn arguments_past_the_fourth_are_on_the_stack_and_the_callee_pops_them() {
    let text = listing("@export(\"watcall32\")\nfn sixth(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) -> i32:\n    return e + f\n");
    let lines = procedure(&text, "sixth_");
    assert!(lines.contains(&"mov eax, dword ptr [esp+4]".to_owned()) && lines.contains(&"add eax, dword ptr [esp+8]".to_owned()), "{lines:?}");
    assert_eq!(lines.last().map(String::as_str), Some("ret 8"));
}

/// A call passes in the same registers, pushes the rest right to left and does not pop what the callee does.
#[test]
fn a_call_loads_the_registers_and_pushes_the_rest_without_popping() {
    let text = listing("@extern(\"watcall32\")\nfn ext6(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) -> i32\n@export(\"watcall32\")\nfn calls() -> i32:\n    unsafe:\n        return ext6(1, 2, 3, 4, 5, 6)\n");
    let lines = procedure(&text, "calls_");
    let at = |line: &str| lines.iter().position(|one| one == line).unwrap_or_else(|| panic!("{line} in {lines:?}"));
    assert!(at("pushd 6") < at("pushd 5") && at("pushd 5") < at("call ext6_"), "{lines:?}");
    for load in ["mov eax, 1", "mov edx, 2", "mov ebx, 3", "mov ecx, 4"] {
        assert!(at(load) < at("call ext6_"), "{lines:?}");
    }
    assert!(!pops_after(&lines), "{lines:?}");
}

/// `wcc386` sent `(int, double, int)` to EAX and two stack cells although EDX was free: the first argument that
/// fits no register sends every later one to the stack. The callee read b from [ebp+16] and popped 12.
#[test]
fn the_argument_after_a_float_is_on_the_stack_too() {
    let text = listing("@export(\"watcall32\")\nfn after_float(a: i32, x: f64, b: i32) -> i32:\n    return b\n");
    let lines = procedure(&text, "after_float_");
    assert!(lines.contains(&"mov eax, dword ptr [esp+12]".to_owned()), "{lines:?}");
    assert_eq!(lines.last().map(String::as_str), Some("ret 12"));
}

/// An explicit cdecl32 keeps the stack: its arguments are pushed, its symbol is `_name`, and the caller pops.
#[test]
fn an_explicit_cdecl32_keeps_stack_arguments_and_caller_cleanup() {
    let text = listing("@extern(\"cdecl32\")\nfn extc(a: i32, b: i32) -> i32\n@export(\"watcall32\")\nfn callsc(x: i32) -> i32:\n    unsafe:\n        return extc(x, 7) + x\n");
    let lines = procedure(&text, "callsc_");
    assert!(lines.contains(&"pushd 7".to_owned()) && lines.contains(&"call _extc".to_owned()), "{lines:?}");
    assert!(pops_after(&lines), "{lines:?}");
}

/// A callee keeps every register but EAX and those its arguments arrive in: `wcc386` pushed ECX and EDX
/// around a body that used them, and `calls` below holds nothing in them across the call.
#[test]
fn a_callee_saves_the_registers_it_uses_that_its_arguments_do_not() {
    let text = listing("@export(\"watcall32\")\nfn fib(n: i32) -> i32:\n    if n < 2:\n        return n\n    return fib(n - 1) + fib(n - 2)\n");
    let lines = procedure(&text, "fib_");
    assert!(lines.first().is_some_and(|one| one.starts_with("push")), "{lines:?}");
    assert!(!lines.iter().any(|one| one == "push eax" || one == "pop eax"), "EAX is the callee's own: {lines:?}");
}

/// The listing of C `source` under `flags`.
fn c_listing(flags: &[&str], source: &str) -> String {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("t.c");
    std::fs::write(&path, source).unwrap();
    let out = directory.path().join("t.asm");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-c")).args(flags).args(["-O2", "-fno-inline-functions", "-S"]).arg(&path).arg("-o").arg(&out).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// Under `-mabi=sysv` a `static` function took the stack too, though no one outside sees it (gcc passes it in registers,
/// `regparm` for a `local` one): its arguments were read from [esp+4..], each call pushed them and popped them after. It takes
/// the description's `private` convention; the exported function keeps sysv's.
#[test]
fn a_private_function_takes_registers_under_sysv_and_an_exported_one_keeps_the_stack() {
    let text = c_listing(&["-m32", "-mabi=sysv"], "static int add3(int a, int b, int c) { return a * 3 + b * 5 + c; }\nint twice(int a, int b, int c) { return a * 2 + b + c; }\nint use(int x) { return add3(x, 2, 3) + add3(3, x, 1); }\n");
    let private = procedure(&text, "_add3");
    assert!(!private.iter().any(|line| line.contains("[esp+")), "{private:?}");
    let exported = procedure(&text, "_twice");
    assert!(exported.iter().any(|line| line.contains("[esp+4]")), "{exported:?}");
    let caller = procedure(&text, "_use");
    assert!(!caller.iter().any(|line| line.starts_with("push") && line.contains("2")) && !pops_after(&caller), "{caller:?}");
}

/// A callee keeps ECX under Watcom's convention unless its arguments arrive there; `g_` (two arguments, in EAX and EDX) called a
/// cdecl routine, which clobbers ECX, and did not save it: a caller holding a value in ECX across `call g_` lost it (found as
/// a crash at the end of tests/run/nib/flat_containers.nib under `-mabi=sysv`, whose private functions took the register convention).
#[test]
fn a_callee_saves_the_register_a_call_it_makes_clobbers() {
    let text = c_listing(&["-m32"], "extern int __cdecl ext(int a, int b);\nint g(int a, int b) { return ext(a, b) + 1; }\n");
    let lines = procedure(&text, "g_");
    assert!(lines.first().is_some_and(|one| one == "push ecx"), "ECX is the callee's to keep: {lines:?}");
    assert!(lines.iter().any(|one| one == "pop ecx"), "{lines:?}");
}
