//! The registers an instruction reads and writes, from its row in `x86.instr`:
//! its operands as `Semantics` has them, and the row's `reads` and `writes` for
//! the registers it uses without naming. LLVM's `MCInstrDesc` operand
//! defs and `ImplicitUses`/`ImplicitDefs`, with the flags the row's iced Code
//! reads and writes.

use iced_x86::Register;
use llrm_lir::{Loc, Operation, Reg, Semantics};

use crate::select;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Effects {
    pub reads: Vec<Register>,
    pub writes: Vec<Register>,
    /// Written only if the instruction runs: a repeated string operation's, but
    /// its count's.
    pub maybe_writes: Vec<Register>,
    /// `RflagsBits`, the writes including those left undefined.
    pub flags_read: u32,
    pub flags_written: u32,
}

/// What a row of `x86.instr` says of its instruction, as `build.rs` writes it
/// out.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub reads: &'static [&'static str],
    pub writes: &'static [&'static str],
    /// What each dest, then each source, may be: `r`, `m`, `i`, `a` or `s`, as
    /// `x86.instr` spells them.
    pub kinds: &'static [&'static str],
    /// The bits of the operation, where it has the one (`stosb`'s 8).
    pub width: u32,
    /// `(source, dest)`: the source is the dest's register, whatever the
    /// semantics name.
    pub ties: &'static [(usize, usize)],
    /// `(is a dest, operand, root)`: the operand is that register when it is
    /// one.
    pub pins: &'static [(bool, usize, &'static str)],
    pub flags_read: u32,
    pub flags_written: u32,
}

/// What the table says of an instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Served {
    Known(Effects),
    /// A transfer of control: its effects are the contract's.
    Unknown,
}

/// The register `name` is at the address size `bits`.
pub fn root(
    name: &str,
    bits: u32,
) -> Register {
    let wide = bits == 32;
    match (name, wide) {
        ("ax", true) => Register::EAX,
        ("bx", true) => Register::EBX,
        ("cx", true) => Register::ECX,
        ("dx", true) => Register::EDX,
        ("si", true) => Register::ESI,
        ("di", true) => Register::EDI,
        ("bp", true) => Register::EBP,
        ("sp", true) => Register::ESP,
        ("ax", _) => Register::AX,
        ("bx", _) => Register::BX,
        ("cx", _) => Register::CX,
        ("dx", _) => Register::DX,
        ("si", _) => Register::SI,
        ("di", _) => Register::DI,
        ("bp", _) => Register::BP,
        ("sp", _) => Register::SP,
        ("es", _) => Register::ES,
        ("ds", _) => Register::DS,
        ("fs", _) => Register::FS,
        ("ss", _) => Register::SS,
        ("ah", _) => Register::AH,
        _ => Register::GS,
    }
}

/// The registers `place` reads or writes as an operand: the register itself, or
/// the ones its address is encoded with.
fn used(
    places: &[Loc],
    bits: u32,
    reads: &mut Vec<Register>,
    mut writes: Option<&mut Vec<Register>>,
) -> bool {
    let mut add = |into: &mut Vec<Register>, register: Register| {
        if register != Register::None && !into.contains(&register) {
            into.push(register);
        }
    };
    for place in places {
        let address = match place {
            Loc::Reg(one) => {
                match writes.as_deref_mut() {
                    Some(into) => add(into, one.register),
                    None => add(reads, one.register),
                }
                continue;
            }
            Loc::Mem(one) => {
                let named = one.through != Register::None || one.index_through != Register::None || one.addr.is_some();
                match select::operand_of(one, bits) {
                    // An address that cannot be encoded (`[sp]` in real mode)
                    // is an instruction that cannot be.
                    None if named => return false,
                    // The string operations' operand names nothing: SI and DI
                    // do.
                    None => continue,
                    found => found,
                }
            }
            Loc::Address(one) => match select::address_operand(one, bits, bits / 8) {
                None => return false,
                found => found,
            },
            Loc::Imm(_) | Loc::St(_) | Loc::Held(_) => continue,
        };
        if let Some((operand, _)) = address {
            add(reads, operand.base);
            add(reads, operand.index);
            if matches!(place, Loc::Mem(_)) {
                // The segment it names, else the default: SS behind the stack's
                // registers, DS otherwise.
                let stack = [Register::BP, Register::EBP, Register::SP, Register::ESP].contains(&operand.base);
                add(
                    reads,
                    if operand.segment_prefix != Register::None {
                        operand.segment_prefix
                    } else if stack {
                        Register::SS
                    } else {
                        Register::DS
                    },
                );
            }
        }
    }
    true
}

/// The bytes `place` is, where it says.
fn bytes_of(place: &Loc) -> Option<u32> {
    match place {
        Loc::Reg(one) => Some(one.width),
        Loc::Mem(one) => Some(one.width),
        Loc::Imm(one) => Some(one.width),
        _ => None,
    }
}

/// The kind letter of an operand.
fn kind_of(place: &Loc) -> char {
    match place {
        Loc::Reg(_) | Loc::Held(_) => 'r',
        Loc::Mem(_) => 'm',
        Loc::Imm(_) => 'i',
        Loc::Address(_) => 'a',
        Loc::St(_) => 's',
    }
}

/// The form of `candidates` whose operands may be `what`'s, the one with fewest
/// ties (`imul cx, dx, 3` is not `imul cx, 3`); else the first (`lea` of a
/// memory operand).
fn form_of(
    candidates: &'static [Row],
    what: &Semantics,
) -> Option<Row> {
    let places: Vec<char> = what.dests.iter().chain(&what.sources).map(kind_of).collect();
    candidates
        .iter()
        .filter(|row| row.kinds.iter().zip(&places).all(|(allowed, kind)| allowed.contains(*kind)))
        .min_by_key(|row| row.ties.len())
        .or_else(|| candidates.first())
        .copied()
}

/// The register `root` names at `bytes`, as a pinned operand is.
fn pinned(
    root_name: &str,
    bytes: u32,
) -> Option<Register> {
    Some(match (bytes, root_name) {
        (_, "es") => Register::ES,
        (_, "ds") => Register::DS,
        (_, "fs") => Register::FS,
        (_, "gs") => Register::GS,
        (_, "ss") => Register::SS,
        (1, "ax" | "bx" | "cx" | "dx") => match root_name {
            "ax" => Register::AL,
            "bx" => Register::BL,
            "cx" => Register::CL,
            _ => Register::DL,
        },
        (2, _) => root(root_name, 16),
        (4, _) => root(root_name, 32),
        _ => return None,
    })
}

/// `places` as the instruction takes them: a source tied to a dest is that
/// dest, and an operand the row pins to a register is that register, whatever
/// the semantics name (the allocator and the encoder see to the rest).
fn resolved(
    row: &Row,
    is_dest: bool,
    places: &[Loc],
    dests: &[Loc],
) -> Vec<Loc> {
    places
        .iter()
        .enumerate()
        .map(|(at, place)| {
            if !is_dest && let Some((_, dest)) = row.ties.iter().find(|(source, _)| *source == at) {
                return dests.get(*dest).unwrap_or(place).clone();
            }
            match (place, row.pins.iter().find(|(dest, index, _)| *dest == is_dest && *index == at)) {
                // A segment register is where the semantics put it: the
                // selector is the allocator's choice.
                (Loc::Reg(one), Some((_, _, root_name))) if !["es", "ds", "fs", "gs", "ss"].contains(root_name) => {
                    pinned(root_name, one.width).map_or(place.clone(), |register| Loc::Reg(Reg { register, ..*one }))
                }
                _ => place.clone(),
            }
        })
        .collect()
}

/// What `what` reads and writes in `bits`-bit code, given the table's `row` for
/// a mnemonic and its operand counts; None where the table has no row.
pub fn effects(
    rows: &dyn Fn(&str, usize, usize) -> &'static [Row],
    bits: u32,
    what: &Semantics,
) -> Option<Served> {
    if matches!(
        what.op,
        Operation::Branch | Operation::Jump | Operation::Call | Operation::Return | Operation::Escape
    )
    {
        return Some(Served::Unknown);
    }
    // The instruction it is encoded as, where it is not the one it names.
    if let Some(steps) = select::lowered(what) {
        let mut all = Effects::default();
        for step in &steps {
            let Some(Served::Known(one)) = effects(rows, bits, step) else { return None };
            for (from, into) in
                [(one.reads, &mut all.reads), (one.writes, &mut all.writes), (one.maybe_writes, &mut all.maybe_writes)]
            {
                for register in from {
                    if !into.contains(&register) {
                        into.push(register);
                    }
                }
            }
            all.flags_read |= one.flags_read;
            all.flags_written |= one.flags_written;
        }
        return Some(Served::Known(all));
    }
    // The machine instruction is the mnemonic's, whatever operation LIR lowered
    // it as; a comparison with none is `cmp`.
    let name = match what.name.as_deref() {
        Some(name) if !name.is_empty() => name,
        _ if what.op == Operation::Compare => "cmp",
        _ => return None,
    };
    let row = form_of(rows(name, what.dests.len(), what.sources.len()), what)?;
    let width = what.sources.iter().filter_map(bytes_of).min().map_or(0, |bytes| bytes * 8);
    let dests = resolved(&row, true, &what.dests, &[]);
    let sources = resolved(&row, false, &what.sources, &dests);
    let mut found = Effects { flags_read: row.flags_read, flags_written: row.flags_written, ..Effects::default() };
    // A mixed-width operation is a different instruction, and `mov edi, sp` is
    // none.
    if row.reads.contains(&"same") {
        let mut sizes = dests
            .iter()
            .chain(&sources)
            .filter_map(|place| if let Loc::Reg(one) = place { Some(one.register.size()) } else { None });
        if let Some(first) = sizes.next()
            && sizes.any(|size| size != first)
        {
            return Some(Served::Unknown);
        }
    }
    // A move of a register to itself is no instruction.
    if row.reads.contains(&"self")
        && matches!(
            (dests.as_slice(), sources.as_slice()),
            ([Loc::Reg(dest)], [Loc::Reg(source)]) if dest.register == source.register
        )
    {
        return Some(Served::Known(Effects::default()));
    }
    // `xor ax, ax` reads nothing.
    let zeroing = row.reads.contains(&"idiom")
        && matches!(
            dests.as_slice(),
            [Loc::Reg(dest)] if sources.iter().all(|source| matches!(source, Loc::Reg(one) if one.register == dest.register))
        );
    if !zeroing {
        if !used(&sources, bits, &mut found.reads, None) {
            return Some(Served::Unknown);
        }
        // `lea ax, [eax+edx]` uses only what the destination is wide: the low
        // word of each.
        if row.reads.contains(&"narrow")
            && let [Loc::Reg(dest)] = dests.as_slice()
            && dest.width == 2
        {
            for register in &mut found.reads {
                if let Some(word) = crate::registers::word_of(*register) {
                    *register = word;
                }
            }
        }
    } else {
        // Nothing is read, but the address of a memory operand would be; there
        // is none.
    }
    // A shift by a count whose low five bits are none changes no flag.
    if row.writes.contains(&"count") && matches!(sources.last(), Some(Loc::Imm(count)) if count.value & 31 == 0) {
        (found.flags_read, found.flags_written) = (0, 0);
    }
    let mut writes = Vec::new();
    if !used(&dests, bits, &mut found.reads, Some(&mut writes)) {
        return Some(Served::Unknown);
    }
    found.writes = writes;
    for (list, into) in [(row.reads, &mut found.reads), (row.writes, &mut found.writes)] {
        for entry in list {
            if ["rep", "idiom", "count", "self", "narrow", "same"].contains(entry) {
                continue;
            }
            let (dropped, entry) = entry.strip_prefix('-').map_or((false, *entry), |rest| (true, rest));
            let (name, only) =
                entry.split_once('@').map_or((entry, None), |(name, width)| (name, width.parse::<u32>().ok()));
            if only.is_some_and(|only| only != width) {
                continue;
            }
            let register = match name.strip_suffix('*') {
                Some(name) => pinned(name, row.width / 8).unwrap_or_else(|| root(name, bits)),
                None => root(name, bits),
            };
            if dropped {
                into.retain(|one| one.full_register() != register.full_register());
            } else if !into.contains(&register) {
                into.push(register);
            }
        }
    }
    if row.writes.contains(&"rep") {
        let (count, rest) = found.writes.iter().partition(|one| one.full_register() == root("cx", 32).full_register());
        (found.writes, found.maybe_writes) = (count, rest);
    }
    Some(Served::Known(found))
}
