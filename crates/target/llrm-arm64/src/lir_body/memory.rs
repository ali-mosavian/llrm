//! Loads, stores and their pairs, atomics, addresses of symbols, hints and
//! barriers.

use super::integer::EXTENDS;
use super::*;
use crate::mc::inst::Arrangement;
use crate::mc::inst::{Addr, Atomic, Barrier, Memory};

const ATOMICS: [(Atomic, &str); 13] = [
    (Atomic::Ldar, "LDAR"),
    (Atomic::Ldapr, "LDAPR"),
    (Atomic::Stlr, "STLR"),
    (Atomic::Swp, "SWP"),
    (Atomic::Ldadd, "LDADD"),
    (Atomic::Ldclr, "LDCLR"),
    (Atomic::Ldeor, "LDEOR"),
    (Atomic::Ldset, "LDSET"),
    (Atomic::Ldsmax, "LDSMAX"),
    (Atomic::Ldsmin, "LDSMIN"),
    (Atomic::Ldumax, "LDUMAX"),
    (Atomic::Ldumin, "LDUMIN"),
    (Atomic::Cas, "CAS"),
];

const BARRIERS: [Barrier; 3] = [Barrier::Ish, Barrier::IshLd, Barrier::IshSt];

/// A general register as a memory operand's base or index: a machine register
/// is the cell's `through`, a virtual one its `Held` value.
fn place(register: Gpr) -> (RegId, Option<Held>) {
    match lir_register(Reg::Gpr(register)) {
        MfReg::Phys(register) => (register, None),
        MfReg::Virt(value) => (RegId::None, Some(Held { value, width: 8, offset: 0 })),
    }
}

fn unplace(
    through: RegId,
    held: Option<Held>,
) -> Gpr {
    gpr_of(&match held {
        Some(held) => Loc::Held(held),
        None => Loc::Reg(LirRegLoc { register: through, width: 8 }),
    })
}

/// The cell an address names, the suffix of the opcode name that says its
/// form, and the base register a pre/post form writes back.
fn cell(
    cx: &mut To,
    addr: &Addr,
) -> Result<(Mem, String, Option<Gpr>), Unmapped> {
    let mut cell = Mem::new(None, 64);
    let base_of = |cell: &mut Mem, base: Gpr| {
        (cell.through, cell.base) = place(base);
    };
    Ok(match addr {
        Addr::Offset { base, imm } => {
            base_of(&mut cell, *base);
            cell.offset = *imm;
            (cell, "ui".into(), None)
        }
        Addr::Pre { base, imm } => {
            base_of(&mut cell, *base);
            cell.offset = *imm;
            (cell, "pre".into(), Some(*base))
        }
        Addr::Post { base, imm } => {
            base_of(&mut cell, *base);
            cell.offset = *imm;
            (cell, "post".into(), Some(*base))
        }
        Addr::Register { base, index, extend, shifted } => {
            base_of(&mut cell, *base);
            (cell.index_through, cell.index) = place(*index);
            (cell, format!("ro{}{}", position(&EXTENDS, *extend), u8::from(*shifted)), None)
        }
        Addr::Slot { slot, offset } => {
            cell.addr = Some(LirAddr::new(Space::Frame, *offset).in_slot(i64::from(*slot)));
            (cell, "fi".into(), None)
        }
        Addr::PageOffset { base, target, got } => {
            base_of(&mut cell, *base);
            cell.addr = cx.address(target)?.addr;
            (cell, if *got { "gotpg" } else { "pg" }.into(), None)
        }
    })
}

fn uncell(
    cx: &Back,
    cell: &Mem,
    suffix: &str,
) -> Addr {
    let base = || unplace(cell.through, cell.base);
    match suffix {
        "ui" => Addr::Offset { base: base(), imm: cell.offset },
        "pre" => Addr::Pre { base: base(), imm: cell.offset },
        "post" => Addr::Post { base: base(), imm: cell.offset },
        "fi" => {
            let at = cell.addr.expect("a frame cell");
            Addr::Slot { slot: at.slot_home().expect("a slot") as u32, offset: at.disp }
        }
        "pg" | "gotpg" => {
            Addr::PageOffset { base: base(), target: cx.target(&AddressRef::new(cell.addr)), got: suffix == "gotpg" }
        }
        ro => {
            let digits = ro.strip_prefix("ro").expect("a register form").as_bytes();
            Addr::Register {
                base: base(),
                index: unplace(cell.index_through, cell.index),
                extend: EXTENDS[usize::from(digits[0] - b'0')],
                shifted: digits[1] == b'1',
            }
        }
    }
}

/// The access: `S` sign-extending, `B`/`H`/`W`/`X` the bytes of an integer,
/// `F1`..`F16` of a float register.
fn kind_name(memory: &Memory) -> String {
    match memory {
        Memory::Int { bytes, sign, .. } => format!(
            "{}{}",
            if *sign { "S" } else { "" },
            match bytes {
                1 => "B",
                2 => "H",
                4 => "W",
                _ => "X",
            }
        ),
        Memory::Fp { bytes, .. } => format!("F{bytes}"),
    }
}

fn memory_loc(memory: &Memory) -> Loc {
    match memory {
        Memory::Int { width, reg, .. } => gpr_loc(*reg, *width),
        Memory::Fp { bytes, reg } => vreg_loc(*reg, u32::from(*bytes) * 8),
    }
}

/// The register a load or store moves, rebuilt from its name and operand.
fn memory_of(
    kind: &str,
    place: &Loc,
) -> Memory {
    if let Some(bytes) = kind.strip_prefix('F') {
        return Memory::Fp { bytes: bytes.parse().expect("float bytes"), reg: vreg_of(place) };
    }
    let sign = kind.starts_with('S');
    let bytes = match kind.trim_start_matches('S') {
        "B" => 1,
        "H" => 2,
        "W" => 4,
        _ => 8,
    };
    Memory::Int { width: if bits_of(place) == 64 { Width::X } else { Width::W }, bytes, sign, reg: gpr_of(place) }
}

fn atomic_name(
    op: Atomic,
    size: u8,
    acquire: bool,
    release: bool,
) -> String {
    let (_, mnemonic) = ATOMICS.iter().find(|(each, _)| *each == op).expect("a listed atomic");
    format!("{mnemonic}.{size}{}{}", if acquire { 'a' } else { '-' }, if release { 'l' } else { '-' })
}

pub(super) fn to(
    cx: &mut To,
    inst: &Inst,
) -> Result<Option<Semantics>, Unmapped> {
    Ok(Some(match inst {
        Inst::Load { mem, addr, .. } => {
            let (cell, suffix, written) = cell(cx, addr)?;
            let mut dests = vec![memory_loc(mem)];
            dests.extend(written.map(|base| gpr_loc(base, Width::X)));
            semantics(Operation::Move, format!("LDR{}.{suffix}", kind_name(mem)), dests, vec![Loc::Mem(cell)], None)
        }
        Inst::Store { mem, addr, .. } => {
            let (cell, suffix, written) = cell(cx, addr)?;
            let dests = written.map(|base| gpr_loc(base, Width::X)).into_iter().collect();
            semantics(
                Operation::Move,
                format!("STR{}.{suffix}", kind_name(mem)),
                dests,
                vec![memory_loc(mem), Loc::Mem(cell)],
                None,
            )
        }
        Inst::LoadPair { first, second, addr } => {
            let (cell, suffix, written) = cell(cx, addr)?;
            let mut dests = vec![memory_loc(first), memory_loc(second)];
            dests.extend(written.map(|base| gpr_loc(base, Width::X)));
            semantics(
                Operation::LoadPair,
                format!("LDP{}.{suffix}", kind_name(first)),
                dests,
                vec![Loc::Mem(cell)],
                None,
            )
        }
        Inst::StorePair { first, second, addr } => {
            let (cell, suffix, written) = cell(cx, addr)?;
            let dests = written.map(|base| gpr_loc(base, Width::X)).into_iter().collect();
            semantics(
                Operation::StorePair,
                format!("STP{}.{suffix}", kind_name(first)),
                dests,
                vec![memory_loc(first), memory_loc(second), Loc::Mem(cell)],
                None,
            )
        }
        Inst::Atomic { op, size, acquire, release, rs, rt, rn } => {
            let mut at = Mem::new(None, 8 << *size);
            (at.through, at.base) = place(*rn);
            let (rs, rt) = (gpr_loc(*rs, Width::X), gpr_loc(*rt, Width::X));
            // The operands and roles are `Inst::visit`'s: a load writes `rt`, a
            // store reads it, `cas` reads and writes `rs`, the rest read `rs`
            // and write `rt`.
            let (dests, mut sources) = match op {
                Atomic::Ldar | Atomic::Ldapr => (vec![rt], vec![]),
                Atomic::Stlr => (vec![], vec![rt]),
                Atomic::Cas => (vec![rs.clone()], vec![rs, rt]),
                _ => (vec![rt], vec![rs]),
            };
            sources.push(Loc::Mem(at));
            let operation = match op {
                Atomic::Ldar | Atomic::Ldapr | Atomic::Stlr => Operation::Move,
                _ => Operation::Exchange,
            };
            semantics(operation, atomic_name(*op, *size, *acquire, *release), dests, sources, None)
        }
        Inst::VectorStoreLane { lanes, rt, index, base } => {
            let mut at = Mem::new(
                None,
                match lanes {
                    Arrangement::B16 => 8,
                    Arrangement::H8 => 16,
                    Arrangement::S4 => 32,
                    Arrangement::D2 => 64,
                },
            );
            (at.through, at.base) = place(*base);
            let name = format!(
                "ST1{}",
                match lanes {
                    Arrangement::B16 => "i8",
                    Arrangement::H8 => "i16",
                    Arrangement::S4 => "i32",
                    Arrangement::D2 => "i64",
                }
            );
            semantics(
                Operation::Move,
                name,
                vec![],
                vec![vreg_loc(*rt, 128), imm(i64::from(*index)), Loc::Mem(at)],
                None,
            )
        }
        Inst::Adr { page, rd, target, got } => semantics(
            Operation::Address,
            format!("{}{}", if *page { "ADRP" } else { "ADR" }, if *got { ".got" } else { "" }),
            vec![gpr_loc(*rd, Width::X)],
            vec![Loc::Address(cx.address(target)?)],
            None,
        ),
        Inst::Prefetch { op, base } => {
            semantics(Operation::Nothing, "PRFM", vec![], vec![imm(i64::from(*op)), gpr_loc(*base, Width::X)], None)
        }
        Inst::Dmb { barrier } => {
            semantics(Operation::Barrier, "DMB", vec![], vec![imm(position(&BARRIERS, *barrier))], None)
        }
        _ => return Ok(None),
    }))
}

/// A cell's `FrameAddress`: `ADDXri` of a frame object, once the frame is laid
/// out.
pub(super) fn frame_address(
    dst: Gpr,
    slot: u32,
    offset: i64,
) -> Semantics {
    let at = LirAddr::new(Space::Frame, offset).in_slot(i64::from(slot));
    semantics(
        Operation::Address,
        "ADDXri.fi",
        vec![gpr_loc(dst, Width::X)],
        vec![Loc::Address(AddressRef::new(Some(at)))],
        None,
    )
}

pub(super) fn back(
    cx: &Back,
    what: &Semantics,
) -> Option<Op> {
    let name = what.name.as_deref()?;
    let (dest, s) = (what.dests.first(), &what.sources);
    if name == "ADDXri.fi" {
        let Loc::Address(address) = &s[0] else { return None };
        let at = address.addr?;
        return Some(Op::FrameAddress { dst: gpr_of(dest?), slot: at.slot_home()? as u32, offset: at.disp });
    }
    let (head, suffix) = name.split_once('.').map_or((name, ""), |(head, suffix)| (head, suffix));
    if let Some(&(op, _)) = ATOMICS.iter().find(|(_, each)| *each == head) {
        let Some(Loc::Mem(at)) = s.last() else { return None };
        let flags = suffix.as_bytes();
        // `rs` of a load-acquire or store-release is no operand: always `xzr`.
        let (rs, rt) = match op {
            Atomic::Ldar | Atomic::Ldapr => (Gpr::R31, gpr_of(dest?)),
            Atomic::Stlr => (Gpr::R31, gpr_of(&s[0])),
            Atomic::Cas => (gpr_of(&s[0]), gpr_of(&s[1])),
            _ => (gpr_of(&s[0]), gpr_of(dest?)),
        };
        return Some(Op::I(Inst::Atomic {
            op,
            size: flags[0] - b'0',
            acquire: flags[1] == b'a',
            release: flags[2] == b'l',
            rs,
            rt,
            rn: unplace(at.through, at.base),
        }));
    }
    if let Some(lanes) = [
        ("ST1i8", Arrangement::B16),
        ("ST1i16", Arrangement::H8),
        ("ST1i32", Arrangement::S4),
        ("ST1i64", Arrangement::D2),
    ]
    .into_iter()
    .find_map(|(each, lanes)| (each == head).then_some(lanes))
    {
        let Some(Loc::Mem(at)) = s.last() else { return None };
        return Some(Op::I(Inst::VectorStoreLane {
            lanes,
            rt: vreg_of(&s[0]),
            index: value_of(&s[1]) as u8,
            base: unplace(at.through, at.base),
        }));
    }
    Some(Op::I(match head {
        "ADRP" | "ADR" => {
            let Loc::Address(address) = &s[0] else { return None };
            Inst::Adr { page: head == "ADRP", rd: gpr_of(dest?), target: cx.target(address), got: suffix == "got" }
        }
        "PRFM" => Inst::Prefetch { op: value_of(&s[0]) as u8, base: gpr_of(&s[1]) },
        "DMB" => Inst::Dmb { barrier: member(&BARRIERS, &s[0]) },
        _ if head.starts_with("LDR") || head.starts_with("STR") => {
            let load = head.starts_with("LDR");
            let Some(Loc::Mem(at)) = s.last() else { return None };
            let place = if load { dest? } else { &s[0] };
            let (mem, addr, mmo) = (memory_of(&head[3..], place), uncell(cx, at, suffix), cx.mmo?.clone());
            if load { Inst::Load { mem, addr, mmo } } else { Inst::Store { mem, addr, mmo } }
        }
        _ if head.starts_with("LDP") || head.starts_with("STP") => {
            let load = head.starts_with("LDP");
            let Some(Loc::Mem(at)) = s.last() else { return None };
            let (one, two) = if load { (what.dests.first()?, what.dests.get(1)?) } else { (&s[0], &s[1]) };
            let (first, second) = (memory_of(&head[3..], one), memory_of(&head[3..], two));
            let addr = uncell(cx, at, suffix);
            if load { Inst::LoadPair { first, second, addr } } else { Inst::StorePair { first, second, addr } }
        }
        _ => return None,
    }))
}
