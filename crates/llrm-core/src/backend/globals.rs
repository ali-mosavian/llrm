//! A MIR module's globals as masm symbols and data, as LLVM's AsmPrinter
//! emits a global's initializer: its bytes, and a relocation for each
//! address in it.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::{CastOp, ConstantExpr, ConstantId, ConstantKind, FloatKind, GlobalId, GlobalKind, Module, Type};

use crate::backend::masm::{Datum, Label, Pointer};
use crate::model::ir::Space;
use crate::support::hash::IndexMap;

/// Where a global's symbol is: defined here, or outside the module.
pub fn space(module: &Module, global: GlobalId) -> Space {
    match &module.global(global).kind {
        GlobalKind::Variable(variable) if variable.initializer.is_some() => Space::Segment,
        GlobalKind::Function(function) if !function.is_declaration() => Space::Segment,
        _ => Space::External,
    }
}

/// Each global's assembler name, keyed as an `Addr` names it: its name as
/// `linked` mangles it, as LLVM's Mangler does. An internal global's name
/// is free, so one no assembler takes becomes `G$n`; an external one must
/// be a symbol once mangled.
pub fn names(module: &Module, linked: &dyn Fn(&str) -> String) -> Result<IndexMap<(Space, i64), String>, String> {
    let mut out = IndexMap::default();
    for (at, global) in module.globals.iter().enumerate() {
        let id = GlobalId(at as u32);
        let name = linked(global.name.as_deref().unwrap_or_default());
        let symbol = if is_symbol(&name) {
            name
        } else if global.linkage == llrm_mir::Linkage::External {
            return Err(format!("@{name} is no assembler symbol"));
        } else {
            format!("G${at}")
        };
        out.insert((space(module, id), i64::from(id.0)), symbol);
    }
    Ok(out)
}

fn is_symbol(name: &str) -> bool {
    let mut characters = name.chars();
    characters.next().is_some_and(|first| first.is_ascii_alphabetic() || "_$@?".contains(first))
        && characters.all(|one| one.is_ascii_alphanumeric() || "_$@?".contains(one))
}

/// A defined variable's data: its label, then its initializer's bytes and
/// relocations.
pub fn datums(module: &Module, global: GlobalId, names: &IndexMap<(Space, i64), String>) -> Result<Vec<Datum>, String> {
    let GlobalKind::Variable(variable) = &module.global(global).kind else { return Err("a function has no data".to_owned()) };
    let initializer = variable.initializer.ok_or("a declaration has no data")?;
    let layout = DataLayout::parse(module.datalayout.as_deref().ok_or("a module with no datalayout")?)?;
    let name = names[&(Space::Segment, i64::from(global.0))].clone();
    let mut out = vec![Datum::Label(Label { name })];
    Initializer { module, layout: &layout, names, out: &mut out }.constant(initializer)?;
    Ok(out)
}

struct Initializer<'a> {
    module: &'a Module,
    layout: &'a DataLayout,
    names: &'a IndexMap<(Space, i64), String>,
    out: &'a mut Vec<Datum>,
}

impl Initializer<'_> {
    fn bytes(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(Datum::Bytes(last)) = self.out.last_mut() {
            last.extend_from_slice(bytes);
        } else {
            self.out.push(Datum::Bytes(bytes.to_vec()));
        }
    }

    fn symbol(&self, global: GlobalId) -> String {
        self.names[&(space(self.module, global), i64::from(global.0))].clone()
    }

    fn constant(&mut self, id: ConstantId) -> Result<(), String> {
        let context = &self.module.context;
        let types = &context.types;
        let constant = context.get(id);
        let size = self.layout.store_size(types, constant.ty) as usize;
        match &constant.kind {
            ConstantKind::Int(bits) => self.bytes(&bits.to_le_bytes()[..size]),
            ConstantKind::Float(bits) => match types.get(constant.ty) {
                Type::Float(FloatKind::Float) => self.bytes(&(*bits as u32).to_le_bytes()),
                _ => self.bytes(&bits.to_le_bytes()[..size]),
            },
            ConstantKind::Null | ConstantKind::Zero => self.bytes(&vec![0; self.layout.alloc_size(types, constant.ty) as usize]),
            ConstantKind::Bytes(bytes) => self.bytes(bytes),
            ConstantKind::Aggregate(members) => match types.get(constant.ty).clone() {
                Type::Struct { .. } | Type::Named(_) => {
                    let (total, offsets) = self.layout.struct_layout(types, constant.ty);
                    let mut at = 0;
                    for (member, offset) in members.clone().into_iter().zip(offsets) {
                        self.bytes(&vec![0; (offset - at) as usize]);
                        self.constant(member)?;
                        at = offset + self.layout.store_size(&self.module.context.types, self.module.context.get(member).ty);
                    }
                    self.bytes(&vec![0; (total - at) as usize]);
                }
                _ => {
                    for member in members.clone() {
                        self.constant(member)?;
                    }
                }
            },
            ConstantKind::Global(_) | ConstantKind::Expr(_) => self.address(id)?,
            ConstantKind::Poison => return Err("a poison initializer".to_owned()),
        }
        Ok(())
    }

    /// A relocated address: a near or far pointer, a segment, or a far
    /// pointer's offset word.
    fn address(&mut self, id: ConstantId) -> Result<(), String> {
        let context = &self.module.context;
        let constant = context.get(id);
        let datum = match (&constant.kind, context.types.get(constant.ty)) {
            (_, Type::Pointer(space @ (0 | 1))) => {
                let (global, offset) = target(self.module, self.layout, id)?;
                Datum::Pointer(Pointer { name: self.symbol(global), offset, far: *space == 1 })
            }
            (ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::AddrSpaceCast, value }), Type::Pointer(2)) => {
                let (global, _) = target(self.module, self.layout, *value)?;
                Datum::SegmentWord(self.symbol(global))
            }
            (ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::PtrToInt, value }), Type::Int(16)) => {
                let (global, offset) = target(self.module, self.layout, *value)?;
                Datum::Pointer(Pointer { name: self.symbol(global), offset, far: false })
            }
            _ => return Err(format!("an initializer of {}", context.types.display(constant.ty))),
        };
        self.out.push(datum);
        Ok(())
    }
}

/// The global an address constant points into, and how far.
pub fn target(module: &Module, layout: &DataLayout, id: ConstantId) -> Result<(GlobalId, i64), String> {
    let context = &module.context;
    match &context.get(id).kind {
        ConstantKind::Global(global) => Ok((*global, 0)),
        ConstantKind::Expr(ConstantExpr::GetElementPtr { source, operands, .. }) => {
            let (global, base) = target(module, layout, operands[0])?;
            let indices: Vec<Option<i128>> = operands[1..]
                .iter()
                .map(|&one| match context.get(one).kind {
                    ConstantKind::Int(bits) => {
                        let width = context.types.int_bits(context.get(one).ty).unwrap_or(64);
                        Some(llrm_mir::context::signed(bits, width))
                    }
                    _ => None,
                })
                .collect();
            let (offset, variable) = layout.collect_offset(&context.types, *source, &indices);
            if !variable.is_empty() {
                return Err("a constant address with no constant offset".to_owned());
            }
            Ok((global, base + offset as i64))
        }
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::AddrSpaceCast, value }) => target(module, layout, *value),
        _ => Err("an address of no global".to_owned()),
    }
}
