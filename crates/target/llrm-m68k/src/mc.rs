//! m68k machine instructions and their assembly text: `M68kInstPrinter` over
//! the asm strings the pin's tables give ([`crate::dagisel::asm`]).
//!
//! DEVIATE: the printer methods of the byte, word and long forms are one each
//! (the pin's differ by type only), `printPCRelImm` takes labels, and
//! `printPCIBDMem`, alias printing (`printAliasInstr`) and `.W`/`.L` index
//! sizes are not translated. Reason: the slice does not reach them. Exit: M4b.
//!
//! # See also
//! `llvm/lib/Target/M68k/MCTargetDesc/M68kInstPrinter.cpp` and
//! `M68kMemOperandPrinter.h`, pin d1106deb71cc.

use llrm_lir::registers::RegId;

use crate::REGISTER_INFO;
use crate::dagisel::asm::asm;

/// One machine operand (`MCOperand`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operand {
    Reg(RegId),
    Imm(i64),
    /// A symbol and an addend (`MCSymbolRefExpr`), or a block's label.
    Symbol(String, i64),
}

/// A machine instruction: the pin's instruction by name, and its operands as
/// the pin's `MCInst` has them (outputs first, a memory operand taking
/// several).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inst {
    pub opcode: &'static str,
    pub operands: Vec<Operand>,
}

/// `getRegisterName`: the views of a register share its name.
fn register_name(id: RegId) -> String {
    let name = REGISTER_INFO.name(id).unwrap_or("?");
    match name {
        "wsp" => "sp".to_owned(),
        _ if name.len() == 3 && (name.starts_with("bd") || name.starts_with("wd")) => format!("d{}", &name[2..]),
        _ if name.len() == 3 && name.starts_with("wa") => format!("a{}", &name[2..]),
        _ => name.to_owned(),
    }
}

fn operand(
    inst: &Inst,
    at: usize,
) -> Result<&Operand, String> {
    inst.operands.get(at).ok_or_else(|| format!("{}: no operand {at}", inst.opcode))
}

/// `printOperand`.
fn print_operand(
    inst: &Inst,
    at: usize,
) -> Result<String, String> {
    Ok(match operand(inst, at)? {
        Operand::Reg(id) => format!("%{}", register_name(*id)),
        Operand::Imm(value) => format!("#{value}"),
        Operand::Symbol(name, addend) => symbol(name, *addend),
    })
}

fn symbol(
    name: &str,
    addend: i64,
) -> String {
    match addend {
        0 => name.to_owned(),
        _ => format!("{name}{addend:+}"),
    }
}

/// `printDisp`.
fn print_disp(
    inst: &Inst,
    at: usize,
) -> Result<String, String> {
    Ok(match operand(inst, at)? {
        Operand::Imm(value) => value.to_string(),
        Operand::Symbol(name, addend) => symbol(name, *addend),
        Operand::Reg(_) => return Err(format!("{}: a register for a displacement", inst.opcode)),
    })
}

/// `printMoveMask`: bits 0 to 7 are `d0` to `d7`, bits 8 to 15 `a0` to `a7`;
/// a run is `d1-d3`, the data and address parts joined by `/`.
fn print_move_mask(
    inst: &Inst,
    at: usize,
) -> Result<String, String> {
    let Operand::Imm(mask) = operand(inst, at)? else { return Err("a move mask is an immediate".to_owned()) };
    let name = |bit: usize| if bit < 8 { format!("%d{bit}") } else { format!("%a{}", bit - 8) };
    let mut parts = Vec::new();
    for half in [0, 8] {
        let bits = (*mask >> half) & 0xFF;
        let mut bit = 0;
        while bit < 8 {
            if bits >> bit & 1 == 0 {
                bit += 1;
                continue;
            }
            let mut end = bit;
            while end + 1 < 8 && bits >> (end + 1) & 1 == 1 {
                end += 1;
            }
            parts.push(if end == bit {
                name(half + bit)
            } else {
                format!("{}-{}", name(half + bit), name(half + end))
            });
            bit = end + 1;
        }
    }
    Ok(parts.join("/"))
}

fn print_method(
    method: &str,
    inst: &Inst,
    at: usize,
) -> Result<String, String> {
    // The type suffix (`8`, `16`, `32`) changes nothing in the text.
    let kind = method.trim_end_matches("Mem").trim_start_matches("print").trim_end_matches(char::is_numeric);
    Ok(match kind {
        "Operand" | "Immediate" => print_operand(inst, at)?,
        "PCRelImm" => print_disp(inst, at)?,
        "MoveMask" => print_move_mask(inst, at)?,
        "ARI" => format!("({})", print_operand(inst, at)?),
        "ARIPI" => format!("({})+", print_operand(inst, at)?),
        "ARIPD" => format!("-({})", print_operand(inst, at)?),
        "ARID" => format!("({},{})", print_disp(inst, at)?, print_operand(inst, at + 1)?),
        "ARII" => {
            format!("({},{},{})", print_disp(inst, at)?, print_operand(inst, at + 1)?, print_operand(inst, at + 2)?)
        }
        "AL" => match operand(inst, at)? {
            Operand::Symbol(name, addend) => symbol(name, *addend),
            Operand::Imm(value) => format!("${value:x}"),
            Operand::Reg(_) => return Err(format!("{}: a register for an absolute address", inst.opcode)),
        },
        "PCD" => format!("({},%pc)", print_disp(inst, at)?),
        "PCI" => format!("({},%pc,{})", print_disp(inst, at)?, print_operand(inst, at + 1)?),
        _ => return Err(format!("{}: printer {method} is not translated", inst.opcode)),
    })
}

/// The assembly text of `inst`, as `llc` prints it (without indentation).
pub fn print(inst: &Inst) -> Result<String, String> {
    let (_, text, operands) = asm(inst.opcode).ok_or_else(|| format!("no instruction {}", inst.opcode))?;
    let mut out = String::new();
    let mut rest = *text;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
        let name = &rest[..end];
        let one = operands.iter().find(|one| one.name == name).ok_or_else(|| format!("{}: no ${name}", inst.opcode))?;
        out.push_str(&print_method(one.method, inst, usize::from(one.at))?);
        rest = &rest[end..];
    }
    out.push_str(rest);
    Ok(out)
}
