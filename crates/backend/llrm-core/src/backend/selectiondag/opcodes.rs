//! Which `x86.instr` form a pin machine opcode is.
//!
//! # How it works
//! The pin names an opcode `<MNEMONIC><width><operand letters>`: `ADD32rr`,
//! `ADD32mi8`, `MOVZX32rr8`. [`OpcodeMap::form`] reads the mnemonic, the
//! width and the letters (`r` register, `m` memory, `i` immediate; a width or
//! `CL`/`1` after them is a size or a fixed operand) and finds the form of
//! that name whose widths hold the width and whose destination and untied
//! sources admit the letters. A form's tied source (`^0`) has no letter: the
//! pin names it once, as the destination. Zero matches or more than one is
//! an [`Unmapped`] opcode, never a guess.
//!
//! ```text
//! ADD32mr  ->  add  rm/^0,rmi   dest m, untied source r
//! IMUL32rri ->  imul r/rm,i      (r/^0,rmi has one untied source, not two)
//! ```
//!
//! # Example
//! `ADD32rr` is the form `add` (`rm/^0,rmi`, 8/16/32); `ADD32ri8` is the same
//! form; `CMOV32rr` is unmapped, [`Unmapped::Condition`]: the pin has one
//! opcode where `x86.instr` expands `{cc}` into one form per condition.
//!
//! # Used by
//! The emitter (D1b), which builds LIR `Semantics` from the form.
//!
//! # See also
//! `llrm_instr::Form`; the pin's `X86GenInstrInfo.inc` for the names.

use llrm_instr::Form;

/// Why an opcode has no single form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unmapped {
    /// A target-independent opcode the emitter handles itself (`COPY`,
    /// `IMPLICIT_DEF`, `EXTRACT_SUBREG`, ...).
    Generic,
    /// An x87 pseudo (`ADD_Fp32m`, `LD_Fp64m`, ...): SD6, until `fpstack.rs`.
    X87,
    /// A vector or other opcode this port leaves out (SD13).
    Vector,
    /// A condition-coded opcode (`CMOV`, `SETCC`, `JCC`): one pin opcode, one
    /// form per condition.
    Condition,
    /// A 64-bit width: the targets here have no 64-bit registers.
    Wide64,
    /// The name has no `<MNEMONIC><width><letters>` shape.
    Name,
    /// No form of that name admits the width and operand letters.
    NoForm,
    /// More than one form does.
    Ambiguous(Vec<usize>),
}

/// `x86.instr`'s forms, by mnemonic.
pub struct OpcodeMap {
    forms: Vec<Form>,
}

impl OpcodeMap {
    pub fn new(forms: Vec<Form>) -> OpcodeMap {
        OpcodeMap { forms }
    }

    pub fn forms(&self) -> &[Form] {
        &self.forms
    }

    /// The index of the one form `name` is, or why it is none.
    pub fn form(
        &self,
        name: &str,
    ) -> Result<usize, Unmapped> {
        let name = name.split('|').next().unwrap_or(name);
        if matches!(
            name,
            "COPY"
                | "IMPLICIT_DEF"
                | "EXTRACT_SUBREG"
                | "INSERT_SUBREG"
                | "SUBREG_TO_REG"
                | "COPY_TO_REGCLASS"
                | "REG_SEQUENCE"
        ) {
            return Err(Unmapped::Generic);
        }
        if name.contains("_Fp") || name.starts_with('F') && name[1..].starts_with(|c: char| c.is_ascii_uppercase()) {
            return Err(Unmapped::X87);
        }
        if name.starts_with("V_")
            || name.starts_with('V') && name.contains("rr")
            || name.contains("MMX")
            || name.contains("PS")
            || name.contains("PD")
        {
            return Err(Unmapped::Vector);
        }
        let Some((mnemonic, width, letters)) = split(name) else { return Err(Unmapped::Name) };
        if matches!(mnemonic.as_str(), "cmov" | "set" | "j") || name.contains("CC") {
            return Err(Unmapped::Condition);
        }
        if width == Some(64) {
            return Err(Unmapped::Wide64);
        }
        let found: Vec<usize> = self
            .forms
            .iter()
            .enumerate()
            .filter(|(_, form)| {
                form.name == mnemonic && width.is_none_or(|w| form.widths.contains(&w)) && admits(form, &letters)
            })
            .map(|(at, _)| at)
            .collect();
        match found[..] {
            [] => Err(Unmapped::NoForm),
            [one] => Ok(one),
            _ => Err(Unmapped::Ambiguous(found)),
        }
    }
}

/// `ADD32ri8` -> (`add`, 32, `ri`); `SHL32rCL` -> (`shl`, 32, `rr`);
/// `MOVZX32rr8` -> (`movzx`, 32, `rr`).
fn split(name: &str) -> Option<(String, Option<u32>, Vec<char>)> {
    let upper = name.find(|c: char| !c.is_ascii_uppercase())?;
    let mnemonic = name[..upper].to_ascii_lowercase();
    let rest = &name[upper..];
    let digits = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    let width = (digits > 0).then(|| rest[..digits].parse().ok()).flatten();
    let mut letters = Vec::new();
    let mut tail = &rest[digits..];
    while let Some(c) = tail.chars().next() {
        match c {
            'r' | 'm' | 'i' => letters.push(c),
            'C' if tail.starts_with("CL") => {
                letters.push('r');
                tail = &tail[1..];
            }
            '1' => letters.push('i'),
            '0'..='9' => {}
            _ => return None,
        }
        tail = &tail[c.len_utf8()..];
    }
    (!letters.is_empty() || width.is_none()).then_some((mnemonic, width, letters))
}

/// Whether `form`'s destination and untied sources admit `letters` (the
/// destination first).
fn admits(
    form: &Form,
    letters: &[char],
) -> bool {
    let untied: Vec<_> = form.sources.iter().filter(|s| s.tied.is_none()).collect();
    if letters.len() != form.dests.len() + untied.len() {
        return false;
    }
    let (dests, sources) = letters.split_at(form.dests.len());
    dests.iter().zip(&form.dests).all(|(&l, d)| d.allows(l)) && sources.iter().zip(&untied).all(|(&l, s)| s.allows(l))
}
