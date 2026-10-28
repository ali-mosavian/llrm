//! An x86-64 frame pushes its callee-saved registers before it reserves its
//! locals, as `X86FrameLowering::emitPrologue` does, and takes the locals back
//! before it pops them.

use std::process::Command;

/// Three callee-saved registers and a 24-byte local: the pin writes `push rbp;
/// push r14; push rbx; sub rsp, 32 ... add rsp, 32; pop rbx; pop r14; pop
/// rbp`. llrm wrote `sub rsp, N` first and the pushes after it, so the CFI of
/// the pushes (`.cfi_offset` after the adjustment) had no prologue to follow.
#[test]
fn test_an_m64_frame_pushes_its_registers_before_it_reserves() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(
        &source,
        "extern int g(int *, int);\nextern int h(int);\n\
         int f(int a, int b, int c) { int buf[6]; int x = g(buf, a); int y = h(b) + x; int z = h(c) + y; return z + buf[2]; }\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-O1", "-fno-builtin", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at = |prefix: &str| {
        lines.iter().position(|line| line.starts_with(prefix)).unwrap_or_else(|| panic!("{prefix}: {text}"))
    };
    let (sub, add) = (at("sub rsp"), at("add rsp"));
    let pushes =
        lines.iter().enumerate().filter(|(_, line)| line.starts_with("push ")).map(|(at, _)| at).collect::<Vec<_>>();
    let pops =
        lines.iter().enumerate().filter(|(_, line)| line.starts_with("pop ")).map(|(at, _)| at).collect::<Vec<_>>();
    assert!(pushes.len() >= 2 && pushes.iter().all(|one| *one < sub), "{text}");
    assert!(pops.len() >= 2 && pops.iter().all(|one| *one > add), "{text}");
}

/// The same function with no frame register: the pin's `sub rsp, 32` (3 pushes
/// and the return address are 32 bytes, the 24-byte local is padded to 16).
/// llrm kept the frame register's cell and wrote `sub rsp, 48`, 16 bytes more
/// stack than the pin on every omitted-frame function with pushed registers.
#[test]
fn test_an_omitted_m64_frame_reserves_no_cell_for_the_frame_register() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(
        &source,
        "extern int g(int *, int);\nextern int h(int);\n\
         int f(int a, int b, int c) { int buf[6]; int x = g(buf, a); int y = h(b) + x; int z = h(c) + y; return z + buf[2]; }\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-O1", "-fno-builtin", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.lines().any(|line| line.trim() == "sub rsp, 32"), "{text}");
}

/// A frame that keeps the frame register and moves the stack pointer
/// (`alloca`) takes its stack back as the pin does, `mov rsp, rbp; pop rbp`.
/// llrm wrote `lea rsp, [rbp-16]; leave`, 2 instructions the pin has not and a
/// `leave` the CFI had to follow.
#[test]
fn test_a_frame_with_alloca_returns_through_the_frame_register_and_a_pop() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(
        &source,
        "extern int use(int *);\nint dyn(int n) { int *p = __builtin_alloca(n * 4); return use(p) + 1; }\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-O1", "-fno-builtin", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at = lines.iter().position(|line| *line == "mov rsp, rbp").unwrap_or_else(|| panic!("{text}"));
    assert_eq!(lines[at + 1], "pop rbp", "{text}");
    assert!(!lines.iter().any(|line| *line == "leave"), "{text}");
}

/// Six callee-saved registers are pushed last of the target's list first, as
/// `spillCalleeSavedRegisters` does: `push rbp; push r15; ... push rbx`. llrm
/// pushed `rbx` first and `rbp` last, which sets the code bytes and the order
/// of the CFI offsets.
#[test]
fn test_callee_saved_registers_are_pushed_in_the_reverse_of_the_target_list() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(
        &source,
        "extern int g(int);\n\
         int six(int a, int b, int c, int d, int e, int f) {\n\
         int s1 = g(a), s2 = g(b), s3 = g(c), s4 = g(d), s5 = g(e), s6 = g(f);\n\
         return s1 + s2 * s3 + s4 * s5 + s6 + a + b + c + d + e + f; }\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-O2", "-fno-builtin", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let pushed: Vec<&str> = text.lines().map(str::trim).filter(|line| line.starts_with("push ")).collect();
    assert_eq!(pushed, ["push rbp", "push r15", "push r14", "push r13", "push r12", "push rbx"], "{text}");
}
