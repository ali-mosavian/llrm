//! Raw pointers (section 8): their types, taking one with `&`, and reading,
//! writing and moving through one. Only reading or writing is unsafe.

use super::*;

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

    /// `spec` when it is already registered: a primitive, a struct, or a raw
    /// pointer to one; unlike `resolve_element`, it registers nothing.
    pub(super) fn resolved_element(&self, spec: &TypeSpec) -> Option<ElementType> {
        match spec {
            TypeSpec::Primitive(type_name) => Some(ElementType::Scalar(*type_name)),
            TypeSpec::Named(name) => self.structs.get(name).map(|one| ElementType::Struct(one.id)),
            TypeSpec::Applied { name, args } if name.starts_with('*') => {
                let [TypeAnnotation::Value(target)] = args.as_slice() else {
                    return None;
                };
                let target = self.resolved_element(target)?;
                let distance = name[1..].split(' ').next()?;
                let mutable = name.ends_with(" mut");
                let spelled = format!("*{distance} {}{}", if mutable { "mut " } else { "" }, self.types[(target.id() - 1) as usize].name);
                let type_id = *self.raw_pointers.get(&spelled)?;
                Some(ElementType::Scalar(TypeName::Pointer { type_id, width: if distance == "near" { 2 } else { 4 }, mutable }))
            }
            _ => None,
        }
    }

    /// Whether `to` is the raw pointer `from` without `mut`, which a
    /// `*mut` one converts to as `&mut` does to `&`.
    pub(super) fn reads_through(&self, from: TypeName, to: TypeName) -> bool {
        let (TypeName::Pointer { type_id: from, mutable: true, .. }, TypeName::Pointer { type_id: to, mutable: false, .. }) = (from, to) else {
            return false;
        };
        self.raw_targets.contains_key(&from)
            && self.raw_targets.contains_key(&to)
            && self.types[(from - 1) as usize].name.replacen("mut ", "", 1) == self.types[(to - 1) as usize].name
    }

    /// What a raw pointer type points to; `None` for any other type.
    pub(super) fn raw_target(&self, type_name: TypeName) -> Option<ElementType> {
        let TypeName::Pointer { type_id, .. } = type_name else {
            return None;
        };
        self.raw_targets.get(&type_id).copied()
    }
}

impl FunctionCompiler<'_> {
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
    /// The address of a sequence's first element, from its data pointer.
    fn data_address(&mut self, data: u32, element: ElementType, pointer_id: u32) -> hir::Operand {
        let result = self.value_type(pointer_id);
        let first = hir::Operand::IndirectPlace { base: data, offset: 0, type_id: element.id(), inbounds: false };
        self.emit("address", vec![result], vec![first], None);
        hir::Operand::Value(result)
    }

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
        if width == 2 && !self.in_dgroup(operand) {
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
        } else if let Some((view, element, _)) = self.array_view(operand, span)? {
            // An array's address is its first element's.
            if mutable && !view.mutable {
                return Err(Diagnostic::new(span, format!("cannot mutably borrow immutable binding {:?}", view.owner)));
            }
            (element.id(), self.address_as(&view, pointer_id))
        } else if let Expr::Member { base, field, .. } = operand {
            let (place, type_name, owned, _) = self.member_place(base, field, span)?;
            if mutable && !owned {
                return Err(Diagnostic::new(span, "cannot mutably borrow a field of an immutable binding"));
            }
            match self.types.sequence_element(type_name) {
                Some(element) => {
                    let data = self.value(type_name);
                    self.emit("load", vec![data], vec![place], None);
                    (element.id(), self.data_address(data, element, pointer_id))
                }
                None => {
                    let result = self.value_type(pointer_id);
                    self.emit("address", vec![result], vec![place], None);
                    (type_id(type_name), hir::Operand::Value(result))
                }
            }
        } else {
            let Expr::Name(name, name_span) = operand else {
                return Err(Diagnostic::new(
                    span,
                    "only a named place, a field or a struct has a raw address",
                ));
            };
            match self.binding(name, *name_span)?.clone() {
                Binding { type_: BindingType::Slice { element, rank }, storage: Storage::Slice(descriptor), .. } if !mutable => {
                    let data = self.slice_data_pointer(descriptor, element, rank);
                    let result = self.value_type(pointer_id);
                    self.emit("copy", vec![result], vec![hir::Operand::Value(data)], None);
                    (element.id(), hir::Operand::Value(result))
                }
                // A string's or vec's, too: a string's bytes end in a NUL.
                binding @ Binding { type_: BindingType::Scalar(sequence), .. }
                    if self.types.sequence_element(sequence).is_some() =>
                {
                    if mutable && !binding.mutable {
                        return Err(Diagnostic::new(span, format!("cannot mutably borrow immutable binding {name:?}")));
                    }
                    let element = self.types.sequence_element(sequence).expect("a sequence");
                    let data = self.string_pointer(&binding, *name_span)?;
                    (element.id(), self.data_address(data, element, pointer_id))
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

    /// The type of `receiver.name[type_arguments]()` when `receiver` is a raw
    /// pointer; a cast's target type must be registered, as preparing does.
    pub(super) fn pointer_method_type(&self, receiver: &Expr, name: &str, type_arguments: &[TypeSpec]) -> Option<TypeName> {
        let type_name = self.expression_type_hint(receiver)?;
        self.types.raw_target(type_name)?;
        match (name, type_arguments) {
            ("offset", []) => Some(type_name),
            ("is_null", []) => Some(TypeName::Bool),
            ("far" | "near", []) => {
                let TypeName::Pointer { mutable, .. } = type_name else {
                    return None;
                };
                let target = self.types.raw_target(type_name)?;
                let name = format!("*{name} {}{}", if mutable { "mut " } else { "" }, self.types.types[(target.id() - 1) as usize].name);
                let id = *self.types.raw_pointers.get(&name)?;
                Some(TypeName::Pointer { type_id: id, width: if name.starts_with("*far") { 4 } else { 2 }, mutable })
            }
            ("cast", [target]) => {
                let TypeName::Pointer { type_id: pointer_id, mutable, .. } = type_name else {
                    return None;
                };
                let distance = self.types.types[(pointer_id - 1) as usize].name[1..].split(' ').next()?;
                let target = self.types.resolved_element(target)?;
                let name = format!("*{distance} {}{}", if mutable { "mut " } else { "" }, self.types.types[(target.id() - 1) as usize].name);
                let id = *self.types.raw_pointers.get(&name)?;
                Some(TypeName::Pointer { type_id: id, width: if distance == "near" { 2 } else { 4 }, mutable })
            }
            _ => None,
        }
    }

    /// Registers the type `p.cast[U]()` or `p.far()` gives, so that hints know it.
    pub(super) fn declare_cast(&mut self, receiver: &Expr, name: &str, type_arguments: &[TypeSpec], span: Span) -> Result<(), Diagnostic> {
        let Some(type_name @ TypeName::Pointer { type_id: pointer_id, mutable, .. }) = self.expression_type_hint(receiver) else {
            return Ok(());
        };
        let Some(pointee) = self.types.raw_target(type_name) else {
            return Ok(());
        };
        match (name, type_arguments) {
            ("cast", [target]) => {
                let distance = self.types.types[(pointer_id - 1) as usize].name[1..].split(' ').next().expect("a distance").to_string();
                let target = self.types.resolve_element(target, span)?;
                self.types.raw_pointer(target, &distance, mutable);
            }
            ("far" | "near", []) => {
                self.types.raw_pointer(pointee, name, mutable);
            }
            _ => {}
        }
        Ok(())
    }

    /// `p[i]` of a raw pointer `p`: `*(p.offset(i))`.
    pub(super) fn pointer_index(&self, base: &Expr, indices: &[Expr], span: Span) -> Option<Expr> {
        self.types.raw_target(self.expression_type_hint(base)?)?;
        let [index] = indices else {
            return None;
        };
        let offset = Expr::MethodCall { receiver: Box::new(base.clone()), name: "offset".into(), type_arguments: Vec::new(), arguments: vec![index.clone()], span };
        Some(Expr::Unary { op: UnaryOp::Deref, operand: Box::new(offset), span })
    }

    /// Whether `place` is in DGROUP: a module variable, a string's or vec's
    /// data, or anything of a library for BASIC, which runs on BASIC's stack.
    fn in_dgroup(&self, place: &Expr) -> bool {
        self.host().is_some()
            || borrows::expression_owner(place).is_some_and(|owner| self.is_module_variable(owner))
            || self.expression_type_hint(place).is_some_and(|one| self.types.sequence_element(one).is_some())
    }

    /// Whether `name` is a module variable, which lives in DGROUP.
    fn is_module_variable(&self, name: &str) -> bool {
        self.scopes.iter().rposition(|scope| scope.contains_key(name)) == Some(0)
    }

    /// `p.offset(n)`, `p.cast[U]()`, `p.far()` or `p.is_null()` of a raw pointer.
    pub(super) fn pointer_method(
        &mut self,
        receiver: &Expr,
        name: &str,
        type_arguments: &[TypeSpec],
        arguments: &[Expr],
        span: Span,
    ) -> Result<Option<TypedOperand>, Diagnostic> {
        let Some(type_name) = self.expression_type_hint(receiver) else {
            return Ok(None);
        };
        let Some(target) = self.types.raw_target(type_name) else {
            return Ok(None);
        };
        let TypeName::Pointer { type_id: pointer_id, mutable, .. } = type_name else {
            unreachable!("a raw pointer")
        };
        let value = self.expression(receiver, None)?;
        let pointer = required(value, span)?;
        let result = match (name, type_arguments, arguments) {
            // The count is signed: a pointer steps back as readily as on.
            ("offset", [], [count]) => {
                let count = self.coerced(count, TypeName::I16)?;
                let step = self.value(TypeName::I16);
                let width = i64::from(self.types.width(target.id()));
                self.emit("mul", vec![step], vec![required(count, span)?, hir::Operand::Constant(type_id(TypeName::I16), width)], None);
                let moved = self.value(type_name);
                self.emit("ptr_offset", vec![moved], vec![pointer, hir::Operand::Value(step)], None);
                TypedOperand { operand: Some(hir::Operand::Value(moved)), type_name }
            }
            ("cast", [target], []) => {
                let target = self.types.resolve_element(target, span)?;
                let distance = self.types.types[(pointer_id - 1) as usize].name[1..].split(' ').next().expect("a distance").to_string();
                let cast = self.types.raw_pointer(target, &distance, mutable);
                let moved = self.value(cast);
                self.emit("copy", vec![moved], vec![pointer], None);
                TypedOperand { operand: Some(hir::Operand::Value(moved)), type_name: cast }
            }
            // The same place, named by DGROUP's segment too; or by its offset
            // alone, which the program vouches is in DGROUP.
            // The offset alone, which the program vouches is in DGROUP: the
            // low word of the far pointer's bytes.
            ("near", [], []) => {
                self.require_unsafe("a far pointer's offset", span)?;
                let near = self.types.raw_pointer(target, "near", mutable);
                let whole = self.local_place("$far", pointer_id, 4, true);
                self.emit("store", Vec::new(), vec![hir::Operand::Place(whole), pointer], None);
                let offset = self.value(near);
                let low = hir::Operand::ProjectedPlace { place: whole, indices: Vec::new(), offset: 0, type_id: type_id(near) };
                self.emit("load", vec![offset], vec![low], None);
                TypedOperand { operand: Some(hir::Operand::Value(offset)), type_name: near }
            }
            ("far", [], []) => {
                let far = self.types.raw_pointer(target, name, mutable);
                let base = self.materialized(pointer, pointer_id);
                let moved = self.value(far);
                let place = hir::Operand::IndirectPlace { base, offset: 0, type_id: target.id(), inbounds: false };
                self.emit("address", vec![moved], vec![place], None);
                TypedOperand { operand: Some(hir::Operand::Value(moved)), type_name: far }
            }
            ("is_null", [], []) => {
                let null = self.value(TypeName::Bool);
                self.emit("eq", vec![null], vec![pointer, hir::Operand::Constant(pointer_id, 0)], None);
                TypedOperand { operand: Some(hir::Operand::Value(null)), type_name: TypeName::Bool }
            }
            _ => return Err(Diagnostic::new(span, format!("a raw pointer has no method {name:?} of these arguments"))),
        };
        Ok(Some(result))
    }

    /// `p == q` and the like of two raw pointers of one type, a `*mut` one
    /// converted to its read-only kind to meet one; only near ones, offsets in
    /// one segment, are ordered.
    pub(super) fn pointer_comparison(&mut self, operation: BinaryOp, left: &TypedOperand, right: &TypedOperand, span: Span) -> Option<Result<TypedOperand, Diagnostic>> {
        self.types.raw_target(left.type_name)?;
        let (left, right) = match self.implicit(left.clone(), right.type_name, span) {
            Ok(left) => (left, right.clone()),
            Err(_) => match self.implicit(right.clone(), left.type_name, span) {
                Ok(right) => (left.clone(), right),
                Err(error) => return Some(Err(error)),
            },
        };
        let near = matches!(left.type_name, TypeName::Pointer { width: 2, .. });
        let op = match operation {
            BinaryOp::Equal => "eq",
            BinaryOp::NotEqual => "ne",
            BinaryOp::Less if near => "below",
            BinaryOp::LessEqual if near => "beloweq",
            BinaryOp::Greater if near => "above",
            BinaryOp::GreaterEqual if near => "aboveeq",
            _ => return Some(Err(Diagnostic::new(span, "raw pointers compare with '==' and '!='; near ones are also ordered"))),
        };
        let result = self.value(TypeName::Bool);
        let mut operands = match (required(left.clone(), span), required(right.clone(), span)) {
            (Ok(left), Ok(right)) => vec![left, right],
            (Err(error), _) | (_, Err(error)) => return Some(Err(error)),
        };
        // Ordered, near pointers compare as their offsets.
        if !matches!(operation, BinaryOp::Equal | BinaryOp::NotEqual) {
            for operand in &mut operands {
                let offset = self.value(TypeName::U16);
                self.emit("pointer_offset", vec![offset], vec![operand.clone()], None);
                *operand = hir::Operand::Value(offset);
            }
        }
        self.emit(op, vec![result], operands, None);
        Some(Ok(TypedOperand { operand: Some(hir::Operand::Value(result)), type_name: TypeName::Bool }))
    }
}
