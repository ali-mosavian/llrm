//! `jsr h` of a function the same file defines was 6 B (absolute long); `jsr
//! (h,%pc)` is 4 B and needs no relocation (memory_expansion: 528 such calls, 1
//! KB). The displacement is 16 bits, so a module with more than 28 KB of code
//! keeps the absolute call.

use llrm_m68k::driver::{Level, assembly, cpu_of};

mod support;

const DATALAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

const SMALL: &str = "define internal i32 @h(i32 %a) noinline {
  %r = xor i32 %a, 100000
  ret i32 %r
}
declare i32 @ext(i32)
define weak i32 @w(i32 %a) {
  ret i32 %a
}
define i32 @f(i32 %a) {
  %x = call i32 @h(i32 %a)
  %y = call i32 @ext(i32 %x)
  %z = call i32 @w(i32 %y)
  ret i32 %z
}
";

#[test]
fn a_call_of_a_function_the_file_defines_is_pc_relative() {
    for cpu in ["68000", "68020"] {
        let out = assembly(&format!("{DATALAYOUT}{SMALL}"), cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(out.contains("jsr\t(h,%pc)"), "{cpu}: local callee:\n{out}");
        assert!(out.contains("jsr\text"), "{cpu}: a declaration stays absolute:\n{out}");
        assert!(out.contains("jsr\tw"), "{cpu}: a weak definition can be replaced, so it stays absolute:\n{out}");
    }
}

/// `n` small functions, then `top`, which calls the first (kept out of line,
/// not a tail call).
fn module_of(n: usize) -> String {
    let mut ir = String::from(DATALAYOUT);
    for k in 0..n {
        let kept = if k == 0 { " noinline" } else { "" };
        ir.push_str(&format!(
            "define i32 @g{k}(i32 %a){kept} {{\n  %x = xor i32 %a, {}\n  %y = add i32 %x, {}\n  ret i32 %y\n}}\n",
            100_001 + k * 7,
            200_003 + k * 11
        ));
    }
    ir.push_str("define i32 @top(i32 %a) {\n  %x = call i32 @g0(i32 %a)\n  %y = add i32 %x, 1\n  ret i32 %y\n}\n");
    ir
}

/// 18 B a function: 1000 are 18 KB, in reach; 2500 are 45 KB, past
/// `pccall::REACH` (28 KB).
#[test]
fn a_module_within_reach_is_pc_relative_and_one_past_it_keeps_absolute_calls() {
    let cpu = cpu_of("68000").unwrap();
    let near = assembly(&module_of(1000), cpu, Level::O2).expect("compiles");
    assert!(near.contains("jsr\t(g0,%pc)"), "18 KB of code is in reach");
    let far = assembly(&module_of(2500), cpu, Level::O2).expect("compiles");
    assert!(far.contains("jsr\tg0") && !far.contains("(g0,%pc)"), "45 KB of code is not");
}

/// `h(x) = x ^ 100000`, `f(n) = h(h(n)) + 1`: the displacement of `jsr (h,%pc)`
/// reaches `h` (n = 5).
#[test]
fn a_pc_relative_call_reaches_its_callee() {
    use llrm_m68k::compile::{Options, items_with};
    use llrm_m68k::dagisel::hooks::Cpu;
    use llrm_m68k::object::{executable, start, write_executable};
    let Some(qemu) = support::qemu() else { return };
    let ir = format!(
        "{DATALAYOUT}define internal i32 @h(i32 %a) noinline {{\n  %r = xor i32 %a, 100000\n  ret i32 %r\n}}\n\
         define i32 @f(i32 %a) {{\n  %x = call i32 @h(i32 %a)\n  %y = call i32 @h(i32 %x)\n  %z = add i32 %y, 1\n  ret i32 %z\n}}\n"
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let options = Options { pc_calls: true, call_targets: false, ..Options::default() };
    let mut all = start("f", &[5]);
    for name in ["f", "h"] {
        all.extend(items_with(&module, name, Cpu::M68000, options).expect("compiles"));
    }
    let path = support::unique_path("pc_calls");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(6));
}
