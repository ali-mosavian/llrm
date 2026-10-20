//! A module's global variables as sections of an object: their bytes, and the
//! relocations their pointers need.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::{ConstantId, ConstantKind, GlobalKind, Linkage, Module, Type, TypeId};

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
    pub public: bool,
    pub bytes: Vec<u8>,
    pub relocs: Vec<DataReloc>,
    pub align: usize,
    pub constant: bool,
    /// Only zeros: a section without contents holds it.
    pub zero: bool,
}

pub fn symbol_name(
    platform: Platform,
    name: &str,
) -> String {
    if platform.underscore() { format!("_{name}") } else { name.to_owned() }
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
        let align = layout.global_align(types, variable.ty, variable.align, false, true) as usize;
        let zero = relocs.is_empty() && bytes.iter().all(|byte| *byte == 0);
        out.push(Placed {
            public: !matches!(global.linkage, Linkage::Internal | Linkage::Private),
            name: symbol_name(platform, &name),
            bytes,
            relocs,
            align: align.max(1),
            constant: variable.constant,
            zero,
        });
    }
    Ok(out)
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
