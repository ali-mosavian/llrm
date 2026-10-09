//! The register effects `x86.instr` states against iced-x86's: every row of each target's description, at each width,
//! assembled and decoded.

use iced_x86::{Decoder, DecoderOptions, InstructionInfoFactory, OpAccess, Register};
use llrm_lir::{Addr, Address, Imm, Loc, Mem, Reg, Semantics, Space};

use llrm_target::Target;
use llrm_x86_m16::instructions::effects::{effects, root, Effects};
use llrm_x86_m16::instructions::parse::{self, Form, Side};

const READS: [OpAccess; 4] = [OpAccess::Read, OpAccess::ReadWrite, OpAccess::CondRead, OpAccess::ReadCondWrite];

fn byte(name: &str) -> Register {
    match name {
        "ax" => Register::AL,
        "bx" => Register::BL,
        "cx" => Register::CL,
        _ => Register::DL,
    }
}

/// What iced says the bytes read and write: registers only, segments left out.
fn decoded(bits: u32, code: &[u8]) -> Option<Effects> {
    let mut found = Effects::default();
    let mut factory = InstructionInfoFactory::new();
    for insn in &mut Decoder::new(bits, code, DecoderOptions::NONE) {
        if insn.is_invalid() {
            return None;
        }
        for used in factory.info(&insn).used_registers() {
            let register = used.register();
            if register == Register::None {
                continue;
            }
            if READS.contains(&used.access()) && !found.reads.contains(&register) {
                found.reads.push(register);
            }
            if [OpAccess::Write, OpAccess::ReadWrite, OpAccess::CondWrite, OpAccess::ReadCondWrite].contains(&used.access()) && !found.writes.contains(&register) {
                found.writes.push(register);
            }
        }
    }
    Some(found)
}

/// The `at`th operand of `side` as kind `kind`, widths in bytes as `Semantics` has them. A register `fixed` pins is
/// the word when the operation is a byte one (`idiv bl` divides AX), and the count of a shift is CL.
fn operand(kind: char, form: &Form, side: Side, at: usize, width: u32, bits: u32, pick: usize) -> Option<Loc> {
    let pinned = form.fixed.iter().find(|(s, i, _)| *s == side && *i == at).map(|(_, _, register)| register.as_str());
    let free: Vec<&str> = ["cx", "dx", "ax", "bx"].into_iter().filter(|name| form.fixed.iter().all(|(_, _, pin)| pin != name)).collect();
    let bytes = width / 8;
    let (base, wide_index) = if bits == 32 { (Register::EBX, Register::ESI) } else { (Register::BX, Register::SI) };
    match kind {
        'r' => {
            let (name, size) = match pinned {
                Some("cx") if form.cost == "shift_ri" => ("cx", 1),
                Some("ax") if form.operation == "fill" => ("ax", bytes),
                // The string operations count and step by the address size.
                Some(name) if matches!(form.operation.as_str(), "fill" | "copy") && name != "es" && name != "fs" => (name, bits / 8),
                Some(name) => (name, bytes.max(2)),
                None => (free[(at + usize::from(side == Side::Source)) % free.len()], bytes),
            };
            let register = match size {
                1 => byte(name),
                2 => root(name, 16),
                _ => root(name, 32),
            };
            Some(Loc::Reg(Reg { register, width: size }))
        }
        // A string operation names its memory by SI and DI, which the row lists.
        'm' if matches!(form.operation.as_str(), "fill" | "copy") => Some(Loc::Mem(Mem::new(None, bytes))),
        'm' => Some(Loc::Mem(match pick % 2 {
            0 => Mem { through: base, ..Mem::new(None, bytes) },
            _ => Mem { addr: Some(Addr { space: Space::Frame, disp: -4, index: 0, base: Register::None, segment: Register::None }), through: Register::None, ..Mem::new(None, bytes) },
        })),
        'a' => {
            // A word `lea` in flat code takes a 16-bit address, behind a prefix.
            let (base, index) = if bytes == 2 { (Register::BX, Register::SI) } else { (base, wide_index) };
            Some(Loc::Address(Address { through: base, index: if pick % 2 == 0 { Register::None } else { index }, scale: 1, ..Address::new(None) }))
        }
        'i' => Some(Loc::Imm(Imm { value: 3, width: if form.cost == "shift_ri" { 1 } else { bytes }, address: None })),
        _ => None,
    }
}

#[test]
fn the_rows_say_what_iced_says() {
    let mut checked = 0;
    let mut wrong = Vec::new();
    for (target, bits) in [(&llrm_x86_m16::M16 as &dyn Target, 16), (&llrm_x86_m32::M32, 32)] {
    let forms = parse::parse(&target.forms_text()).unwrap();
    for form in &forms {
        let widths = if form.widths.is_empty() { vec![32] } else { form.widths.clone() };
        for &width in &widths {
            // The selector emits no byte multiply (`select_sweep.rs` has none), so no semantics of one to check against.
            if width == 8 && form.operation == "mul" && form.dests.len() == 2 {
                continue;
            }
            {
                let kinds = |operands: &[parse::Operand]| -> Vec<Vec<char>> { operands.iter().map(|one| one.kinds.chars().collect()).collect() };
                let (dest_kinds, source_kinds) = (kinds(&form.dests), kinds(&form.sources));
                let mut variants: Vec<(Vec<Loc>, Vec<Loc>)> = Vec::new();
                for pick in 0..4 {
                    let build = |side: Side, all: &[Vec<char>], dests: &[Loc]| -> Option<Vec<Loc>> {
                        all.iter().enumerate().map(|(at, choices)| {
                            if let Some(dest) = form.operand(side, at)?.tied {
                                return dests.get(dest).cloned();
                            }
                            let kind = choices[pick % choices.len()];
                            operand(kind, form, side, at, width, bits, pick)
                        }).collect()
                    };
                    let Some(dests) = build(Side::Dest, &dest_kinds, &[]) else { continue };
                    let Some(sources) = build(Side::Source, &source_kinds, &dests) else { continue };
                    variants.push((dests, sources));
                }
                for (dests, sources) in variants {
                    let what = Semantics { op: llrm_lir::Operation::named(&form.operation).unwrap(), name: Some(form.name.clone()), dests, sources, target: None, indirect: false };
                    let Some(emitted) = llrm_x86::select::emit_in(bits, &what, 0, None, false, false, None) else { continue };
                    let Some(want) = decoded(bits, &emitted.code) else { continue };
                    let Some(mut got) = effects(&forms, bits, &what) else { continue };
                    let (mut left, mut right) = (want.clone(), got.clone());
                    for side in [&mut left.reads, &mut left.writes, &mut right.reads, &mut right.writes] {
                        side.sort_by_key(|one| *one as u32);
                    }
                    got = right;
                    checked += 1;
                    if left != got {
                        wrong.push(format!("x86.instr:{} {} w{width} m{bits}: iced {:?}, row {:?}", form.line, form.name, left, got));
                    }
                }
            }
        }
    }
    }
    assert!(checked > 100, "only {checked} instances compared");
    wrong.sort();
    wrong.dedup();
    assert!(wrong.is_empty(), "{checked} compared, {} differ:\n{}", wrong.len(), wrong.join("\n"));
}
