//! The 6502's ca65 text as an ELF32 relocatable object, as `llvm-mc -triple=mos`
//! writes one: the container is `llrm-elf`'s; this is the machine number
//! (EM_MOS), the `R_MOS_*` types, and a small assembler for the text this
//! backend prints.
//!
//! # How it works
//! Lines are read once, in order. A `.segment` picks the section
//! (`CODE`→`.text`, `RODATA`, `DATA`, `BSS`); a label is a symbol at the
//! section's end; an instruction is its opcode ([`opcodes`]) and an operand
//! whose mode follows its shape (`#e`, `e`, `e,x`, `(e),y`...). A zero-page
//! operand (`rcN`) is `R_MOS_ADDR8` against `__rcN`, as the imaginary registers
//! are in llvm-mos; any other symbol is `R_MOS_ADDR16`, `#<(e)` and `#>(e)` the
//! low and high byte. A branch to a label of the section is resolved; its
//! range is the assembler's error, as in ca65 (no relaxation).
//!
//! # Example
//! `lda tab,x` is `BD 00 00` with `R_MOS_ADDR16` at offset 1 against `tab`;
//! `sta (rc2),y` is `91 00` with `R_MOS_ADDR8` against `__rc2`.
//!
//! # Complexity
//! O(lines + symbols): one pass, and a symbol table keyed by name.
//!
//! # See also
//! `llvm/lib/Target/MOS/MCTargetDesc/MOSELFObjectWriter.cpp`, llvm-mos.

mod opcodes;

use std::collections::HashMap;

use llrm_elf::{Machine, Relocation};
use llrm_object::assembler::AsmBackend;
use llrm_object::{
    Binding, Definition, Fix, FixupKind, FixupKindInfo, Object, Reloc, Role, Section, Symbol, Target, Unsupported,
};

/// `EM_MOS` (llvm-mos `ELF.h`).
const EM_MOS: u16 = 6502;
/// `EF_MOS_ARCH_6502 | EF_MOS_ARCH_6502_BCD`, the flags `llvm-mc` writes for
/// the base CPU.
const EF_MOS_6502: u32 = 0x3;

const R_MOS_IMM8: u32 = 1;
const R_MOS_ADDR8: u32 = 2;
const R_MOS_ADDR16: u32 = 3;
const R_MOS_ADDR16_LO: u32 = 4;
const R_MOS_ADDR16_HI: u32 = 5;
const R_MOS_PCREL_8: u32 = 6;

/// The target's fixup kinds.
pub mod fixup {
    use llrm_object::FixupKind;
    pub const IMM8: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0);
    pub const ADDR8: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 1);
    pub const ADDR16: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 2);
    pub const ADDR16_LO: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 3);
    pub const ADDR16_HI: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 4);
    pub const PCREL_8: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 5);
}

struct MosElf;

impl Machine for MosElf {
    const BITS: u8 = 16;
    const WIDE: bool = false;
    const NUMBER: u16 = EM_MOS;
    const ADDEND: bool = true;
    const FLAGS: u32 = EF_MOS_6502;

    fn backend() -> &'static dyn AsmBackend {
        &Backend
    }

    fn relocation(reloc: &Reloc) -> Result<Relocation, Unsupported> {
        let (kind, width) = match reloc.kind {
            fixup::IMM8 => (R_MOS_IMM8, 1),
            fixup::ADDR8 => (R_MOS_ADDR8, 1),
            fixup::ADDR16 => (R_MOS_ADDR16, 2),
            fixup::ADDR16_LO => (R_MOS_ADDR16_LO, 1),
            fixup::ADDR16_HI => (R_MOS_ADDR16_HI, 1),
            fixup::PCREL_8 => (R_MOS_PCREL_8, 1),
            other => return Err(Unsupported(format!("ELF mos has no relocation for {other:?}"))),
        };
        Ok(Relocation { kind, width })
    }
}

/// `MOSAsmBackend`: byte and word fields, little endian.
struct Backend;

impl AsmBackend for Backend {
    fn kind_info(
        &self,
        kind: FixupKind,
    ) -> FixupKindInfo {
        let (name, bits) = match kind {
            fixup::IMM8 => ("imm8", 8),
            fixup::ADDR8 => ("addr8", 8),
            fixup::ADDR16 => ("addr16", 16),
            fixup::ADDR16_LO => ("addr16_lo", 8),
            fixup::ADDR16_HI => ("addr16_hi", 8),
            fixup::PCREL_8 => ("pcrel8", 8),
            other => return other.info(),
        };
        FixupKindInfo { name, offset: 0, bits }
    }

    /// `nop` is one byte.
    fn write_nop_data(
        &self,
        out: &mut Vec<u8>,
        count: usize,
    ) -> bool {
        out.extend(std::iter::repeat_n(0xEA, count));
        true
    }
}

/// `object` as an ELF32 relocatable file.
pub fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    llrm_elf::write::<MosElf>(object)
}

/// An addressing mode, as [`opcodes::OPCODES`] lists them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Imp,
    Acc,
    Imm,
    Zp,
    ZpX,
    ZpY,
    Abs,
    AbsX,
    AbsY,
    Ind,
    IndX,
    IndY,
    ZpInd,
    AbsIndX,
    Rel,
}

fn opcode(
    mnemonic: &str,
    mode: Mode,
) -> Option<(u8, usize)> {
    opcodes::OPCODES
        .iter()
        .find(|(name, found, ..)| *name == mnemonic && *found == mode)
        .map(|(_, _, code, size, _)| (*code, usize::from(*size)))
}

/// Which part of its value an expression asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Part {
    Whole,
    Low,
    High,
}

/// `symbol + addend`, or a plain number.
#[derive(Clone, Debug, PartialEq)]
struct Expr {
    symbol: Option<String>,
    addend: i64,
    part: Part,
}

fn number(text: &str) -> Option<i64> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix('$') {
        i64::from_str_radix(hex, 16).ok()
    } else if let Some(bin) = text.strip_prefix('%') {
        i64::from_str_radix(bin, 2).ok()
    } else {
        text.parse().ok()
    }
}

/// `sym`, `sym+3`, `$82`, `<(e)`, `>(e)`, `<e`.
fn expr(
    text: &str,
    equates: &HashMap<String, i64>,
) -> Result<Expr, String> {
    let text = text.trim();
    for (prefix, part) in [('<', Part::Low), ('>', Part::High)] {
        if let Some(rest) = text.strip_prefix(prefix) {
            let rest = rest.trim();
            let inner = rest.strip_prefix('(').and_then(|one| one.strip_suffix(')')).unwrap_or(rest);
            return Ok(Expr { part, ..expr(inner, equates)? });
        }
    }
    let mut result = Expr { symbol: None, addend: 0, part: Part::Whole };
    let mut sign = 1;
    let mut term = String::new();
    let mut terms = Vec::new();
    for (index, c) in text.char_indices() {
        if (c == '+' || c == '-') && index > 0 {
            terms.push((sign, std::mem::take(&mut term)));
            sign = if c == '+' { 1 } else { -1 };
        } else if c == '-' {
            sign = -1;
        } else {
            term.push(c);
        }
    }
    terms.push((sign, term));
    for (sign, term) in terms {
        let term = term.trim();
        if let Some(value) = number(term).or_else(|| equates.get(term).copied()) {
            result.addend += sign * value;
        } else if term.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') && !term.is_empty() {
            if result.symbol.is_some() || sign < 0 {
                return Err(format!("an expression of two symbols: {text}"));
            }
            result.symbol = Some(term.to_owned());
        } else {
            return Err(format!("cannot read the expression `{text}`"));
        }
    }
    Ok(result)
}

/// `rcN` is `__rcN` in the object; other names are as written.
fn linked(symbol: &str) -> String {
    let imaginary = symbol.strip_prefix("rc").is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()));
    if imaginary { format!("__{symbol}") } else { symbol.to_owned() }
}

struct Piece {
    name: String,
    role: Role,
    image: Vec<u8>,
    relocs: Vec<(usize, FixupKind, String, i64)>,
}

/// The ca65 `text` of one module as an object.
pub fn object(
    name: &str,
    text: &str,
) -> Result<Object, String> {
    let mut equates: HashMap<String, i64> = HashMap::new();
    let mut zero_page: HashMap<String, ()> = HashMap::new();
    let mut sections: Vec<Piece> = Vec::new();
    let mut current: Option<usize> = None;
    let mut labels: Vec<(String, usize, usize)> = Vec::new();
    let mut exported: Vec<String> = Vec::new();

    let section_of = |sections: &mut Vec<Piece>, segment: &str| -> usize {
        let (spelled, role) = match segment {
            "CODE" => (".text", Role::Text),
            "RODATA" => (".rodata", Role::ROData),
            "BSS" => (".bss", Role::Bss),
            _ => (".data", Role::Data),
        };
        if let Some(at) = sections.iter().position(|one| one.name == spelled) {
            return at;
        }
        sections.push(Piece { name: spelled.to_owned(), role, image: Vec::new(), relocs: Vec::new() });
        sections.len() - 1
    };

    for (number_of_line, raw) in text.lines().enumerate() {
        let fail = |message: String| format!("line {}: {message}: {raw}", number_of_line + 1);
        let line = raw.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        // `name = value`
        if let Some((left, right)) = line.split_once('=') {
            let left = left.trim();
            if !left.is_empty() && left.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                let value = expr(right, &equates).map_err(&fail)?;
                if value.symbol.is_some() {
                    return Err(fail("an equate of a symbol".into()));
                }
                if (0..256).contains(&value.addend) {
                    zero_page.insert(left.to_owned(), ());
                }
                equates.insert(left.to_owned(), value.addend);
                continue;
            }
        }
        // `label:` alone or before an instruction
        let mut rest = line;
        while let Some((label, after)) = rest.split_once(':') {
            let label = label.trim();
            if label.is_empty() || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '@')
            {
                break;
            }
            let at = current.ok_or_else(|| fail("a label outside a segment".into()))?;
            labels.push((label.to_owned(), at, sections[at].image.len()));
            rest = after.trim();
        }
        if rest.is_empty() {
            continue;
        }
        if let Some(directive) = rest.strip_prefix('.') {
            let (word, args) = directive.split_once(char::is_whitespace).unwrap_or((directive, ""));
            let args = args.trim();
            match word {
                "segment" => current = Some(section_of(&mut sections, args.trim_matches('"'))),
                "export" | "globl" | "global" => exported.extend(args.split(',').map(|one| one.trim().to_owned())),
                "import" => {}
                "res" => {
                    let at = current.ok_or_else(|| fail("data outside a segment".into()))?;
                    let count = expr(args, &equates).map_err(&fail)?.addend as usize;
                    let fill = sections[at].image.len() + count;
                    sections[at].image.resize(fill, 0);
                }
                "byte" | "word" => {
                    let at = current.ok_or_else(|| fail("data outside a segment".into()))?;
                    for item in args.split(',') {
                        let item = item.trim();
                        if let Some(string) = item.strip_prefix('"').and_then(|one| one.strip_suffix('"')) {
                            sections[at].image.extend(string.bytes());
                            continue;
                        }
                        let value = expr(item, &equates).map_err(&fail)?;
                        let (kind, width) = match (word, value.part) {
                            ("word", _) => (fixup::ADDR16, 2),
                            (_, Part::Low) => (fixup::ADDR16_LO, 1),
                            (_, Part::High) => (fixup::ADDR16_HI, 1),
                            _ => (fixup::ADDR8, 1),
                        };
                        let place = sections[at].image.len();
                        match value.symbol {
                            Some(symbol) => {
                                sections[at].relocs.push((place, kind, linked(&symbol), value.addend));
                                sections[at].image.extend(std::iter::repeat_n(0, width));
                            }
                            None => sections[at].image.extend(&value.addend.to_le_bytes()[..width]),
                        }
                    }
                }
                other => return Err(fail(format!("the directive .{other} is not read"))),
            }
            continue;
        }
        // An instruction.
        let at = current.ok_or_else(|| fail("an instruction outside a segment".into()))?;
        let (mnemonic, operand) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let mnemonic = mnemonic.to_ascii_lowercase();
        let operand = operand.trim();
        let is_zero_page = |value: &Expr| -> bool {
            match &value.symbol {
                Some(symbol) => zero_page.contains_key(symbol.as_str()) || equates.contains_key(symbol.as_str()),
                None => (0..256).contains(&value.addend),
            }
        };
        let (mode, value): (Mode, Option<Expr>) = if operand.is_empty() {
            (Mode::Imp, None)
        } else if operand.eq_ignore_ascii_case("a") {
            (Mode::Acc, None)
        } else if let Some(imm) = operand.strip_prefix('#') {
            (Mode::Imm, Some(expr(imm, &equates).map_err(&fail)?))
        } else if let Some(inner) = operand.strip_prefix('(') {
            if let Some(index) = inner.strip_suffix(",x)") {
                (Mode::IndX, Some(expr(index, &equates).map_err(&fail)?))
            } else if let Some(index) = inner.strip_suffix("),y") {
                (Mode::IndY, Some(expr(index, &equates).map_err(&fail)?))
            } else if let Some(index) = inner.strip_suffix(')') {
                let value = expr(index, &equates).map_err(&fail)?;
                (if mnemonic == "jmp" { Mode::Ind } else { Mode::ZpInd }, Some(value))
            } else {
                return Err(fail("an indirect operand".into()));
            }
        } else if let Some(index) = operand.strip_suffix(",x") {
            let value = expr(index, &equates).map_err(&fail)?;
            (if is_zero_page(&value) && opcode(&mnemonic, Mode::ZpX).is_some() { Mode::ZpX } else { Mode::AbsX }, Some(value))
        } else if let Some(index) = operand.strip_suffix(",y") {
            let value = expr(index, &equates).map_err(&fail)?;
            (if is_zero_page(&value) && opcode(&mnemonic, Mode::ZpY).is_some() { Mode::ZpY } else { Mode::AbsY }, Some(value))
        } else {
            let value = expr(operand, &equates).map_err(&fail)?;
            let mode = if opcode(&mnemonic, Mode::Rel).is_some() {
                Mode::Rel
            } else if is_zero_page(&value) && opcode(&mnemonic, Mode::Zp).is_some() {
                Mode::Zp
            } else {
                Mode::Abs
            };
            (mode, Some(value))
        };
        let (code, size) = opcode(&mnemonic, mode).ok_or_else(|| fail(format!("no {mnemonic} in mode {mode:?}")))?;
        let start = sections[at].image.len();
        sections[at].image.push(code);
        if size > 1 {
            let value = value.expect("an operand");
            let (kind, width) = match (mode, value.part) {
                (Mode::Imm, Part::Low) => (fixup::ADDR16_LO, 1),
                (Mode::Imm, Part::High) => (fixup::ADDR16_HI, 1),
                (Mode::Imm, Part::Whole) => (fixup::IMM8, 1),
                (Mode::Rel, _) => (fixup::PCREL_8, 1),
                _ if size == 2 => (fixup::ADDR8, 1),
                _ => (fixup::ADDR16, 2),
            };
            let place = start + 1;
            match (&value.symbol, mode) {
                (Some(symbol), Mode::Rel) => {
                    // Resolved in the second pass, with the labels known.
                    sections[at].relocs.push((place, fixup::PCREL_8, symbol.clone(), value.addend));
                    sections[at].image.push(0);
                }
                (Some(symbol), _) => {
                    sections[at].relocs.push((place, kind, linked(symbol), value.addend));
                    sections[at].image.extend(std::iter::repeat_n(0, width));
                }
                (None, _) => sections[at].image.extend(&value.addend.to_le_bytes()[..width]),
            }
        }
    }

    // Symbols: the labels, then the names fixups use that are not labels.
    let mut symbols: Vec<Symbol> = Vec::new();
    let mut index_of: HashMap<String, usize> = HashMap::new();
    for (position, (label, at, offset)) in labels.iter().enumerate() {
        let end = labels
            .iter()
            .skip(position + 1)
            .find(|(_, other, _)| other == at)
            .map_or(sections[*at].image.len(), |(_, _, next)| *next);
        let public = exported.iter().any(|one| one == label);
        index_of.insert(label.clone(), symbols.len());
        symbols.push(Symbol {
            name: label.clone(),
            binding: if public { Binding::Public } else { Binding::Local },
            definition: Definition::Defined { section: *at, offset: *offset },
            size: if public { end - offset } else { 0 },
            group: None,
        });
    }
    let defined: HashMap<String, (usize, usize)> =
        labels.iter().map(|(label, at, offset)| (label.clone(), (*at, *offset))).collect();

    let mut out_sections = Vec::new();
    for (at, part) in sections.iter().enumerate() {
        let mut image = part.image.clone();
        let mut relocs = Vec::new();
        for (place, kind, symbol, addend) in &part.relocs {
            if *kind == fixup::PCREL_8 {
                if let Some(&(target_section, target)) = defined.get(symbol)
                    && target_section == at
                {
                    let displacement = target as i64 + addend - (*place as i64 + 1);
                    if !(-128..128).contains(&displacement) {
                        return Err(format!("{symbol}: a branch of {displacement} bytes is out of range"));
                    }
                    image[*place] = displacement as i8 as u8;
                    continue;
                }
            }
            let target = *index_of.entry(symbol.clone()).or_insert_with(|| {
                symbols.push(Symbol {
                    name: symbol.clone(),
                    binding: Binding::Public,
                    definition: Definition::Undefined,
                    size: 0,
                    group: None,
                });
                symbols.len() - 1
            });
            let fix = if *kind == fixup::PCREL_8 { Fix::relative(*kind, 0) } else { Fix::plain(*kind) };
            relocs.push(Reloc::new(*place, fix, Target::Symbol(target), *addend));
        }
        out_sections.push(Section {
            name: part.name.clone(),
            role: part.role,
            near: true,
            align: 1,
            spans: vec![[0, image.len()]],
            image,
            relocs,
        });
    }
    Ok(Object {
        name: name.to_owned(),
        bits: 16,
        sections: out_sections,
        symbols,
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        frames: Vec::new(),
    })
}

/// The ca65 `text` of one module as an ELF32 relocatable file.
pub fn relocatable(
    name: &str,
    text: &str,
) -> Result<Vec<u8>, String> {
    write(&object(name, text)?).map_err(|error| error.0)
}
