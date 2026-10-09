//! The register effects `x86.instr` states against the ones the encoder's bytes decode to, in the lanes the passes see:
//! every row of each target's description, at each width.

use iced_x86::Register;
use llrm_core::backend::peephole::{_effects_by_decoding, _effects_by_table, _moved_lanes, _moved_lanes_by_decoding};
use llrm_core::model::lir::Insn;
use llrm_lir::{Addr, Address, Imm, Loc, Mem, Reg, Semantics, Space};
use llrm_target::Target;
use llrm_x86_m16::instructions::effects::root;
use llrm_x86_m16::instructions::parse::{self, Form, Side};

/// Whether the encoder has no bytes for `what`.
fn refused_by_encoder(
    bits: u32,
    what: &Semantics,
) -> bool {
    llrm_x86::select::emit_in(bits, what, 0, None, false, false, None).is_none()
}

fn byte(name: &str) -> Register {
    match name {
        "ax" => Register::AL,
        "bx" => Register::BL,
        "cx" => Register::CL,
        _ => Register::DL,
    }
}

/// The `at`th operand of `side` as kind `kind`, widths in bytes as `Semantics` has them. A register `fixed` pins is
/// the word when the operation is a byte one (`idiv bl` divides AX), and the count of a shift is CL.
fn operand(
    kind: char,
    form: &Form,
    side: Side,
    at: usize,
    width: u32,
    bits: u32,
    pick: usize,
) -> Option<Loc> {
    let pinned = form.fixed.iter().find(|(s, i, _)| *s == side && *i == at).map(|(_, _, register)| register.as_str());
    let order = if pick & 64 != 0 { ["ax", "cx", "dx", "bx"] } else { ["cx", "dx", "ax", "bx"] };
    let free: Vec<&str> = order.into_iter().filter(|name| form.fixed.iter().all(|(_, _, pin)| pin != name)).collect();
    let bytes = width / 8;
    let (base, wide_index) = if bits == 32 { (Register::EBX, Register::ESI) } else { (Register::BX, Register::SI) };
    match kind {
        'r' => {
            let (name, size) = match pinned {
                Some("cx") if form.cost == "shift_ri" => ("cx", 1),
                Some("ax") if form.operation == "fill" => ("ax", bytes),
                // The string operations count and step by the address size.
                Some(name) if matches!(form.operation.as_str(), "fill" | "copy") && name != "es" && name != "fs" => {
                    (name, bits / 8)
                }
                Some(name) => (name, bytes.max(2)),
                None => (free[(at + usize::from(side == Side::Source && pick & 32 == 0)) % free.len()], bytes),
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
            _ => Mem {
                addr: Some(Addr {
                    space: Space::Frame,
                    disp: -4,
                    index: 0,
                    base: Register::None,
                    segment: Register::None,
                }),
                through: Register::None,
                ..Mem::new(None, bytes)
            },
        })),
        'a' => {
            // A word `lea` in flat code takes a 16-bit address, behind a prefix.
            let (base, index) = if bytes == 2 { (Register::BX, Register::SI) } else { (base, wide_index) };
            Some(Loc::Address(Address {
                through: base,
                index: if pick % 2 == 0 { Register::None } else { index },
                scale: 1,
                ..Address::new(None)
            }))
        }
        'i' => Some(Loc::Imm(Imm {
            value: if pick & 32 != 0 { 0 } else { 3 },
            width: if form.cost == "shift_ri" { 1 } else { bytes },
            address: None,
        })),
        _ => None,
    }
}

#[test]
fn the_table_gives_the_effects_the_decoder_does() {
    let mut checked = 0;
    let mut refused = 0;
    let mut shifts = 0;
    let mut unanswered: Vec<String> = Vec::new();
    let mut wrong = Vec::new();
    for (target, bits) in [(&llrm_x86_m16::M16 as &dyn Target, 16), (&llrm_x86_m32::M32, 32)] {
        let forms = parse::parse(&target.forms_text()).unwrap();
        for form in &forms {
            let widths = if form.widths.is_empty() { vec![32] } else { form.widths.clone() };
            for &width in &widths {
                // The selector emits no byte multiply (`select_sweep.rs` has none), so no semantics of one to check
                // against.
                if width == 8 && form.operation == "mul" && form.dests.len() == 2 {
                    continue;
                }
                {
                    let kinds = |operands: &[parse::Operand]| -> Vec<Vec<char>> {
                        operands.iter().map(|one| one.kinds.chars().collect()).collect()
                    };
                    let (dest_kinds, source_kinds) = (kinds(&form.dests), kinds(&form.sources));
                    let mut variants: Vec<(Vec<Loc>, Vec<Loc>)> = Vec::new();
                    for pick in 0..128 {
                        let build = |side: Side, all: &[Vec<char>], dests: &[Loc]| -> Option<Vec<Loc>> {
                            all.iter()
                                .enumerate()
                                .map(|(at, choices)| {
                                    if let Some(dest) = form.operand(side, at)?.tied {
                                        // The allocator keeps a tied source in the dest's register, but a pass before
                                        // it may leave another.
                                        return match dests.get(dest) {
                                            Some(Loc::Reg(one)) if pick & 16 != 0 => Some(Loc::Reg(Reg {
                                                register: root(&"bx", if one.width == 4 { 32 } else { 16 }),
                                                ..*one
                                            }))
                                            .filter(|_| one.width > 1),
                                            other => other.cloned(),
                                        };
                                    }
                                    let kind = choices[(pick >> at) % choices.len()];
                                    operand(kind, form, side, at, width, bits, pick)
                                })
                                .collect()
                        };
                        let Some(dests) = build(Side::Dest, &dest_kinds, &[]) else { continue };
                        let Some(sources) = build(Side::Source, &source_kinds, &dests) else { continue };
                        variants.push((dests, sources));
                    }
                    for (dests, sources) in variants {
                        let what = Semantics {
                            op: llrm_lir::Operation::named(&form.operation).unwrap(),
                            name: Some(form.name.clone()),
                            dests,
                            sources,
                            target: None,
                            indirect: false,
                        };
                        let one = Insn::new(0, None, None, vec![], vec![]);
                        let with = Insn::new(0, None, Some(what.clone()), vec![], vec![]);
                        shifts += usize::from(_moved_lanes_by_decoding(bits, &with).is_some());
                        let (moved, decoded) = (_moved_lanes(bits, &with), _moved_lanes_by_decoding(bits, &with));
                        if moved != decoded && !(decoded.is_none() && refused_by_encoder(bits, &what)) {
                            wrong.push(format!(
                                "x86.instr:{} {} w{width} m{bits}: moved lanes differ: {what:?}",
                                form.line, form.name
                            ));
                        }
                        for (may_write, flags) in [(false, false), (false, true), (true, false), (true, true)] {
                            let by_decoding = _effects_by_decoding(bits, &one, &what, may_write, flags);
                            let Some(by_table) = _effects_by_table(bits, &one, &what, may_write, flags) else {
                                unanswered.push(format!("{} {}", form.operation, form.name));
                                continue;
                            };
                            // An instruction the encoder refuses has no bytes to decode; the table still says.
                            if by_decoding.is_none() && !by_table.is_none() {
                                refused += 1;
                                continue;
                            }
                            checked += 1;
                            if by_table != by_decoding {
                                wrong.push(format!(
                                    "x86.instr:{} {} w{width} m{bits} may_write={may_write} flags={flags}: {what:?}: decoded {by_decoding:?}, table {by_table:?}",
                                    form.line, form.name
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 100 && shifts > 10, "only {checked} instances and {shifts} constant shifts compared");
    wrong.sort();
    wrong.dedup();
    unanswered.sort();
    unanswered.dedup();
    assert!(unanswered.is_empty(), "no row for {unanswered:?}");
    assert!(
        wrong.is_empty(),
        "{checked} compared ({refused} the encoder refuses), {} differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

/// Instructions the encoder lowers to others (`push ss; pop es`, `cbw`, `push eax; pop ax; pop dx`), which no row of
/// `x86.instr` describes by its own name.
#[test]
fn the_table_follows_the_encoder_into_what_it_lowers_to() {
    let reg = |register, width| Loc::Reg(Reg { register, width });
    let imm = Loc::Imm(Imm {
        value: 0,
        width: 2,
        address: Some(Addr { space: Space::Group, disp: 0, index: 0, base: Register::None, segment: Register::None }),
    });
    let step = |op, name: &str, dests, sources| Semantics {
        op,
        name: Some(name.to_owned()),
        dests,
        sources,
        target: None,
        indirect: false,
    };
    let cases = [
        step(
            llrm_lir::Operation::Restore,
            "restore",
            vec![reg(Register::AX, 2), reg(Register::DX, 2)],
            vec![reg(Register::EAX, 4)],
        ),
        step(
            llrm_lir::Operation::Restore,
            "restore",
            vec![reg(Register::CX, 2), reg(Register::BX, 2)],
            vec![reg(Register::ECX, 4)],
        ),
        step(llrm_lir::Operation::Move, "mov", vec![reg(Register::ES, 2)], vec![imm]),
        step(llrm_lir::Operation::Move, "mov", vec![reg(Register::DS, 2)], vec![reg(Register::ES, 2)]),
        step(llrm_lir::Operation::Extend, "movzx", vec![reg(Register::AX, 2)], vec![reg(Register::AL, 1)]),
        step(llrm_lir::Operation::Extend, "movzx", vec![reg(Register::BX, 2)], vec![reg(Register::BL, 1)]),
        step(llrm_lir::Operation::Extend, "movsx", vec![reg(Register::AX, 2)], vec![reg(Register::AL, 1)]),
        step(llrm_lir::Operation::Extend, "movsx", vec![reg(Register::EAX, 4)], vec![reg(Register::AX, 2)]),
    ];
    let mut checked = 0;
    for bits in [16, 32] {
        for what in &cases {
            let one = Insn::new(0, None, None, vec![], vec![]);
            for (may_write, flags) in [(false, false), (false, true), (true, false), (true, true)] {
                let Some(by_decoding) = _effects_by_decoding(bits, &one, what, may_write, flags) else { continue };
                let by_table = _effects_by_table(bits, &one, what, may_write, flags);
                assert_eq!(by_table, Some(Some(by_decoding)), "m{bits} {what:?}");
                checked += 1;
            }
        }
    }
    assert!(checked >= 16, "only {checked} compared");
}
