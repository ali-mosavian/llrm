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

/// The place a borrow borrows from: an owner, told apart by its storage
/// alone, and the fields below it; the name is for diagnostics.
#[derive(Clone, Debug)]
pub(super) struct Root {
    pub(super) owner: BorrowKey,
    pub(super) name: String,
    pub(super) life: Life,
    pub(super) path: Vec<String>,
    /// The borrow is of exactly `path`, not of something somewhere below
    /// it, as a call's result or an element is: a field of it is `path`'s.
    pub(super) exact: bool,
}

impl Root {
    pub(super) fn new(owner: BorrowKey, name: &str, life: Life) -> Self {
        Root { owner, name: name.to_owned(), life, path: Vec::new(), exact: true }
    }

    /// Whether a borrow of it and one of `path` in `owner` may overlap: one
    /// place holds the other. An element's fields are not told apart.
    pub(super) fn overlaps(&self, owner: BorrowKey, path: &[String]) -> bool {
        self.owner == owner && (self.path.starts_with(path) || path.starts_with(&self.path))
    }

    fn field(mut self, field: &str) -> Self {
        if self.exact {
            self.path.push(field.to_owned());
        }
        self
    }

    fn somewhere(self) -> Self {
        Root { exact: false, ..self }
    }
}

impl Root {
    fn identity(&self) -> (BorrowKey, Life, &[String], bool) {
        (self.owner, self.life, &self.path, self.exact)
    }
}

impl PartialEq for Root {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
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
        self.identity().cmp(&other.identity())
    }
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
            // A frame's field holds only what its caller lent it.
            Expr::Member { base, field, span } if self.frame_borrow(base, expression, *span) => {
                let frame = self.struct_expression_type(base, *span).ok().flatten().and_then(|id| self.types.frames.get(&id));
                let lent = frame.and_then(|one| one.lent.get(field)).expect("a frame's field");
                BTreeSet::from([Root { exact: false, ..Root::new(BorrowKey::Place(*lent), field, Life::Lent) }])
            }
            // A reference read out of a field or an element borrows what
            // the owner holds, not the owner.
            Expr::Member { base, .. } | Expr::Index { base, .. } if self.reference_type(expression) => self.held_roots(base),
            Expr::Member { base, field, .. } => self.roots(base).into_iter().map(|root| root.field(field)).collect(),
            Expr::Index { base, .. } | Expr::Slice { base, .. } => self.roots(base).into_iter().map(Root::somewhere).collect(),
            Expr::Borrow { operand: base, .. } => self.roots(base),
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

    /// Whether `member`, a field of the generator frame `base`, holds a
    /// borrow: a view or reference it keeps, or a generator.
    fn frame_borrow(&self, base: &Expr, member: &Expr, span: Span) -> bool {
        let in_frame = self.struct_expression_type(base, span).ok().flatten().is_some_and(|id| self.types.frames.contains_key(&id));
        let element = match self.struct_expression_type(member, span).ok().flatten() {
            Some(id) => Some(ElementType::Struct(id)),
            None => self.expression_type_hint(member).map(ElementType::Scalar),
        };
        let kept_view = |one: ElementType| matches!(one, ElementType::Struct(id) if self.types.kept_views.contains_key(&id));
        in_frame && element.is_some_and(|one| kept_view(one) || self.frame_of(one).is_some() || self.holds_reference(one))
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
        Some(Root::new(owner, name, life))
    }

    /// The binding `name` names here, as a root.
    pub(super) fn named_root(&self, name: &str) -> Option<Root> {
        let (depth, binding) = self.resolve(name)?;
        self.root(name, depth, binding)
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
            .map(Root::somewhere)
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

    /// Records that the binding at `place`, a value of `element` made from
    /// `value`, holds the borrows `value` holds, if any: binding one is a
    /// store.
    pub(super) fn keep_borrows(&mut self, place: u32, element: ElementType, value: &Expr) {
        let roots = match value {
            // A literal also keeps the views its fields hold.
            Expr::StructLiteral { .. } => self.roots(value),
            _ => self.value_roots(value, element),
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
    pub(super) fn check_returned_borrows(&mut self, expression: &Expr, span: Span) -> Result<(), Diagnostic> {
        let roots = match (self.signature.view, self.signature.slot) {
            (Some(_), _) => self.roots(expression),
            (None, Some(struct_id)) => self.value_roots(expression, ElementType::Struct(struct_id)),
            (None, None) => self.value_roots(expression, ElementType::Scalar(self.signature.result)),
        };
        match roots.iter().find(|one| !Life::Lent.may_hold(one)) {
            Some(local) => Err(Diagnostic::new(span, format!("a returned borrow of {:?} would dangle; only a borrowed parameter's can be returned", local.name))),
            None => {
                self.keep_lent(&roots);
                Ok(())
            }
        }
    }

    /// Errs when a binding in scope, other than `owner` itself, borrows `owner`.
    /// Whether `binding` may change what it borrows: a `&mut`, or a value
    /// holding one.
    fn may_change(&self, binding: &Binding) -> bool {
        match (&binding.storage, binding.type_) {
            (Storage::Reference(_) | Storage::Slice(_), _) if binding.mutable && !self.owns(&binding.storage) => true,
            (_, BindingType::Scalar(type_name)) => self.holds_reference_where(ElementType::Scalar(type_name), true),
            (_, BindingType::Struct(id)) => self.holds_reference_where(ElementType::Struct(id), true),
            _ => false,
        }
    }

    /// Notes a shared borrow of `place`, refused if a `&mut` borrow that
    /// may change it is used later.
    pub(super) fn check_shareable(&mut self, place: &Expr, span: Span) -> Result<(), Diagnostic> {
        let Some((owner, via, targets)) = self.targets(place) else {
            return Ok(());
        };
        for target in targets {
            self.share_borrowed(target.owner, &target.path, via, Diagnostic::new(span, format!("{owner:?} is mutably borrowed here, so it cannot be borrowed")));
        }
        Ok(())
    }

    /// Notes a change to `place`, refused if a borrow of it, or of a place
    /// holding or held by it, is used later.
    pub(super) fn check_unborrowed(&mut self, place: &Expr, span: Span) -> Result<(), Diagnostic> {
        let Some((owner, via, targets)) = self.targets(place) else {
            return Ok(());
        };
        for target in targets {
            self.change_borrowed(target.owner, &target.path, via, Diagnostic::new(span, format!("{owner:?} is borrowed here, so it cannot be changed")))?;
        }
        Ok(())
    }

    /// The places writing or borrowing `place` touches, with the name it
    /// is written by: its owner's, or what that borrows when it is a
    /// reference or a view, which is then the binding it goes through.
    fn targets<'e>(&self, place: &'e Expr) -> Option<(&'e str, Option<BorrowKey>, Vec<Root>)> {
        let (name, path, exact) = owner_path(place)?;
        let (depth, binding) = self.resolve(name)?;
        // An element a loop walks is changed under the walk's own borrow.
        if let Storage::ArrayView { .. } = binding.storage {
            return None;
        }
        let extend = |root: Root| path.iter().fold(root, |root, field| root.field(field));
        let narrowed = |root: Root| if exact { root } else { root.somewhere() };
        match (&binding.storage, borrow_key(&binding.storage).and_then(|key| self.borrowed_from.get(&key))) {
            (Storage::Reference(_) | Storage::Slice(_), Some(roots)) => {
                Some((name, borrow_key(&binding.storage), roots.iter().cloned().map(extend).map(narrowed).collect()))
            }
            _ => Some((name, None, self.root(name, depth, binding).into_iter().map(extend).map(narrowed).collect())),
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
                self.change_borrowed(key, &[], None, error)
            }
            None => Ok(()),
        }
    }

    /// The bindings in scope, other than `owner` itself and the reference
    /// `via` that the change goes through, that borrow `path` in it, or a
    /// place holding or held by it; only those that may change it when
    /// `exclusive`.
    pub(super) fn holders(&self, owner: BorrowKey, path: &[String], exclusive: bool, via: Option<BorrowKey>) -> BTreeSet<BorrowKey> {
        let borrows = |roots: Option<&BTreeSet<Root>>| roots.is_some_and(|roots| roots.iter().any(|root| root.overlaps(owner, path)));
        let mut holders = BTreeSet::new();
        for binding in self.bindings() {
            let itself = identity(&binding.storage);
            if itself == Some(owner) || itself.is_some() && itself == via || exclusive && !self.may_change(binding) {
                continue;
            }
            let lent = borrow_key(&binding.storage).filter(|key| borrows(self.borrowed_from.get(key)));
            let held = identity(&binding.storage).filter(|key| borrows(self.held.get(key)));
            holders.extend(lent.into_iter().chain(held));
        }
        holders
    }

    /// What the bindings in scope borrow, each with whether the binding may
    /// change it, and the binding.
    pub(super) fn held_borrows(&self) -> Vec<(Root, bool, BorrowKey)> {
        let mut borrows = Vec::new();
        for binding in self.bindings() {
            let mutable = self.may_change(binding);
            let lent = borrow_key(&binding.storage).and_then(|key| Some((key, self.borrowed_from.get(&key)?)));
            let held = identity(&binding.storage).and_then(|key| Some((key, self.held.get(&key)?)));
            for (holder, roots) in lent.into_iter().chain(held) {
                borrows.extend(roots.iter().map(|root| (root.clone(), mutable, holder)));
            }
        }
        borrows
    }

    /// The bindings in scope, and those of the code a lambda is inlined in.
    fn bindings(&self) -> impl Iterator<Item = &Binding> {
        self.enclosing.iter().flatten().chain(&self.scopes).flat_map(|scope| scope.values())
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
            // Lent through itself, it would alias what it holds.
            if roots.iter().any(|root| root.owner == target.owner) {
                return Err(Diagnostic::new(span, format!("{:?} would hold a borrow of itself", target.name)));
            }
            if matches!(target.life, Life::Frame | Life::Scope(_)) {
                self.held.entry(target.owner).or_default().extend(roots.iter().cloned());
            } else {
                // What outlives the call keeps what it is given.
                self.keep_lent(&roots);
            }
        }
        Ok(())
    }

    /// The owners a store into `container` writes: its root binding, or
    /// what that binding borrows when it is a reference or a view.
    fn store_targets(&self, container: &Expr) -> Vec<Root> {
        self.targets(container).map(|(_, _, targets)| targets).unwrap_or_default()
    }

    /// What a call lends through each argument: a borrow lends what it
    /// reaches, a value the borrows it holds, which it writes through when
    /// one is `&mut`.
    pub(super) fn lent(&self, arguments: &[Expr], parameters: &[SignatureParameter]) -> Vec<Lent> {
        arguments
            .iter()
            .zip(parameters)
            .map(|(argument, parameter)| match *parameter {
                SignatureParameter::Borrowed { mutable, .. } => self.lent_borrow(argument, mutable),
                SignatureParameter::Adapter { .. } => self.lent_borrow(argument, true),
                SignatureParameter::Scalar(type_name) => self.lent_value(argument, ElementType::Scalar(type_name)),
                SignatureParameter::Owned { struct_id, .. } => self.lent_value(argument, ElementType::Struct(struct_id)),
            })
            .collect()
    }

    /// What a struct, enum or generator's state built from `values`, of
    /// `fields`, lends: a field holding a reference or a view takes a borrow
    /// as a parameter does, any other the borrows its value holds.
    pub(super) fn lent_to_fields(&self, values: &[&Expr], fields: &[ElementType]) -> Vec<Lent> {
        values
            .iter()
            .zip(fields)
            .map(|(value, field)| match *field {
                ElementType::Scalar(type_name @ TypeName::Pointer { mutable, .. }) if self.types.referent(type_name).is_some() => self.lent_borrow(value, mutable),
                ElementType::Struct(id) if self.types.kept_views.contains_key(&id) => self.lent_borrow(value, self.types.writable_views.contains(&id)),
                element => self.lent_value(value, element),
            })
            .collect()
    }

    fn lent_borrow(&self, value: &Expr, mutable: bool) -> Lent {
        Lent { roots: self.reach(value), mutable }
    }

    fn lent_value(&self, value: &Expr, element: ElementType) -> Lent {
        Lent { roots: self.value_roots(value, element), mutable: self.holds_reference_where(element, true) }
    }

    /// Stores what a call is lent: it may keep what any other argument
    /// lends in what a `&mut` argument holds.
    pub(super) fn store_call_borrows(&mut self, arguments: &[Expr], parameters: &[SignatureParameter], lent: &[Lent], span: Span) -> Result<(), Diagnostic> {
        for (at, (argument, parameter)) in arguments.iter().zip(parameters).enumerate() {
            if let SignatureParameter::Borrowed { mutable: true, target, .. } = *parameter {
                if self.holds_reference(binding_element(target)) {
                    let others = lent.iter().enumerate().filter(|(other, _)| *other != at).flat_map(|(_, one)| one.roots.iter().cloned()).collect();
                    self.store_borrow(argument, others, span)?;
                }
            }
        }
        Ok(())
    }

    /// Stores what assigning `value`, of `element`, through `target` keeps.
    pub(super) fn store_assigned_borrows(&mut self, target: &AssignTarget, value: &Expr, element: ElementType, span: Span) -> Result<(), Diagnostic> {
        let roots = self.value_roots(value, element);
        self.store_borrow(&target.expression(span), roots, span)
    }
}

/// What a call lends through one argument: the owners its callee reaches,
/// and whether it may write them.
pub(super) struct Lent {
    pub(super) roots: BTreeSet<Root>,
    pub(super) mutable: bool,
}

/// Errs when two of a call's lends reach one owner and either writes it.
pub(super) fn check_disjoint(lent: &[Lent], spans: &[Span]) -> Result<(), Diagnostic> {
    for (at, one) in lent.iter().enumerate() {
        for other in lent[..at].iter().filter(|other| one.mutable || other.mutable) {
            if let Some(shared) = one.roots.iter().find(|root| other.roots.iter().any(|them| them.overlaps(root.owner, &root.path))) {
                return Err(Diagnostic::new(spans[at], format!("borrow of {:?} aliases a mutable argument", shared.name)));
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

/// The owner `place` is in, the fields down to it, and whether those name
/// it exactly: an element is somewhere in its sequence.
pub(super) fn owner_path(place: &Expr) -> Option<(&str, Vec<String>, bool)> {
    fn walk(place: &Expr) -> Option<(&str, Vec<String>, bool)> {
        match place {
            Expr::Name(name, _) => Some((name, Vec::new(), true)),
            Expr::Member { base, field, .. } => {
                let (owner, mut path, exact) = walk(base)?;
                if exact {
                    path.push(field.clone());
                }
                Some((owner, path, exact))
            }
            Expr::Index { base, .. } | Expr::Slice { base, .. } => walk(base).map(|(owner, path, _)| (owner, path, false)),
            _ => None,
        }
    }
    walk(place)
}

pub(super) fn expression_owner(expression: &Expr) -> Option<&str> {
    match expression {
        Expr::Name(name, _) => Some(name),
        Expr::Member { base, .. } | Expr::Index { base, .. } | Expr::Slice { base, .. } => expression_owner(base),
        _ => None,
    }
}
