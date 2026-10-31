//! The assembly printer: `llc`'s text for one function, from the strings of
//! `WebAssemblyInstrInfo.td` (`NAME_S` records).
//!
//! # See also
//! `WebAssemblyInstPrinter.cpp` (`printWebAssemblyP2AlignOperand`), pin
//! d1106deb71cc.

use crate::asm_strings::find;
use crate::mir::{Binding, Function, Inst, Operand, ValType};
use crate::set_p2align::default_p2align;

/// The column a trailing comment starts at.
const COMMENT_COLUMN: usize = 40;

fn type_name(ty: ValType) -> &'static str {
    match ty {
        ValType::I32 => "i32",
        ValType::I64 => "i64",
        ValType::F32 => "f32",
        ValType::F64 => "f64",
    }
}

fn type_list(types: &[ValType]) -> String {
    types.iter().map(|t| type_name(*t)).collect::<Vec<_>>().join(", ")
}

/// `text` padded to the comment column, then `comment`.
fn with_comment(
    text: &str,
    comment: &str,
) -> String {
    format!("{text:<COMMENT_COLUMN$}{comment}")
}

/// One instruction after the passes: its stack-form string with the operands
/// filled in.
pub fn instruction(
    f: &Function,
    inst: &Inst,
) -> String {
    let asm = find(inst.opcode).unwrap_or_else(|| panic!("opcode {} has no assembly string", inst.opcode));
    let mut text = asm.text.to_string();
    if inst.opcode == crate::dagisel::post_ids::CALL_INDIRECT {
        // `printOperand` of the table, then the type as `(params) ->
        // (results)`; the table keeps a space.
        let mut immediates = inst.operands.iter().filter(|o| o.is_immediate());
        let (Some(&Operand::Imm(at)), Some(&Operand::Sym(table))) = (immediates.next(), immediates.next()) else {
            unreachable!("a call_indirect names its type and its table")
        };
        let (params, results) = &f.signatures[at as usize];
        return format!(
            "call_indirect\t {}, ({}) -> ({})",
            f.symbols[table as usize].0,
            type_list(params),
            type_list(results)
        );
    }
    if asm.names == ["brl"] {
        // `printBrList`: every target operand, the default last.
        let list: Vec<String> = inst.operands.iter().filter(|o| o.is_immediate()).map(operand_text).collect();
        return text.replace("$brl", &format!("{{{}}}", list.join(", ")));
    }
    for (name, operand) in asm.names.iter().zip(inst.operands.iter().filter(|o| o.is_immediate())) {
        let shown = match *operand {
            Operand::Imm(value) if *name == "p2align" => {
                if default_p2align(inst.opcode) == Some(value as u64) {
                    String::new()
                } else {
                    format!(":p2align={value}")
                }
            }
            Operand::Imm(value) if *name == "sig" => match value {
                0x40 => String::new(),
                0x7f => "i32".to_string(),
                0x7e => "i64".to_string(),
                0x7d => "f32".to_string(),
                0x7c => "f64".to_string(),
                other => panic!("block type {other:#x}"),
            },
            Operand::Imm(value) => value.to_string(),
            Operand::Sym(at) => {
                let (symbol, offset) = &f.symbols[at as usize];
                match offset {
                    0 => symbol.clone(),
                    offset if *offset > 0 => format!("{symbol}+{offset}"),
                    offset => format!("{symbol}{offset}"),
                }
            }
            Operand::Block(block) => format!(".LBB{block}"),
            Operand::Frame(_) => unreachable!("the frame lowering removes frame indices"),
            Operand::F32(bits) => hex_float(u64::from(bits), 23, 8),
            Operand::F64(bits) => hex_float(bits, 52, 11),
            Operand::Def(_) | Operand::Use(_) => unreachable!("filtered as non-immediate"),
        };
        text = text.replace(&format!("${{{name}}}"), &shown).replace(&format!("${name}"), &shown);
    }
    text
}

fn operand_text(operand: &Operand) -> String {
    match *operand {
        Operand::Imm(value) => value.to_string(),
        Operand::Block(block) => format!(".LBB{block}"),
        _ => unreachable!("a branch list holds depths and blocks"),
    }
}

/// The function as `llc` prints it, from `.section` to the end comment.
pub fn function(f: &Function) -> String {
    let name = &f.name;
    let begin = format!("# -- Begin function {name}");
    let mut out = vec![format!("\t.section\t.text.{name},\"\",@")];
    // The begin-function comment rides on the first directive: `.globl` or
    // `.weak` where there is one.
    match f.binding {
        Binding::Local => out.push(with_comment(&format!("\t.type\t{name},@function"), &begin)),
        Binding::Global | Binding::Weak => {
            let directive = if f.binding == Binding::Weak { "weak" } else { "globl" };
            out.push(with_comment(&format!("\t.{directive}\t{name}"), &begin));
            out.push(format!("\t.type\t{name},@function"));
        }
    }
    out.push(with_comment(&format!("{name}:"), &format!("# @{name}")));
    out.push(format!("\t.functype\t{name} ({}) -> ({})", type_list(&f.params), type_list(&f.results)));
    if !f.locals.is_empty() {
        out.push(format!("\t.local  \t{}", type_list(&f.locals)));
    }
    out.push("# %bb.0:".to_string());
    out.extend(f.blocks.iter().flatten().map(|i| format!("\t{}", instruction(f, i))));
    out.push(with_comment("", "# -- End function"));
    out.join("\n") + "\n"
}

/// `APFloat::convertToHexString` as `WebAssemblyInstPrinter` shows a float
/// immediate: `0x1.8p0`, no `+` on the exponent, trailing zero digits dropped.
/// `bits` hold `mantissa_bits` of fraction under `exponent_bits`.
fn hex_float(
    bits: u64,
    mantissa_bits: u32,
    exponent_bits: u32,
) -> String {
    let sign = if bits >> (mantissa_bits + exponent_bits) & 1 == 1 { "-" } else { "" };
    let mantissa = bits & ((1 << mantissa_bits) - 1);
    let exponent = (bits >> mantissa_bits) & ((1 << exponent_bits) - 1);
    let bias = (1i64 << (exponent_bits - 1)) - 1;
    if exponent == (1 << exponent_bits) - 1 {
        return if mantissa == 0 { format!("{sign}infinity") } else { format!("{sign}nan:0x{mantissa:x}") };
    }
    // The fraction as whole hex digits.
    let digits = mantissa_bits.div_ceil(4);
    let fraction = mantissa << (digits * 4 - mantissa_bits);
    let (lead, scale) = match (exponent, mantissa) {
        (0, 0) => return format!("{sign}0x0p0"),
        (0, _) => (0, 1 - bias),
        _ => (1, exponent as i64 - bias),
    };
    let text = format!("{fraction:0width$x}", width = digits as usize);
    let text = text.trim_end_matches('0');
    if text.is_empty() { format!("{sign}0x{lead}p{scale}") } else { format!("{sign}0x{lead}.{text}p{scale}") }
}
