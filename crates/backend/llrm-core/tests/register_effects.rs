//! The register effects `x86.instr` states against the ones the encoder's bytes
//! decode to, in the lanes the passes see: every row of each target's
//! description, at each width.

use iced_x86::Register;
use iced_x86::{Decoder, DecoderOptions, FlowControl, InstructionInfoFactory, Mnemonic, OpAccess, OpKind};
use llrm_core::backend::lanes::Lanes;
use llrm_core::backend::peephole::{_effects_by_table, _flag_lanes, _lanes, _moved_by, _moved_lanes};
use llrm_core::backend::target::named as named_register;
use llrm_core::model::lir::Insn;
use llrm_lir::{Addr, AddressRef, Imm, Loc, Mem, Reg, Semantics, Space};
use llrm_target::Target;
use llrm_x86::effects::root;
use llrm_x86_m16::instructions::parse::{self, Form, Side};

/// The machine instructions `what` encodes to.
fn decoded(
    bits: u32,
    what: &Semantics,
) -> Option<Vec<iced_x86::Instruction>> {
    let encoded = llrm_core::backend::select::priced_in(bits, what, 0, None, false, false, None)?;
    Some((&mut Decoder::new(bits, &encoded.code, DecoderOptions::NONE)).into_iter().collect())
}

/// The accesses that read an operand.
const READS: [OpAccess; 4] = [OpAccess::Read, OpAccess::ReadWrite, OpAccess::CondRead, OpAccess::ReadCondWrite];

/// What the encoder's bytes read and write, in the lanes the passes see: the
/// effects iced states of the bytes, which `x86.instr` must state of the
/// instruction.
fn by_decoding(
    bits: u32,
    one: &Insn,
    what: &Semantics,
    may_write: bool,
    flags: bool,
) -> Option<(Lanes, Lanes)> {
    let instructions = decoded(bits, what)?;
    let mut reads: Lanes = one
        .requires
        .iter()
        .flat_map(|(held, register)| _lanes(named_register(*register, i64::from(held.width))))
        .collect();
    let mut writes: Lanes = one
        .delivers
        .iter()
        .flat_map(|(held, register)| _lanes(named_register(*register, i64::from(held.width))))
        .collect();
    let mut info = InstructionInfoFactory::new();
    for insn in &instructions {
        if insn.is_invalid() || insn.flow_control() != FlowControl::Next {
            return None;
        }
        if flags {
            let read: Lanes = _flag_lanes(insn.rflags_read()).minus(&writes);
            reads.extend(read);
            writes.extend(_flag_lanes(insn.rflags_modified()));
        }
        let used: Vec<(Register, OpAccess)> =
            info.info(insn).used_registers().iter().map(|access| (access.register(), access.access())).collect();
        for (register, access) in &used {
            if READS.contains(access) {
                let read: Lanes = _lanes(*register).minus(&writes);
                reads.extend(read);
            }
        }
        for (register, access) in &used {
            if [OpAccess::Write, OpAccess::ReadWrite].contains(access)
                || may_write && [OpAccess::CondWrite, OpAccess::ReadCondWrite].contains(access)
            {
                writes.extend(_lanes(*register));
            }
        }
        // `rep` counts its register down to where it stops, which iced calls a
        // conditional write.
        if insn.has_rep_prefix() || insn.has_repe_prefix() || insn.has_repne_prefix() {
            for (register, _) in used.iter().filter(|(register, _)| register.full_register32() == Register::ECX) {
                writes.extend(_lanes(*register));
            }
        }
    }
    Some((reads, writes))
}

/// The lanes a constant shift moves, from the decoded instruction.
fn moved_by_decoding(
    bits: u32,
    what: &Semantics,
) -> Option<(Vec<(llrm_core::backend::lanes::Lane, llrm_core::backend::lanes::Lane)>, Lanes)> {
    let instructions = decoded(bits, what)?;
    let [insn] = instructions.as_slice() else { return None };
    let register = |index: u32| (insn.op_kind(index) == OpKind::Register).then(|| insn.op_register(index));
    let shift = match insn.mnemonic() {
        Mnemonic::Shl | Mnemonic::Shr if insn.op_count() == 2 && insn.op1_kind() == OpKind::Immediate8 => {
            (insn.mnemonic() == Mnemonic::Shl, register(0), None, insn.immediate8())
        }
        Mnemonic::Shld | Mnemonic::Shrd if insn.op_count() == 3 && insn.op2_kind() == OpKind::Immediate8 => {
            (insn.mnemonic() == Mnemonic::Shld, register(0), register(1), insn.immediate8())
        }
        _ => return None,
    };
    _moved_by(shift)
}

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

/// The `at`th operand of `side` as kind `kind`, widths in bytes as `Semantics`
/// has them. A register `fixed` pins is the word when the operation is a byte
/// one (`idiv bl` divides AX), and the count of a shift is CL.
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
        // A string operation names its memory by SI and DI, which the row
        // lists.
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
            // A word `lea` in flat code takes a 16-bit address, behind a
            // prefix.
            let (base, index) = if bytes == 2 { (Register::BX, Register::SI) } else { (base, wide_index) };
            Some(Loc::Address(AddressRef {
                through: base,
                index_through: if pick % 2 == 0 { Register::None } else { index },
                scale: 1,
                ..AddressRef::new(None)
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
                // The selector emits no byte multiply (`select_sweep.rs` has
                // none), so no semantics of one to check
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
                                        // The allocator keeps a tied source in
                                        // the dest's register, but a pass
                                        // before
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
                        shifts += usize::from(moved_by_decoding(bits, &what).is_some());
                        let (moved, decoded) = (_moved_lanes(bits, &with), moved_by_decoding(bits, &what));
                        if moved != decoded && !(decoded.is_none() && refused_by_encoder(bits, &what)) {
                            wrong.push(format!(
                                "x86.instr:{} {} w{width} m{bits}: moved lanes differ: {what:?}",
                                form.line, form.name
                            ));
                        }
                        for (may_write, flags) in [(false, false), (false, true), (true, false), (true, true)] {
                            let by_decoding = by_decoding(bits, &one, &what, may_write, flags);
                            let Some(by_table) = _effects_by_table(bits, &one, &what, may_write, flags) else {
                                unanswered.push(format!("{} {}", form.operation, form.name));
                                continue;
                            };
                            // An instruction the encoder refuses has no bytes
                            // to decode; the table still says.
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

/// Instructions the encoder lowers to others (`push ss; pop es`, `cbw`, `push
/// eax; pop ax; pop dx`), which no row of `x86.instr` describes by its own
/// name.
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
                let Some(by_decoding) = by_decoding(bits, &one, what, may_write, flags) else { continue };
                let by_table = _effects_by_table(bits, &one, what, may_write, flags);
                assert_eq!(by_table, Some(Some(by_decoding)), "m{bits} {what:?}");
                checked += 1;
            }
        }
    }
    assert!(checked >= 16, "only {checked} compared");
}

/// A name the encoder emits that `x86.instr` has no row for is not known to the
/// passes, which then treat it as reading and writing anything.
/// `select_sweep.rs` holds what the encoder emits.
#[test]
fn every_name_the_encoder_emits_has_a_row() {
    let sweep = include_str!("../../../target/llrm-x86/src/select_sweep.rs");
    let mut emitted: Vec<(String, String)> = Vec::new();
    for line in sweep.lines().filter(|line| line.contains(", 0, false, false, None, None, Some((\"")) {
        let Some(rest) = line.trim_start().strip_prefix("check(sem(Op::") else { continue };
        let Some((op, rest)) = rest.split_once(", ") else { continue };
        let Some(name) = rest.strip_prefix("Some(\"").and_then(|rest| rest.split_once('"')).map(|(name, _)| name)
        else {
            continue;
        };
        if !name.is_empty() && !emitted.iter().any(|(was, one)| was == op && one == name) {
            emitted.push((op.to_owned(), name.to_owned()));
        }
    }
    assert!(emitted.len() > 40, "only {} names read from the sweep", emitted.len());
    // Some target's: `les` is real mode's.
    let forms: Vec<_> = [&llrm_x86_m16::M16 as &dyn Target, &llrm_x86_m32::M32]
        .iter()
        .flat_map(|target| parse::parse(&target.forms_text()).unwrap())
        .collect();
    let control = ["Branch", "Jump", "Call", "Return", "Escape", "Restore"];
    let missing: Vec<String> = emitted
        .iter()
        .filter(|(op, name)| !control.contains(&op.as_str()) && !forms.iter().any(|form| &form.name == name))
        .map(|(op, name)| format!("{op} {name}"))
        .collect();
    assert!(missing.is_empty(), "no row for {missing:?}");
}
