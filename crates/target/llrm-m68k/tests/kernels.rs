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

/// `ir`'s function `k`, run with `args`: its exit status. `None` without qemu.
fn run_ir(
    ir: &str,
    args: &[Arg],
) -> Option<i32> {
    if !std::path::Path::new("/usr/bin/qemu-m68k").exists() {
        return None;
    }
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    let code = items(&module, "k", Cpu::M68000).expect("compiles");
    let exe = std::env::temp_dir().join(format!("llrm-m68k-ir-{}-{}", std::process::id(), ir.len()));
    write_executable(&exe, &executable(&program(args, code)).unwrap()).unwrap();
    let status = Command::new("timeout").args(["10", "qemu-m68k", "-cpu", "m68000"]).arg(&exe).status().unwrap();
    std::fs::remove_file(&exe).ok();
    status.code()
}

/// [`run_ir`] with the runtime the libcalls of a 68000 name (`__mulsi3` and the others) linked in.
fn run_ir_with_runtime(
    ir: &str,
    args: &[Arg],
) -> Option<i32> {
    if !std::path::Path::new("/usr/bin/qemu-m68k").exists() {
        return None;
    }
    let runtime = llrm_m68k::kernels::source_in("library", "rt");
    let text = format!(
        "target datalayout = \"{DATALAYOUT}\"\n{ir}\n{}",
        runtime.split_once('\n').map_or("", |(_, rest)| rest)
    );
    let module = llrm_mir::parse::module(&text).expect("parses");
    let mut code = Vec::new();
    for (_, global, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
        let name = global.name.as_deref().expect("a named function");
        code.extend(items(&module, name, Cpu::M68000).expect("compiles"));
    }
    let exe = std::env::temp_dir().join(format!("llrm-m68k-rt-{}-{}", std::process::id(), ir.len()));
    write_executable(&exe, &executable(&program(args, code)).unwrap()).unwrap();
    let status = Command::new("timeout").args(["10", "qemu-m68k", "-cpu", "m68000"]).arg(&exe).status().unwrap();
    std::fs::remove_file(&exe).ok();
    status.code()
}

/// A `switch` made blocks of its own that the function builder refused ("a switch lowering made
/// blocks"): every generated program has one. Each case, the shared one and the default.
#[test]
fn a_switch_takes_the_case_it_names() {
    let ir = "define i32 @k(i32 %x) {\nentry:\n  switch i32 %x, label %d [ i32 0, label %a\n i32 1, label %b\n i32 2, label %c\n i32 5, label %a ]\n\
        a:\n  br label %m\nb:\n  br label %m\nc:\n  br label %m\nd:\n  br label %m\n\
        m:\n  %r = phi i32 [10, %a], [20, %b], [30, %c], [40, %d]\n  ret i32 %r\n}";
    for (x, want) in [(0, 10), (1, 20), (2, 30), (5, 10), (3, 40), (9, 40), (-1, 40)] {
        if let Some(got) = run_ir(ir, &[Arg::Imm(x)]) {
            assert_eq!(got, want, "switch on {x}");
        }
    }
}

/// The low word moved up: 0x00AB1234 << 16 is 0x12340000, whose top byte is 0x12.
#[test]
fn a_shift_left_by_16_moves_the_low_word_up_and_clears_the_rest() {
    let ir = "define i32 @k(i32 %x) {\n  %s = shl i32 %x, 16\n  %r = lshr i32 %s, 24\n  ret i32 %r\n}";
    if let Some(got) = run_ir(ir, &[Arg::Imm(0x00AB1234)]) {
        assert_eq!(got, 0x12);
    }
}

/// A value shifted left by 16 and fed back round a loop kept the old low word: the clear of the
/// low word (`and.l #-65536`) ended up after the loop, so the next trip multiplied the swapped
/// word with garbage in its low half. Seed 96 of the differential driver gave 169 for 139.
#[test]
fn a_value_shifted_up_16_round_a_loop_has_its_low_word_cleared_each_trip() {
    let ir = "define i32 @k(i32 %x, i32 %y) {\nentry:\n  br label %l\n\
        l:\n  %p = phi i32 [ %x, %entry ], [ %s, %l ]\n  %i = phi i32 [ 4, %entry ], [ %i1, %l ]\n\
        \x20 %m = mul i32 %p, %y\n  %n = xor i32 %m, -1\n  %s = shl i32 %n, 16\n  %i1 = sub i32 %i, 1\n\
        \x20 %c = icmp eq i32 %i1, 0\n  br i1 %c, label %done, label %l\n\
        done:\n  %h = lshr i32 %s, 16\n  %r = xor i32 %h, %s\n  ret i32 %r\n}";
    let (mut p, y) = (0x0001_0003u32, 0x0007_0005u32);
    for _ in 0..4 {
        p = (!p.wrapping_mul(y)) << 16;
    }
    let want = ((p >> 16) ^ p) & 0xFF;
    if let Some(got) = run_ir_with_runtime(ir, &[Arg::Imm(0x0001_0003), Arg::Imm(0x0007_0005)]) {
        assert_eq!(got, want as i32);
    }
}

/// A word loaded into an address register (`move.w (..), %a0` sign-extends) and zero-extended
/// read it back with `move.w %a0, %d2`, which the pin's `MOV16dd` encodes as `%d0`: the quotient
/// came from the wrong register (differential seed 437 gave 0 for 6). Data registers hold the
/// argument bytes, so the load is given an address register; the word at offset 2 of "hello" is
/// 0x6c6c, over 3 is 0x2424.
#[test]
fn a_word_zero_extended_is_read_from_a_data_register() {
    let ir = "define i16 @k(ptr %p, i8 %a, i8 %b, i8 %c, i8 %d) {\n\
        \x20 %ci = zext i8 %c to i32\n  %q = getelementptr [2 x i8], ptr %p, i32 %ci\n  %w = load i16, ptr %q\n\
        \x20 %bo = or i8 %b, 1\n  %bz = zext i8 %bo to i16\n  %dv = udiv i16 %w, %bz\n  %t = trunc i16 %dv to i8\n\
        \x20 %a16 = add i8 %a, 16\n  %o = or i8 %a16, %t\n  %cmp = icmp ugt i8 %o, %c\n  %cz = zext i1 %cmp to i16\n\
        \x20 %oz = zext i8 %o to i16\n  %dz = zext i8 %d to i16\n  %s = shl i16 %dz, 1\n  %s2 = add i16 %s, 272\n\
        \x20 %x = xor i16 %s2, %oz\n  %r = xor i16 %x, %cz\n  ret i16 %r\n}";
    let args = [Arg::Data(0), Arg::Imm(5), Arg::Imm(3), Arg::Imm(1), Arg::Imm(2)];
    if let Some(got) = run_ir_with_runtime(ir, &args) {
        // 0x114 ^ 0x35 ^ 1, the low byte of the status
        assert_eq!(got, 0x20);
    }
}

/// `llvm.{u,s}{add,sub}.with.overflow` was refused in every width ("convert node to libcall",
/// differential seed 452). The value plus the overflow bit, per case, as `LowerXALUO` gives them.
/// The multiplies of 8 and 16 bits were refused (differential seed 2039, "Unsupported UMulO i8"); the pin
/// answers them with a constant zero overflow, which is wrong: `200 * 2` in a byte overflows.
#[test]
fn arithmetic_with_overflow_gives_the_wrapped_value_and_the_carry_or_overflow() {
    // (intrinsic, width, a, b, value + overflow bit, modulo the status byte)
    let cases: [(&str, u32, i64, i64, i64); 15] = [
        ("uadd", 8, 200, 100, 44 + 1),
        ("uadd", 16, 60000, 10000, (4464 + 1) & 255),
        ("uadd", 32, 0xFFFF_FFFF, 2, 1 + 1),
        ("uadd", 8, 1, 2, 3),
        ("sadd", 8, 100, 100, 200 + 1),
        ("sadd", 16, 3, 4, 7),
        ("usub", 8, 3, 5, 254 + 1),
        ("ssub", 8, 156, 100, 56 + 1),
        ("ssub", 32, 0x8000_0000, 1, 0x7FFF_FFFF + 1),
        ("umul", 8, 200, 2, 144 + 1),
        ("umul", 8, 3, 4, 12),
        ("umul", 16, 300, 300, (24464 + 1) & 255),
        ("smul", 8, 100, 2, 200 + 1),
        ("smul", 8, 253, 4, 244),
        ("smul", 16, 200, 200, (40000 + 1) & 255),
    ];
    for (name, bits, a, b, want) in cases {
        let ty = format!("i{bits}");
        let wide = if bits == 32 {
            "  %s = add i32 %v, %oo\n".to_owned()
        } else {
            format!("  %z = zext {ty} %v to i32\n  %s = add i32 %z, %oo\n")
        };
        let ir = format!(
            "declare {{ {ty}, i1 }} @llvm.{name}.with.overflow.{ty}({ty}, {ty})\n\
             define i32 @k({ty} %a, {ty} %b) {{\n\
             \x20 %r = call {{ {ty}, i1 }} @llvm.{name}.with.overflow.{ty}({ty} %a, {ty} %b)\n\
             \x20 %v = extractvalue {{ {ty}, i1 }} %r, 0\n  %o = extractvalue {{ {ty}, i1 }} %r, 1\n\
             \x20 %oo = zext i1 %o to i32\n{wide}  ret i32 %s\n}}"
        );
        if let Some(got) = run_ir_with_runtime(&ir, &[Arg::Imm(a), Arg::Imm(b)]) {
            assert_eq!(got, (want & 255) as i32, "{name} i{bits} {a} {b}");
        }
    }
}

/// Three division libcalls whose results feed each other, with work for the next one in between.
/// The ILP scheduler began the arguments of the last call inside the one before it, and the stack
/// slots they share were overwritten (differential seeds 1203 and 36 more gave wrong values): the
/// call-frame resource keeps one call sequence at a time, as `DelayForLiveRegsBottomUp` does.
#[test]
fn the_arguments_of_two_libcalls_are_not_interleaved() {
    let ir = "define i32 @k(i32 %a, i32 %b) {\n\
        \x20 %bo = or i32 %b, 1\n  %v1 = udiv i32 %a, %bo\n  %x = xor i32 %a, 118\n  %x2 = shl i32 %x, 3\n\
        \x20 %v1o = or i32 %v1, 1\n  %m = urem i32 %v1, %v1o\n  %mo = or i32 %m, 1\n  %w = udiv i32 %x2, %mo\n\
        \x20 %r = add i32 %w, %v1\n  ret i32 %r\n}";
    let (a, b) = (1_000_000u32, 6u32);
    let v1 = a / (b | 1);
    let m = v1 % (v1 | 1);
    let expected = ((a ^ 118) << 3) / (m | 1) + v1;
    if let Some(got) = run_ir_with_runtime(ir, &[Arg::Imm(a.into()), Arg::Imm(b.into())]) {
        assert_eq!(got, (expected & 255) as i32);
    }
}
