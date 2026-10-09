//! Ownership of strings (section 9.5): who drops a heap buffer, and when.
//!
//! An owning local is nulled when its value moves, so its drop, which
//! skips null, runs on every path without a separate drop flag. After a
//! move that happens on every path the optimizer folds the drop away. A
//! type with a `drop` method has no null, so its owners carry a flag
//! (drops.rs).

use llrm_core::abi::nib as rt;

use super::*;

/// Where a value of an owning type was read from.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Origin {
    /// An owning local's place: moving the value nulls it.
    Local(u32),
    /// A field of a generator's frame, which is one of its body's locals:
    /// moving the value nulls it too.
    Frame(hir::Operand),
    /// A field of an owner, which moving nulls, alone (#135).
    Field(FieldMove, hir::Operand),
    /// A borrow, field, or element: it cannot move.
    Borrowed,
    /// A literal: static, so a move copies it and nothing drops it.
    Static,
}

/// A move of a field out of the owner a path names: the owner is then
/// partly moved, its other fields still its own.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct FieldMove {
    owner: moves::Owner,
    name: String,
    path: Vec<String>,
    /// Why it cannot move, when it cannot.
    refusal: Option<String>,
    /// It holds a type with a `drop`: no null marks it moved, so its drop is
    /// left out where it surely moved, and flagged where it may have.
    dropped: bool,
}

/// What to do with each owning value an aggregate or vec holds.
#[derive(Clone, Copy)]
pub(super) enum Owned {
    Drop,
    Duplicate,
}

pub(super) fn needs_drop(type_name: TypeName) -> bool {
    matches!(
        type_name,
        TypeName::String | TypeName::Vector { .. } | TypeName::Dictionary { .. }
    )
}

impl FunctionCompiler<'_> {
    /// Records what an owning value read by name came from.
    pub(super) fn record_origin(
        &mut self,
        operand: &hir::Operand,
        type_name: TypeName,
        storage: &Storage,
    ) {
        let hir::Operand::Value(value) = operand else {
            return;
        };
        if !needs_drop(type_name) {
            return;
        }
        let origin = match storage {
            Storage::Place(place) if self.owned_places.contains(place) => Origin::Local(*place),
            _ => Origin::Borrowed,
        };
        self.origins.insert(*value, origin);
    }

    /// A fresh owned value, dropped at the end of its statement unless it moves.
    pub(super) fn temporary_owned(
        &mut self,
        operand: hir::Operand,
        type_name: TypeName,
    ) -> hir::Operand {
        if needs_drop(type_name) {
            self.temporaries.push((operand.clone(), type_name));
        }
        operand
    }

    /// A binding that owns what it holds and drops it at scope exit.
    /// Whether this binding's storage is dropped at its scope's end.
    pub(super) fn owns(
        &self,
        storage: &Storage,
    ) -> bool {
        match storage {
            Storage::Place(place) => self.owned_places.contains(place),
            Storage::Reference(pointer) => self.owned_references.contains(pointer),
            _ => false,
        }
    }

    pub(super) fn own(
        &mut self,
        place: u32,
    ) {
        self.owned_places.insert(place);
    }

    pub(super) fn is_static(
        &self,
        value: &TypedOperand,
    ) -> bool {
        matches!(
            &value.operand,
            Some(hir::Operand::Value(id)) if self.origins.get(id) == Some(&Origin::Static)
        )
    }

    /// Takes ownership of `value` for a new owner.
    pub(super) fn consume(
        &mut self,
        value: &TypedOperand,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !needs_drop(value.type_name) {
            return Ok(());
        }
        let Some(operand) = &value.operand else {
            return Ok(());
        };
        if let Some(at) = self.temporaries.iter().position(|(one, _)| one == operand) {
            self.temporaries.remove(at);
            return Ok(());
        }
        let hir::Operand::Value(id) = operand else {
            return Ok(());
        };
        let null = hir::Operand::Constant(type_id(value.type_name), 0);
        match self.origins.get(id).cloned() {
            Some(Origin::Local(place)) => {
                self.check_movable((false, place), span)?;
                let null = hir::Operand::Constant(type_id(value.type_name), 0);
                self.emit("store", Vec::new(), vec![hir::Operand::Place(place), null], None);
                self.mark_moved(((false, place), Vec::new()));
                Ok(())
            }
            Some(Origin::Frame(place)) => {
                self.emit("store", Vec::new(), vec![place, null], None);
                Ok(())
            }
            Some(Origin::Field(moving, place)) => {
                self.move_field(&moving, span)?;
                self.emit("store", Vec::new(), vec![place, null], None);
                Ok(())
            }
            Some(Origin::Static) => Ok(()),
            Some(Origin::Borrowed) | None => {
                Err(Diagnostic::new(span, "cannot move out of a borrow, field, or element; use .copy()"))
            }
        }
    }

    /// Drops what the current statement made and did not move.
    pub(super) fn drop_temporaries(&mut self) {
        self.drop_pending();
        self.temporaries.clear();
        self.aggregate_temporaries.clear();
    }

    /// The same, on a path that leaves the statement early: the rest of it still owns them.
    pub(super) fn drop_pending(&mut self) {
        for (operand, type_name) in self.temporaries.clone() {
            self.emit_drop(operand, type_name);
        }
        for view in self.aggregate_temporaries.clone() {
            self.drop_view(&view);
        }
    }

    /// Drops the owning locals of scopes `depth..`, innermost first.
    pub(super) fn drop_scopes(
        &mut self,
        depth: usize,
    ) {
        let mut owned = Vec::new();
        for scope in self.scopes[depth..].iter().rev() {
            let mut own: Vec<_> = scope
                .iter()
                .filter(|(_, binding)| self.owns(&binding.storage))
                .map(|(name, binding)| (name.clone(), binding.clone()))
                .collect();
            // Reverse construction order: storage is made as its owner is
            // bound, parameters' before any local's.
            own.sort_by_key(|(_, binding)| moves::owner(&binding.storage).map(|(reference, id)| (!reference, id)));
            owned.extend(own.into_iter().rev());
        }
        for (name, binding) in owned {
            // A drop frees what the owner holds: a change to it.
            if let Some(owner) = borrows::identity(&binding.storage) {
                let error =
                    Diagnostic::new(self.statement_span, format!("{name:?} is dropped here while still borrowed"));
                self.drop_borrowed(owner, error);
            }
            match (binding.type_, &binding.storage) {
                (BindingType::Scalar(type_name), Storage::Place(place)) => {
                    let value = self.value(type_name);
                    self.emit("load", vec![value], vec![hir::Operand::Place(*place)], None);
                    self.emit_drop(hir::Operand::Value(value), type_name);
                }
                (BindingType::Struct(struct_id), storage) => {
                    let view = binding_view(struct_id, storage, true, &name).expect("an owned aggregate has storage");
                    self.drop_owner(&view);
                }
                (BindingType::Array { element, shape }, storage) => {
                    let array = self.types.array(element, shape);
                    let view = binding_view(array, storage, true, &name).expect("an owned array has storage");
                    self.drop_owner(&view);
                }
                _ => unreachable!("only strings and aggregates are owned"),
            }
        }
    }

    pub(super) fn emit_drop(
        &mut self,
        operand: hir::Operand,
        type_name: TypeName,
    ) {
        debug_assert!(needs_drop(type_name));
        let element = self.types.owned_element(type_name).expect("an owning buffer");
        if self.element_needs_drop(element) {
            // A moved-from vec is null and owns no elements.
            let hir::Operand::Value(vector) = operand else { unreachable!("a vec with owning elements is a value") };
            let done = self.block();
            self.branch_unless("ne", operand.clone(), hir::Operand::Constant(type_id(type_name), 0), done);
            self.each_element(vector, element, Owned::Drop);
            self.terminate(jump(done));
            self.current = done;
        }
        self.emit_builtin(rt::BUFFER_DROP, vec![operand]);
    }
}

impl FunctionCompiler<'_> {
    /// Whether a value of this type holds something to drop.
    pub(super) fn element_needs_drop(
        &self,
        element: ElementType,
    ) -> bool {
        match element {
            ElementType::Scalar(type_name) => needs_drop(type_name),
            ElementType::Struct(id) if self.types.array_of(id).is_some() => {
                self.element_needs_drop(self.types.array_of(id).expect("an array").0)
            }
            ElementType::Struct(id) => {
                let layout = self.types.structure(id).expect("registered layout");
                self.types.dropped.contains_key(&id)
                    || layout.fields.values().any(|field| self.element_needs_drop(field.type_))
            }
        }
    }

    pub(super) fn drop_view(
        &mut self,
        view: &StructView,
    ) {
        self.each_owned(view, Owned::Drop);
    }

    /// Applies `action` to what an aggregate owns; an enum, only what its
    /// current variant does.
    pub(super) fn each_owned(
        &mut self,
        view: &StructView,
        action: Owned,
    ) {
        if let Some((element, shape)) = self.types.array_of(view.struct_id) {
            return self.owned_array(view, element, shape, action);
        }
        if let Owned::Drop = action {
            self.call_drop(view);
        }
        if let Some(layout) = self.types.enum_of(ElementType::Struct(view.struct_id)).cloned() {
            let tag = self.load_tag(view, &layout);
            for variant in &layout.variants {
                if !variant.fields.iter().any(|(_, field)| self.element_needs_drop(field.type_)) {
                    continue;
                }
                let expected = hir::Operand::Constant(type_id(layout.tag), variant.tag);
                let next = self.block();
                self.branch_unless("eq", hir::Operand::Value(tag), expected, next);
                for (_, field) in &variant.fields {
                    self.owned_field(view, *field, action);
                }
                self.terminate(jump(next));
                self.current = next;
            }
            return;
        }
        let layout = self.types.structure(view.struct_id).expect("registered layout").clone();
        for (name, field) in &layout.fields {
            let flag = match action {
                Owned::Drop => self.frame_flag(view, name),
                Owned::Duplicate => None,
            };
            self.when_live(flag, |this| this.owned_field(view, *field, action));
        }
    }

    pub(super) fn owned_field(
        &mut self,
        view: &StructView,
        field: FieldLayout,
        action: Owned,
    ) {
        if let Some(shape) = field.shape {
            let struct_id = self.types.array(field.type_, shape);
            let array = StructView { struct_id, offset: view.offset + field.offset, ..view.clone() };
            return self.owned_array(&array, field.type_, shape, action);
        }
        match field.type_ {
            ElementType::Scalar(type_name) if needs_drop(type_name) => {
                let place = self.field_place(view, field.offset, type_name);
                self.owned_leaf(place, type_name, action);
            }
            ElementType::Struct(struct_id) if self.element_needs_drop(field.type_) => {
                let inner = StructView { struct_id, offset: view.offset + field.offset, ..view.clone() };
                self.each_owned(&inner, action);
            }
            ElementType::Scalar(_) | ElementType::Struct(_) => {}
        }
    }

    /// Applies `action` to what each element owns of the array of `element`
    /// and `shape` at `array`.
    pub(super) fn owned_array(
        &mut self,
        array: &StructView,
        element: ElementType,
        shape: Shape,
        action: Owned,
    ) {
        if !self.element_needs_drop(element) {
            return;
        }
        let slot = FieldLayout { type_: element, offset: 0, shape: None };
        for index in 0..shape.len() {
            let one = self.element_view(array, element, shape, index);
            self.owned_field(&one, slot, action);
        }
    }

    /// Drops the owning value at `place`, or replaces it with its own copy.
    pub(super) fn owned_leaf(
        &mut self,
        place: hir::Operand,
        type_name: TypeName,
        action: Owned,
    ) {
        let value = self.value(type_name);
        self.emit("load", vec![value], vec![place.clone()], None);
        match action {
            Owned::Drop => self.emit_drop(hir::Operand::Value(value), type_name),
            Owned::Duplicate => {
                let copy = self.emit_copy(hir::Operand::Value(value), type_name);
                self.emit("store", Vec::new(), vec![place, copy], None);
            }
        }
    }

    /// Takes an aggregate's contents from `expression` for a new owner:
    /// a local is left empty, which its drop skips, by `stores` made after
    /// the copy's.
    pub(super) fn consume_aggregate(
        &mut self,
        expression: &Expr,
        source: &StructView,
        span: Span,
        stores: &mut Vec<Store>,
    ) -> Result<(), Diagnostic> {
        if !self.element_needs_drop(ElementType::Struct(source.struct_id)) {
            return Ok(());
        }
        // A statement's temporary is moved by no longer dropping it.
        let temporary = self
            .aggregate_temporaries
            .iter()
            .position(|one| one.place == source.place && one.pointer == source.pointer);
        if let (Some(index), None) = (temporary, source.pointer) {
            self.aggregate_temporaries.remove(index);
            return Ok(());
        }
        if let Expr::Member { base, field, .. } = expression {
            if let Some(flag) = self.frame_field(base, field, span)? {
                stores.extend(self.zero_stores(source, self.types.copy_units(ElementType::Struct(source.struct_id))));
                if let Some(flag) = flag {
                    stores.push(Store::One(flag, hir::Operand::Constant(BOOL, 0)));
                }
                return Ok(());
            }
        }
        if let Some(moving) = self.field_move(expression) {
            self.move_field(&moving, span)?;
            stores.extend(self.zero_stores(source, self.types.copy_units(ElementType::Struct(source.struct_id))));
            return Ok(());
        }
        let owned_local = match expression {
            Expr::Name(name, _) => {
                let storage = self.binding(name, span)?.storage.clone();
                self.owns(&storage).then_some(storage)
            }
            _ => None,
        };
        let Some(storage) = owned_local else {
            return Err(Diagnostic::new(span, "cannot move out of a borrow, field, or element; use .copy()"));
        };
        if let Some(owner) = moves::owner(&storage) {
            self.check_movable(owner, span)?;
            self.mark_moved((owner, Vec::new()));
            self.set_live(owner, false);
        }
        stores.extend(self.zero_stores(source, self.types.copy_units(ElementType::Struct(source.struct_id))));
        Ok(())
    }

    /// The move `place` would make, when it is a field of an owner this
    /// function owns: a field of a struct with a `drop` cannot move, as the
    /// `drop` sees the whole.
    pub(super) fn field_move(
        &self,
        place: &Expr,
    ) -> Option<FieldMove> {
        let (owner, name, path) = self.projected(place)?;
        let binding = self.visible(&name)?;
        if !self.owns(&binding.storage) {
            return None;
        }
        let BindingType::Struct(mut id) = binding.type_ else {
            return None;
        };
        let mut refusal = None;
        let mut element = ElementType::Struct(id);
        for field in &path {
            let layout = self.types.structure(id)?;
            if self.types.dropped.contains_key(&id) {
                refusal.get_or_insert(format!("cannot move a field out of {}, which has a drop", layout.name));
            }
            let found = layout.fields.get(field)?;
            if found.shape.is_some() {
                return None;
            }
            element = found.type_;
            if let ElementType::Struct(next) = element {
                id = next;
            }
        }
        let dropped = self.holds_user_drop(element);
        Some(FieldMove { owner, name, path, refusal, dropped })
    }

    /// Moves `moving`'s field out of its owner, which is then partly moved.
    fn move_field(
        &mut self,
        moving: &FieldMove,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if let Some(refusal) = &moving.refusal {
            return Err(Diagnostic::new(span, refusal.clone()));
        }
        let key = match moving.owner {
            (false, place) => borrows::BorrowKey::Place(place),
            (true, value) => borrows::BorrowKey::Value(value),
        };
        let error = Diagnostic::new(span, format!("{:?} is borrowed here, so it cannot be moved", moving.name));
        self.change_borrowed(key, &moving.path, None, error)?;
        self.mark_moved((moving.owner, moving.path.clone()));
        if moving.dropped {
            let at = self.current_block_mut().instructions.len();
            self.field_moves.push(((moving.owner, moving.path.clone()), self.current, at));
        }
        Ok(())
    }

    /// Stores of zero to each of `units`, (offset, type, count) runs of cells at `view`.
    pub(super) fn zero_stores(
        &self,
        view: &StructView,
        units: Vec<(u32, TypeName, u32)>,
    ) -> Vec<Store> {
        // An array's cells are a run even when one: its place is projected by element.
        let array = self.types.array_of(view.struct_id).is_some();
        units
            .into_iter()
            .map(|(offset, type_name, count)| {
                let zero = hir::Operand::Constant(type_id(type_name), 0);
                match count {
                    1 if !array => Store::One(self.projected_place(view, offset, type_name), zero),
                    _ => Store::Run {
                        destination: StructView { offset: view.offset + offset, ..view.clone() },
                        element: ElementType::Scalar(type_name),
                        count,
                        source: RunSource::Value(zero),
                    },
                }
            })
            .collect()
    }
}
