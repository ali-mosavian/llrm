//! The module's defined variables as sdasz80 data, laid out by the shared
//! `llrm_codegen::backend::globals` (the pin's `emitGlobalConstant` split:
//! bytes, zero runs and relocated addresses).

use llrm_codegen::backend::globals;
use llrm_codegen::backend::masm::Datum;
use llrm_mir::module::GlobalKind;

use crate::driver::{Request, program};

/// The assembler symbol of the C name `name`: SDCC's underscore, and `$` and
/// the byte in hex for a character sdasz80 cannot spell (`.str.1`).
pub fn symbol(name: &str) -> String {
    let mut out = String::from("_");
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push_str(&format!("${:02x}", c as u32));
        }
    }
    out
}

/// Every defined variable with an initializer: constants in `_CONST`, zeros
/// in `_BSS`, the rest in `_DATA`.
pub fn text(
    ir: &str,
    request: &Request,
) -> Result<String, String> {
    text_at(ir, &crate::compile::effective(ir, request)?)
}

/// [`text`] at `request.level` as it stands (no `oz_best_of` choice).
pub(crate) fn text_at(
    ir: &str,
    request: &Request,
) -> Result<String, String> {
    let program = program(ir, request)?;
    let module = &program.modules[0];
    let spaces = crate::driver::machine().spaces();
    let names = globals::names(module, &spaces, &|name| symbol(name))?;
    let mut out = String::new();
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Variable(variable) = &global.kind else { continue };
        if variable.initializer.is_none() || global.name.as_deref().is_some_and(globals::is_compiler_only) {
            continue;
        }
        let id = llrm_mir::GlobalId(at as u32);
        let datums = globals::datums(module, id, &names)?;
        // a variable of zeros is bss: space the program's start clears, not
        // bytes of the image
        let bss = !variable.constant
            && datums.iter().all(|datum| match datum {
                Datum::Label(_) | Datum::Object(_) => true,
                Datum::Bytes(bytes) => bytes.iter().all(|byte| *byte == 0),
                Datum::Fill(fill) => fill.byte.unwrap_or(0) == 0,
                _ => false,
            });
        out.push_str(match (variable.constant, bss) {
            (true, _) => "\t.area _CONST\n",
            (false, true) => "\t.area _BSS\n",
            (false, false) => "\t.area _DATA\n",
        });
        for datum in datums {
            match datum {
                Datum::Bytes(bytes) if bss => out.push_str(&format!("\t.ds {}\n", bytes.len())),
                Datum::Fill(fill) if bss => out.push_str(&format!("\t.ds {}\n", fill.size)),
                Datum::Label(label) | Datum::Object(label) => {
                    out.push_str(&format!("\t.globl {0}\n{0}::\n", label.name));
                }
                Datum::Bytes(bytes) => {
                    for chunk in bytes.chunks(16) {
                        let list: Vec<String> = chunk.iter().map(u8::to_string).collect();
                        out.push_str(&format!("\t.db {}\n", list.join(",")));
                    }
                }
                Datum::Fill(fill) => match fill.byte {
                    None => out.push_str(&format!("\t.ds {}\n", fill.size)),
                    Some(byte) => (0..fill.size).for_each(|_| out.push_str(&format!("\t.db {byte}\n"))),
                },
                Datum::Pointer(pointer) if !pointer.far && matches!(pointer.bytes, 2 | 4) => {
                    out.push_str(&format!("\t.dw {}{}\n", pointer.name, offset(pointer.offset)));
                    if pointer.bytes == 4 {
                        out.push_str("\t.dw 0\n");
                    }
                }
                other => return Err(format!("a datum the Z80 cannot lay out: {other:?}")),
            }
        }
    }
    Ok(out)
}

fn offset(by: i64) -> String {
    if by == 0 { String::new() } else { format!("{by:+}") }
}

/// The bytes `text` puts in the linked image: `.db`, `.dw` and `.ds` outside
/// `_BSS` (the program's start clears that).
pub(crate) fn image_bytes(text: &str) -> u32 {
    let mut bss = false;
    let mut bytes = 0;
    for line in text.lines().map(str::trim) {
        if let Some(area) = line.strip_prefix(".area ") {
            bss = area.trim() == "_BSS";
        } else if let Some(list) = line.strip_prefix(".db ") {
            bytes += list.split(',').count() as u32;
        } else if line.starts_with(".dw ") {
            bytes += 2;
        } else if let Some(size) = line.strip_prefix(".ds ") {
            if !bss {
                bytes += size.trim().parse::<u32>().unwrap_or(0);
            }
        }
    }
    bytes
}
