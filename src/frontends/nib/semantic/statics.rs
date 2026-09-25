//! Module variables (section 14): a `var` is data in DGROUP holding its
//! initial value, which every function names as a place of its own.

use super::*;
use crate::frontends::nib::syntax::{Module, Static};

#[derive(Clone, Copy, Debug)]
pub(super) struct StaticLayout {
    symbol: u32,
    binding: BindingType,
    type_id: u32,
    extent: u32,
    /// Shared with an interrupt handler, which may run between any two
    /// instructions: every access reads or writes memory.
    volatile: bool,
}

/// The module variables an `interrupt16` function names. A variable that
/// only a function it calls names is not among them.
pub(super) fn shared(module: &Module) -> BTreeSet<String> {
    let mut named = BTreeSet::new();
    let handlers = module.functions.iter().filter(|one| module.exports.get(&one.name).is_some_and(|export| export.abi.is_some_and(Abi::interrupt)));
    for handler in handlers {
        for statement in &mut handler.body.clone() {
            let mut name_in = |expression: &mut Expr| {
                if let Expr::Name(name, _) = expression {
                    named.insert(name.clone());
                }
                Ok::<(), ()>(())
            };
            statement.each_mut(&mut |one| {
                if let Statement::Assign { target, .. } = one {
                    let _ = match target {
                        AssignTarget::Name(name) => name_in(&mut Expr::Name(name.clone(), Span::new(0, 0, 0))),
                        AssignTarget::Index { base, .. } | AssignTarget::Member { base, .. } | AssignTarget::Deref(base) => base.walk_mut(&mut name_in),
                    };
                }
            });
            let _ = statement.walk_mut(&mut name_in);
        }
    }
    named.retain(|name| module.statics.iter().any(|one| &one.name == name));
    named
}

impl TypeRegistry {
    /// Lays out each module variable as writable data, its value encoded;
    /// those in `shared` are volatile.
    pub(super) fn register_statics(&mut self, statics: &[Static], shared: &BTreeSet<String>, literals: &mut LiteralPool) -> Result<(), Diagnostic> {
        for declared in statics {
            let (binding, type_id, element, dims) = match &declared.annotation {
                TypeAnnotation::Value(spec) => match self.resolve_element(spec, declared.span)? {
                    ElementType::Scalar(type_name) => (BindingType::Scalar(type_name), super::type_id(type_name), ElementType::Scalar(type_name), Vec::new()),
                    ElementType::Struct(id) => (BindingType::Struct(id), id, ElementType::Struct(id), Vec::new()),
                },
                TypeAnnotation::Array { element, dims } => {
                    let element = self.resolve_element(element, declared.span)?;
                    let shape = Shape::new(dims);
                    (BindingType::Array { element, shape }, self.array(element, shape), element, dims.clone())
                }
                TypeAnnotation::Slice { .. } => return Err(Diagnostic::new(declared.span, "a module variable owns its storage: give it a length")),
            };
            let count: u32 = dims.iter().product();
            let mut bytes = vec![0; (count * self.width(element.id())) as usize];
            self.encode(&mut bytes, 0, &declared.value, element, &dims, declared.span)?;
            let extent = bytes.len() as u32;
            let symbol = literals.object(&format!("$var_{}", declared.name), bytes, false);
            let volatile = shared.contains(&declared.name);
            self.statics.insert(declared.name.clone(), StaticLayout { symbol, binding, type_id, extent, volatile });
        }
        Ok(())
    }
}

impl FunctionCompiler<'_> {
    /// Each module variable, named in the outermost scope.
    pub(super) fn bind_statics(&mut self) {
        let statics: Vec<(String, StaticLayout)> = self.types.statics.iter().map(|(name, one)| (name.clone(), *one)).collect();
        for (name, layout) in statics {
            let id = self.next_place;
            self.next_place += 1;
            self.places.push(hir::Place {
                id,
                name: name.clone(),
                type_id: layout.type_id,
                mutable: true,
                offset: 0,
                extent: layout.extent,
                storage: "module",
                symbol: layout.symbol,
                volatile: layout.volatile,
            });
            self.scopes[0].insert(name, Binding { type_: layout.binding, mutable: true, storage: Storage::Place(id) });
        }
    }
}

impl TypeRegistry {
    /// Writes `value` into `bytes` at `at`: `element`s filling `dims`,
    /// row-major, a struct's fields at their offsets.
    fn encode(&self, bytes: &mut [u8], at: u32, value: &Expr, element: ElementType, dims: &[u32], span: Span) -> Result<(), Diagnostic> {
        let width = self.width(element.id());
        if !dims.is_empty() {
            let count: u32 = dims.iter().product();
            if let Expr::Repeat { value, .. } = value {
                for index in 0..count {
                    self.encode(bytes, at + index * width, value, element, &[], span)?;
                }
                return Ok(());
            }
            for (place, item) in literal_elements(value, dims, span)? {
                let index = place.iter().zip(dims).fold(0, |linear, (one, extent)| linear * extent + one);
                self.encode(bytes, at + index * width, item, element, &[], span)?;
            }
            return Ok(());
        }
        let type_name = match element {
            ElementType::Scalar(type_name) => type_name,
            ElementType::Struct(id) => {
                let layout = self.structure(id).filter(|_| self.enum_of(element).is_none());
                let (Some(layout), Expr::StructLiteral { fields, .. }) = (layout, value) else {
                    return Err(Diagnostic::new(value.span(), "a module variable's struct starts as a struct literal"));
                };
                for (name, field_value, field_span) in fields {
                    let field = layout.fields[name];
                    let dims = field.shape.map(|shape| shape.dims().to_vec()).unwrap_or_default();
                    self.encode(bytes, at + field.offset, field_value, field.type_, &dims, *field_span)?;
                }
                return Ok(());
            }
        };
        if super::ownership::needs_drop(type_name) {
            return Err(Diagnostic::new(value.span(), "a module variable holds no heap value"));
        }
        let folded = super::super::consts::folded(value, &BTreeMap::new());
        let number = match folded.as_ref() {
            Some(Expr::Boolean(value, _)) => i64::from(*value),
            Some(Expr::Character(value, _)) => i64::from(*value),
            Some(one) => super::super::consts::integer(one).ok_or_else(|| Diagnostic::new(value.span(), "a module variable starts as an integer, char or bool"))?,
            None => return Err(Diagnostic::new(value.span(), "a module variable's value is a compile-time value")),
        };
        let start = at as usize;
        bytes[start..start + width as usize].copy_from_slice(&number.to_le_bytes()[..width as usize]);
        Ok(())
    }
}
