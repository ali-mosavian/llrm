//! A module's global variables as assembly: the section each goes in, its
//! label, its bytes and the pointers in them. The bytes and pointers are
//! `llrm_mir::image`'s, shared with the other targets.

use std::fmt::Write;

use llrm_mir::Module;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::image::{Export, Variable, variables};

/// The variables `module` defines, as assembly text. Empty if it defines none.
pub fn assembly(
    module: &Module,
    layout: &DataLayout,
) -> Result<String, String> {
    let symbol_of = |name: &str| symbol(name);
    let mut out = String::new();
    for variable in variables(module, layout, &symbol_of)? {
        if matches!(
            variable.name.as_str(),
            "llvm.global_ctors" | "llvm.global_dtors"
        ) {
            return Err(format!("{}: constructors and destructors are not supported", variable.name));
        }
        object(&mut out, &variable);
    }
    Ok(out)
}

/// `\01` marks a name to be used as written.
pub(crate) fn symbol(name: &str) -> String {
    name.strip_prefix('\u{1}').unwrap_or(name).to_owned()
}

fn section(variable: &Variable) -> &'static str {
    if variable.zero && !variable.constant {
        ".bss"
    } else if variable.constant {
        ".rodata"
    } else {
        ".data"
    }
}

fn object(
    out: &mut String,
    variable: &Variable,
) {
    let name = symbol(&variable.name);
    let _ = writeln!(out, "\t.section\t{}", section(variable));
    let _ = writeln!(out, "\t.balign\t{}", variable.align);
    match variable.export {
        Export::Public => {
            let _ = writeln!(out, "\t.globl\t{name}");
        }
        Export::Weak => {
            let _ = writeln!(out, "\t.weak\t{name}");
        }
        Export::Local => {}
    }
    let _ = writeln!(out, "{name}:");
    if section(variable) == ".bss" {
        let _ = writeln!(out, "\t.skip\t{}", variable.bytes.len().max(1));
        return;
    }
    let mut relocs: Vec<_> = variable.relocs.iter().collect();
    relocs.sort_by_key(|reloc| reloc.at);
    let mut at = 0;
    for reloc in relocs {
        bytes(out, &variable.bytes[at..reloc.at]);
        match reloc.addend {
            0 => {
                let _ = writeln!(out, "\t.long\t{}", reloc.symbol);
            }
            addend => {
                let _ = writeln!(out, "\t.long\t{}{addend:+}", reloc.symbol);
            }
        }
        at = reloc.at + 4;
    }
    bytes(out, &variable.bytes[at..]);
}

/// `.byte` lines, sixteen to a line; a run of zeros longer than that is a
/// `.skip`.
fn bytes(
    out: &mut String,
    mut rest: &[u8],
) {
    while !rest.is_empty() {
        let zeros = rest.iter().take_while(|&&byte| byte == 0).count();
        if zeros >= 16 {
            let _ = writeln!(out, "\t.skip\t{zeros}");
            rest = &rest[zeros..];
            continue;
        }
        let line = rest.len().min(16);
        let text: Vec<String> = rest[..line].iter().map(u8::to_string).collect();
        let _ = writeln!(out, "\t.byte\t{}", text.join(", "));
        rest = &rest[line..];
    }
}
