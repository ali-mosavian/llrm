//! `g(1); g(2); ... g(6)` at `-Os` was six `jsr g` (6 B each, 36 B). gcc loads
//! `g` into `%a2` once and calls `jsr (%a2)` (2 B each): 6 B for the load, 4 B
//! for the saved register, 12 B for the calls, 22 B in all
//! (memory_expansion: 900 calls, 3 KB of its +7.9 KB against gcc).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const HEADER: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @g(i32)
declare void @h(i32)
";

fn calls(
    count: i32,
    callee: &str,
) -> String {
    let body: String = (1..=count).map(|n| format!("  call void @{callee}(i32 {n})\n")).collect();
    format!("{HEADER}define void @f() optsize {{\nentry:\n{body}  ret void\n}}\n")
}

fn lines(
    ir: &str,
    cpu: &str,
    level: Level,
) -> Vec<String> {
    let out = assembly(ir, cpu_of(cpu).unwrap(), level).expect("compiles");
    out.lines().map(|line| line.trim().to_owned()).collect()
}

#[test]
fn a_callee_called_six_times_is_called_through_a_register() {
    let out = lines(&calls(6, "g"), "68000", Level::Os);
    let through = out.iter().filter(|line| line.starts_with("jsr") && line.contains("(%a")).count();
    assert_eq!(through, 6, "calls stay absolute:\n{}", out.join("\n"));
}

/// `-O1` is judged by clocks (user ruling 2026-11-14): the `lea` and the saved register are ~12 static
/// clk in straight-line code, so six `jsr g` stay absolute; -Os, above, takes the register for its bytes.
#[test]
fn a_callee_called_six_times_stays_absolute_at_o1() {
    let plain = calls(6, "g").replace(" optsize", "");
    for cpu in ["68000", "68020"] {
        let out = lines(&plain, cpu, Level::O1);
        let through = out.iter().filter(|line| line.starts_with("jsr") && line.contains("(%a")).count();
        assert_eq!(through, 0, "{cpu}: calls went through a register:\n{}", out.join("\n"));
    }
}

/// Two calls cannot pay for the load and the saved register.
#[test]
fn a_callee_called_twice_stays_absolute() {
    let out = lines(&calls(2, "g"), "68000", Level::Os);
    assert!(out.iter().all(|line| !line.contains("(%a")), "a register for two calls:\n{}", out.join("\n"));
}

mod support;

/// The register holds the callee for every call and the caller's own `%a2`
/// comes back: `g(a) = 2a + 1` summed over 1..=6 is 48.
#[test]
fn calls_through_the_register_reach_the_callee_and_restore_it() {
    use llrm_m68k::compile::{Options, items_with};
    use llrm_m68k::dagisel::hooks::Cpu;
    use llrm_m68k::object::{executable, start, write_executable};
    let Some(qemu) = support::qemu() else { return };
    let mut body = String::new();
    for n in 1..=6 {
        body += &format!("  %c{n} = call i32 @g(i32 {n})\n");
    }
    let mut adds = String::new();
    let mut last = "%c1".to_owned();
    for n in 2..=6 {
        adds += &format!("  %s{n} = add i32 {last}, %c{n}\n");
        last = format!("%s{n}");
    }
    let ir = format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define internal i32 @g(i32 %a) #0 {{
  %t = shl i32 %a, 1
  %r = add i32 %t, 1
  ret i32 %r
}}
define i32 @f() optsize {{
{body}{adds}  ret i32 {last}
}}
attributes #0 = {{ noinline }}"
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let options = Options { size: true, ..Options::default() };
    let mut all = start("f", &[]);
    for name in ["f", "g"] {
        all.extend(items_with(&module, name, Cpu::M68000, options).expect("compiles"));
    }
    let path = support::unique_path("calltarget");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(48));
}

/// `lea g, %a2` was "not in the tables": the symbol operand of `lea` and `pea`
/// had no addressing mode, so no call target could be priced on any cpu.
#[test]
fn a_lea_of_a_symbol_has_a_price_on_every_cpu() {
    use llrm_m68k::isa::Cpu;
    use llrm_m68k::timing::cost_on;
    for cpu in [Cpu::M68000, Cpu::M68010, Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060] {
        for text in ["lea\tg, %a2", "pea\tg"] {
            assert!(cost_on(cpu, text, 6, false).is_ok(), "{cpu:?}: {text}");
        }
    }
    assert_eq!(cost_on(Cpu::M68000, "lea\tg, %a2", 6, false).unwrap().clocks, 12);
}

/// Five callees called four times each: `%a2` to `%a5` took four, the fifth
/// stayed 6 B a call (memory_expansion: ~190 calls). `%a6` is free when the
/// frame has no pointer.
#[test]
fn a_fifth_callee_goes_in_a6() {
    let mut ir =
        String::from("target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n");
    for name in ["g1", "g2", "g3", "g4", "g5"] {
        ir += &format!("declare void @{name}(i32)\n");
    }
    ir += "define void @f() optsize {\nentry:\n";
    for n in 1..=4 {
        for name in ["g1", "g2", "g3", "g4", "g5"] {
            ir += &format!("  call void @{name}(i32 {n})\n");
        }
    }
    ir += "  ret void\n}\n";
    let out = lines(&ir, "68000", Level::Os);
    assert!(
        out.iter().any(|line| line.starts_with("jsr") && line.contains("(%a6)")),
        "no call through %a6:\n{}",
        out.join("\n")
    );
}
