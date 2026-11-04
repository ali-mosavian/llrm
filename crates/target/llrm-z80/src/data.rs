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

/// Every defined variable with an initializer: constants in `_CONST`, the
/// rest in `_DATA`.
pub fn text(
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
        out.push_str(if variable.constant { "\t.area _CONST\n" } else { "\t.area _DATA\n" });
        for datum in globals::datums(module, id, &names)? {
            match datum {
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
