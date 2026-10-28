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
