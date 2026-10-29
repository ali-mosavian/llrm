//! A module's global variables as sections of an object: their bytes, and the
//! relocations their pointers need. `llvm.global_ctors` becomes the object's
//! init-function pointers (`__mod_init_func`, `.init_array`); `llvm.used` and
//! `llvm.compiler.used` are not laid out.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::{ConstantId, ConstantKind, GlobalKind, Linkage, Module, Type, TypeId};
use llrm_object::Binding;

use crate::machine::Platform;

/// A pointer in a global's bytes: where, to what, and the addend.
#[derive(Clone, Debug)]
pub struct DataReloc {
    pub at: usize,
    pub symbol: String,
    pub addend: i64,
}

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
    match linkage {
        Linkage::Internal | Linkage::Private => Binding::Local,
        Linkage::Weak
        | Linkage::WeakOdr
        | Linkage::LinkOnce
        | Linkage::LinkOnceOdr
        | Linkage::Common
        | Linkage::ExternWeak => Binding::Weak,
        _ => Binding::Public,
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
    let types = &module.context.types;
    let mut out = Vec::new();
    for global in &module.globals {
        let GlobalKind::Variable(variable) = &global.kind else { continue };
        let Some(initializer) = variable.initializer else { continue };
        let name = global.name.owned().ok_or("an unnamed variable")?;
        let size = layout.alloc_size(types, variable.ty) as usize;
        let mut bytes = vec![0u8; size];
        let mut relocs = Vec::new();
        let writer = Writer { module, layout, platform };
        writer.write(initializer, variable.ty, &mut bytes, 0, &mut relocs)?;
        let align = layout.global_align(types, variable.ty, variable.align, false, true, true) as usize;
        let zero = relocs.is_empty() && bytes.iter().all(|byte| *byte == 0);
        // LLVM keeps these two for the compiler and the linker only.
        if matches!(name.as_str(), "llvm.used" | "llvm.compiler.used") {
            continue;
        }
        if name == "llvm.global_dtors" {
            return Err("llvm.global_dtors: destructors are not supported".to_owned());
        }
        if name == "llvm.global_ctors" {
            let (bytes, relocs) = constructors(&bytes, &relocs);
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
            binding: binding_of(global.linkage),
            name: symbol_name(platform, &name),
            bytes,
            relocs,
            align: align.max(1),
            constant: variable.constant,
            zero,
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

struct Writer<'a> {
    module: &'a Module,
    layout: &'a DataLayout,
    platform: Platform,
}

impl Writer<'_> {
    fn write(
        &self,
        id: ConstantId,
        ty: TypeId,
        out: &mut [u8],
        at: usize,
        relocs: &mut Vec<DataReloc>,
    ) -> Result<(), String> {
        let context = &self.module.context;
        let types = &context.types;
        let constant = context.get(id);
        match (&constant.kind, types.get(ty)) {
            (ConstantKind::Zero | ConstantKind::Null | ConstantKind::Undef | ConstantKind::Poison, _) => Ok(()),
            (ConstantKind::Int(value), Type::Int(bits)) => {
                let bytes = (*bits as usize).div_ceil(8);
                out[at..at + bytes].copy_from_slice(&value.to_le_bytes()[..bytes]);
                Ok(())
            }
            (ConstantKind::Float(bits), Type::Float(kind)) => {
                let bytes = match kind {
                    llrm_mir::FloatKind::Float => 4,
                    llrm_mir::FloatKind::Double => 8,
                    other => return Err(format!("a global of type {other:?}")),
                };
                out[at..at + bytes].copy_from_slice(&bits.to_le_bytes()[..bytes]);
                Ok(())
            }
            (ConstantKind::Bytes(data), _) => {
                out[at..at + data.len()].copy_from_slice(data);
                Ok(())
            }
            (ConstantKind::Aggregate(members), Type::Array { element, .. }) => {
                let stride = self.layout.alloc_size(types, *element) as usize;
                for (index, member) in members.iter().enumerate() {
                    self.write(*member, *element, out, at + index * stride, relocs)?;
                }
                Ok(())
            }
            (ConstantKind::Aggregate(members), _) if types.fields(ty).is_some() => {
                let (_, offsets) = self.layout.struct_layout(types, ty);
                let fields = types.fields(ty).expect("a struct").to_vec();
                for (index, member) in members.iter().enumerate() {
                    self.write(*member, fields[index], out, at + offsets[index] as usize, relocs)?;
                }
                Ok(())
            }
            (ConstantKind::Global(global), Type::Pointer(_)) => {
                let name = self.module.global(*global).name.owned().ok_or("an unnamed global")?;
                relocs.push(DataReloc { at, symbol: symbol_name(self.platform, &name), addend: 0 });
                Ok(())
            }
            (ConstantKind::Expr(llrm_mir::ConstantExpr::GetElementPtr { source, operands, .. }), Type::Pointer(_)) => {
                let base = context.get(operands[0]);
                let ConstantKind::Global(global) = base.kind else {
                    return Err("a constant gep on a non-global".into());
                };
                let mut indices = Vec::new();
                for &index in &operands[1..] {
                    let index = context.get(index);
                    indices.push(match index.kind {
                        ConstantKind::Int(value) => {
                            Some(llrm_mir::context::signed(value, types.int_bits(index.ty).unwrap_or(64)))
                        }
                        ConstantKind::Zero => Some(0),
                        _ => return Err("a constant gep with a variable index".into()),
                    });
                }
                let (offset, variable) = self.layout.collect_offset(types, *source, &indices);
                if !variable.is_empty() {
                    return Err("a constant gep with a variable index".into());
                }
                let name = self.module.global(global).name.owned().ok_or("an unnamed global")?;
                relocs.push(DataReloc { at, symbol: symbol_name(self.platform, &name), addend: offset as i64 });
                Ok(())
            }
            (kind, ty) => Err(format!("the global initializer {kind:?} of type {ty:?}")),
        }
    }
}
