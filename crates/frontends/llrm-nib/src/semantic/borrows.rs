//! Borrows (section 8). Each reference or view binding records the owners it
//! borrows from, its roots: bindings, by the storage the compiler gave each,
//! never by spelling. A returned borrow must root in a borrowed parameter, a
//! stored one must not outlive its roots, and no owner is written while a
//! binding in scope borrows it.

use super::*;
use crate::syntax::Pattern;

/// What holds a borrow: a reference's or view's value, or a struct's place
/// that keeps a view. Also a binding's identity, as a root.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum BorrowKey {
    Value(u32),
    Place(u32),
}

fn borrow_key(storage: &Storage) -> Option<BorrowKey> {
    match storage {
        Storage::Reference(value) | Storage::Slice(value) => Some(BorrowKey::Value(*value)),
        Storage::Place(place) => Some(BorrowKey::Place(*place)),
        _ => None,
    }
}

/// The binding `storage` is, as an owner.
pub(super) fn identity(storage: &Storage) -> Option<BorrowKey> {
    match storage {
        Storage::Parameter(value) => Some(BorrowKey::Value(*value)),
        _ => borrow_key(storage),
    }
}

/// How long a root lives, longest first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Life {
    Module,
    /// What the caller lent: a borrowed parameter's referent.
    Lent,
    /// An owned parameter, dropped as the frame ends.
    Frame,
    /// A local, by the depth of its scope.
    Scope(usize),
}

/// An owner a borrow borrows from.
#[derive(Clone, Debug)]
pub(super) struct Root {
    pub(super) owner: BorrowKey,
    pub(super) name: String,
    pub(super) life: Life,
}

impl Root {
    /// Whether a borrow of it may outlive the frame: only what the caller lent.
    pub(super) fn outlives_frame(&self) -> bool {
        self.life == Life::Lent
    }
}

impl PartialEq for Root {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner
    }
}

impl Eq for Root {}

impl PartialOrd for Root {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Root {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.owner.cmp(&other.owner)
    }
}

impl FunctionCompiler<'_> {
    /// The owners a borrow of `expression` borrows from, by name.
    pub(super) fn roots(&self, expression: &Expr) -> BTreeSet<Root> {
        match expression {
            Expr::Name(name, _) => match self.resolve(name) {
                Some((depth, binding)) => match borrow_key(&binding.storage).and_then(|key| self.borrowed_from.get(&key)) {
                    Some(roots) => roots.clone(),
                    None => self.root(name, depth, binding).into_iter().collect(),
                },
                None => BTreeSet::new(),
            },
            // What a caller lent outlives the struct that keeps it.
            Expr::Member { base, span, .. } if self.kept_borrow(base, expression, *span) => BTreeSet::new(),
            Expr::Borrow { operand: base, .. }
            | Expr::Slice { base, .. }
            | Expr::Member { base, .. }
            | Expr::Index { base, .. } => self.roots(base),
            Expr::Conditional { then, otherwise, .. } => {
                self.roots(then).into_iter().chain(self.roots(otherwise)).collect()
            }
            Expr::MethodCall { receiver, name, .. } if name == "bytes" => self.roots(receiver),
            Expr::Call { .. } | Expr::MethodCall { .. } => self.call_roots(expression),
            // A struct borrows what the views and references it keeps borrow.
            Expr::StructLiteral { name, fields, .. } => {
                let Some(layout) = self.types.structs.get(name) else {
                    return BTreeSet::new();
                };
                fields
                    .iter()
                    .filter_map(|(field, value, _)| match layout.fields.get(field)?.type_ {
                        ElementType::Struct(id) if self.types.kept_views.contains_key(&id) => Some(self.roots(value)),
                        kept => Some(self.value_roots(value, kept)),
                    })
                    .flatten()
                    .collect()
            }
            _ => BTreeSet::new(),
        }
    }

    /// The innermost binding of `name` outside the hidden scopes, with the
    /// depth of its scope.
    fn resolve(&self, name: &str) -> Option<(usize, &Binding)> {
        self.scopes
            .iter()
            .enumerate()
            .rev()
            .filter(|(index, _)| !self.hidden.iter().any(|range| range.contains(index)))
            .find_map(|(index, scope)| Some((index, scope.get(name)?)))
    }

    /// `binding`, named `name` in the scope at `depth`, as a root.
    fn root(&self, name: &str, depth: usize, binding: &Binding) -> Option<Root> {
        // An element of an array borrows the array.
        let owner = match binding.storage {
            Storage::ArrayView { place, .. } => BorrowKey::Place(place),
            _ => identity(&binding.storage)?,
        };
        let module = matches!(owner, BorrowKey::Place(place) if self.places.iter().any(|one| one.id == place && one.storage == "module"));
        let life = match self.parameter_lives.get(&owner) {
            _ if module => Life::Module,
            Some(life) => *life,
            None => Life::Scope(depth),
        };
        Some(Root { owner, name: name.to_owned(), life })
    }

    /// The binding `name` names here, as a root.
    pub(super) fn named_root(&self, name: &str) -> Option<Root> {
        let (depth, binding) = self.resolve(name)?;
        self.root(name, depth, binding)
    }

    /// Whether `member`, a field of `base`, borrows only what a caller lent:
    /// a kept view, or a frame's field that holds a borrow, which the frame
    /// keeps only of what its caller lent it (escaping.rs).
    fn kept_borrow(&self, base: &Expr, member: &Expr, span: Span) -> bool {
        let element = match self.struct_expression_type(member, span).ok().flatten() {
            Some(id) => Some(ElementType::Struct(id)),
            None => self.expression_type_hint(member).map(ElementType::Scalar),
        };
        let kept_view = matches!(element, Some(ElementType::Struct(id)) if self.types.kept_views.contains_key(&id));
        let in_frame = self.struct_expression_type(base, span).ok().flatten().is_some_and(|id| self.types.frames.contains_key(&id));
        kept_view || in_frame && element.is_some_and(|one| self.frame_of(one).is_some() || self.holds_reference(one))
    }

    /// A call's result borrows from every argument it borrowed (section 8).
    fn call_roots(&self, expression: &Expr) -> BTreeSet<Root> {
        let call = self.method_as_call(expression).unwrap_or_else(|| expression.clone());
        let Expr::Call { name, arguments, .. } = &call else {
            return BTreeSet::new();
        };
        let Some(signature) = self.known_signature(name) else {
            return BTreeSet::new();
        };
        arguments
            .iter()
            .zip(&signature.parameters)
            .filter(|(_, parameter)| matches!(parameter, SignatureParameter::Borrowed { .. }))
            .flat_map(|(argument, _)| self.roots(argument))
            .collect()
    }

    /// The owners the references inside `expression`, a value of `element`,
    /// borrow from: a reference's roots, and those of each reference field.
    fn value_roots(&self, expression: &Expr, element: ElementType) -> BTreeSet<Root> {
        if !self.holds_reference(element) {
            return BTreeSet::new();
        }
        if let ElementType::Scalar(_) = element {
            return self.roots(expression);
        }
        let ElementType::Struct(id) = element else {
            unreachable!("an element is a scalar or a struct")
        };
        match expression {
            Expr::Conditional { then, otherwise, .. } => {
                self.value_roots(then, element).into_iter().chain(self.value_roots(otherwise, element)).collect()
            }
            Expr::Variant { name, arguments, .. } => {
                let Some(variant) = self.types.enum_of(element).and_then(|one| one.variants.iter().find(|one| &one.name == name)) else {
                    return BTreeSet::new();
                };
                let fields: Vec<_> = variant.fields.iter().map(|(_, field)| field.type_).collect();
                arguments.iter().zip(fields).flat_map(|(argument, field)| self.value_roots(argument, field)).collect()
            }
            Expr::Tuple(items, _) => {
                let layout = self.types.structure(id).expect("a tuple layout");
                items.iter().zip(&layout.order).flat_map(|(item, field)| self.value_roots(item, layout.fields[field].type_)).collect()
            }
            Expr::StructLiteral { fields, .. } => {
                let layout = self.types.structure(id).expect("a struct layout");
                fields
                    .iter()
                    .filter_map(|(name, value, _)| Some(self.value_roots(value, layout.fields.get(name)?.type_)))
                    .flatten()
                    .collect()
            }
            // Any other value holding a reference is a name or a call.
            _ => self.roots(expression),
        }
    }

    /// Whether a value of `element` holds a reference.
    pub(super) fn holds_reference(&self, element: ElementType) -> bool {
        match element {
            ElementType::Scalar(type_name) => self.types.referent(type_name).is_some(),
            ElementType::Struct(id) => self
                .types
                .structure(id)
                .is_some_and(|layout| layout.fields.values().any(|field| self.holds_reference(field.type_))),
        }
    }

    /// Records that `binding`, just made from `source`, borrows what it does.
    pub(super) fn record_borrow(&mut self, binding: &Binding, source: &Expr) {
        if let Some(key) = borrow_key(&binding.storage) {
            let roots = self.roots(source);
            self.borrowed_from.insert(key, roots);
        }
    }

    /// Records that the struct at `place` borrows what the views `value`
    /// keeps borrow, if it keeps any.
    pub(super) fn keep_borrows(&mut self, place: u32, value: &Expr) {
        let roots = match value {
            Expr::StructLiteral { .. } => self.roots(value),
            Expr::Name(name, _) => match self.visible(name).map(|one| one.storage.clone()) {
                Some(Storage::Place(source)) => self.borrowed_from.get(&BorrowKey::Place(source)).cloned().unwrap_or_default(),
                _ => BTreeSet::new(),
            },
            _ => BTreeSet::new(),
        };
        if !roots.is_empty() {
            self.borrowed_from.insert(BorrowKey::Place(place), roots);
        }
    }

    /// Binds `name` to `binding`, a borrow made from `source`. A view bound
    /// with `let mut` gets a descriptor of its own, which assignment reseats.
    pub(super) fn bind_borrow(&mut self, name: &str, reseatable: bool, binding: Binding, source: &Expr) {
        let binding = match (reseatable, &binding) {
            (true, Binding { type_: BindingType::Slice { element, rank }, storage: Storage::Slice(descriptor), .. }) => {
                let own = self.view_slot(*element, *rank);
                self.copy_view(*descriptor, own, *element, *rank);
                self.reseatable.insert(own);
                Binding { storage: Storage::Slice(own), ..binding }
            }
            _ => binding,
        };
        self.record_borrow(&binding, source);
        self.scopes.last_mut().expect("scope").insert(name.to_owned(), binding);
    }

    /// `name = value` of a view bound with `let mut`: it views what `value`
    /// does from now on. `false` when `name` is no such view.
    pub(super) fn reseat(&mut self, name: &str, value: &Expr, span: Span) -> Result<bool, Diagnostic> {
        let Some(Binding { type_: BindingType::Slice { element, rank }, storage: Storage::Slice(own), .. }) = self.visible(name).cloned() else {
            return Ok(false);
        };
        if !self.reseatable.contains(&own) {
            return Ok(false);
        }
        self.check_outlives(name, &self.roots(value), span)?;
        let source = match self.view_of(value)? {
            Some((descriptor, found, found_rank)) if (found, found_rank) == (element, rank) => descriptor,
            Some(_) => return Err(Diagnostic::new(span, format!("{name:?} views another element type or rank"))),
            None => {
                let pointer_type = self.types.slice_pointer(element, rank);
                let (hir::Operand::Value(descriptor), _) = self.borrow_argument(value, false, BindingType::Slice { element, rank }, pointer_type)? else {
                    unreachable!("a view is a descriptor pointer")
                };
                descriptor
            }
        };
        self.copy_view(source, own, element, rank);
        let roots = self.roots(value);
        self.borrowed_from.insert(BorrowKey::Value(own), roots);
        Ok(true)
    }

    /// Records that the names `pattern` just bound borrow from `subject`:
    /// its views and references, and the copies that share what it owns.
    pub(super) fn record_pattern_borrows(&mut self, pattern: &Pattern, subject: &Expr) {
        let roots = self.roots(subject);
        for name in pattern.names() {
            let Some(binding) = self.visible(name).cloned() else {
                continue;
            };
            let shares = match binding.type_ {
                BindingType::Scalar(type_name) => ownership::needs_drop(type_name),
                BindingType::Struct(id) => self.element_needs_drop(ElementType::Struct(id)),
                _ => false,
            };
            // A copied struct borrows by the references it holds.
            let refers = matches!(binding.type_, BindingType::Struct(id) if self.holds_reference(ElementType::Struct(id)));
            match borrow_key(&binding.storage) {
                Some(key @ BorrowKey::Value(_)) => {
                    self.borrowed_from.entry(key).or_insert_with(|| roots.clone());
                }
                Some(key @ BorrowKey::Place(_)) if refers || (shares && !self.owns(&binding.storage)) => {
                    self.borrowed_from.entry(key).or_insert_with(|| roots.clone());
                }
                _ => {}
            }
        }
    }

    /// Errs unless every borrow `expression` returns roots in a parameter:
    /// a local would be gone when the caller reads it.
    pub(super) fn check_returned_borrows(&self, expression: &Expr, span: Span) -> Result<(), Diagnostic> {
        let roots = match (self.signature.view, self.signature.slot) {
            (Some(_), _) => self.roots(expression),
            (None, Some(struct_id)) => self.value_roots(expression, ElementType::Struct(struct_id)),
            (None, None) => self.value_roots(expression, ElementType::Scalar(self.signature.result)),
        };
        match roots.iter().find(|one| !one.outlives_frame()) {
            Some(local) => Err(Diagnostic::new(span, format!("a returned borrow of {:?} would dangle; only a borrowed parameter's can be returned", local.name))),
            None => Ok(()),
        }
    }

    /// Errs when a binding in scope, other than `owner` itself, borrows `owner`.
    pub(super) fn check_unborrowed(&self, owner: &str, span: Span) -> Result<(), Diagnostic> {
        if self.resolve(owner).and_then(|(_, binding)| identity(&binding.storage)).is_some_and(|key| self.is_borrowed(key)) {
            return Err(Diagnostic::new(span, format!("{owner:?} is borrowed here, so it cannot be changed")));
        }
        Ok(())
    }

    /// Errs when the binding that is `owner`, about to move, is borrowed.
    pub(super) fn check_movable(&self, owner: moves::Owner, span: Span) -> Result<(), Diagnostic> {
        let key = match owner {
            (false, place) => BorrowKey::Place(place),
            (true, value) => BorrowKey::Value(value),
        };
        let name = self.scopes.iter().rev().flat_map(|scope| scope.iter()).find(|(_, one)| moves::owner(&one.storage) == Some(owner));
        match name {
            Some((name, _)) if self.is_borrowed(key) => {
                Err(Diagnostic::new(span, format!("{name:?} is borrowed here, so it cannot be moved")))
            }
            _ => Ok(()),
        }
    }

    fn is_borrowed(&self, owner: BorrowKey) -> bool {
        let borrowed = self.scopes.iter().flat_map(|scope| scope.values()).any(|binding| {
            identity(&binding.storage) != Some(owner)
                && borrow_key(&binding.storage)
                    .and_then(|key| self.borrowed_from.get(&key))
                    .is_some_and(|roots| roots.iter().any(|root| root.owner == owner))
        });
        borrowed || self.iterated.contains(&owner)
    }

    /// Errs unless each field `place` writes through was declared `mut`
    /// (section 5).
    pub(super) fn check_mutable_fields(&self, place: &Expr) -> Result<(), Diagnostic> {
        match place {
            Expr::Member { base, field, span } => {
                if let Some(owner) = self.receiver_type(base) {
                    // An instance of a generic type is declared by its template.
                    let declared = owner.split('[').next().unwrap_or(&owner);
                    if self.types.fixed_fields.contains(&(declared.to_owned(), field.clone())) {
                        return Err(Diagnostic::new(*span, format!("field {field:?} of {owner} is not declared 'mut'")));
                    }
                }
                self.check_mutable_fields(base)
            }
            Expr::Index { base, .. } | Expr::Slice { base, .. } => self.check_mutable_fields(base),
            _ => Ok(()),
        }
    }

    /// Errs when a borrow rooted in `roots` is stored where `target` holds
    /// it: `target` must not outlive any of them.
    pub(super) fn check_outlives(&self, target: &str, roots: &BTreeSet<Root>, span: Span) -> Result<(), Diagnostic> {
        let Some(target) = self.named_root(target) else {
            return Ok(());
        };
        match roots.iter().find(|root| root.life > target.life) {
            Some(root) => Err(Diagnostic::new(span, format!("{:?} would outlive {:?}, which it borrows", target.name, root.name))),
            None => Ok(()),
        }
    }

    /// Errs when assigning `value`, of `element`, through `target` would
    /// store a borrow of something `target`'s owner outlives.
    pub(super) fn check_assigned_borrows(&self, target: &AssignTarget, value: &Expr, element: ElementType, span: Span) -> Result<(), Diagnostic> {
        let Some(owner) = written_owner(target) else {
            return Ok(());
        };
        self.check_outlives(owner, &self.value_roots(value, element), span)
    }
}

/// The name an assignment writes through: its root binding.
pub(super) fn written_owner(target: &AssignTarget) -> Option<&str> {
    match target {
        AssignTarget::Name(name) => Some(name),
        AssignTarget::Index { base, .. } | AssignTarget::Member { base, .. } => expression_owner(base),
        AssignTarget::Deref(_) => None,
    }
}

pub(super) fn expression_owner(expression: &Expr) -> Option<&str> {
    match expression {
        Expr::Name(name, _) => Some(name),
        Expr::Member { base, .. } | Expr::Index { base, .. } | Expr::Slice { base, .. } => expression_owner(base),
        _ => None,
    }
}
