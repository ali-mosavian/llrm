//! Every bench kernel, compiled and run under `qemu-m68k -cpu m68000`, must
//! exit with the value worked out by hand. A run is bounded (`timeout 10`) and
//! no trace is written; a runaway program exits 124.
//!
//! These failed before the fixes: `max`, `clamp`, `arrmax`, `countgt` and
//! `copy` exited with the wrong value (the `CMOV` pseudo of a select encoded
//! as zero bytes), and `gcd` never ended.

use std::process::Command;

use llrm_m68k::compile::items;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::kernels::{Arg, DATALAYOUT, program, source, table};
use llrm_m68k::object::{executable, write_executable};

/// The kernel's hand-derived result and the exit status of its run.
fn run_kernel(name: &str) -> Option<(i32, i32)> {
    if !std::path::Path::new("/usr/bin/qemu-m68k").exists() {
        eprintln!("qemu-m68k is not installed: not run");
        return None;
    }
    let (_, args, expected) = table().into_iter().find(|(one, ..)| *one == name).expect("a known kernel");
    let module = llrm_mir::parse::module(&source(name)).expect("parses");
    let code = items(&module, "k", Cpu::M68000).expect("compiles");
    let exe = std::env::temp_dir().join(format!("llrm-m68k-kernel-{}-{name}", std::process::id()));
    write_executable(&exe, &executable(&program(&args, code)).unwrap()).unwrap();
    let status = Command::new("timeout").args(["10", "qemu-m68k", "-cpu", "m68000"]).arg(&exe).status().unwrap();
    std::fs::remove_file(&exe).ok();
    assert_ne!(status.code(), Some(124), "{name} did not end within 10 s");
    Some((expected, status.code().unwrap_or(-1)))
}

fn check(name: &str) {
    if let Some((expected, got)) = run_kernel(name) {
        assert_eq!(got, expected, "{name} exited with the wrong value");
    }
}

macro_rules! kernel_tests {
    ($($name:ident),*) => { $( #[test] fn $name() { check(stringify!($name)); } )* };
}

kernel_tests!(sum, popcount, max, absdiff, clamp, fib, gcd, arrsum, arrmax, countgt, strlen, copy);

/// clang given the m68k triple has 32-bit `int`, `long`, pointers and
/// `size_t`, and prints the datalayout the kernels carry.
#[test]
fn clang_agrees_with_the_target_on_sizes_and_datalayout() {
    let Ok(out) = Command::new("clang")
        .args(["--target=m68k-linux-gnu", "-S", "-emit-llvm", "-O0", "-x", "c", "-", "-o", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            let source = "_Static_assert(sizeof(int) == 4, \"int\");\n_Static_assert(sizeof(long) == 4, \"long\");\n\
                          _Static_assert(sizeof(void *) == 4, \"pointer\");\n_Static_assert(sizeof(__SIZE_TYPE__) == 4, \"size_t\");\n\
                          int f(int *p) { return *p; }\n";
            child.stdin.take().unwrap().write_all(source.as_bytes())?;
            child.wait_with_output()
        })
    else {
        eprintln!("clang is not installed: not run");
        return;
    };
    let ir = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "clang rejected the size assertions: {}", String::from_utf8_lossy(&out.stderr));
    assert!(
        ir.contains(&format!("target datalayout = \"{}\"", llrm_m68k::kernels::DATALAYOUT)),
        "datalayout differs:\n{ir}"
    );
}

/// Twenty arguments xor-ed with a counter and summed in a loop: more values
/// live at once than the eight data registers hold. It stopped with "no
/// register for v14"; now the surplus is spilled with `movem`, which leaves
/// the condition codes alone. Sum over three trips, hand-derived: 1150, so
/// the exit status is 1150 mod 256.
#[test]
fn more_live_values_than_data_registers_are_spilled() {
    if !std::path::Path::new("/usr/bin/qemu-m68k").exists() {
        eprintln!("qemu-m68k is not installed: not run");
        return;
    }
    let params: Vec<String> = (0..20).map(|k| format!("i32 %a{k}")).collect();
    let mut ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\ndefine i32 @k({}) {{\nentry:\n  br label %l\nl:\n",
        params.join(", ")
    );
    ir.push_str("  %i = phi i32 [0, %entry], [%i1, %l]\n  %s = phi i32 [0, %entry], [%s1, %l]\n");
    let mut sum = "%s".to_owned();
    for k in 0..20 {
        ir.push_str(&format!("  %m{k} = xor i32 %a{k}, %i\n  %t{k} = add i32 {sum}, %m{k}\n"));
        sum = format!("%t{k}");
    }
    ir.push_str(&format!("  %s1 = add i32 {sum}, 1\n  %i1 = add i32 %i, 1\n  %c = icmp slt i32 %i1, %a0\n"));
    ir.push_str("  br i1 %c, label %l, label %x\nx:\n  ret i32 %s1\n}\n");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let code = items(&module, "k", Cpu::M68000).expect("compiles");
    let args: Vec<Arg> = std::iter::once(3).chain(11..30).map(Arg::Imm).collect();
    let exe = std::env::temp_dir().join(format!("llrm-m68k-spill-{}", std::process::id()));
    write_executable(&exe, &executable(&program(&args, code)).unwrap()).unwrap();
    let status = Command::new("timeout").args(["10", "qemu-m68k", "-cpu", "m68000"]).arg(&exe).status().unwrap();
    std::fs::remove_file(&exe).ok();
    assert_eq!(status.code(), Some(1150 % 256));
}
