//! `divsl.l %d0, %d0:%d1` was in no clock table: `exec_loop` (QB `exec.c`,
//! 99.9% of its backend) could not be priced, so every priced trial on it kept
//! nothing. Every instruction the backend prints is priced on each CPU that has
//! it.

use llrm_m68k::dagisel::asm::{ASM, AsmOperand};
use llrm_m68k::dagisel::{divrem, local, scaled, short, widemul};
use llrm_m68k::isa::Cpu;
use llrm_m68k::timing::cost_on;

fn samples(method: &str) -> &'static [&'static str] {
    let kind = method.trim_end_matches("Mem").trim_start_matches("print").trim_end_matches(char::is_numeric);
    match kind {
        "Operand" => &["%d0", "%a0", "#1"],
        "Immediate" => &["#1"],
        "ARI" => &["(%a0)"],
        "ARIPI" => &["(%a0)+"],
        "ARIPD" => &["-(%a0)"],
        "ARID" => &["(4,%a0)"],
        "ARII" => &["(4,%a0,%d0.l)"],
        "AL" => &["sym", "$1000"],
        "PCD" => &["(4,%pc)"],
        "PCI" | "PCIBD" => &["(4,%pc,%d0.l)"],
        "PCRelImm" => &["sym"],
        "MoveMask" => &["%d0/%d1"],
        _ => &["%d0"],
    }
}

/// `template` with each `$operand` replaced by every sample of its printer.
fn texts(
    template: &str,
    operands: &[AsmOperand],
) -> Vec<String> {
    let mut all = vec![String::new()];
    let mut rest = template;
    while let Some(at) = rest.find('$') {
        let literal = &rest[..at];
        rest = &rest[at + 1..];
        let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
        let one = operands.iter().find(|one| one.name == &rest[..end]).expect("an operand the template names");
        all = all
            .iter()
            .flat_map(|head| samples(one.method).iter().map(move |sample| format!("{head}{literal}{sample}")))
            .collect();
        rest = &rest[end..];
    }
    all.iter().map(|text| format!("{text}{rest}")).collect()
}

/// Never printed by the selector (the FPU is soft, `trap` and the rest have no
/// pattern), or a pin pattern for an operand no instruction takes (`clr` to a
/// PC-relative address, `movem` to `(An)+` or from `-(An)`), or one the
/// manual's tables have no mode column for (`cmpi`, `btst #` and `movem` of a
/// PC-indexed operand).
fn excluded(
    name: &str,
    template: &str,
) -> bool {
    let mnemonic = template.split('\t').next().unwrap_or(template);
    mnemonic.starts_with(';')
        || mnemonic.starts_with('f')
        || ["bkpt", "illegal", "rte", "trapv", "trap"].contains(&mnemonic)
        || ["CLR8k", "CLR16k", "CLR32k", "CLR8q", "CLR16q", "CLR32q"].contains(&name)
        || ["BTST8ki", "CMP8ki", "CMP16ki", "CMP32ki"].contains(&name)
        || ["MOVM16me", "MOVM32me", "MOVM16mk", "MOVM32mk", "MOVM16om", "MOVM32om"].contains(&name)
}

#[test]
fn every_instruction_the_backend_prints_has_a_clock_price_on_each_cpu_that_has_it() {
    let mut names: Vec<&str> = ASM.iter().map(|(name, ..)| *name).collect();
    names.extend(divrem::names());
    names.extend(widemul::names());
    names.extend(scaled::names());
    names.extend(short::names());
    let mut missing = Vec::new();
    for name in names {
        let Some((_, template, operands)) = local::text(name) else { continue };
        if excluded(name, template) {
            continue;
        }
        let mnemonic = template.split('\t').next().unwrap_or(template);
        let candidates = texts(template, operands);
        let forms = llrm_m68k::isa::forms_of(&mnemonic.replace('.', ""));
        for cpu in Cpu::ALL {
            let priced = |text: &String| cost_on(cpu, text.replace('\t', " ").as_str(), 4, false).is_ok();
            if forms.iter().any(|form| form.on(cpu)) && !candidates.iter().any(priced) {
                missing.push(format!("{name} (`{mnemonic}`) on the {}", cpu.name()));
            }
        }
    }
    assert!(missing.is_empty(), "no clock price for: {missing:#?}");
}
