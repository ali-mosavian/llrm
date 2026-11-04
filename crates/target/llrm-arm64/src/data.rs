//! A module's global variables as sections of an object: their bytes, and the
//! relocations their pointers need. `llvm.global_ctors` becomes the object's
//! init-function pointers (`__mod_init_func`, `.init_array`); `llvm.used` and
//! `llvm.compiler.used` are not laid out.

use llrm_mir::datalayout::DataLayout;
/// A pointer in a global's bytes: where, to what, and the addend.
pub use llrm_mir::image::Reloc as DataReloc;
use llrm_mir::{Linkage, Module};
use llrm_object::Binding;

use crate::machine::Platform;

/// One global's contents.
pub struct Placed {
    pub name: String,
    pub binding: Binding,
    pub bytes: Vec<u8>,
    pub relocs: Vec<DataReloc>,
    pub align: usize,
    pub constant: bool,
    /// Only zeros: a section without contents holds it.
    pub zero: bool,
    /// Pointers the loader calls before `main`: `llvm.global_ctors` after
    /// [`constructors`]. It has no symbol.
    pub init_functions: bool,
}

/// How an object exports a global of `linkage`: not at all, or as a definition
/// another may replace.
pub fn binding_of(linkage: Linkage) -> Binding {
    match llrm_mir::image::Export::of(linkage) {
        llrm_mir::image::Export::Local => Binding::Local,
        llrm_mir::image::Export::Weak => Binding::Weak,
        llrm_mir::image::Export::Public => Binding::Public,
    }
}

pub fn symbol_name(
    platform: Platform,
    name: &str,
) -> String {
    // `\01` marks a name to be used as written, with no platform prefix.
    if let Some(verbatim) = name.strip_prefix('\u{1}') {
        verbatim.to_owned()
    } else if platform.underscore() {
        format!("_{name}")
    } else {
        name.to_owned()
    }
}

/// Every global variable `module` defines, laid out.
pub fn globals(
    module: &Module,
    layout: &DataLayout,
    platform: Platform,
) -> Result<Vec<Placed>, String> {
    let symbol_of = |name: &str| symbol_name(platform, name);
    let mut out = Vec::new();
    for variable in llrm_mir::image::variables(module, layout, &symbol_of)? {
        if variable.name == "llvm.global_dtors" {
            return Err("llvm.global_dtors: destructors are not supported".to_owned());
        }
        if variable.name == "llvm.global_ctors" {
            let (bytes, relocs) = constructors(&variable.bytes, &variable.relocs);
            out.push(Placed {
                binding: Binding::Local,
                name: String::new(),
                zero: false,
                bytes,
                relocs,
                align: 8,
                constant: false,
                init_functions: true,
            });
            continue;
        }
        out.push(Placed {
            binding: binding_of(variable.linkage),
            name: symbol_name(platform, &variable.name),
            bytes: variable.bytes,
            relocs: variable.relocs,
            align: variable.align,
            constant: variable.constant,
            zero: variable.zero,
            init_functions: false,
        });
    }
    Ok(out)
}

/// The function pointers of `llvm.global_ctors`' `{ i32 priority, ptr function,
/// ptr data }` entries (24 bytes each on a 64-bit target), lowest priority
/// first and in order within one, as `AsmPrinter::emitXXStructorList` sorts
/// them; an entry without a function is dropped.
fn constructors(
    bytes: &[u8],
    relocs: &[DataReloc],
) -> (Vec<u8>, Vec<DataReloc>) {
    let mut entries: Vec<(u32, &DataReloc)> = bytes
        .chunks(24)
        .enumerate()
        .filter_map(|(index, entry)| {
            let function = relocs.iter().find(|reloc| reloc.at == index * 24 + 8)?;
            Some((u32::from_le_bytes(entry[..4].try_into().expect("four bytes")), function))
        })
        .collect();
    entries.sort_by_key(|(priority, _)| *priority);
    let relocs = entries
        .iter()
        .enumerate()
        .map(|(index, (_, function))| DataReloc { at: index * 8, ..(*function).clone() })
        .collect();
    (vec![0; entries.len() * 8], relocs)
}
