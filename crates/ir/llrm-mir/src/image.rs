//! A constant as the bytes it occupies in memory, and the pointers in them.
//!
//! What every target's data section needs of a global's initializer: the
//! layout is the module's data layout (sizes, struct offsets, byte order); what
//! a symbol is called in the object is the caller's.

use crate::context::signed;
use crate::datalayout::DataLayout;
use crate::{ConstantExpr, ConstantId, ConstantKind, FloatKind, GlobalKind, Linkage, Module, Type, TypeId};

/// A pointer in a constant's bytes: where, to what, and the addend.
#[derive(Clone, Debug)]
pub struct Reloc {
    pub at: usize,
    pub symbol: String,
    pub addend: i64,
}

/// Writes constant `id` of type `ty` into `out` at `at`; the pointers go to
/// `relocs`, named by `symbol_of`. Bytes it does not write stay as they are
/// (zero for `zeroinitializer`, `undef`, `poison` and `null`).
pub struct Writer<'a> {
    pub module: &'a Module,
    pub layout: &'a DataLayout,
    pub symbol_of: &'a dyn Fn(&str) -> String,
}

impl Writer<'_> {
    pub fn write(
        &self,
        id: ConstantId,
        ty: TypeId,
        out: &mut [u8],
        at: usize,
        relocs: &mut Vec<Reloc>,
    ) -> Result<(), String> {
        let context = &self.module.context;
        let types = &context.types;
        let constant = context.get(id);
        match (&constant.kind, types.get(ty)) {
            (ConstantKind::Zero | ConstantKind::Null | ConstantKind::Undef | ConstantKind::Poison, _) => Ok(()),
            (ConstantKind::Int(value), Type::Int(bits)) => {
                let bytes = (*bits as usize).div_ceil(8);
                self.put(out, at, &value.to_le_bytes()[..bytes]);
                Ok(())
            }
            (ConstantKind::Float(bits), Type::Float(kind)) => {
                let bytes = match kind {
                    FloatKind::Float => 4,
                    FloatKind::Double => 8,
                    other => return Err(format!("a global of type {other:?}")),
                };
                self.put(out, at, &bits.to_le_bytes()[..bytes]);
                Ok(())
            }
            // The bytes are the memory image of an `[N x i8]` (`c"..."`, a data
            // object's image): copied as they are. Wider elements would need
            // the layout's byte order, and nothing builds that.
            (ConstantKind::Bytes(data), Type::Array { element, .. }) if matches!(types.get(*element), Type::Int(8)) => {
                out[at..at + data.len()].copy_from_slice(data);
                Ok(())
            }
            (ConstantKind::Bytes(_), ty) => Err(format!("a byte-backed constant of type {ty:?}, not an i8 array")),
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
                relocs.push(Reloc { at, symbol: (self.symbol_of)(&name), addend: 0 });
                Ok(())
            }
            (ConstantKind::Expr(ConstantExpr::GetElementPtr { source, operands, .. }), Type::Pointer(_)) => {
                let base = context.get(operands[0]);
                let ConstantKind::Global(global) = base.kind else {
                    return Err("a constant gep on a non-global".into());
                };
                let mut indices = Vec::new();
                for &index in &operands[1..] {
                    let index = context.get(index);
                    indices.push(match index.kind {
                        ConstantKind::Int(value) => Some(signed(value, types.int_bits(index.ty).unwrap_or(64))),
                        ConstantKind::Zero => Some(0),
                        _ => return Err("a constant gep with a variable index".into()),
                    });
                }
                let (offset, variable) = self.layout.collect_offset(types, *source, &indices);
                if !variable.is_empty() {
                    return Err("a constant gep with a variable index".into());
                }
                let name = self.module.global(global).name.owned().ok_or("an unnamed global")?;
                relocs.push(Reloc { at, symbol: (self.symbol_of)(&name), addend: offset as i64 });
                Ok(())
            }
            (kind, ty) => Err(format!("the global initializer {kind:?} of type {ty:?}")),
        }
    }

    /// `little`, the value's low bytes first, in the layout's byte order.
    fn put(
        &self,
        out: &mut [u8],
        at: usize,
        little: &[u8],
    ) {
        let target = &mut out[at..at + little.len()];
        target.copy_from_slice(little);
        if self.layout.big_endian {
            target.reverse();
        }
    }
}

/// How an object exports a global of some linkage: not at all, as a
/// definition another may replace, or plainly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Export {
    Local,
    Weak,
    Public,
}

impl Export {
    pub fn of(linkage: Linkage) -> Self {
        match linkage {
            Linkage::Internal | Linkage::Private => Export::Local,
            Linkage::Weak
            | Linkage::WeakOdr
            | Linkage::LinkOnce
            | Linkage::LinkOnceOdr
            | Linkage::Common
            | Linkage::ExternWeak => Export::Weak,
            _ => Export::Public,
        }
    }
}

/// One global variable with an initializer, laid out.
#[derive(Clone, Debug)]
pub struct Variable {
    /// The name in the IR.
    pub name: String,
    pub linkage: Linkage,
    pub export: Export,
    pub bytes: Vec<u8>,
    pub relocs: Vec<Reloc>,
    pub align: usize,
    pub constant: bool,
    /// Only zeros and no pointers: a section without contents holds it.
    pub zero: bool,
}

/// Every global variable `module` defines. `llvm.used` and
/// `llvm.compiler.used` are for the compiler and the linker only and are left
/// out; `llvm.global_ctors` and `llvm.global_dtors` are returned as they are,
/// for the target to lay out as its object format wants.
pub fn variables(
    module: &Module,
    layout: &DataLayout,
    symbol_of: &dyn Fn(&str) -> String,
) -> Result<Vec<Variable>, String> {
    let types = &module.context.types;
    let mut out = Vec::new();
    for global in &module.globals {
        let GlobalKind::Variable(variable) = &global.kind else { continue };
        let Some(initializer) = variable.initializer else { continue };
        let name = global.name.owned().ok_or("an unnamed variable")?;
        if matches!(name.as_str(), "llvm.used" | "llvm.compiler.used") {
            continue;
        }
        let size = layout.alloc_size(types, variable.ty) as usize;
        let mut bytes = vec![0u8; size];
        let mut relocs = Vec::new();
        Writer { module, layout, symbol_of }.write(initializer, variable.ty, &mut bytes, 0, &mut relocs)?;
        let align = layout.global_align(types, variable.ty, variable.align, false, true, true) as usize;
        let zero = relocs.is_empty() && bytes.iter().all(|byte| *byte == 0);
        out.push(Variable {
            name,
            linkage: global.linkage,
            export: Export::of(global.linkage),
            bytes,
            relocs,
            align: align.max(1),
            constant: variable.constant,
            zero,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_of(
        text: &str,
        name: &str,
    ) -> Vec<u8> {
        let module = crate::parse::module(text).expect("parses");
        let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("a layout");
        let symbol_of = |name: &str| name.to_owned();
        let all = variables(&module, &layout, &symbol_of).expect("laid out");
        all.into_iter().find(|one| one.name == name).expect("the variable").bytes
    }

    /// The byte order is the data layout's: an i32 and an i16 in an `E`
    /// layout start with their most significant byte, and in an `e` layout
    /// with the least. Without the layout's order every target wrote the
    /// little-endian bytes (m68k's `-2` would have read back as `-1`).
    #[test]
    fn integers_are_written_in_the_layouts_byte_order() {
        let body = "@w = global i16 258\n@l = global i32 16909060\n";
        let big =
            format!("target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n{body}");
        let little = format!("target datalayout = \"e-m:e-p:32:32-i64:32-n8:16:32\"\n{body}");
        assert_eq!(bytes_of(&big, "w"), [1, 2]);
        assert_eq!(bytes_of(&big, "l"), [1, 2, 3, 4]);
        assert_eq!(bytes_of(&little, "w"), [2, 1]);
        assert_eq!(bytes_of(&little, "l"), [4, 3, 2, 1]);
    }

    /// `ConstantKind::Bytes` is the image of an `[N x i8]`; given a wider
    /// element type its bytes would be copied in the order they were made in,
    /// wrong on a big-endian target. It is refused, not copied.
    #[test]
    fn a_byte_backed_constant_of_wider_elements_is_refused() {
        let module = crate::parse::module(
            "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n",
        )
        .expect("parses");
        let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("a layout");
        let (i8, i16) = (module.context.types.int(8), module.context.types.int(16));
        let narrow = module.context.types.intern(Type::Array { element: i8, count: 2 });
        let wide = module.context.types.intern(Type::Array { element: i16, count: 2 });
        let symbol_of = |name: &str| name.to_owned();
        let mut out = [0u8; 4];
        let mut relocs = Vec::new();
        let ok = module.context.constant(crate::Constant { ty: narrow, kind: ConstantKind::Bytes(vec![1, 2]) });
        let bad = module.context.constant(crate::Constant { ty: wide, kind: ConstantKind::Bytes(vec![1, 2, 3, 4]) });
        let writer = Writer { module: &module, layout: &layout, symbol_of: &symbol_of };
        assert!(writer.write(ok, narrow, &mut out, 0, &mut relocs).is_ok());
        assert_eq!(out[..2], [1, 2]);
        assert!(writer.write(bad, wide, &mut out, 0, &mut relocs).is_err(), "copied unswapped");
    }
}
