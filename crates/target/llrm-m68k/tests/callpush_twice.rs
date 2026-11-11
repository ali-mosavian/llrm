//! Callpush turns a stack-argument store into a push and may fold the load
//! that feeds it. `a + a` on a float is `__addsf3(a, a)`: the first push took
//! the load's memory and dropped the load, the second pushed a `%d0` nothing
//! had written (`poly` -O2 printed 631 for 643). Generated calls repeat
//! arguments of every width; every pushed register must be written first.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const LAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";
const TYPES: [&str; 5] = ["i16", "i32", "float", "double", "ptr"];

fn next(state: &mut u64) -> usize {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*state >> 33) as usize
}

/// A call `g(args)` where each argument repeats a parameter or a prior call's result.
fn module(state: &mut u64) -> String {
    let params: Vec<&str> = (0..1 + next(state) % 2).map(|_| TYPES[next(state) % TYPES.len()]).collect();
    let mut values: Vec<(String, &str)> = params.iter().enumerate().map(|(i, t)| (format!("%p{i}"), *t)).collect();
    let mut body = String::new();
    if next(state) % 2 == 0 {
        body += "  %r = call i32 @h()\n";
        values.push(("%r".into(), "i32"));
    }
    let args: Vec<(String, &str)> = (0..2 + next(state) % 4).map(|_| values[next(state) % values.len()].clone()).collect();
    let sig: Vec<String> = args.iter().map(|(_, t)| t.to_string()).collect();
    let used: Vec<String> = args.iter().map(|(v, t)| format!("{t} {v}")).collect();
    let formals: Vec<String> = params.iter().enumerate().map(|(i, t)| format!("{t} %p{i}")).collect();
    format!(
        "{LAYOUT}declare i32 @g({})\ndeclare i32 @h()\ndefine i32 @f({}) {{\n{body}  %x = call i32 @g({})\n  ret i32 %x\n}}\n",
        sig.join(", "),
        formals.join(", "),
        used.join(", ")
    )
}

fn unwritten_push(asm: &str) -> Option<String> {
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    for (at, line) in lines.iter().enumerate() {
        let Some(rest) = line.strip_prefix("move.").and_then(|rest| rest.split_once('\t')) else { continue };
        let Some(reg) = rest.1.strip_suffix(", -(%sp)").filter(|reg| reg.starts_with('%')) else { continue };
        let written = lines[..at].iter().any(|before| {
            before.starts_with("jsr") && ["%d0", "%d1", "%a0", "%a1"].contains(&reg)
                || before.split_once('\t').is_some_and(|(_, operands)| operands.rsplit(',').next().is_some_and(|last| last.trim() == reg))
        });
        if !written {
            return Some(format!("{reg} pushed before anything wrote it"));
        }
    }
    None
}

#[test]
fn a_register_pushed_twice_is_loaded() {
    let mut state = 0x6d68_6b36_3800_0001;
    for case in 0..150 {
        let ir = module(&mut state);
        for cpu in ["68000", "68020"] {
            for level in [Level::O2, Level::Os] {
                let asm = assembly(&ir, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}\n{ir}"));
                if let Some(why) = unwritten_push(&asm) {
                    panic!("case {case} {cpu} {level:?}: {why}\n{ir}\n{asm}");
                }
            }
        }
    }
}
