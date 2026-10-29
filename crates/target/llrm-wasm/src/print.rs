//! The assembly printer: `llc`'s text for one function, from the strings of
//! `WebAssemblyInstrInfo.td` (`NAME_S` records).
//!
//! DEVIATE: operands print as decimal integers, except `p2align`, which is
//! omitted at its default; floating-point immediates, symbols and block
//! labels are not printed yet. Reason: the slice has none. Exit: calls and
//! branches (W9).
//!
//! # See also
//! `WebAssemblyInstPrinter.cpp` (`printWebAssemblyP2AlignOperand`), pin
//! d1106deb71cc.

use crate::asm_strings::find;
use crate::mir::{Function, Inst, Operand, ValType};
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
pub fn instruction(inst: &Inst) -> String {
    let asm = find(inst.opcode).unwrap_or_else(|| panic!("opcode {} has no assembly string", inst.opcode));
    let immediates = inst.operands.iter().filter_map(|o| if let Operand::Imm(v) = o { Some(*v) } else { None });
    let mut text = asm.text.to_string();
    for (name, value) in asm.names.iter().zip(immediates) {
        let shown = if *name == "p2align" {
            if default_p2align(inst.opcode) == Some(value as u64) { String::new() } else { format!(":p2align={value}") }
        } else {
            value.to_string()
        };
        text = text.replace(&format!("${{{name}}}"), &shown).replace(&format!("${name}"), &shown);
    }
    text
}

/// The function as `llc` prints it, from `.section` to the end comment.
pub fn function(f: &Function) -> String {
    let name = &f.name;
    let mut out = vec![
        format!("\t.section\t.text.{name},\"\",@"),
        with_comment(&format!("\t.globl\t{name}"), &format!("# -- Begin function {name}")),
        format!("\t.type\t{name},@function"),
        with_comment(&format!("{name}:"), &format!("# @{name}")),
        format!("\t.functype\t{name} ({}) -> ({})", type_list(&f.params), type_list(&f.results)),
    ];
    if !f.locals.is_empty() {
        out.push(format!("\t.local  \t{}", type_list(&f.locals)));
    }
    out.push("# %bb.0:".to_string());
    out.extend(f.blocks[0].iter().map(|i| format!("\t{}", instruction(i))));
    out.push(with_comment("", "# -- End function"));
    out.join("\n") + "\n"
}
