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

/// An owner a borrow borrows from. A generator's placeholders share one
/// storage, so the name is part of what tells roots apart.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Root {
    pub(super) owner: BorrowKey,
    pub(super) name: String,
    pub(super) life: Life,
}

impl Life {
    /// Whether an owner living this long may hold a borrow of `root`. What
    /// the caller lent holds only what it lent: the callee cannot know how
    /// the rest compares.
    pub(super) fn may_hold(self, root: &Root) -> bool {
        match self {
            Life::Lent => root.life == Life::Lent,
            held => root.life <= held,
        }
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
            // A reference read out of a field or an element borrows what
            // the owner holds, not the owner.
            Expr::Member { base, .. } | Expr::Index { base, .. } if self.reference_type(expression) => self.held_roots(base),
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
    pub(super) fn value_roots(&self, expression: &Expr, element: ElementType) -> BTreeSet<Root> {
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
            Expr::Name(..) => self.held_roots(expression),
            // Any other value holding a reference is a call.
            _ => self.roots(expression),
        }
    }

    /// The borrows `expression`'s value holds: what its owner holds when
    /// that is known, else its own roots, which outlive none of it.
    fn held_roots(&self, expression: &Expr) -> BTreeSet<Root> {
        let held = match expression {
            Expr::Name(name, _) => self.resolve(name).and_then(|(_, binding)| self.held.get(&identity(&binding.storage)?)),
            _ => None,
        };
        held.cloned().unwrap_or_else(|| self.roots(expression))
    }

    /// Every owner lending `argument` lets a callee reach: what it borrows,
    /// and what the borrowed value holds.
    pub(super) fn reach(&self, argument: &Expr) -> BTreeSet<Root> {
        let place = match argument {
            Expr::Borrow { operand, .. } => operand,
            _ => argument,
        };
        self.roots(argument).into_iter().chain(self.held_roots(place)).collect()
    }

    /// Whether `expression`, not a name, is a reference: a field's, an
    /// element's or a call's. A name is bound as what it refers to.
    pub(super) fn reference_type(&self, expression: &Expr) -> bool {
        !matches!(expression, Expr::Name(..)) && self.expression_type_hint(expression).is_some_and(|one| self.types.referent(one).is_some())
    }

    /// Whether a value of `element` holds a reference.
    pub(super) fn holds_reference(&self, element: ElementType) -> bool {
        self.holds_reference_where(element, false)
    }

    /// Whether a value of `element` holds a reference, a `&mut` one when
    /// `exclusive`.
    fn holds_reference_where(&self, element: ElementType, exclusive: bool) -> bool {
        match element {
            ElementType::Scalar(type_name @ TypeName::Pointer { mutable, .. }) if self.types.referent(type_name).is_some() => mutable || !exclusive,
            ElementType::Scalar(type_name) => self.types.sequence_element(type_name).is_some_and(|element| self.holds_reference_where(element, exclusive)),
            ElementType::Struct(id) => self
                .types
                .structure(id)
                .is_some_and(|layout| layout.fields.values().any(|field| self.holds_reference_where(field.type_, exclusive))),
        }
    }

    /// Records that `binding`, just made from `source`, borrows what it does.
    pub(super) fn record_borrow(&mut self, binding: &Binding, source: &Expr) {
        if let Some(key) = borrow_key(&binding.storage) {
            let roots = self.roots(source);
            self.borrowed_from.insert(key, roots);
        }
    }

    /// Records that the struct at `place` holds what the views and
    /// references `value` keeps borrow, if it keeps any.
    pub(super) fn keep_borrows(&mut self, place: u32, value: &Expr) {
        let roots = match value {
            Expr::StructLiteral { .. } => self.roots(value),
            Expr::Name(name, _) => match self.visible(name).and_then(|one| identity(&one.storage)) {
                Some(source) => self.held.get(&source).cloned().unwrap_or_default(),
                None => BTreeSet::new(),
            },
            _ => BTreeSet::new(),
        };
        if !roots.is_empty() {
            self.held.insert(BorrowKey::Place(place), roots);
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
        if let Some(target) = self.named_root(name) {
            check_holds(&target, &self.roots(value), span)?;
        }
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
        let held = self.held_roots(subject);
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
                Some(key @ BorrowKey::Place(_)) if shares && !self.owns(&binding.storage) => {
                    self.borrowed_from.entry(key).or_insert_with(|| roots.clone());
                }
                Some(key @ BorrowKey::Place(_)) if refers => {
                    self.held.entry(key).or_insert_with(|| held.clone());
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
        match roots.iter().find(|one| !Life::Lent.may_hold(one)) {
            Some(local) => Err(Diagnostic::new(span, format!("a returned borrow of {:?} would dangle; only a borrowed parameter's can be returned", local.name))),
            None => Ok(()),
        }
    }

    /// Errs when a binding in scope, other than `owner` itself, borrows `owner`.
    pub(super) fn check_unborrowed(&mut self, owner: &str, span: Span) -> Result<(), Diagnostic> {
        match self.resolve(owner).and_then(|(_, binding)| identity(&binding.storage)) {
            Some(key) => self.change_borrowed(key, Diagnostic::new(span, format!("{owner:?} is borrowed here, so it cannot be changed"))),
            None => Ok(()),
        }
    }

    /// Errs when the binding that is `owner`, about to move, is borrowed.
    pub(super) fn check_movable(&mut self, owner: moves::Owner, span: Span) -> Result<(), Diagnostic> {
        let key = match owner {
            (false, place) => BorrowKey::Place(place),
            (true, value) => BorrowKey::Value(value),
        };
        let name = self.scopes.iter().rev().flat_map(|scope| scope.iter()).find(|(_, one)| moves::owner(&one.storage) == Some(owner));
        match name {
            Some((name, _)) => {
                let error = Diagnostic::new(span, format!("{name:?} is borrowed here, so it cannot be moved"));
                self.change_borrowed(key, error)
            }
            None => Ok(()),
        }
    }

    /// The bindings in scope, other than `owner` itself, that borrow it.
    pub(super) fn holders(&self, owner: BorrowKey) -> BTreeSet<BorrowKey> {
        let mut holders = BTreeSet::new();
        for binding in self.scopes.iter().flat_map(|scope| scope.values()) {
            if identity(&binding.storage) == Some(owner) {
                continue;
            }
            let lent = borrow_key(&binding.storage).filter(|key| self.borrowed_from.get(key).is_some_and(|roots| roots.iter().any(|root| root.owner == owner)));
            let held = identity(&binding.storage).filter(|key| self.held.get(key).is_some_and(|roots| roots.iter().any(|root| root.owner == owner)));
            holders.extend(lent.into_iter().chain(held));
        }
        holders
    }

    /// Stores a borrow rooted in `roots` where `container` keeps it: the
    /// one check of every store. Each owner `container` writes into must not
    /// outlive a root, and borrows them from then on.
    pub(super) fn store_borrow(&mut self, container: &Expr, roots: BTreeSet<Root>, span: Span) -> Result<(), Diagnostic> {
        if roots.is_empty() {
            return Ok(());
        }
        for target in self.store_targets(container) {
            check_holds(&target, &roots, span)?;
            if matches!(target.life, Life::Frame | Life::Scope(_)) {
                self.held.entry(target.owner).or_default().extend(roots.iter().cloned());
            }
        }
        Ok(())
    }

    /// The owners a store into `container` writes: its root binding, or
    /// what that binding borrows when it is a reference or a view.
    fn store_targets(&self, container: &Expr) -> Vec<Root> {
        let Some(name) = expression_owner(container) else {
            return Vec::new();
        };
        let Some((depth, binding)) = self.resolve(name) else {
            return Vec::new();
        };
        match (&binding.storage, borrow_key(&binding.storage).and_then(|key| self.borrowed_from.get(&key))) {
            (Storage::Reference(_) | Storage::Slice(_), Some(roots)) => roots.iter().cloned().collect(),
            _ => self.root(name, depth, binding).into_iter().collect(),
        }
    }

    /// What a call lends through each argument: a borrow lends what it
    /// reaches, a value the borrows it holds, which it writes through when
    /// one is `&mut`.
    pub(super) fn lent(&self, arguments: &[Expr], parameters: &[SignatureParameter]) -> Vec<Lent> {
        arguments
            .iter()
            .zip(parameters)
            .map(|(argument, parameter)| {
                let held = |element: ElementType| Lent { roots: self.value_roots(argument, element), mutable: self.holds_reference_where(element, true) };
                match *parameter {
                    SignatureParameter::Borrowed { mutable, .. } => Lent { roots: self.reach(argument), mutable },
                    SignatureParameter::Adapter { .. } => Lent { roots: self.reach(argument), mutable: true },
                    SignatureParameter::Scalar(type_name) => held(ElementType::Scalar(type_name)),
                    SignatureParameter::Owned { struct_id, .. } => held(ElementType::Struct(struct_id)),
                }
            })
            .collect()
    }

    /// Stores what a call is lent: it may keep any of it in what a `&mut`
    /// argument holds.
    pub(super) fn store_call_borrows(&mut self, arguments: &[Expr], parameters: &[SignatureParameter], lent: &[Lent], span: Span) -> Result<(), Diagnostic> {
        let roots: BTreeSet<Root> = lent.iter().flat_map(|one| one.roots.iter().cloned()).collect();
        for (argument, parameter) in arguments.iter().zip(parameters) {
            if let SignatureParameter::Borrowed { mutable: true, target, .. } = *parameter {
                if self.holds_reference(binding_element(target)) {
                    self.store_borrow(argument, roots.clone(), span)?;
                }
            }
        }
        Ok(())
    }

    /// Stores what assigning `value`, of `element`, through `target` keeps.
    pub(super) fn store_assigned_borrows(&mut self, target: &AssignTarget, value: &Expr, element: ElementType, span: Span) -> Result<(), Diagnostic> {
        let Some(owner) = written_owner(target) else {
            return Ok(());
        };
        let roots = self.value_roots(value, element);
        self.store_borrow(&Expr::Name(owner.to_owned(), span), roots, span)
    }
}

/// What a call lends through one argument: the owners its callee reaches,
/// and whether it may write them.
pub(super) struct Lent {
    pub(super) roots: BTreeSet<Root>,
    pub(super) mutable: bool,
}

/// Errs when two of a call's lends reach one owner and either writes it.
pub(super) fn check_disjoint(lent: &[Lent], arguments: &[Expr]) -> Result<(), Diagnostic> {
    for (at, one) in lent.iter().enumerate() {
        for other in lent[..at].iter().filter(|other| one.mutable || other.mutable) {
            if let Some(shared) = one.roots.iter().find(|root| other.roots.iter().any(|them| them.owner == root.owner)) {
                return Err(Diagnostic::new(arguments[at].span(), format!("borrow of {:?} aliases a mutable argument", shared.name)));
            }
        }
    }
    Ok(())
}

/// Errs unless `target` may hold a borrow of each of `roots`.
fn check_holds(target: &Root, roots: &BTreeSet<Root>, span: Span) -> Result<(), Diagnostic> {
    match roots.iter().find(|root| !target.life.may_hold(root)) {
        Some(root) => Err(Diagnostic::new(span, format!("{:?} would outlive {:?}, which it borrows", target.name, root.name))),
        None => Ok(()),
    }
}

/// What a binding of `type_` holds, element by element.
fn binding_element(type_: BindingType) -> ElementType {
    match type_ {
        BindingType::Scalar(type_name) => ElementType::Scalar(type_name),
        BindingType::Struct(id) => ElementType::Struct(id),
        BindingType::Array { element, .. } | BindingType::Slice { element, .. } => element,
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
