//! Foreign interoperability: functions an `extern "cdecl16":` block imports,
//! functions an `export` block exposes, raw pointers, and the `unsafe`
//! blocks that call and take them. Modern functions already follow cdecl16
//! -- far calls, arguments pushed right to left, the caller cleaning up --
//! so a foreign call differs only in its symbol and what may cross it.

use super::*;
use crate::frontends::modern::syntax::Extern;

impl TypeRegistry {
    /// `*far T`, `*near mut T` and the like. A huge pointer is a far one
    /// whose foreign user keeps it normalized.
    pub(super) fn raw_pointer(&mut self, target: ElementType, distance: &str, mutable: bool) -> TypeName {
        let name = format!("*{distance} {}{}", if mutable { "mut " } else { "" }, self.types[(target.id() - 1) as usize].name);
        let far = distance != "near";
        let type_id = match self.raw_pointers.get(&name) {
            Some(id) => *id,
            None => {
                let id = self.pointer_type(name.clone(), target.id(), 0, far);
                self.raw_pointers.insert(name, id);
                self.raw_targets.insert(id, target);
                id
            }
        };
        TypeName::Pointer { type_id, width: if far { 4 } else { 2 }, mutable }
    }

    /// What a raw pointer type points to; `None` for any other type.
    pub(super) fn raw_target(&self, type_name: TypeName) -> Option<ElementType> {
        let TypeName::Pointer { type_id, .. } = type_name else {
            return None;
        };
        self.raw_targets.get(&type_id).copied()
    }
}

/// Whether a value of `type_name` may cross a foreign ABI: a scalar, or a
/// raw pointer to one or to a represented struct; never a buffer's owner.
fn crosses(types: &TypeRegistry, type_name: TypeName) -> bool {
    match type_name {
        TypeName::Pointer { type_id, .. } => {
            let target = types.types[(type_id - 1) as usize]
                .element
                .expect("a pointer has a target");
            types.types[(target - 1) as usize].kind != "opaque"
                || types.represented.contains(&target)
        }
        TypeName::String | TypeName::Vector { .. } => false,
        other => !ownership::needs_drop(other),
    }
}

/// Checks that `signature`, of the function `name`, may cross a foreign ABI.
pub(super) fn check_foreign(
    types: &TypeRegistry,
    signature: &Signature,
    name: &str,
    span: Span,
) -> Result<(), Diagnostic> {
    for (parameter, (formal, _)) in signature.parameters.iter().zip(&signature.formals) {
        match parameter {
            SignatureParameter::Scalar(type_name) if crosses(types, *type_name) => {}
            _ => {
                return Err(Diagnostic::new(
                    span,
                    format!(
                        "{name}'s parameter {formal:?} cannot cross a foreign ABI; pass a scalar or a *far pointer"
                    ),
                ));
            }
        }
    }
    // A represented struct of 4 bytes or less comes back in registers, as C's does.
    let result_crosses = match signature.slot {
        Some(struct_id) => signature.in_registers(types).is_some() && types.represented.contains(&struct_id),
        None => signature.view.is_none() && crosses(types, signature.result),
    };
    if !result_crosses {
        return Err(Diagnostic::new(
            span,
            format!("{name}'s result cannot cross a foreign ABI; return a scalar or a small represented struct"),
        ));
    }
    Ok(())
}

/// The signature an `extern` function is called by: its object symbol, defined elsewhere.
pub(super) fn foreign_signature(
    types: &mut TypeRegistry,
    declared: &Extern,
    id: u32,
) -> Result<Signature, Diagnostic> {
    let mut signature = signature(types, &declared.function, id)?;
    check_foreign(
        types,
        &signature,
        &declared.function.name,
        declared.function.span,
    )?;
    signature.name = declared.symbol.clone();
    signature.foreign = true;
    signature.abi = declared.abi;
    Ok(signature)
}

impl FunctionCompiler<'_> {
    /// `unsafe: body`.
    pub(super) fn unsafe_block(&mut self, body: &[Statement]) -> Result<(), Diagnostic> {
        self.unsafe_depth += 1;
        let result = self.scoped(body);
        self.unsafe_depth -= 1;
        result
    }

    pub(super) fn require_unsafe(&self, what: &str, span: Span) -> Result<(), Diagnostic> {
        if self.unsafe_depth == 0 {
            return Err(Diagnostic::new(
                span,
                format!("{what} is unsafe: put it in an 'unsafe:' block"),
            ));
        }
        Ok(())
    }

    /// `*pointer`: a hidden name for the place a raw pointer points to,
    /// which reads and writes through it as a reference's name does.
    pub(super) fn dereferenced(&mut self, pointer: &Expr, span: Span) -> Result<String, Diagnostic> {
        self.require_unsafe("reading or writing through a raw pointer", span)?;
        let value = self.expression(pointer, None)?;
        let Some(target) = self.types.raw_target(value.type_name) else {
            return Err(Diagnostic::new(span, "only a raw pointer is read with '*'"));
        };
        let TypeName::Pointer { mutable, .. } = value.type_name else {
            unreachable!("a raw pointer")
        };
        let pointer_type = type_id(value.type_name);
        let address = self.materialized(required(value, span)?, pointer_type);
        let binding = Binding {
            type_: match target {
                ElementType::Scalar(type_name) => BindingType::Scalar(type_name),
                ElementType::Struct(id) => BindingType::Struct(id),
            },
            mutable,
            storage: Storage::Reference(address),
        };
        // Named as written, so that a diagnostic reads as the source does.
        let name = match pointer {
            Expr::Name(written, _) => format!("*{written}"),
            _ => self.hidden("pointee"),
        };
        self.scopes.last_mut().expect("scope").insert(name.clone(), binding);
        Ok(name)
    }

    /// `&place` as the raw pointer `pointer`.
    pub(super) fn raw_address(
        &mut self,
        operand: &Expr,
        mutable: bool,
        pointer: TypeName,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        self.require_unsafe("taking a raw pointer", span)?;
        let TypeName::Pointer {
            type_id: pointer_id,
            width,
            mutable: writes,
        } = pointer
        else {
            unreachable!("a raw pointer type")
        };
        if writes && !mutable {
            return Err(Diagnostic::new(span, "a *mut pointer is taken with '&mut'"));
        }
        if width == 2 {
            return Err(Diagnostic::new(
                span,
                "a near pointer reaches only static data; take a *far one",
            ));
        }
        let pointee = self.types.types[(pointer_id - 1) as usize]
            .element
            .expect("a pointer has a target");
        let borrow = Expr::Borrow {
            mutable,
            operand: Box::new(operand.clone()),
            span,
        };
        let (target, address) = if let Some(struct_id) =
            self.struct_expression_type(operand, span)?
        {
            (
                struct_id,
                self.borrow_argument(&borrow, mutable, BindingType::Struct(struct_id), pointer_id)?
                    .0,
            )
        } else {
            let Expr::Name(name, name_span) = operand else {
                return Err(Diagnostic::new(
                    span,
                    "only a named place or a struct has a raw address",
                ));
            };
            match self.binding(name, *name_span)?.clone() {
                // An array's address is its first element's.
                Binding {
                    type_: BindingType::Array { element, .. },
                    storage: Storage::Place(place),
                    mutable: owned,
                } => {
                    if mutable && !owned {
                        return Err(Diagnostic::new(
                            span,
                            format!("cannot mutably borrow immutable binding {name:?}"),
                        ));
                    }
                    let result = self.value_type(pointer_id);
                    self.emit(
                        "address",
                        vec![result],
                        vec![hir::Operand::Place(place)],
                        None,
                    );
                    (element.id(), hir::Operand::Value(result))
                }
                Binding { type_: BindingType::Slice { element, rank }, storage: Storage::Slice(descriptor), .. } if !mutable => {
                    (element.id(), hir::Operand::Value(self.slice_data_pointer(descriptor, element, rank)))
                }
                // A string's or vec's, too: a string's bytes end in a NUL.
                binding @ Binding { type_: BindingType::Scalar(sequence), .. }
                    if !mutable && self.types.sequence_element(sequence).is_some() =>
                {
                    let element = self.types.sequence_element(sequence).expect("a sequence");
                    let data = self.string_pointer(&binding, *name_span)?;
                    let result = self.value_type(pointer_id);
                    let first = hir::Operand::IndirectPlace { base: data, offset: 0, type_id: element.id(), inbounds: false };
                    self.emit("address", vec![result], vec![first], None);
                    (element.id(), hir::Operand::Value(result))
                }
                Binding {
                    type_: type_ @ BindingType::Scalar(type_name),
                    ..
                } => (
                    type_id(type_name),
                    self.borrow_argument(&borrow, mutable, type_, pointer_id)?.0,
                ),
                _ => {
                    return Err(Diagnostic::new(
                        span,
                        "only a scalar, struct, or sequence has a raw address",
                    ));
                }
            }
        };
        if target != pointee {
            return Err(Diagnostic::new(
                span,
                "the raw pointer's target type differs from the place's",
            ));
        }
        Ok(TypedOperand {
            operand: Some(address),
            type_name: pointer,
        })
    }
}
